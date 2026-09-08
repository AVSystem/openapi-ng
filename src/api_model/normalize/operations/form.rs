//! `multipart/form-data` and `application/x-www-form-urlencoded` body
//! lowering: a flat, statically enumerated field list.

use std::collections::BTreeMap;

use crate::api_model::canonical::{BodyField, BodyFieldType};
use crate::api_model::schema::{SchemaProperty, SchemaScalar, SchemaType};
use crate::error::{Context, Diagnostic, Reporter, bail_policy};
use crate::identifier::Identifier;
use crate::parse::openapi_model::{AdditionalProperties, MediaType, Schema};

use super::super::schema::normalize_schema;
use super::super::{SchemaWalk, unsupported};
use super::URL_ENCODED;

/// The form flavour, the operation position and the diagnostic sink every
/// rejection message needs.
#[derive(Clone, Copy)]
pub(super) struct FormBody<'a> {
  kind: FormKind,
  method: &'a str,
  path: &'a str,
  reporter: &'a Reporter,
}

impl<'a> FormBody<'a> {
  pub(super) const fn new(
    kind: FormKind,
    method: &'a str,
    path: &'a str,
    reporter: &'a Reporter,
  ) -> Self {
    Self {
      kind,
      method,
      path,
      reporter,
    }
  }
}

/// Which form flavour a body declares.
#[derive(Clone, Copy)]
pub(super) enum FormKind {
  Multipart,
  UrlEncoded,
}

/// Why a form body or one of its fields was rejected. Paired with a
/// [`FormKind`] it names the stable subcode consumers route on.
#[derive(Clone, Copy)]
pub(super) enum Reject {
  /// The declared body schema does not resolve to an object.
  NonObjectBody,
  /// The body declares `additionalProperties`, so its fields are open-ended.
  OpenSchema,
  /// A field's type is an object.
  NestedObject,
  /// A field's type is a composition, a map, a nullable, or an array of
  /// something other than a scalar or binary.
  ComposedField,
}

impl FormKind {
  /// Label used in diagnostic prose.
  const fn label(self) -> &'static str {
    match self {
      Self::Multipart => "multipart",
      Self::UrlEncoded => "urlencoded",
    }
  }

  /// Stable kebab-case subcode for `reject` under this flavour.
  const fn subcode(self, reject: Reject) -> &'static str {
    match (self, reject) {
      (Self::Multipart, Reject::NonObjectBody) => "multipart-non-object-body",
      (Self::Multipart, Reject::OpenSchema) => "multipart-open-schema",
      (Self::Multipart, Reject::NestedObject) => "multipart-nested-object",
      (Self::Multipart, Reject::ComposedField) => "multipart-composed-field",
      (Self::UrlEncoded, Reject::NonObjectBody) => "urlencoded-non-object-body",
      (Self::UrlEncoded, Reject::OpenSchema) => "urlencoded-open-schema",
      (Self::UrlEncoded, Reject::NestedObject) => "urlencoded-nested-object",
      (Self::UrlEncoded, Reject::ComposedField) => "urlencoded-composed-field",
    }
  }
}

/// The `format` marking a binary field.
const BINARY: &str = "binary";

/// Subcode for a binary field in a urlencoded body, scalar or array.
const URLENCODED_BINARY_FIELD: &str = "urlencoded-binary-field";

/// Flattens a form body's schema into an alphabetically sorted field
/// list. The returned name is `Some` when the body was declared as a
/// top-level `$ref`. `format: binary` is detected for an inline body only.
pub(super) fn normalize_form_body_fields(
  media: &MediaType,
  body: FormBody<'_>,
  schema_index: &BTreeMap<&str, &SchemaType>,
) -> Result<(Option<Box<str>>, Vec<BodyField>), Diagnostic> {
  let raw_schema = declared_schema(media, body)?;
  reject_open_schema(raw_schema, body)?;

  let walk = SchemaWalk::root(
    Context::RequestBody {
      method: body.method,
      path: body.path,
    },
    body.reporter,
  );
  let normalized = normalize_schema(raw_schema, walk)?;
  let (body_ref, resolved) = match &normalized {
    SchemaType::Ref(name) => (
      Some(name.clone()),
      referenced_schema(name.as_ref(), body, schema_index)?,
    ),
    other => (None, other),
  };

  let SchemaType::InlineObject { properties } = resolved else {
    bail_policy!(
      body.reporter,
      body.kind.subcode(Reject::NonObjectBody),
      "requestBody for {} {}: {} body schema must resolve to an object.",
      body.method,
      body.path,
      body.kind.label(),
    );
  };

  let raw_formats = collect_raw_property_formats(raw_schema);
  let mut fields = properties
    .iter()
    .map(|property| {
      let raw_format = raw_formats
        .get(property.name.as_ref())
        .copied()
        .unwrap_or_default();
      body_field(property, raw_format, body)
    })
    .collect::<Result<Vec<BodyField>, Diagnostic>>()?;

  fields.sort_by(|left, right| left.name.cmp(&right.name));
  Ok((body_ref, fields))
}

