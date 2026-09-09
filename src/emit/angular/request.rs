use crate::api_model::canonical::BodyFieldType;
use crate::emit::ts::{
  Doc, Member, Position, Render, Writer, interface_block, member_declaration, w, wln,
  write_separated,
};
use crate::identifier::TypeName;
use crate::plan::artifact_plan::{
  PlannedFormField, PlannedHeader, PlannedOperation, PlannedRequestBody, PlannedRequestContract,
  PlannedRequestField, RequestFieldKind,
};

/// Which runtime constructor the form-body IIFE builds.
#[derive(Clone, Copy)]
enum FormKind {
  Multipart,
  UrlEncoded,
}

pub(super) fn render_requestful_builder(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  interface_name: &TypeName,
) {
  buffer.open_block(&format!("(request: {interface_name}) =>"));

  let headers = HeaderObject(&operation.request.headers);
  let destructured: Vec<&str> = request_members(&operation.request, &headers)
    .map(|member| member.name)
    .collect();
  if !destructured.is_empty() {
    wln!(buffer, "const {{ {} }} = request;", destructured.join(", "));
  }

  buffer.open_block("return");
  wln!(buffer, "method: '{}',", operation.method);
  write_path_template_line(buffer, &operation.path);
  write_params_line(buffer, operation);
  write_body_line(buffer, operation);
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

/// Writes the `url:` line, expanding each `{name}` placeholder.
fn write_path_template_line(buffer: &mut Writer, path: &str) {
  buffer.push("url: `");
  write_path_template_into(buffer, path);
  buffer.push("`,\n");
}

pub(super) fn render_request_interface(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  request_name: &TypeName,
) {
  let headers = HeaderObject(&operation.request.headers);
  interface_block(
    buffer,
    request_name.as_str(),
    Doc::default(),
    request_members(&operation.request, &headers),
    true,
  );
}

/// The interface's members, in emitted order: fields, body, `headers`.
fn request_members<'a>(
  request: &'a PlannedRequestContract<'a>,
  headers: &'a HeaderObject<'a>,
) -> impl Iterator<Item = Member<'a>> {
  request
    .fields
    .iter()
    .map(field_member)
    .chain(body_members(request.body.as_ref()))
    .chain(headers.member())
}

fn field_member<'a>(field: &'a PlannedRequestField<'a>) -> Member<'a> {
  Member {
    name: field.name.as_ref(),
    optional: field.optional,
    type_expr: field.schema,
    doc: Doc::default(),
  }
}

fn form_member<'a>(field: &'a PlannedFormField<'a>) -> Member<'a> {
  Member {
    name: field.name.as_str(),
    optional: field.optional,
    type_expr: field.field_type,
    doc: Doc::default(),
  }
}

/// The members a body contributes; at most one arm is non-empty.
fn body_members<'a>(body: Option<&'a PlannedRequestBody<'a>>) -> impl Iterator<Item = Member<'a>> {
  let nested = body.and_then(|body| match body {
    PlannedRequestBody::Nested { schema, optional } => Some(Member {
      name: "body",
      optional: *optional,
      type_expr: *schema,
      doc: Doc::default(),
    }),
    _ => None,
  });
  let hoisted_json = body.and_then(|body| match body {
    PlannedRequestBody::FlatJson { properties, .. } => Some(properties.iter().map(field_member)),
    _ => None,
  });
  let hoisted_form = body.and_then(|body| match body {
    PlannedRequestBody::Multipart { fields } | PlannedRequestBody::UrlEncoded { fields } => {
      Some(fields.iter().map(form_member))
    }
    _ => None,
  });

  nested
    .into_iter()
    .chain(hoisted_json.into_iter().flatten())
    .chain(hoisted_form.into_iter().flatten())
}

/// Emits an operation's error interface: its body types keyed by status.
pub(super) fn render_error_interface(
  buffer: &mut Writer,
  operation: &PlannedOperation<'_>,
  error_name: &TypeName,
) {
  buffer.open_block(&format!("export interface {error_name}"));
  operation.errors.iter().for_each(|error| {
    w!(buffer, "{}: ", error.status);
    error.body.render(buffer, Position::Standalone);
    buffer.push(";\n");
  });
  buffer.close_block("");
}

