mod operations;
pub(crate) mod schema;
mod semantic;
#[cfg(test)]
mod tests;
mod walk;

use std::collections::BTreeMap;

use crate::error::{Diagnostic, DiagnosticCode, Reporter};
use crate::ir::canonical::{ApiInfo, ApiModel};
use crate::ir::schema::SchemaType;
use crate::options::ResponseTypeMapping;
use crate::parse::openapi_model::{OpenApiDocument, Schema};
use operations::normalize_operations;
use schema::normalize_schemas;
pub(crate) use walk::SchemaWalk;

/// Hard cap on `Schema` nesting, enforced by [`SchemaWalk::check_depth`].
///
/// Real specs nest a handful of levels — the deepest committed fixture is
/// 5 layers of `allOf` — and the cap sits below serde's own recursion
/// limit of roughly 60, so a spec that reaches it is an unsupported shape
/// and not a parser-rejected one.
pub(crate) const MAX_NORMALIZE_DEPTH: u16 = 32;

pub(crate) fn normalize_api_model(
  document: &OpenApiDocument,
  response_type_mapping: &[ResponseTypeMapping],
  reporter: &Reporter,
) -> Result<ApiModel, Diagnostic> {
  let schemas = normalize_schemas(&document.components.schemas, reporter)?;
  let schema_index: BTreeMap<&str, &SchemaType> =
    schemas.iter().map(|m| (m.name.as_ref(), &m.body)).collect();
  let operations = normalize_operations(
    &document.paths,
    &schema_index,
    response_type_mapping,
    reporter,
  )?;

  let mut model = ApiModel {
    info: ApiInfo {
      spec_version: document.openapi.clone(),
      title: document.info.title.clone(),
    },
    schemas,
    operations,
  };

  semantic::finalize(&mut model, reporter)?;

  Ok(model)
}

/// Diagnostic for a spec shape outside the supported subset, pointing the
/// reader at the documented subset.
pub(crate) fn unsupported(reporter: &Reporter, detail: impl AsRef<str>) -> Diagnostic {
  reporter.error(
    DiagnosticCode::UnsupportedSemantic,
    format!(
      "Unsupported OpenAPI semantic shape: {}. See the supported subset documented in README.md ('Out of Scope' section).",
      detail.as_ref(),
    ),
  )
}

/// Diagnostic for a shape rejected by a rule that `detail` already names.
/// Appends no pointer to the documented subset.
pub(crate) fn unsupported_rule(reporter: &Reporter, detail: impl AsRef<str>) -> Diagnostic {
  reporter.error(
    DiagnosticCode::UnsupportedSemantic,
    format!("Unsupported OpenAPI semantic shape: {}", detail.as_ref()),
  )
}

/// Returns an [`unsupported`] diagnostic from the enclosing function.
macro_rules! bail_unsupported {
  ($reporter:expr, $($message:tt)*) => {
    return ::core::result::Result::Err($crate::ir::normalize::unsupported(
      $reporter,
      ::std::format!($($message)*),
    ))
  };
}

/// Returns an [`unsupported_rule`] diagnostic from the enclosing function.
macro_rules! bail_unsupported_rule {
  ($reporter:expr, $($message:tt)*) => {
    return ::core::result::Result::Err($crate::ir::normalize::unsupported_rule(
      $reporter,
      ::std::format!($($message)*),
    ))
  };
}

pub(crate) use {bail_unsupported, bail_unsupported_rule};

pub(crate) fn check_unsupported_not(
  schema: &Schema,
  walk: SchemaWalk<'_>,
) -> Result<(), Diagnostic> {
  if schema.not.is_some() {
    bail_unsupported!(
      walk.reporter(),
      "{} uses not, which is outside the supported subset.",
      walk.here()
    );
  }
  Ok(())
}

#[cfg(test)]
pub(crate) fn normalize_document(
  document: &serde_json::Value,
  reporter: &Reporter,
) -> Result<ApiModel, Diagnostic> {
  let doc: OpenApiDocument = serde_json::from_value(document.clone())
    .expect("test document must be a valid OpenApiDocument");
  normalize_api_model(&doc, &[], reporter)
}
