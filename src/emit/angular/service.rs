use crate::api_model::canonical::ResponseContent;
use crate::emit::ts::{Doc, Position, Render, Writer, jsdoc, type_reexport_line, w, wln};
use crate::identifier::TypeName;
use crate::plan::artifact_plan::{PlannedOperation, ServicePlan};
use crate::plan::naming::service_file_stem;

use super::imports::render_service_imports;
use super::request::{
  render_error_interface, render_request_interface, render_requestful_builder,
  render_zero_arg_builder,
};

#[must_use]
pub(crate) fn emit_service(service_plan: &ServicePlan<'_>) -> String {
  // ~512 bytes per operation, with a 2 KB floor for the preamble.
  let capacity = (service_plan.operations.len() * 512).max(2048);
  let mut buffer = Writer::with_capacity(capacity);

  render_service_imports(&mut buffer, &service_plan.operations, "../rest.util");
  buffer.blank_line();
  buffer.line("@Injectable({");
  buffer.line("  providedIn: 'root',");
  buffer.line("})");
  buffer.open_block(&format!("export class {}", service_plan.class_name));

  service_plan.operations.iter().for_each(|operation| {
    buffer.blank_line();
    render_operation_property(&mut buffer, operation, operation.request_interface.as_ref());
  });

  buffer.close_block("");

  // Grouped per operation, so a reader finds a property, its params and its error map contiguously.
  for operation in &service_plan.operations {
    let request_name = operation.request_interface.as_ref();
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
      if let Some(error_name) = &operation.error_interface {
        render_error_interface(&mut buffer, operation, error_name);
      }
    }
  }

  buffer.into_string()
}

/// Class for the `services` + `operations` layout: one `withInjector()` line per operation,
/// plus type re-exports so `import type { ListPetsParams } from './rest/pet.rest'` resolves the
/// same way it does under `services`.
#[must_use]
pub(crate) fn emit_bound_service(service_plan: &ServicePlan<'_>) -> String {
  // The barrel `rest/<group>/index.ts` is imported by its directory.
  let specifier = format!("./{}", service_file_stem(&service_plan.group_name));
  let mut buffer = Writer::with_capacity((service_plan.operations.len() * 96).max(512));

  buffer.line("import { Injectable } from '@angular/core';");
  wln!(buffer, "import * as ops from '{specifier}';");
  buffer.blank_line();
  buffer.line("@Injectable({");
  buffer.line("  providedIn: 'root',");
  buffer.line("})");
  buffer.open_block(&format!("export class {}", service_plan.class_name));

  // One-liners stay contiguous; a blank line separates documented properties from their neighbours.
  let mut previous_documented = false;
  for (index, operation) in service_plan.operations.iter().enumerate() {
    let documented = !Doc::new(operation.description.as_deref(), operation.deprecated).is_empty();
    if index > 0 && (documented || previous_documented) {
      buffer.blank_line();
    }
    jsdoc(
      &mut buffer,
      Doc::new(operation.description.as_deref(), operation.deprecated),
    );
    let name = &operation.method_name;
    wln!(buffer, "readonly {name} = ops.{name}.withInjector();");
    previous_documented = documented;
  }
  buffer.close_block("");

  // Params before Error, per operation, in class order.
  let reexports: Vec<&str> = service_plan
    .operations
    .iter()
    .flat_map(|operation| {
      [
        operation.request_interface.as_ref(),
        operation.error_interface.as_ref(),
      ]
    })
    .flatten()
    .map(TypeName::as_str)
    .collect();
  if !reexports.is_empty() {
    buffer.blank_line();
    type_reexport_line(
      &mut buffer,
      reexports.iter().map(|name| (*name, *name)),
      &specifier,
    );
  }

  buffer.into_string()
}

fn render_operation_property(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  request_name: Option<&TypeName>,
) {
  let property_name = &operation.method_name;

  jsdoc(
    buffer,
    Doc::new(operation.description.as_deref(), operation.deprecated),
  );
  w!(buffer, "readonly {property_name} = ");
  write_call_site(buffer, "requestFactory", operation.response, request_name);
  buffer.push("(\n");
  buffer.indent();
  match request_name {
    Some(name) => render_requestful_builder(buffer, operation, name),
    None => render_zero_arg_builder(buffer, operation),
  }
  buffer.dedent();
  buffer.line(");");
}

