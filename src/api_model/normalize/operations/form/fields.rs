//! Classifying one form-body property into the type the request carries.

use std::collections::BTreeMap;

use crate::api_model::canonical::BodyFieldType;
use crate::api_model::schema::{SchemaScalar, SchemaType};
use crate::error::Diagnostic;
use crate::parse::openapi_model::Schema;

use super::super::URL_ENCODED;
use super::{BINARY, FormBody, FormKind, Reject, URLENCODED_BINARY_FIELD};

/// One body property's raw `format` hints: `own` from the property schema, `items` from its
/// array-item schema.
#[derive(Clone, Copy, Default)]
pub(super) struct RawPropertyFormat<'a> {
  pub(super) own: Option<&'a str>,
  pub(super) items: Option<&'a str>,
}

/// Collects the per-property `format` hints `SchemaType` does not carry.
#[must_use]
pub(super) fn collect_raw_property_formats(
  raw_schema: &Schema,
) -> BTreeMap<&str, RawPropertyFormat<'_>> {
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

/// Classifies one form-body property.
pub(super) fn classify_body_field_type(
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
#[must_use]
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
