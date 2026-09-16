//! `$ref` resolution.

use crate::error::Diagnostic;

use super::super::{SchemaWalk, bail_unsupported};

const INTERNAL_SCHEMA_PREFIX: &str = "#/components/schemas/";

/// Returns the bare schema name a `$ref` targets.
pub(in crate::api_model::normalize::schema) fn normalize_reference(
  reference: &str,
  walk: SchemaWalk<'_>,
) -> Result<Box<str>, Diagnostic> {
  let Some(name) = reference.strip_prefix(INTERNAL_SCHEMA_PREFIX) else {
    bail_unsupported!(
      walk.reporter(),
      "{} uses unsupported reference {reference}.",
      walk.here()
    );
  };
  if name.is_empty() {
    bail_unsupported!(
      walk.reporter(),
      "{} $ref target name is empty (reference {reference}).",
      walk.here()
    );
  }
  Ok(Box::from(name))
}
