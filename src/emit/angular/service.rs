use std::fmt::Write as _;

use crate::emit::typescript::{Position, Writer, render_type};
use crate::ir::canonical::ResponseContent;
use crate::plan::artifact_plan::{PlannedOperation, ServicePlan};
use crate::plan::naming::{error_interface_name, request_interface_name};

use super::imports::render_service_imports;
use super::request::{
  render_error_interface, render_request_interface, render_requestful_builder,
  render_zero_arg_builder,
};

pub(crate) fn emit_service(service_plan: &ServicePlan<'_>) -> String {
  // Each operation produces ~512 bytes (request interface + factory
  // triplet + URL/body construction); 2KB floor covers the @Injectable
  // header + import block.
  let capacity = (service_plan.operations.len() * 512).max(2048);
  let mut buffer = Writer::with_capacity(capacity);

  render_service_imports(&mut buffer, &service_plan.operations, "../rest.util");
  buffer.blank_line();
  buffer.line("@Injectable({");
  buffer.line("  providedIn: 'root',");
  buffer.line("})");
  buffer.open_block(&format!("export class {}", service_plan.class_name));

  // Cache request interface names computed once per operation
  let request_names: std::collections::HashMap<&str, String> = service_plan
    .operations
    .iter()
    .filter(|operation| has_request_interface(operation))
    .map(|operation| {
      (
        operation.method_name.as_str(),
        request_interface_name(&operation.method_name),
      )
    })
    .collect();

  for operation in &service_plan.operations {
    buffer.blank_line();
    render_operation_property(
      &mut buffer,
      operation,
      request_names.get(operation.method_name.as_str()),
    );
  }

  buffer.close_block("");

  // Per-operation tail: for each operation, emit its `{Pascal}Params`
  // interface (when the operation has any inputs) followed by its
  // `{Pascal}Error` interface (when it declares any 4xx/5xx with a JSON
  // schema). Per-operation grouping beats kind-grouping when the file
  // grows long — a reader searching for "UpdatePet" finds the property,
  // its params, and its error map contiguously.
  for operation in &service_plan.operations {
    let request_name = request_names.get(operation.method_name.as_str());
    let has_errors = !operation.errors.is_empty();
    if request_name.is_none() && !has_errors {
      continue;
    }
    buffer.blank_line();
    if let Some(name) = request_name {
      render_request_interface(&mut buffer, operation, name);
    }
    if has_errors {
      if request_name.is_some() {
        buffer.blank_line();
      }
      let error_name = error_interface_name(&operation.method_name);
      render_error_interface(&mut buffer, operation, &error_name);
    }
  }

  buffer.into_string()
}

fn render_operation_property(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  request_name: Option<&String>,
) {
  let property_name = &operation.method_name;

  crate::emit::typescript::jsdoc(
    buffer,
    operation.description.as_deref(),
    operation.deprecated,
  );
  write!(buffer, "readonly {property_name} = ").unwrap();
  write_response_call_site(buffer, operation.response, request_name);
  buffer.push("(\n");
  buffer.indent();
  match request_name {
    Some(name) => render_requestful_builder(buffer, operation, name),
    None => render_zero_arg_builder(buffer, operation),
  }
  buffer.dedent();
  buffer.line(");");
}

const fn has_request_interface(operation: &PlannedOperation<'_>) -> bool {
  !operation.request.fields.is_empty()
    || operation.request.body.is_some()
    || !operation.request.headers.is_empty()
}

