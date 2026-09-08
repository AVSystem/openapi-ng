//! Request-body lowering: content-type dispatch onto JSON, multipart or
//! urlencoded.

use crate::error::{Context, Diagnostic, bail_policy};
use crate::ir::canonical::{BodyContent, RequestBodyDef};
use crate::ir::schema::SchemaType;
use crate::parse::openapi_model::RequestBody;

use super::super::schema::normalize_schema;
use super::super::{SchemaWalk, bail_unsupported, unsupported};
use super::OperationCx;
use super::form::{FormBody, FormKind, normalize_form_body_fields};

pub(super) fn normalize_request_body(
  request_body: Option<&RequestBody>,
  cx: OperationCx<'_>,
) -> Result<Option<RequestBodyDef>, Diagnostic> {
  let (method, path, reporter) = (cx.method(), cx.path(), cx.reporter());
  let Some(body) = request_body else {
    return Ok(None);
  };

  if body.content.len() > 1 {
    bail_policy!(
      reporter,
      "multi-content-body",
      "requestBody for {method} {path} must declare exactly one content type."
    );
  }

  let Some((mime, media)) = body.content.iter().next() else {
    return Ok(None);
  };
  // OpenAPI permits MIME case variation (`Application/JSON`).
  let mime_lc = mime.to_ascii_lowercase();

  let content = match mime_lc.as_str() {
    "application/json" => {
      let schema = media.schema.as_ref().ok_or_else(|| {
        unsupported(
          reporter,
          format!("requestBody for {method} {path} must define schema."),
        )
      })?;

      let walk = SchemaWalk::root(Context::RequestBody { method, path }, reporter);
      let ty = normalize_schema(schema, walk)?;

      if matches!(ty, SchemaType::Any) {
        bail_unsupported!(
          reporter,
          "requestBody for {method} {path} must define a concrete schema."
        );
      }

      BodyContent::Json(ty)
    }
    "multipart/form-data" => {
      let (body_ref, fields) = normalize_form_body_fields(
        media,
        FormBody::new(FormKind::Multipart, method, path, reporter),
        cx.schemas(),
      )?;
      BodyContent::Multipart { body_ref, fields }
    }
    "application/x-www-form-urlencoded" => {
      let (body_ref, fields) = normalize_form_body_fields(
        media,
        FormBody::new(FormKind::UrlEncoded, method, path, reporter),
        cx.schemas(),
      )?;
      BodyContent::UrlEncoded { body_ref, fields }
    }
    other => {
      bail_policy!(
        reporter,
        "unsupported-body-content-type",
        "requestBody for {method} {path}: unsupported content type {other:?}. Use application/json, multipart/form-data, or application/x-www-form-urlencoded."
      );
    }
  };

  Ok(Some(RequestBodyDef {
    required: body.required,
    content,
  }))
}

#[cfg(test)]
mod tests {
  use super::super::OperationCx;

  fn test_cx<'a>(
    schemas: &'a BTreeMap<&'a str, &'a SchemaType>,
    reporter: &'a crate::error::Reporter,
  ) -> OperationCx<'a> {
    OperationCx::new("POST", "/x", schemas, &[], reporter)
  }
  use std::collections::BTreeMap;

  use super::normalize_request_body;
  use crate::ir::schema::SchemaType;
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
