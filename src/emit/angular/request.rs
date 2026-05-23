use crate::emit::typescript::{self as ts, Position, Writer, render_type, safe_property_name};
use crate::ir::canonical::BodyFieldType;
use crate::ir::schema::{SchemaProperty, SchemaType};
use crate::plan::artifact_plan::{PlannedOperation, PlannedRequestBody, RequestFieldKind};
use crate::wln;

/// Which form-body flavor the inline IIFE builds.
///
/// Emit-local — distinct from the normalize-stage `FormKind` because the
/// concerns differ: normalize uses it to dispatch the body walker, while
/// emit uses it to pick the runtime constructor (`FormData` vs
/// `URLSearchParams`) and the TS return type. Sharing the enum across
/// stages would couple emit to normalize for no real reuse.
#[derive(Clone, Copy)]
enum FormKind {
  Multipart,
  UrlEncoded,
}

pub(super) fn render_requestful_builder(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  interface_name: &str,
) {
  buffer.open_block(&format!("(request: {interface_name}) =>"));

  let mut destructured: Vec<&str> = operation
    .request
    .fields
    .iter()
    .map(|f| f.name.as_ref())
    .collect();
  // Body destructure depends on the body's layout: `Nested` introduces a
  // single `body` identifier, while flat-JSON/form bodies destructure
  // each field by name so the builder can reference them as bare
  // identifiers in the assembled `body:` expression (object literal /
  // `fd.append('name', name)`).
  match &operation.request.body {
    None => {}
    Some(PlannedRequestBody::Nested { .. }) => destructured.push("body"),
    Some(PlannedRequestBody::FlatJson { properties, .. }) => {
      destructured.extend(properties.iter().map(|p| p.name.as_ref()));
    }
    Some(PlannedRequestBody::Multipart { fields } | PlannedRequestBody::UrlEncoded { fields }) => {
      destructured.extend(fields.iter().map(|f| f.name.as_ref()));
    }
  }
  if !operation.request.headers.is_empty() {
    destructured.push("headers");
  }
  if !destructured.is_empty() {
    wln!(buffer, "const {{ {} }} = request;", destructured.join(", "));
  }

  buffer.open_block("return");
  wln!(buffer, "method: '{}',", operation.method);
  write_path_template_line(buffer, &operation.path);
  if let Some(params_expression) = render_params_expression(operation) {
    wln!(buffer, "params: {params_expression},");
  }
  if let Some(body_expression) = render_body_expression(operation) {
    wln!(buffer, "body: {body_expression},");
  }
  if !operation.request.headers.is_empty() {
    buffer.line("headers,");
  }
  buffer.close_block(";");

  buffer.close_block(",");
}

pub(super) fn render_zero_arg_builder(buffer: &mut Writer, operation: &PlannedOperation<'_>) {
  buffer.line("() => ({");
  buffer.indent();
  wln!(buffer, "method: '{}',", operation.method);
  write_path_template_line(buffer, &operation.path);
  buffer.dedent();
  buffer.line("}),");
}

/// Stream `url: \`<rendered-path>\`,\n` into `buffer`, expanding each
/// `{name}` placeholder to `${encodeURIComponent(name)}` without
/// allocating a separate `String` per template.
fn write_path_template_line(buffer: &mut Writer, path: &str) {
  buffer.push("url: `");
  write_path_template_into(buffer, path);
  buffer.push("`,\n");
}

