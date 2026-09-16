//! The `params:`, `body:` and `url:` lines inside a request builder.

use crate::api_model::canonical::BodyFieldType;
use crate::emit::ts::{Writer, w, wln, write_separated};
use crate::plan::artifact_plan::{
  PlannedFormField, PlannedOperation, PlannedRequestBody, RequestFieldKind,
};

use super::FormKind;

pub(super) fn write_params_line(buffer: &mut Writer, operation: &PlannedOperation<'_>) {
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

/// Writes the `body: …,` line for whichever body layout the operation declares.
pub(super) fn write_body_line(buffer: &mut Writer, operation: &PlannedOperation<'_>) {
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

/// Writes the IIFE that materializes a form-body payload.
pub(super) fn write_form_body(
  buffer: &mut Writer,
  fields: &[PlannedFormField<'_>],
  kind: FormKind,
) {
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

/// Writes `path`, expanding each `{name}` to `${encodeURIComponent(name)}`.
/// An unmatched `{` emits the remainder verbatim, so adversarial IR yields
/// wrong output instead of a panic across the NAPI boundary.
pub(super) fn write_path_template_into(buffer: &mut Writer, path: &str) {
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
