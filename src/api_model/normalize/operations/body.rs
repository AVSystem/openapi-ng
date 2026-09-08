//! Request-body lowering: content-type dispatch onto JSON, multipart or
//! urlencoded.

use crate::api_model::canonical::{BodyContent, BodyField, RequestBodyDef};
use crate::api_model::schema::SchemaType;
use crate::error::{Context, Diagnostic, bail_policy};
use crate::parse::openapi_model::{MediaType, RequestBody};

use super::super::schema::normalize_schema;
use super::super::{SchemaWalk, bail_unsupported, unsupported};
use super::form::{FormBody, FormKind, normalize_form_body_fields};
use super::{JSON, LoweringContext, MULTIPART, URL_ENCODED};

pub(super) fn normalize_request_body(
  request_body: Option<&RequestBody>,
  context: LoweringContext<'_>,
) -> Result<Option<RequestBodyDef>, Diagnostic> {
  let Some(body) = request_body else {
    return Ok(None);
  };

  if body.content.len() > 1 {
    bail_policy!(
      context.reporter(),
      "multi-content-body",
      "requestBody for {} {} must declare exactly one content type.",
      context.method(),
      context.path()
    );
  }

  let Some((mime, media)) = body.content.iter().next() else {
    return Ok(None);
  };

  Ok(Some(RequestBodyDef {
    required: body.required,
    // OpenAPI permits MIME case variation (`Application/JSON`).
    content: normalize_content(&mime.to_ascii_lowercase(), media, context)?,
  }))
}

fn normalize_content(
  mime: &str,
  media: &MediaType,
  context: LoweringContext<'_>,
) -> Result<BodyContent, Diagnostic> {
  match mime {
    JSON => normalize_json_body(media, context).map(BodyContent::Json),
    MULTIPART => {
      let (body_ref, fields) = normalize_form(FormKind::Multipart, media, context)?;
      Ok(BodyContent::Multipart { body_ref, fields })
    }
    URL_ENCODED => {
      let (body_ref, fields) = normalize_form(FormKind::UrlEncoded, media, context)?;
      Ok(BodyContent::UrlEncoded { body_ref, fields })
    }
    other => bail_policy!(
      context.reporter(),
      "unsupported-body-content-type",
      "requestBody for {} {}: unsupported content type {other:?}. Use {JSON}, {MULTIPART}, or {URL_ENCODED}.",
      context.method(),
      context.path()
    ),
  }
}

/// Rejects a JSON body that declares no schema, or one no more precise than
/// `{}`.
fn normalize_json_body(
  media: &MediaType,
  context: LoweringContext<'_>,
) -> Result<SchemaType, Diagnostic> {
  let (method, path, reporter) = (context.method(), context.path(), context.reporter());
  let declared = media.schema.as_ref().ok_or_else(|| {
    unsupported(
      reporter,
      format!("requestBody for {method} {path} must define schema."),
    )
  })?;

  let walk = SchemaWalk::root(Context::RequestBody { method, path }, reporter);
  let schema = normalize_schema(declared, walk)?;

  if matches!(schema, SchemaType::Any) {
    bail_unsupported!(
      reporter,
      "requestBody for {method} {path} must define a concrete schema."
    );
  }
  Ok(schema)
}

fn normalize_form(
  kind: FormKind,
  media: &MediaType,
  context: LoweringContext<'_>,
) -> Result<(Option<Box<str>>, Vec<BodyField>), Diagnostic> {
  normalize_form_body_fields(
    media,
    FormBody::new(kind, context.method(), context.path(), context.reporter()),
    context.schemas(),
  )
}

#[cfg(test)]
mod tests {
  use super::super::LoweringContext;

  fn test_cx<'a>(
    schemas: &'a BTreeMap<&'a str, &'a SchemaType>,
    reporter: &'a crate::error::Reporter,
  ) -> LoweringContext<'a> {
    LoweringContext::new("POST", "/x", schemas, &[], reporter)
  }
  use std::collections::BTreeMap;

  use super::normalize_request_body;
  use crate::api_model::schema::SchemaType;
  use crate::parse::openapi_model::RequestBody;
  use crate::test_support::test_reporter;

  fn parse_request_body(yaml: &str) -> RequestBody {
    serde_yml::from_str(yaml).expect("fixture parses as RequestBody")
  }

  fn empty_schema_index<'a>() -> BTreeMap<&'a str, &'a SchemaType> {
    BTreeMap::new()
  }

  #[test]
  fn rejects_body_with_multiple_content_types() {
    let yaml = r#"
content:
  application/json:
    schema: { type: object, properties: { x: { type: string } } }
  multipart/form-data:
    schema: { type: object, properties: { x: { type: string } } }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("multi-content should fail");
    assert_eq!(err.subcode, Some("multi-content-body"));
  }

  #[test]
  fn rejects_unsupported_body_content_type() {
    let yaml = r#"
content:
  application/xml:
    schema: { type: object, properties: { x: { type: string } } }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("xml body should fail");
    assert_eq!(err.subcode, Some("unsupported-body-content-type"));
  }
}