// Arity and response variant pick the `.zeroArg` / `.blob` / `.text` /
// `.arrayBuffer` suffix on `factory`.
pub(super) fn write_call_site(
  buffer: &mut Writer,
  factory: &str,
  response: Option<&ResponseContent>,
  request_name: Option<&TypeName>,
) {
  let variant = match response {
    Some(ResponseContent::Blob) => Some("blob"),
    Some(ResponseContent::Text) => Some("text"),
    Some(ResponseContent::ArrayBuffer) => Some("arrayBuffer"),
    Some(ResponseContent::Json(_)) | None => None,
  };

  match (variant, request_name) {
    (Some(kind), Some(request)) => {
      w!(buffer, "{factory}.{kind}<{request}>");
    }
    (Some(kind), None) => {
      w!(buffer, "{factory}.zeroArg.{kind}");
    }
    (None, Some(request)) => {
      w!(buffer, "{factory}<{request}, ");
      write_response_type(buffer, response);
      buffer.push(">");
    }
    (None, None) => {
      w!(buffer, "{factory}.zeroArg<");
      write_response_type(buffer, response);
      buffer.push(">");
    }
  }
}

fn write_response_type(buffer: &mut Writer, response: Option<&ResponseContent>) {
  match response {
    Some(ResponseContent::Json(Some(ty))) => {
      ty.render(buffer, Position::Standalone);
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
  use crate::api_model::canonical::ErrorResponse;
  use crate::api_model::canonical::{HttpMethod, ResponseContent};
  use crate::api_model::schema::{SchemaScalar, SchemaType};
  use crate::plan::artifact_plan::{PlannedRequestContract, ServicePlan};
  use crate::test_support::{empty_request, op_with, path_field, string_schema};

  // The four tests below pin the helper expression emitted by
  // render_operation_property across every ResponseContent variant.
  // JSON uses the bare requestFactory<Req, Res>(…); Blob/Text/ArrayBuffer
  // use the static-method variant (requestFactory.blob<Req>(…) etc.) —
  // no Response generic and no { responseKind: '…' } option line.

  fn render_property(op: &PlannedOperation<'_>, request_name: &str) -> String {
    let mut buf = Writer::with_capacity(512);
    let owned = TypeName::new(request_name.to_string());
    render_operation_property(&mut buf, op, Some(&owned));
    buf.into_string()
  }

  fn op_with_response_and_path<'a>(
    method_name: &str,
    path_schema: &'a SchemaType,
    response: &'a ResponseContent,
  ) -> PlannedOperation<'a> {
    op_with(
      method_name,
      HttpMethod::Get,
      "/x/{id}",
      PlannedRequestContract {
        fields: vec![path_field("id", path_schema)],
        headers: vec![],
        body: None,
      },
      Some(response),
    )
  }

  #[test]
  fn request_factory_call_uses_bare_helper_for_json_response() {
    let str_schema = string_schema();
    let json = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let op = op_with_response_and_path("listPets", &str_schema, &json);
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
    let str_schema = string_schema();
    let op = op_with_response_and_path("download", &str_schema, &ResponseContent::Blob);
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
    let str_schema = string_schema();
    let op = op_with_response_and_path("rawConfig", &str_schema, &ResponseContent::Text);
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
    let str_schema = string_schema();
    let op = op_with_response_and_path("fetch", &str_schema, &ResponseContent::ArrayBuffer);
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

  #[test]
  fn bound_service_groups_documented_properties_and_reexports_types() {
    let str_schema = string_schema();
    let json = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let not_found = [ErrorResponse {
      status: 404,
      body: SchemaType::Scalar(SchemaScalar::String),
    }];
    let mut get_pet = op_with_response_and_path("getPet", &str_schema, &json);
    get_pet.errors = &not_found;
    get_pet.error_interface = Some(crate::plan::naming::error_interface_name(
      &get_pet.method_name,
    ));
    let mut list_pets = op_with(
      "listPets",
      HttpMethod::Get,
      "/pets",
      empty_request(),
      Some(&json),
    );
    list_pets.description = Some("List pets.".to_string());
    let ping = op_with("ping", HttpMethod::Get, "/ping", empty_request(), None);
    let plan = ServicePlan {
      group_name: "pet".to_string(),
      class_name: TypeName::new("PetRest".to_string()),
      artifact_path: "rest/pet.rest.ts".to_string(),
      operations_barrel_path: Some("rest/pet/index.ts".to_string()),
      operations: vec![get_pet, list_pets, ping],
    };

    let out = emit_bound_service(&plan);

    assert_eq!(
      out,
      "import { Injectable } from '@angular/core';\n\
       import * as ops from './pet';\n\
       \n\
       @Injectable({\n\
      \x20 providedIn: 'root',\n\
       })\n\
       export class PetRest {\n\
      \x20 readonly getPet = ops.getPet.withInjector();\n\
       \n\
      \x20 /**\n\
      \x20  * List pets.\n\
      \x20  */\n\
      \x20 readonly listPets = ops.listPets.withInjector();\n\
       \n\
      \x20 readonly ping = ops.ping.withInjector();\n\
       }\n\
       \n\
       export type { GetPetParams, GetPetError } from './pet';\n"
    );
  }
}
