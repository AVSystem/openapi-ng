use napi_derive::napi;

use crate::ir::canonical::ApiModel;

#[napi(object)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerateSummary {
  /// Display-normalized path of the source spec, as it appears in the
  /// generated-artifact banner and in diagnostics' `path` field. Lets
  /// consumers correlate a result with the input they passed; the value
  /// is the supplied path with separators normalized, never resolved.
  pub normalized_source_path: String,
  pub spec_version: String,
  pub title: String,
  // u32 in the canonical IR; surfaces as a plain JS `number` (no BigInt
  // gymnastics) — lossless for any plausible spec size.
  pub path_count: u32,
  pub operation_count: u32,
  pub schema_count: u32,
}

impl GenerateSummary {
  /// Builds a summary from the canonical `ApiModel`. Counts are derived
  /// from the IR — what the generator will actually emit — rather than
  /// from the pre-normalize document, so a normalization that drops or
  /// fails on a schema is reflected in the user-facing summary.
  pub(crate) fn from_ir(normalized_source_path: String, ir: &ApiModel) -> Self {
    // Operation paths repeat per HTTP method (GET/POST/... on the same path
    // count as one path), so dedup. Vec + sort_unstable + dedup avoids the
    // per-node allocation of BTreeSet for what's only used as a count.
    let mut paths: Vec<&str> = ir.operations.iter().map(|op| op.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    // Per-document caps in `src/options.rs` keep these well below u32::MAX;
    // the clamp is a defence-in-depth guard, and the debug_assert traps any
    // future cap relaxation that would actually exceed the surface type.
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

fn clamp_count(n: usize) -> u32 {
  debug_assert!(n <= U32_MAX_AS_USIZE, "IR count exceeded u32::MAX: {n}");
  u32::try_from(usize::min(n, U32_MAX_AS_USIZE)).unwrap_or(u32::MAX)
}

/// A single generated artifact. `contents` always carries the emitted
/// source; callers that only need on-disk output can pass `outputPath`
/// and ignore the array.
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
}