pub(super) fn render_request_interface(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  request_name: &str,
) {
  // Manual emit (instead of `ts::interface_block`) because the body's
  // hoisted fields can mix `SchemaType` (flat-JSON body properties) with
  // `BodyFieldType` (form-body fields). The two share no enum — form-field
  // types are deliberately constrained (`Scalar | ArrayOfScalar | Binary
  // | ArrayOfBinary`) — so we render each group with its own type printer
  // and keep the ordering invariant: path → query → body → headers.
  buffer.open_block(&format!("export interface {request_name}"));

  // Path / query parameters at the top.
  for field in &operation.request.fields {
    ts::write_property_declaration(buffer, field.name.as_ref(), field.optional, field.ty);
    buffer.push(";\n");
  }

  // Body. Smart-flatten dispatches on the body kind:
  //   - Nested → single `body: T` field (preserves named-ref identity).
  //   - FlatJson → hoist each property as a top-level field (matches the
  //     spec's authorial intent for unnamed object bodies).
  //   - Multipart / UrlEncoded → hoist each form-field as a top-level
  //     entry rendered through the BodyFieldType printer.
  match &operation.request.body {
    None => {}
    Some(PlannedRequestBody::Nested { ty, optional }) => {
      ts::write_property_declaration(buffer, "body", *optional, ty);
      buffer.push(";\n");
    }
    Some(PlannedRequestBody::FlatJson { properties, .. }) => {
      for prop in properties {
        ts::write_property_declaration(buffer, prop.name.as_ref(), prop.optional, prop.ty);
        buffer.push(";\n");
      }
    }
    Some(PlannedRequestBody::Multipart { fields } | PlannedRequestBody::UrlEncoded { fields }) => {
      for form in fields {
        let name = safe_property_name(form.name.as_ref()).into_owned();
        let optional_marker = if form.optional { "?" } else { "" };
        let ts_type = ts::render_body_field_type(form.ty);
        wln!(buffer, "{name}{optional_marker}: {ts_type};");
      }
    }
  }

  // Synthetic `headers` block. Materialized here at the writer level
  // (not at plan time) so the plan's `headers` list stays a simple
  // sibling of `fields`. Headers carry no per-field deprecation —
  // OpenAPI's Parameter Object has `deprecated` on Operation/Schema
  // but not on header parameters — so each property's trailing flag
  // is `false`.
  if !operation.request.headers.is_empty() {
    let header_props: Vec<SchemaProperty> = operation
      .request
      .headers
      .iter()
      .map(|h| SchemaProperty {
        name: h.name.clone(),
        required: !h.optional,
        ty: h.ty.clone(),
        description: None,
        deprecated: false,
      })
      .collect();
    let all_optional = operation.request.headers.iter().all(|h| h.optional);
    let headers_ty = SchemaType::InlineObject {
      properties: header_props,
    };
    ts::write_property_declaration(buffer, "headers", all_optional, &headers_ty);
    buffer.push(";\n");
  }

  buffer.close_block("");
}

/// Renders the per-operation `{Pascal}Error` interface — a numeric-status-keyed
/// map of error body types, e.g.
///
/// ```ignore
/// export interface UpdatePetError {
///   400: ValidationProblem;
///   500: { traceId: string };
/// }
/// ```
///
/// Lives in the service file (alongside `{Pascal}Params`) so the per-operation
/// typed surfaces are colocated. Consumers access individual body types via
/// `UpdatePetError[400]` and cast `HttpErrorResponse.error` themselves — the
/// framework types `.error` as `any`, so this is a documentation/help type,
/// not a runtime guarantee.
pub(super) fn render_error_interface(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  error_name: &str,
) {
  buffer.open_block(&format!("export interface {error_name}"));
  for error in operation.errors {
    buffer.push(&error.status.to_string());
    buffer.push(": ");
    render_type(buffer, &error.body, Position::Standalone);
    buffer.push(";\n");
  }
  buffer.close_block("");
}

fn render_params_expression(operation: &PlannedOperation<'_>) -> Option<String> {
  let query_fields: Vec<&str> = operation
    .request
    .fields
    .iter()
    .filter(|f| f.kind == RequestFieldKind::Query)
    .map(|f| f.name.as_ref())
    .collect();
  if query_fields.is_empty() {
    return None;
  }

  // When all query fields are optional and undefined at call time, the
  // emitted `httpParams({...})` produces an empty `HttpParams` (the helper
  // in templates/angular/rest.util.ts skips undefined values). We keep the
  // unconditional emit instead of a per-call runtime guard because the
  // empty-params path is a cheap no-op and the alternative spread guard
  // (`...(a !== undefined ? { params: ... } : {})`) is noisier than the
  // cost it saves.
  Some(format!("httpParams({{ {} }})", query_fields.join(", "),))
}