fn declared_schema<'a>(media: &'a MediaType, body: FormBody<'_>) -> Result<&'a Schema, Diagnostic> {
  media.schema.as_ref().ok_or_else(|| {
    Diagnostic::policy_violation(
      body.reporter,
      "missing-body-schema",
      format!(
        "requestBody for {} {} must define schema.",
        body.method, body.path
      ),
    )
  })
}

/// Only `additionalProperties: false` and its absence leave the field set
/// closed.
fn reject_open_schema(raw_schema: &Schema, body: FormBody<'_>) -> Result<(), Diagnostic> {
  if let Some(additional) = &raw_schema.additional_properties
    && !matches!(additional, AdditionalProperties::Boolean(false))
  {
    bail_policy!(
      body.reporter,
      body.kind.subcode(Reject::OpenSchema),
      "requestBody for {} {}: {} bodies must not declare additionalProperties; every field must be enumerated.",
      body.method,
      body.path,
      body.kind.label(),
    );
  }
  Ok(())
}

/// The schema a top-level `$ref` names, from the document's schema index.
fn referenced_schema<'a>(
  name: &str,
  body: FormBody<'_>,
  schema_index: &'a BTreeMap<&str, &'a SchemaType>,
) -> Result<&'a SchemaType, Diagnostic> {
  schema_index.get(name).copied().ok_or_else(|| {
    unsupported(
      body.reporter,
      format!(
        "requestBody for {} {} references unknown schema '{name}'.",
        body.method, body.path
      ),
    )
  })
}

/// Lowers one body property.
fn body_field(
  property: &SchemaProperty,
  raw_format: RawPropertyFormat<'_>,
  body: FormBody<'_>,
) -> Result<BodyField, Diagnostic> {
  let FormBody {
    method,
    path,
    reporter,
    ..
  } = body;
  let Some(name) = Identifier::parse(property.name.as_ref()) else {
    bail_policy!(
      reporter,
      "invalid-form-field-name",
      "body field '{name}' in {method} {path}: name is not a valid JavaScript identifier. Rename the field or split this body into a non-generated client.",
      name = property.name.as_ref(),
    );
  };

  Ok(BodyField {
    name,
    required: property.required,
    field_type: classify_body_field_type(
      &property.schema,
      raw_format,
      property.name.as_ref(),
      body,
    )?,
  })
}

/// One body property's raw `format` hints: `own` from the property
/// schema, `items` from its array-item schema.
#[derive(Clone, Copy, Default)]
struct RawPropertyFormat<'a> {
  own: Option<&'a str>,
  items: Option<&'a str>,
}

/// Collects the per-property `format` hints `SchemaType` does not carry.
/// Empty when the body is a top-level `$ref`.
fn collect_raw_property_formats(raw_schema: &Schema) -> BTreeMap<&str, RawPropertyFormat<'_>> {
  raw_schema
    .properties
    .iter()
    .flat_map(|properties| properties.iter())
    .map(|(name, schema)| {
      let format = RawPropertyFormat {
        own: schema.format.as_deref(),
        items: schema
          .items
          .as_deref()
          .and_then(|item_schema| item_schema.format.as_deref()),
      };
      (name.as_str(), format)
    })
    .collect()
}

