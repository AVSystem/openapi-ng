//! The `{Pascal}Params` and `{Pascal}Error` interfaces an operation declares.

use crate::emit::ts::{
  Doc, Member, Position, Render, Writer, interface_block, member_declaration, w,
};
use crate::identifier::TypeName;
use crate::plan::artifact_plan::{
  PlannedFormField, PlannedHeader, PlannedOperation, PlannedRequestBody, PlannedRequestContract,
  PlannedRequestField,
};

pub(crate) fn render_request_interface(
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
pub(super) fn request_members<'a>(
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

#[must_use]
fn field_member<'a>(field: &'a PlannedRequestField<'a>) -> Member<'a> {
  Member {
    name: field.name.as_ref(),
    optional: field.optional,
    type_expr: field.schema,
    doc: Doc::default(),
  }
}

#[must_use]
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
pub(crate) fn render_error_interface(
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
pub(super) struct HeaderObject<'a>(pub(super) &'a [PlannedHeader<'a>]);

impl<'a> HeaderObject<'a> {
  #[must_use]
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