fn render_body_expression(operation: &PlannedOperation<'_>) -> Option<String> {
  match operation.request.body.as_ref()? {
    // Nested bodies forward verbatim via property shorthand — `body,` in
    // the builder return literal.
    PlannedRequestBody::Nested { .. } => Some("body".to_string()),
    // Flat-JSON bodies re-assemble the hoisted properties into an object
    // literal by destructured name, restoring the original body shape on
    // the wire.
    PlannedRequestBody::FlatJson { properties, .. } => {
      let names: Vec<&str> = properties.iter().map(|p| p.name.as_ref()).collect();
      Some(format!("{{ {} }}", names.join(", ")))
    }
    PlannedRequestBody::Multipart { fields } => Some(render_form_body(fields, FormKind::Multipart)),
    PlannedRequestBody::UrlEncoded { fields } => {
      Some(render_form_body(fields, FormKind::UrlEncoded))
    }
  }
}

/// Build the inline IIFE that materializes a form-body request payload.
///
/// Returns a multi-line `String` whose first line starts with `((): ... => {`
/// and whose final line ends with `})()` — to be interpolated as the value
/// of a `body:` property at indent level 2 inside `render_requestful_builder`.
/// The `Writer` re-indents each `\n`-terminated line with its current cache,
/// so the leading spaces on continuation lines stack on top of that prefix.
///
/// The outer builder destructures each form field from `request` directly
/// (smart-flatten hoists form fields to top-level), so the IIFE references
/// each by bare identifier in the append calls.
///
/// Per-field append rules are keyed on `BodyFieldType`:
/// - `Scalar` and `ArrayOfScalar` wrap the value in `String(...)` because
///   `FormData.append` / `URLSearchParams.append` accept only string or Blob.
/// - `Binary` and `ArrayOfBinary` skip the cast — `File`/`Blob` are valid
///   `FormData` entries as-is; `URLSearchParams` doesn't support binary so
///   normalize rejects those fields upstream.
/// - Optional fields wrap in `if (name !== undefined) ...` to preserve the
///   "no key present" semantics; required fields emit unguarded.
fn render_form_body(
  fields: &[crate::plan::artifact_plan::PlannedFormField<'_>],
  kind: FormKind,
) -> String {
  let (ctor, var, ts_type) = match kind {
    FormKind::Multipart => ("new FormData()", "fd", "FormData"),
    FormKind::UrlEncoded => ("new URLSearchParams()", "params", "URLSearchParams"),
  };
  let mut out = String::new();
  out.push_str(&format!("((): {ts_type} => {{\n"));
  out.push_str(&format!("  const {var} = {ctor};\n"));
  for f in fields {
    let name = f.name.as_ref();
    let guard_open = if f.optional {
      format!("if ({name} !== undefined) ")
    } else {
      String::new()
    };
    let append_call = match f.ty {
      BodyFieldType::Scalar(_) => format!("{var}.append('{name}', String({name}));"),
      BodyFieldType::ArrayOfScalar(_) => {
        format!("for (const v of {name}) {var}.append('{name}', String(v));")
      }
      BodyFieldType::Binary => format!("{var}.append('{name}', {name});"),
      BodyFieldType::ArrayOfBinary => {
        format!("for (const v of {name}) {var}.append('{name}', v);")
      }
    };
    out.push_str("  ");
    out.push_str(&guard_open);
    out.push_str(&append_call);
    out.push('\n');
  }
  out.push_str(&format!("  return {var};\n"));
  out.push_str("})()");
  out
}

/// Stream `path` into `buffer`, expanding each `{name}` placeholder to
/// `${encodeURIComponent(name)}`. Operates on string slices so the
/// per-placeholder name never lands in its own heap allocation; the
/// caller's buffer absorbs every byte directly.
///
/// Balanced braces are a normalize-stage invariant
/// (`validate_path_template` rejects unmatched `{` / `}` before this
/// runs), so the loop never encounters a stray brace. The unbalanced-`{`
/// branch survives as a defensive fallback that emits the remainder
/// verbatim rather than panicking — preferable to surfacing a
/// generator panic on adversarial IR.
fn write_path_template_into(buffer: &mut Writer, path: &str) {
  let mut rest = path;
  while let Some(open) = rest.find('{') {
    buffer.push(&rest[..open]);
    let after_open = &rest[open + 1..];
    let Some(close) = after_open.find('}') else {
      debug_assert!(
        false,
        "path template `{path}` reached emit with an unmatched '{{'; normalize must reject it"
      );
      buffer.push(after_open);
      return;
    };
    buffer.push("${encodeURIComponent(");
    buffer.push(&after_open[..close]);
    buffer.push(")}");
    rest = &after_open[close + 1..];
  }
  buffer.push(rest);
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::canonical::{BodyFieldType, ErrorResponse, HttpMethod};
  use crate::ir::schema::{SchemaProperty, SchemaScalar};
  use crate::plan::artifact_plan::{PlannedHeader, PlannedRequestContract};
  use crate::test_support::{
    body_field, flat_json_body, nested_body, op_with, op_with_errors, op_with_multipart_fields,
    op_with_multipart_fields_full, op_with_urlencoded_fields, path_field, query_field, string_ty,
  };

  // ── render_error_interface ────────────────────────────────────────────────

  fn render_errors(error_name: &str, errors: &[ErrorResponse]) -> String {
    let op = op_with_errors("op", errors);
    let mut buf = Writer::with_capacity(256);
    render_error_interface(&mut buf, &op, error_name);
    buf.into_string()
  }

  #[test]
  fn error_interface_emits_numeric_status_keys_in_source_order() {
    let errors = vec![
      ErrorResponse {
        status: 400,
        body: SchemaType::Ref("ValidationProblem".into()),
      },
      ErrorResponse {
        status: 500,
        body: SchemaType::Ref("ServerError".into()),
      },
    ];
    let out = render_errors("UpdatePetError", &errors);
    assert!(out.contains("export interface UpdatePetError {"));
    let four = out.find("400: ValidationProblem;").expect("400 entry");
    let five = out.find("500: ServerError;").expect("500 entry");
    assert!(four < five, "entries must follow input order, got:\n{out}");
  }

  #[test]
  fn error_interface_emits_inline_object_bodies_verbatim() {
    let body = SchemaType::InlineObject {
      properties: vec![SchemaProperty {
        name: "code".into(),
        required: true,
        ty: SchemaType::Scalar(SchemaScalar::String),
        description: None,
        deprecated: false,
      }],
    };
    let errors = vec![ErrorResponse { status: 422, body }];
    let out = render_errors("CreatePetError", &errors);
    assert!(out.contains("422: {"));
    assert!(out.contains("code: string;"));
  }

  // ── render_requestful_builder ──────────────────────────────────────────────

  #[test]
  fn requestful_builder_renders_get_with_path_param_only() {
    let ty = string_ty();
    let op = op_with(
      "getPet",
      HttpMethod::Get,
      "/pets/{petId}",
      PlannedRequestContract {
        fields: vec![path_field("petId", &ty)],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "GetPetParams");
    let out = buf.into_string();

    assert!(out.contains("(request: GetPetParams) =>"));
    assert!(out.contains("const { petId } = request;"));
    assert!(out.contains("method: 'GET',"));
    assert!(out.contains("url: `/pets/${encodeURIComponent(petId)}`,"));
    // GET with path-only: no params/body/headers lines.
    assert!(!out.contains("params:"));
    assert!(!out.contains("body:"));
    assert!(!out.contains("headers,"));
  }

  #[test]
  fn requestful_builder_renders_post_with_ref_body_and_headers() {
    let str_ty = string_ty();
    let body_ref = SchemaType::Ref("CreatePetPayload".into());
    let op = op_with(
      "createPet",
      HttpMethod::Post,
      "/pets",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![PlannedHeader {
          name: "X-Trace-Id".into(),
          optional: false,
          ty: &str_ty,
        }],
        body: Some(nested_body(&body_ref, false)),
      },
      None,
    );

    let mut buf = Writer::with_capacity(1024);
    render_requestful_builder(&mut buf, &op, "CreatePetParams");
    let out = buf.into_string();

    assert!(out.contains("(request: CreatePetParams) =>"));
    assert!(out.contains("const { body, headers } = request;"));
    assert!(out.contains("method: 'POST',"));
    assert!(out.contains("url: `/pets`,"));
    // Nested body forwards verbatim via shorthand.
    assert!(out.contains("body: body,"));
    assert!(out.contains("headers,"));
  }

  #[test]
  fn requestful_builder_assembles_object_literal_for_flat_json_body() {
    // Smart-flatten: inline JSON object bodies hoist properties to
    // top-level fields. The builder re-assembles them into an object
    // literal at the `body:` slot.
    let str_ty = string_ty();
    let bool_ty = SchemaType::Scalar(SchemaScalar::Boolean);
    let op = op_with(
      "decide",
      HttpMethod::Post,
      "/decide",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: Some(flat_json_body(
          vec![
            body_field("csvImportId", false, &str_ty),
            body_field("doImport", false, &bool_ty),
          ],
          true,
        )),
      },
      None,
    );

    let mut buf = Writer::with_capacity(1024);
    render_requestful_builder(&mut buf, &op, "DecideParams");
    let out = buf.into_string();

    assert!(out.contains("const { csvImportId, doImport } = request;"));
    assert!(out.contains("body: { csvImportId, doImport },"));
  }

  #[test]
  fn requestful_builder_renders_query_params_via_http_params() {
    let str_ty = string_ty();
    let op = op_with(
      "listPets",
      HttpMethod::Get,
      "/pets",
      PlannedRequestContract {
        fields: vec![
          query_field("limit", true, &str_ty),
          query_field("offset", true, &str_ty),
        ],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "ListPetsParams");
    let out = buf.into_string();

    assert!(out.contains("const { limit, offset } = request;"));
    assert!(out.contains("params: httpParams({ limit, offset }),"));
    assert!(!out.contains("body:"));
  }

  #[test]
  fn requestful_builder_renders_non_object_json_body_as_nested_shorthand() {
    let payload_ty = SchemaType::Scalar(SchemaScalar::String);
    let op = op_with(
      "uploadPayload",
      HttpMethod::Post,
      "/upload",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: Some(nested_body(&payload_ty, false)),
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "UploadPayloadParams");
    let out = buf.into_string();

    // Non-object JSON bodies have no property structure to hoist, so they
    // stay nested under `body` and forward via property shorthand.
    assert!(out.contains("const { body } = request;"));
    assert!(out.contains("body: body,"));
  }

  // ── render_zero_arg_builder ────────────────────────────────────────────────

  #[test]
  fn zero_arg_builder_renders_no_request_destructure() {
    let op = op_with(
      "ping",
      HttpMethod::Get,
      "/ping",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(256);
    render_zero_arg_builder(&mut buf, &op);
    let out = buf.into_string();

    assert!(out.starts_with("() => ({\n"));
    assert!(out.contains("method: 'GET',"));
    assert!(out.contains("url: `/ping`,"));
    assert!(!out.contains("request:"));
    assert!(!out.contains("headers"));
    assert!(!out.contains("body:"));
    assert!(!out.contains("params:"));
  }

  // ── render_request_interface ───────────────────────────────────────────────

  #[test]
  fn request_interface_renders_ref_body_as_nested_alongside_headers() {
    let str_ty = string_ty();
    let payload_ref = SchemaType::Ref("CreatePetPayload".into());
    let op = op_with(
      "createPet",
      HttpMethod::Post,
      "/pets",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![
          PlannedHeader {
            name: "X-Trace-Id".into(),
            optional: false,
            ty: &str_ty,
          },
          PlannedHeader {
            name: "X-Idempotency-Key".into(),
            optional: true,
            ty: &str_ty,
          },
        ],
        body: Some(nested_body(&payload_ref, false)),
      },
      None,
    );

    let mut buf = Writer::with_capacity(1024);
    render_request_interface(&mut buf, &op, "CreatePetParams");
    let out = buf.into_string();

    assert!(out.contains("export interface CreatePetParams"));
    // Ref body keeps its named type nested under the literal `body` slot.
    assert!(out.contains("body: CreatePetPayload;"));
    // Synthetic `headers` is required when any header is required, optional
    // only when all headers are optional. Mixed (one required) ⇒ required.
    assert!(out.contains("headers: {"));
    // Header names with `-` are quoted via safe_property_name.
    assert!(out.contains("'X-Trace-Id': string;"));
    assert!(out.contains("'X-Idempotency-Key'?: string;"));
  }

  #[test]
  fn request_interface_hoists_flat_json_body_properties_to_top_level() {
    // Smart-flatten: inline-object bodies surface as top-level fields,
    // matching the spec author's intent (loose parameter bag rather than
    // a named DTO).
    let str_ty = string_ty();
    let bool_ty = SchemaType::Scalar(SchemaScalar::Boolean);
    let op = op_with(
      "decide",
      HttpMethod::Post,
      "/decide",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: Some(flat_json_body(
          vec![
            body_field("csvImportId", false, &str_ty),
            body_field("doImport", false, &bool_ty),
          ],
          true,
        )),
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "DecideParams");
    let out = buf.into_string();

    assert!(out.contains("export interface DecideParams"));
    assert!(out.contains("csvImportId: string;"));
    assert!(out.contains("doImport: boolean;"));
    // No nested `body:` field for FlatJson — the properties are hoisted.
    assert!(!out.contains("body:"));
  }

  #[test]
  fn request_interface_marks_nested_body_optional_when_envelope_not_required() {
    let payload_ref = SchemaType::Ref("MaybePayload".into());
    let op = op_with(
      "savePet",
      HttpMethod::Put,
      "/pets",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: Some(nested_body(&payload_ref, true)),
      },
      None,
    );
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "SavePetParams");
    let out = buf.into_string();
    assert!(out.contains("body?: MaybePayload;"));
  }

  #[test]
  fn request_interface_marks_headers_optional_when_all_headers_optional() {
    let str_ty = string_ty();
    let op = op_with(
      "getPet",
      HttpMethod::Get,
      "/pets/{id}",
      PlannedRequestContract {
        fields: vec![path_field("id", &str_ty)],
        headers: vec![PlannedHeader {
          name: "X-Trace-Id".into(),
          optional: true,
          ty: &str_ty,
        }],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "GetPetParams");
    let out = buf.into_string();

    // All-optional headers ⇒ the synthetic `headers` field itself is `?:`.
    assert!(out.contains("headers?: {"));
  }

  #[test]
  fn request_interface_omits_headers_block_when_absent() {
    let str_ty = string_ty();
    let op = op_with(
      "getPet",
      HttpMethod::Get,
      "/pets/{id}",
      PlannedRequestContract {
        fields: vec![path_field("id", &str_ty)],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "GetPetParams");
    let out = buf.into_string();

    assert!(out.contains("id: string;"));
    assert!(!out.contains("headers"));
  }

  #[test]
  fn request_interface_renders_binary_as_blob_or_file_union() {
    let str_ty = string_ty();
    let binary = BodyFieldType::Binary;
    let op = op_with_multipart_fields_full(
      vec![path_field("petId", &str_ty)], // path
      vec![],                             // headers
      vec![("avatar", false, &binary)],   // form fields
    );
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("export interface OpParams"));
    assert!(out.contains("petId: string;"));
    assert!(out.contains("avatar: Blob | File;"));
  }

  #[test]
  fn request_interface_renders_array_of_binary_as_blob_or_file_array() {
    let arr_binary = BodyFieldType::ArrayOfBinary;
    let op = op_with_multipart_fields_full(vec![], vec![], vec![("galleries", false, &arr_binary)]);
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("galleries: (Blob | File)[];"));
  }

  #[test]
  fn request_interface_renders_optional_form_field_with_question_mark() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let op = op_with_multipart_fields_full(vec![], vec![], vec![("nickname", true, &scalar)]);
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    // Form fields hoist to top-level — no nested `body:` wrapper.
    assert!(out.contains("nickname?: string;"));
    assert!(!out.contains("body:"));
  }

  #[test]
  fn request_interface_renders_mixed_required_form_fields_at_top_level() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let op = op_with_multipart_fields_full(
      vec![],
      vec![],
      vec![("status", false, &scalar), ("nickname", true, &scalar)],
    );
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("status: string;"));
    assert!(out.contains("nickname?: string;"));
    assert!(!out.contains("body:"));
  }

  // ── path-template expansion ────────────────────────────────────────────────

  #[test]
  fn write_path_template_expands_every_placeholder() {
    let mut buf = Writer::with_capacity(128);
    write_path_template_into(&mut buf, "/pets/{petId}/owners/{ownerId}");
    assert_eq!(
      buf.into_string(),
      "/pets/${encodeURIComponent(petId)}/owners/${encodeURIComponent(ownerId)}"
    );
  }

  #[test]
  fn write_path_template_leaves_literal_paths_alone() {
    let mut buf = Writer::with_capacity(64);
    write_path_template_into(&mut buf, "/pets");
    assert_eq!(buf.into_string(), "/pets");
  }

  // ── multipart form-body builder ────────────────────────────────────────────

  #[test]
  fn multipart_builder_renders_required_scalar_as_unguarded_append() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let op = op_with_multipart_fields(vec![
      ("status", false /* optional? */, &scalar), // required
    ]);

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "OpParams");
    let out = buf.into_string();

    // Form fields are destructured directly from `request` (smart-flatten
    // hoists them to top-level) and referenced by bare identifier in the
    // FormData appends.
    assert!(out.contains("const { status } = request;"));
    assert!(out.contains("const fd = new FormData();"));
    assert!(out.contains("fd.append('status', String(status));"));
    assert!(!out.contains("if (status !==")); // required ⇒ no guard
  }

  #[test]
  fn multipart_builder_renders_optional_scalar_with_undefined_guard() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let op = op_with_multipart_fields(vec![
      ("nickname", true, &scalar), // optional
    ]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("if (nickname !== undefined) fd.append('nickname', String(nickname));"));
  }

  #[test]
  fn multipart_builder_renders_required_array_as_for_loop() {
    let arr = BodyFieldType::ArrayOfScalar(SchemaScalar::Number);
    let op = op_with_multipart_fields(vec![("tagIds", false, &arr)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("for (const v of tagIds) fd.append('tagIds', String(v));"));
    assert!(!out.contains("if (tagIds")); // required ⇒ no guard
  }

  #[test]
  fn multipart_builder_renders_required_binary_without_string_cast() {
    let binary = BodyFieldType::Binary;
    let op = op_with_multipart_fields(vec![("avatar", false, &binary)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("fd.append('avatar', avatar);"));
    assert!(!out.contains("String(avatar)"));
  }

  #[test]
  fn multipart_builder_renders_array_of_binary_as_for_loop_without_cast() {
    let arr_binary = BodyFieldType::ArrayOfBinary;
    let op = op_with_multipart_fields(vec![("galleries", false, &arr_binary)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("for (const v of galleries) fd.append('galleries', v);"));
    assert!(!out.contains("String(v)"));
  }

  // ── url-encoded form-body builder ──────────────────────────────────────────

  #[test]
  fn urlencoded_builder_uses_url_search_params_constructor() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let arr = BodyFieldType::ArrayOfScalar(SchemaScalar::Number);
    let op = op_with_urlencoded_fields(vec![("status", false, &scalar), ("tagIds", true, &arr)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, "OpParams");
    let out = buf.into_string();
    assert!(out.contains("const params = new URLSearchParams();"));
    assert!(out.contains("params.append('status', String(status));"));
    assert!(out.contains(
      "if (tagIds !== undefined) for (const v of tagIds) params.append('tagIds', String(v));"
    ));
  }
}