/// The synthetic `headers` member's inline object type.
struct HeaderObject<'a>(&'a [PlannedHeader<'a>]);

impl<'a> HeaderObject<'a> {
  fn member(&'a self) -> Option<Member<'a>> {
    (!self.0.is_empty()).then(|| Member {
      name: "headers",
      optional: self.0.iter().all(|header| header.optional),
      type_expr: self,
      doc: Doc::default(),
    })
  }
}

impl Render for HeaderObject<'_> {
  fn render(&self, out: &mut Writer, _at: Position) {
    out.inline_block(|out| {
      self.0.iter().for_each(|header| {
        member_declaration(out, header.name.as_ref(), header.optional, &header.schema);
      });
    });
  }
}

/// Writes `params: httpParams({ … }),` when the operation declares query
/// parameters, including when every one of them is optional.
fn write_params_line(buffer: &mut Writer, operation: &PlannedOperation<'_>) {
  let mut query = operation
    .request
    .fields
    .iter()
    .filter(|field| field.kind == RequestFieldKind::Query)
    .map(|field| field.name.as_ref())
    .peekable();
  if query.peek().is_none() {
    return;
  }

  buffer.push("params: httpParams({ ");
  write_separated(buffer, query, ", ", Writer::push);
  buffer.push(" }),\n");
}

/// Writes the `body: …,` line for whichever body layout the operation
/// declares.
fn write_body_line(buffer: &mut Writer, operation: &PlannedOperation<'_>) {
  let Some(body) = operation.request.body.as_ref() else {
    return;
  };
  match body {
    PlannedRequestBody::Nested { .. } => buffer.push("body: body,\n"),
    PlannedRequestBody::FlatJson { properties, .. } => {
      buffer.push("body: { ");
      write_separated(buffer, properties, ", ", |out, property| {
        out.push(property.name.as_ref());
      });
      buffer.push(" },\n");
    }
    PlannedRequestBody::Multipart { fields } => {
      write_form_body(buffer, fields, FormKind::Multipart);
    }
    PlannedRequestBody::UrlEncoded { fields } => {
      write_form_body(buffer, fields, FormKind::UrlEncoded);
    }
  }
}

/// Writes the IIFE that materializes a form-body payload. A scalar field
/// is wrapped in `String(…)`, a binary passes through, and an optional
/// one is guarded so an absent value leaves its key out.
fn write_form_body(buffer: &mut Writer, fields: &[PlannedFormField<'_>], kind: FormKind) {
  let (constructor, variable, ts_type) = match kind {
    FormKind::Multipart => ("new FormData()", "fd", "FormData"),
    FormKind::UrlEncoded => ("new URLSearchParams()", "params", "URLSearchParams"),
  };

  wln!(buffer, "body: ((): {ts_type} => {{");
  buffer.indent();
  wln!(buffer, "const {variable} = {constructor};");
  fields.iter().for_each(|field| {
    let name = field.name.as_str();
    if field.optional {
      w!(buffer, "if ({name} !== undefined) ");
    }
    match field.field_type {
      BodyFieldType::Scalar(_) => {
        wln!(buffer, "{variable}.append('{name}', String({name}));");
      }
      BodyFieldType::ArrayOfScalar(_) => {
        wln!(
          buffer,
          "for (const v of {name}) {variable}.append('{name}', String(v));"
        );
      }
      BodyFieldType::Binary => wln!(buffer, "{variable}.append('{name}', {name});"),
      BodyFieldType::ArrayOfBinary => {
        wln!(
          buffer,
          "for (const v of {name}) {variable}.append('{name}', v);"
        );
      }
    }
  });
  wln!(buffer, "return {variable};");
  buffer.dedent();
  buffer.push("})(),\n");
}

/// Writes `path` into `buffer`, expanding each `{name}` placeholder to
/// `${encodeURIComponent(name)}`. `validate_path_template` has already
/// balanced the braces; an unmatched `{` emits the remainder verbatim.
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

  fn type_name(name: &str) -> TypeName {
    TypeName::new(name.to_string())
  }
  use crate::api_model::canonical::{BodyFieldType, ErrorResponse, HttpMethod};
  use crate::api_model::schema::{SchemaProperty, SchemaScalar, SchemaType};
  use crate::plan::artifact_plan::{PlannedHeader, PlannedRequestContract};
  use crate::test_support::{
    body_field, flat_json_body, nested_body, op_with, op_with_errors, op_with_multipart_fields,
    op_with_multipart_fields_full, op_with_urlencoded_fields, path_field, query_field,
    string_schema,
  };

  fn render_errors(error_name: &str, errors: &[ErrorResponse]) -> String {
    let op = op_with_errors("op", errors);
    let mut buf = Writer::with_capacity(256);
    render_error_interface(&mut buf, &op, &type_name(error_name));
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
        schema: SchemaType::Scalar(SchemaScalar::String),
        description: None,
        deprecated: false,
      }],
    };
    let errors = vec![ErrorResponse { status: 422, body }];
    let out = render_errors("CreatePetError", &errors);
    assert!(out.contains("422: {"));
    assert!(out.contains("code: string;"));
  }

  #[test]
  fn requestful_builder_renders_get_with_path_param_only() {
    let schema = string_schema();
    let op = op_with(
      "getPet",
      HttpMethod::Get,
      "/pets/{petId}",
      PlannedRequestContract {
        fields: vec![path_field("petId", &schema)],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("GetPetParams"));
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
    let str_schema = string_schema();
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
          schema: &str_schema,
        }],
        body: Some(nested_body(&body_ref, false)),
      },
      None,
    );

    let mut buf = Writer::with_capacity(1024);
    render_requestful_builder(&mut buf, &op, &type_name("CreatePetParams"));
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
    // Inline JSON object bodies hoist their properties to top-level
    // fields, re-assembled into an object literal at the `body:` slot.
    let str_schema = string_schema();
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
            body_field("csvImportId", false, &str_schema),
            body_field("doImport", false, &bool_ty),
          ],
          true,
        )),
      },
      None,
    );

    let mut buf = Writer::with_capacity(1024);
    render_requestful_builder(&mut buf, &op, &type_name("DecideParams"));
    let out = buf.into_string();

    assert!(out.contains("const { csvImportId, doImport } = request;"));
    assert!(out.contains("body: { csvImportId, doImport },"));
  }

  #[test]
  fn requestful_builder_renders_query_params_via_http_params() {
    let str_schema = string_schema();
    let op = op_with(
      "listPets",
      HttpMethod::Get,
      "/pets",
      PlannedRequestContract {
        fields: vec![
          query_field("limit", true, &str_schema),
          query_field("offset", true, &str_schema),
        ],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("ListPetsParams"));
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
    render_requestful_builder(&mut buf, &op, &type_name("UploadPayloadParams"));
    let out = buf.into_string();

    // Non-object JSON bodies stay nested under `body`.
    assert!(out.contains("const { body } = request;"));
    assert!(out.contains("body: body,"));
  }

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

  #[test]
  fn request_interface_renders_ref_body_as_nested_alongside_headers() {
    let str_schema = string_schema();
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
            schema: &str_schema,
          },
          PlannedHeader {
            name: "X-Idempotency-Key".into(),
            optional: true,
            schema: &str_schema,
          },
        ],
        body: Some(nested_body(&payload_ref, false)),
      },
      None,
    );

    let mut buf = Writer::with_capacity(1024);
    render_request_interface(&mut buf, &op, &type_name("CreatePetParams"));
    let out = buf.into_string();

    assert!(out.contains("export interface CreatePetParams"));
    // Ref body keeps its named type nested under the literal `body` slot.
    assert!(out.contains("body: CreatePetPayload;"));
    // Synthetic `headers` is optional only when every header is.
    assert!(out.contains("headers: {"));
    // Header names with `-` are quoted via safe_property_name.
    assert!(out.contains("'X-Trace-Id': string;"));
    assert!(out.contains("'X-Idempotency-Key'?: string;"));
  }

  #[test]
  fn request_interface_hoists_flat_json_body_properties_to_top_level() {
    let str_schema = string_schema();
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
            body_field("csvImportId", false, &str_schema),
            body_field("doImport", false, &bool_ty),
          ],
          true,
        )),
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, &type_name("DecideParams"));
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
    render_request_interface(&mut buf, &op, &type_name("SavePetParams"));
    let out = buf.into_string();
    assert!(out.contains("body?: MaybePayload;"));
  }

  #[test]
  fn request_interface_marks_headers_optional_when_all_headers_optional() {
    let str_schema = string_schema();
    let op = op_with(
      "getPet",
      HttpMethod::Get,
      "/pets/{id}",
      PlannedRequestContract {
        fields: vec![path_field("id", &str_schema)],
        headers: vec![PlannedHeader {
          name: "X-Trace-Id".into(),
          optional: true,
          schema: &str_schema,
        }],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, &type_name("GetPetParams"));
    let out = buf.into_string();

    // All-optional headers ⇒ the synthetic `headers` field itself is `?:`.
    assert!(out.contains("headers?: {"));
  }

  #[test]
  fn request_interface_omits_headers_block_when_absent() {
    let str_schema = string_schema();
    let op = op_with(
      "getPet",
      HttpMethod::Get,
      "/pets/{id}",
      PlannedRequestContract {
        fields: vec![path_field("id", &str_schema)],
        headers: vec![],
        body: None,
      },
      None,
    );

    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, &type_name("GetPetParams"));
    let out = buf.into_string();

    assert!(out.contains("id: string;"));
    assert!(!out.contains("headers"));
  }

  #[test]
  fn request_interface_renders_binary_as_blob_or_file_union() {
    let str_schema = string_schema();
    let binary = BodyFieldType::Binary;
    let op = op_with_multipart_fields_full(
      vec![path_field("petId", &str_schema)], // path
      vec![],                                 // headers
      vec![("avatar", false, &binary)],       // form fields
    );
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, &type_name("OpParams"));
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
    render_request_interface(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("galleries: (Blob | File)[];"));
  }

  #[test]
  fn request_interface_renders_optional_form_field_with_question_mark() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let op = op_with_multipart_fields_full(vec![], vec![], vec![("nickname", true, &scalar)]);
    let mut buf = Writer::with_capacity(512);
    render_request_interface(&mut buf, &op, &type_name("OpParams"));
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
    render_request_interface(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("status: string;"));
    assert!(out.contains("nickname?: string;"));
    assert!(!out.contains("body:"));
  }

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

  #[test]
  fn multipart_builder_renders_required_scalar_as_unguarded_append() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let op = op_with_multipart_fields(vec![
      ("status", false /* optional? */, &scalar), // required
    ]);

    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();

    // Hoisted form fields destructure from `request` and are referenced
    // by bare identifier in the appends.
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
    render_requestful_builder(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("if (nickname !== undefined) fd.append('nickname', String(nickname));"));
  }

  #[test]
  fn multipart_builder_renders_required_array_as_for_loop() {
    let arr = BodyFieldType::ArrayOfScalar(SchemaScalar::Number);
    let op = op_with_multipart_fields(vec![("tagIds", false, &arr)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("for (const v of tagIds) fd.append('tagIds', String(v));"));
    assert!(!out.contains("if (tagIds")); // required ⇒ no guard
  }

  #[test]
  fn multipart_builder_renders_required_binary_without_string_cast() {
    let binary = BodyFieldType::Binary;
    let op = op_with_multipart_fields(vec![("avatar", false, &binary)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("fd.append('avatar', avatar);"));
    assert!(!out.contains("String(avatar)"));
  }

  #[test]
  fn multipart_builder_renders_array_of_binary_as_for_loop_without_cast() {
    let arr_binary = BodyFieldType::ArrayOfBinary;
    let op = op_with_multipart_fields(vec![("galleries", false, &arr_binary)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("for (const v of galleries) fd.append('galleries', v);"));
    assert!(!out.contains("String(v)"));
  }

  #[test]
  fn urlencoded_builder_uses_url_search_params_constructor() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let arr = BodyFieldType::ArrayOfScalar(SchemaScalar::Number);
    let op = op_with_urlencoded_fields(vec![("status", false, &scalar), ("tagIds", true, &arr)]);
    let mut buf = Writer::with_capacity(512);
    render_requestful_builder(&mut buf, &op, &type_name("OpParams"));
    let out = buf.into_string();
    assert!(out.contains("const params = new URLSearchParams();"));
    assert!(out.contains("params.append('status', String(status));"));
    assert!(out.contains(
      "if (tagIds !== undefined) for (const v of tagIds) params.append('tagIds', String(v));"
    ));
  }
}
