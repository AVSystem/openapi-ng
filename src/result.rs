use napi_derive::napi;

use crate::api_model::canonical::ApiModel;

#[napi(object)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerateSummary {
  /// The source spec's path as supplied, with separators normalised and nothing resolved.
  pub normalized_source_path: String,
  pub spec_version: String,
  pub title: String,
  // `u32` reaches JS as a plain `number`.
  pub path_count: u32,
  pub operation_count: u32,
  pub schema_count: u32,
}

impl GenerateSummary {
  /// Builds a summary whose counts come from the IR, and so describe what the generator emits
  /// rather than what the document declared.
  #[must_use]
  pub(crate) fn from_ir(normalized_source_path: String, model: &ApiModel) -> Self {
    // One path carries an operation per method, so the list repeats.
    let mut paths: Vec<&str> = model
      .operations
      .iter()
      .map(|operation| operation.path.as_str())
      .collect();
    paths.sort_unstable();
    paths.dedup();
    // The caps in `crate::parse::limits` keep every count far below
    // `u32::MAX`; the clamp is defence in depth.
    Self {
      normalized_source_path,
      spec_version: model.info.spec_version.clone(),
      title: model.info.title.clone(),
      path_count: clamp_count(paths.len()),
      operation_count: clamp_count(model.operations.len()),
      schema_count: clamp_count(model.schemas.len()),
    }
  }
}

const U32_MAX_AS_USIZE: usize = u32::MAX as usize;

#[must_use]
fn clamp_count(count: usize) -> u32 {
  debug_assert!(
    count <= U32_MAX_AS_USIZE,
    "IR count exceeded u32::MAX: {count}"
  );
  u32::try_from(usize::min(count, U32_MAX_AS_USIZE)).unwrap_or(u32::MAX)
}

/// One generated artifact.
#[napi(object)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
  pub path: String,
  pub contents: String,
}

impl GeneratedArtifact {
  #[must_use]
  pub(crate) const fn new(path: String, contents: String) -> Self {
    Self { path, contents }
  }
}
