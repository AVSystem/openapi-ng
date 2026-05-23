mod operations;
pub(crate) mod schema;
mod semantic;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::error::{Context, Diagnostic, DiagnosticCode, Reporter};
use crate::ir::canonical::{ApiInfo, ApiModel};
use crate::ir::schema::SchemaType;
use crate::options::ResponseTypeMapping;
use crate::parse::openapi_model::{OpenApiDocument, Schema};
use operations::normalize_operations;
use schema::normalize_schemas;

/// Hard cap on `Schema` recursion during normalize. Realistic OpenAPI
/// specs nest a handful of levels (the deepest committed fixture is
/// 5 layers of allOf); a value of 32 leaves a healthy margin above that
/// while still rejecting pathological / cyclic specs before they
/// overflow the thread stack. The cap sits below the serde YAML/JSON
/// recursion limit (~60), so any spec that reaches this guard already
/// represents an unsupported shape rather than a parser-rejected one.
///
/// Threaded as a `u16` argument through the recursive callers in
/// `schema.rs` — operations.rs starts each schema walk at depth 0.
pub(crate) const MAX_NORMALIZE_DEPTH: u16 = 32;

pub(crate) fn normalize_api_model(
  document: &OpenApiDocument,
  response_type_mapping: &[ResponseTypeMapping],
  reporter: &mut Reporter<'_>,
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

  // Final semantic step: sort schemas, narrow discriminator member
  // properties for TS emit, and validate `$ref` resolution.
  semantic::finalize(&mut model, reporter)?;

  Ok(model)
}

pub(crate) fn unsupported(
  detail: impl AsRef<str>,
  reporter: &Reporter<'_>,
  include_readme: bool,
) -> Diagnostic {
  let suffix = if include_readme {
    ". See the supported subset documented in README.md ('Out of Scope' section)."
  } else {
    ""
  };
  reporter.error(
    DiagnosticCode::UnsupportedSemantic,
    format!(
      "Unsupported OpenAPI semantic shape: {}{}",
      detail.as_ref(),
      suffix
    ),
  )
}

pub(crate) fn check_unsupported_not(
  schema: &Schema,
  context: &Context<'_>,
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  if schema.not.is_some() {
    return Err(unsupported(
      format!(
        "{} uses not, which is outside the supported subset.",
        context.render()
      ),
      reporter,
      true,
    ));
  }
  Ok(())
}

/// Test helper: deserialize a raw JSON value into an OpenApiDocument and normalize it.
#[cfg(test)]
pub(crate) fn normalize_document(
  document: &serde_json::Value,
  reporter: &mut Reporter<'_>,
) -> Result<ApiModel, Diagnostic> {
  let doc: OpenApiDocument = serde_json::from_value(document.clone())
    .expect("test document must be a valid OpenApiDocument");
  normalize_api_model(&doc, &[], reporter)
}
