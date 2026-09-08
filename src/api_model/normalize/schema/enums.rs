//! `enum` lowering. Only string enums are supported; they become a
//! TypeScript literal union.

use crate::error::Diagnostic;

use super::super::{SchemaWalk, bail_unsupported};
use crate::parse::openapi_model::Schema;

/// Collects the enum's values, rejecting a non-string member or a value
/// carrying a null byte.
pub(super) fn normalize_string_enum(
  values: &[serde_json::Value],
  walk: SchemaWalk<'_>,
) -> Result<Vec<String>, Diagnostic> {
  values
    .iter()
    .map(|entry| {
      let Some(value) = entry.as_str() else {
        bail_unsupported!(
          walk.reporter(),
          "{} enum must contain only strings.",
          walk.here()
        );
      };
      if value.contains('\u{0000}') {
        bail_unsupported!(
          walk.reporter(),
          "{} enum values must not contain null bytes.",
          walk.here()
        );
      }
      Ok(value.to_string())
    })
    .collect()
}

/// Accepts `type: string` or an absent `type`; every other declared type
/// rejects.
pub(super) fn validate_string_enum_type(
  schema: &Schema,
  walk: SchemaWalk<'_>,
) -> Result<(), Diagnostic> {
  match schema.type_.as_deref() {
    Some("string") | None => Ok(()),
    Some(other) => bail_unsupported!(
      walk.reporter(),
      "{} enum is supported only for string schemas, found type {other}.",
      walk.here()
    ),
  }
}