/// Writes the full helper call prefix into `buffer`. The arity of the
/// operation (does it take a typed `Request`?) and the response variant
/// pick one of four call shapes — explicit at the generator boundary,
/// so the runtime no longer needs the `reqFn.length === 0` probe.
///
/// Mapping (see docs/superpowers/specs/2026-05-19-request-factory-variants-design.md):
///
/// |                | Requestful                    | Zero-arg                              |
/// |----------------|-------------------------------|----------------------------------------|
/// | JSON / void    | `requestFactory<Req, Res>`    | `requestFactory.zeroArg<Res>`          |
/// | Blob           | `requestFactory.blob<Req>`    | `requestFactory.zeroArg.blob`          |
/// | Text           | `requestFactory.text<Req>`    | `requestFactory.zeroArg.text`          |
/// | ArrayBuffer    | `requestFactory.arrayBuffer<Req>` | `requestFactory.zeroArg.arrayBuffer` |
fn write_response_call_site(
  buffer: &mut Writer,
  response: Option<&ResponseContent>,
  request_name: Option<&String>,
) {
  let variant = match response {
    Some(ResponseContent::Blob) => Some("blob"),
    Some(ResponseContent::Text) => Some("text"),
    Some(ResponseContent::ArrayBuffer) => Some("arrayBuffer"),
    Some(ResponseContent::Json(_)) | None => None,
  };

  match (variant, request_name) {
    (Some(kind), Some(request)) => {
      write!(buffer, "requestFactory.{kind}<{request}>").unwrap();
    }
    (Some(kind), None) => {
      write!(buffer, "requestFactory.zeroArg.{kind}").unwrap();
    }
    (None, Some(request)) => {
      write!(buffer, "requestFactory<{request}, ").unwrap();
      write_response_type(buffer, response);
      buffer.push(">");
    }
    (None, None) => {
      buffer.push("requestFactory.zeroArg<");
      write_response_type(buffer, response);
      buffer.push(">");
    }
  }
}

