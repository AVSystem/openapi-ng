use napi_derive::napi;

use crate::api_model::canonical::ApiModel;

#[napi(object)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerateSummary {
  /// The source spec's path as supplied, with separators normalised and
  /// nothing resolved. The same string appears in every diagnostic's
  /// `path`.
  pub normalized_source_path: String,
  pub spec_version: String,
  pub title: String,
  // `u32` reaches JS as a plain `number`.
  pub path_count: u32,
  pub operation_count: u32,
  pub schema_count: u32,
}

impl GenerateSummary {
  /// Builds a summary whose counts come from the IR, and so describe what
  /// the generator emits rather than what the document declared.
  pub(crate) fn from_ir(normalized_source_path: String, ir: &ApiModel) -> Self {
    // One path carries an operation per method, so the list repeats.
    let mut paths: Vec<&str> = ir
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
      spec_version: ir.info.spec_version.clone(),
      title: ir.info.title.clone(),
      path_count: clamp_count(paths.len()),
      operation_count: clamp_count(ir.operations.len()),
      schema_count: clamp_count(ir.schemas.len()),
    }
  }
}

const U32_MAX_AS_USIZE: usize = u32::MAX as usize;

fn clamp_count(count: usize) -> u32 {
  debug_assert!(
    count <= U32_MAX_AS_USIZE,
    "IR count exceeded u32::MAX: {count}"
  );
  u32::try_from(usize::min(count, U32_MAX_AS_USIZE)).unwrap_or(u32::MAX)
}

/// One generated artifact. `contents` carries the emitted source whether
/// or not the caller also asked for it on disk.
#[napi(object)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifact {
  pub path: String,
  pub contents: String,
}

impl GeneratedArtifact {
  pub(crate) const fn new(path: String, contents: String) -> Self {
    Self { path, contents }
  }

  /// Prefixes the do-not-edit banner onto the contents.
  pub(crate) fn with_banner(mut self, banner: &str) -> Self {
    self.contents.insert_str(0, banner);
    self
  }
}