/// Classifies one form-body property. Accepts a scalar, a binary, or an
/// array of either; every other shape fails with the matching [`Reject`].
fn classify_body_field_type(
  schema: &SchemaType,
  raw_format: RawPropertyFormat<'_>,
  field_name: &str,
  body: FormBody<'_>,
) -> Result<BodyFieldType, Diagnostic> {
  match schema {
    SchemaType::Scalar(SchemaScalar::String) if raw_format.own == Some(BINARY) => {
      binary_field(BodyFieldType::Binary, "binary", field_name, body)
    }
    SchemaType::Array(inner)
      if matches!(inner.as_ref(), SchemaType::Scalar(SchemaScalar::String))
        && raw_format.items == Some(BINARY) =>
    {
      binary_field(
        BodyFieldType::ArrayOfBinary,
        "array-of-binary",
        field_name,
        body,
      )
    }
    SchemaType::Scalar(scalar) => Ok(BodyFieldType::Scalar(scalar.clone())),
    SchemaType::Array(inner) => match inner.as_ref() {
      SchemaType::Scalar(scalar) => Ok(BodyFieldType::ArrayOfScalar(scalar.clone())),
      _ => Err(reject_field(
        Reject::ComposedField,
        "array items must be scalar or binary.".to_string(),
        field_name,
        body,
      )),
    },
    SchemaType::InlineObject { .. } | SchemaType::Ref(_) => Err(reject_field(
      Reject::NestedObject,
      format!(
        "nested objects are not supported in {} bodies.",
        body.kind.label()
      ),
      field_name,
      body,
    )),
    _ => Err(reject_field(
      Reject::ComposedField,
      format!(
        "composed schemas are not supported in {} bodies.",
        body.kind.label()
      ),
      field_name,
      body,
    )),
  }
}

/// A binary field, which only multipart can carry.
fn binary_field(
  carried: BodyFieldType,
  label: &str,
  field_name: &str,
  body: FormBody<'_>,
) -> Result<BodyFieldType, Diagnostic> {
  match body.kind {
    FormKind::Multipart => Ok(carried),
    FormKind::UrlEncoded => Err(Diagnostic::policy_violation(
      body.reporter,
      URLENCODED_BINARY_FIELD,
      format!(
        "body field '{field_name}' in {} {}: {label} fields are not supported in {URL_ENCODED}.",
        body.method, body.path
      ),
    )),
  }
}

/// A rejected field, its `detail` appended to the field's position.
fn reject_field(
  reject: Reject,
  detail: String,
  field_name: &str,
  body: FormBody<'_>,
) -> Diagnostic {
  Diagnostic::policy_violation(
    body.reporter,
    body.kind.subcode(reject),
    format!(
      "body field '{field_name}' in {} {}: {detail}",
      body.method, body.path
    ),
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
  use super::URLENCODED_BINARY_FIELD;
  use std::collections::BTreeMap;

  use crate::api_model::canonical::{BodyContent, BodyFieldType};
  use crate::api_model::schema::{SchemaProperty, SchemaScalar, SchemaType};
  use crate::parse::openapi_model::RequestBody;
  use crate::test_support::test_reporter;

  use super::super::body::normalize_request_body;

  fn parse_request_body(yaml: &str) -> RequestBody {
    serde_yml::from_str(yaml).expect("fixture parses as RequestBody")
  }

  fn empty_schema_index<'a>() -> BTreeMap<&'a str, &'a SchemaType> {
    BTreeMap::new()
  }

  #[test]
  fn accepts_multipart_with_scalar_array_and_binary_fields() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      required: [status, avatar]
      properties:
        status: { type: string }
        tagIds: { type: array, items: { type: number } }
        avatar: { type: string, format: binary }
        nickname: { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let result = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect("normalize ok")
      .expect("body present");

    match result.content {
      BodyContent::Multipart { body_ref, fields } => {
        assert_eq!(body_ref, None);
        let names: Vec<&str> = fields.iter().map(|field| field.name.as_str()).collect();
        assert_eq!(names, vec!["avatar", "nickname", "status", "tagIds"]);
        let avatar = fields
          .iter()
          .find(|field| field.name.as_str() == "avatar")
          .unwrap();
        assert_eq!(avatar.field_type, BodyFieldType::Binary);
        assert!(avatar.required);
        let status = fields
          .iter()
          .find(|field| field.name.as_str() == "status")
          .unwrap();
        assert!(matches!(
          status.field_type,
          BodyFieldType::Scalar(SchemaScalar::String)
        ));
        assert!(status.required);
        let nickname = fields
          .iter()
          .find(|field| field.name.as_str() == "nickname")
          .unwrap();
        assert!(!nickname.required);
        let tag_ids = fields
          .iter()
          .find(|field| field.name.as_str() == "tagIds")
          .unwrap();
        assert!(matches!(
          tag_ids.field_type,
          BodyFieldType::ArrayOfScalar(SchemaScalar::Number)
        ));
      }
      other => panic!("expected Multipart, got {other:?}"),
    }
  }

  #[test]
  fn accepts_multipart_with_array_of_binary_fields() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      required: [galleries]
      properties:
        galleries: { type: array, items: { type: string, format: binary } }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let result = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect("normalize ok")
      .expect("body present");

    match result.content {
      BodyContent::Multipart { fields, .. } => {
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_type, BodyFieldType::ArrayOfBinary);
      }
      other => panic!("expected Multipart, got {other:?}"),
    }
  }

  #[test]
  fn accepts_multipart_with_ref_to_named_object() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      $ref: '#/components/schemas/UploadForm'