fn write_response_type(buffer: &mut Writer, response: Option<&ResponseContent>) {
  match response {
    Some(ResponseContent::Json(Some(ty))) => {
      render_type(buffer, ty, Position::Standalone);
    }
    Some(ResponseContent::Json(None)) | None => {
      buffer.push("void");
    }
    Some(ResponseContent::Blob | ResponseContent::Text | ResponseContent::ArrayBuffer) => {
      unreachable!("non-JSON variants handled above");
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::canonical::{HttpMethod, ResponseContent};
  use crate::ir::schema::{SchemaScalar, SchemaType};
  use crate::plan::artifact_plan::PlannedRequestContract;
  use crate::test_support::{op_with, path_field, string_ty};

  // The four tests below pin the helper expression emitted by
  // render_operation_property across every ResponseContent variant.
  // JSON uses the bare requestFactory<Req, Res>(…); Blob/Text/ArrayBuffer
  // use the static-method variant (requestFactory.blob<Req>(…) etc.) —
  // no Response generic and no { responseKind: '…' } option line.

  fn render_property(op: &PlannedOperation<'_>, request_name: &str) -> String {
    let mut buf = Writer::with_capacity(512);
    let owned = request_name.to_string();
    render_operation_property(&mut buf, op, Some(&owned));
    buf.into_string()
  }

  fn op_with_response_and_path<'a>(
    method_name: &str,
    path_ty: &'a SchemaType,
    response: &'a ResponseContent,
  ) -> PlannedOperation<'a> {
    op_with(
      method_name,
      HttpMethod::Get,
      "/x/{id}",
      PlannedRequestContract {
        fields: vec![path_field("id", path_ty)],
        headers: vec![],
        body: None,
      },
      Some(response),
    )
  }

  #[test]
  fn request_factory_call_uses_bare_helper_for_json_response() {
    let str_ty = string_ty();
    let json = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let op = op_with_response_and_path("listPets", &str_ty, &json);
    let out = render_property(&op, "ListPetsParams");

    assert!(
      out.contains("requestFactory<ListPetsParams, string>"),
      "expected bare requestFactory<Req, Res>, got:\n{out}"
    );
    assert!(
      !out.contains("requestFactory.blob")
        && !out.contains("requestFactory.text")
        && !out.contains("requestFactory.arrayBuffer"),
      "expected no static-method variant for JSON response, got:\n{out}"
    );
    assert!(
      !out.contains("responseKind"),
      "expected no responseKind option for JSON response, got:\n{out}"
    );
  }

  #[test]
  fn request_factory_call_uses_blob_variant_for_blob_response() {
    let str_ty = string_ty();
    let op = op_with_response_and_path("download", &str_ty, &ResponseContent::Blob);
    let out = render_property(&op, "DownloadParams");

    assert!(
      out.contains("requestFactory.blob<DownloadParams>"),
      "expected requestFactory.blob<Req>(…) call, got:\n{out}"
    );
    assert!(
      !out.contains(", Blob>"),
      "expected no Response generic for blob variant (Raw is fixed), got:\n{out}"
    );
    assert!(
      !out.contains("responseKind"),
      "expected no responseKind option, got:\n{out}"
    );
  }

  #[test]
  fn request_factory_call_uses_text_variant_for_text_response() {
    let str_ty = string_ty();
    let op = op_with_response_and_path("rawConfig", &str_ty, &ResponseContent::Text);
    let out = render_property(&op, "RawConfigParams");

    assert!(
      out.contains("requestFactory.text<RawConfigParams>"),
      "expected requestFactory.text<Req>(…) call, got:\n{out}"
    );
    assert!(
      !out.contains(", string>"),
      "expected no Response generic for text variant, got:\n{out}"
    );
    assert!(
      !out.contains("responseKind"),
      "expected no responseKind option, got:\n{out}"
    );
  }

  #[test]
  fn request_factory_call_uses_array_buffer_variant_for_array_buffer_response() {
    let str_ty = string_ty();
    let op = op_with_response_and_path("fetch", &str_ty, &ResponseContent::ArrayBuffer);
    let out = render_property(&op, "FetchParams");

    assert!(
      out.contains("requestFactory.arrayBuffer<FetchParams>"),
      "expected requestFactory.arrayBuffer<Req>(…) call, got:\n{out}"
    );
    assert!(
      !out.contains(", ArrayBuffer>"),
      "expected no Response generic for arrayBuffer variant, got:\n{out}"
    );
    assert!(
      !out.contains("responseKind"),
      "expected no responseKind option, got:\n{out}"
    );
  }

  // The four tests below pin the zero-arg call site emitted when the
  // operation has no inputs/headers/body. Codegen routes them through
  // the dedicated `requestFactory.zeroArg(.kind?)` entry points instead
  // of relying on a runtime `reqFn.length === 0` probe.

  fn op_with_response_no_request<'a>(
    method_name: &str,
    response: &'a ResponseContent,
  ) -> PlannedOperation<'a> {
    op_with(
      method_name,
      HttpMethod::Get,
      "/x",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: None,
      },
      Some(response),
    )
  }

  fn render_zero_arg_property(op: &PlannedOperation<'_>) -> String {
    let mut buf = Writer::with_capacity(512);
    render_operation_property(&mut buf, op, None);
    buf.into_string()
  }

  #[test]
  fn request_factory_zero_arg_json_uses_zero_arg_helper() {
    let json = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let op = op_with_response_no_request("listPets", &json);
    let out = render_zero_arg_property(&op);

    assert!(
      out.contains("requestFactory.zeroArg<string>"),
      "expected requestFactory.zeroArg<Res>(…) for zero-arg JSON, got:\n{out}"
    );
  }

  #[test]
  fn request_factory_zero_arg_blob_uses_nested_helper() {
    let op = op_with_response_no_request("download", &ResponseContent::Blob);
    let out = render_zero_arg_property(&op);

    assert!(
      out.contains("requestFactory.zeroArg.blob"),
      "expected requestFactory.zeroArg.blob(…) for zero-arg blob, got:\n{out}"
    );
  }

  #[test]
  fn request_factory_zero_arg_text_uses_nested_helper() {
    let op = op_with_response_no_request("rawConfig", &ResponseContent::Text);
    let out = render_zero_arg_property(&op);

    assert!(
      out.contains("requestFactory.zeroArg.text"),
      "expected requestFactory.zeroArg.text(…) for zero-arg text, got:\n{out}"
    );
  }

  #[test]
  fn request_factory_zero_arg_array_buffer_uses_nested_helper() {
    let op = op_with_response_no_request("fetch", &ResponseContent::ArrayBuffer);
    let out = render_zero_arg_property(&op);

    assert!(
      out.contains("requestFactory.zeroArg.arrayBuffer"),
      "expected requestFactory.zeroArg.arrayBuffer(…) for zero-arg arrayBuffer, got:\n{out}"
    );
  }
}
