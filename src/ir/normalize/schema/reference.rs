//! `$ref` resolution. The supported form is an internal reference into
//! `components.schemas`.

use crate::error::Diagnostic;

use super::super::{SchemaWalk, bail_unsupported};

const INTERNAL_SCHEMA_PREFIX: &str = "#/components/schemas/";

/// Returns the bare schema name a `$ref` targets.
///
/// Rejects a reference outside `components.schemas` — an external file, a
/// URL, another component section — and one whose target name is empty.
pub(in crate::ir::normalize::schema) fn normalize_reference(
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