"#;
    let body = parse_request_body(yaml);
    let upload_form_body = SchemaType::InlineObject {
      properties: vec![SchemaProperty {
        name: "status".into(),
        required: true,
        schema: SchemaType::Scalar(SchemaScalar::String),
        description: None,
        deprecated: false,
      }],
    };
    let schema_index = BTreeMap::from([("UploadForm", &upload_form_body)]);
    let ctx = test_reporter();
    let result = normalize_request_body(Some(&body), test_cx(&schema_index, &ctx))
      .expect("normalize ok")
      .expect("body present");

    match result.content {
      BodyContent::Multipart { body_ref, fields } => {
        assert_eq!(body_ref.as_deref(), Some("UploadForm"));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.as_str(), "status");
      }
      other => panic!("expected Multipart, got {other:?}"),
    }
  }

  #[test]
  fn rejects_multipart_with_nested_object_field() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      properties:
        metadata:
          type: object
          properties:
            authorId: { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("nested object should fail");
    assert_eq!(err.subcode, Some("multipart-nested-object"));
  }

  #[test]
  fn rejects_multipart_with_composed_field() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      properties:
        variant:
          oneOf:
            - { type: string }
            - { type: number }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("composed field should fail");
    assert_eq!(err.subcode, Some("multipart-composed-field"));
  }

  #[test]
  fn rejects_multipart_with_additional_properties_true() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      additionalProperties: true
      properties:
        status: { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("open schema should fail");
    assert_eq!(err.subcode, Some("multipart-open-schema"));
  }

  #[test]
  fn rejects_multipart_with_non_object_top_level_schema() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: string
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("non-object body should fail");
    assert_eq!(err.subcode, Some("multipart-non-object-body"));
  }

  #[test]
  fn accepts_urlencoded_with_scalar_and_array_of_scalar_fields() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      required: [status]
      properties:
        status: { type: string }
        tagIds: { type: array, items: { type: number } }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let result = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect("normalize ok")
      .expect("body present");

    match result.content {
      BodyContent::UrlEncoded { fields, .. } => {
        assert_eq!(
          fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
          vec!["status", "tagIds"]
        );
      }
      other => panic!("expected UrlEncoded, got {other:?}"),
    }
  }

  #[test]
  fn rejects_urlencoded_with_binary_field() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        avatar: { type: string, format: binary }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("binary in urlencoded should fail");
    assert_eq!(err.subcode, Some(URLENCODED_BINARY_FIELD));
  }

  #[test]
  fn rejects_urlencoded_with_nested_object_field() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        metadata:
          type: object
          properties:
            authorId: { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("nested object should fail");
    assert_eq!(err.subcode, Some("urlencoded-nested-object"));
  }

  #[test]
  fn rejects_urlencoded_with_composed_field() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        variant:
          oneOf:
            - { type: string }
            - { type: number }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("composed field should fail");
    assert_eq!(err.subcode, Some("urlencoded-composed-field"));
  }

  #[test]
  fn rejects_urlencoded_with_non_object_top_level_schema() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: string
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("non-object urlencoded body should fail");
    assert_eq!(err.subcode, Some("urlencoded-non-object-body"));
  }

  #[test]
  fn rejects_urlencoded_with_additional_properties_true() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      additionalProperties: true
      properties:
        status: { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("open urlencoded schema should fail");
    assert_eq!(err.subcode, Some("urlencoded-open-schema"));
  }

  #[test]
  fn rejects_multipart_with_invalid_field_name_kebab_case() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      properties:
        x-y: { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("kebab-case field name must reject");
    assert_eq!(err.subcode, Some("invalid-form-field-name"));
  }

  #[test]
  fn rejects_urlencoded_with_invalid_field_name_digits_first() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        "1foo": { type: string }
"#;
    let body = parse_request_body(yaml);
    let ctx = test_reporter();
    let err = normalize_request_body(Some(&body), test_cx(&empty_schema_index(), &ctx))
      .expect_err("digits-first field name must reject");
    assert_eq!(err.subcode, Some("invalid-form-field-name"));
  }
}
