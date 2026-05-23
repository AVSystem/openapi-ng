use napi::bindgen_prelude::{Function, JsObjectValue, Object, Unknown};
use napi::{Env, Error, Status};
use napi_derive::napi;

use crate::{
  error::{Diagnostic, DiagnosticCode, GeneratorDiagnostic},
  options::{GenerateConfig, MappedType, ResponseTypeMapping},
  pipeline::{GenerateFailure, GenerateResult as ApplicationGenerateResult},
  result::{GenerateSummary, GeneratedArtifact},
};

/// Per-target emit selection. The `emit` option is the set of artifact
/// families to produce; each entry maps to one or more files.
#[napi(string_enum = "lowercase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EmitTarget {
  Models,
  Angular,
}

/// User-facing naming config crossing the NAPI boundary. The JS wrapper
/// in `lib/index.js` unpacks each JS `RegExp` into the `{ source, flags
/// }` shape carried here, so Rust sees pure data on this side.
#[napi(object)]
#[derive(Clone, Debug)]
pub struct NamingOptions {
  pub method_name: Option<NamingValue>,
  pub group: Option<NamingValue>,
}

/// Discriminated union: a string shorthand, a single rule, or a chain
/// of rules-or-shorthands. NAPI cannot express true sum types, so we
/// use exclusive fields: exactly one of `string`, `rule`, or `chain`
/// must be set. The JS wrapper enforces this; the Rust validator
/// double-checks at config resolution.
#[napi(object)]
#[derive(Clone, Debug)]
pub struct NamingValue {
  /// `{ string: '...' }` — bare format-string shorthand.
  pub string: Option<String>,
  /// `{ rule: { ... } }` — a single Rule.
  pub rule: Option<NamingRuleEntry>,
  /// `{ chain: [...] }` — a sequence; each item is an exclusive
  /// `{ string }` or `{ rule }`.
  pub chain: Option<Vec<NamingChainItem>>,
}

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NamingChainItem {
  pub string: Option<String>,
  pub rule: Option<NamingRuleEntry>,
}

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NamingRuleEntry {
  pub from: Option<String>,
  pub parse: Option<NamingParseSpec>,
  pub format: Option<String>,
  /// Lowercase per spec: 'camel' | 'pascal' | 'snake' | 'kebab' | 'constant'.
  #[napi(js_name = "case")]
  pub case_: Option<String>,
}

#[napi(object)]
#[derive(Clone, Debug)]
pub struct NamingParseSpec {
  pub source: String,
  pub flags: String,
}

#[napi(object)]
pub struct GenerateOptions {
  /// Path to the spec on disk. Mutually exclusive with `input_contents`;
  /// the option validator rejects requests that set both or neither.
  pub input_path: Option<String>,
  /// Raw spec source. When set, `display_path` is required and the
  /// 16 MiB byte cap applies to `input_contents.as_bytes().len()`.
  /// JS wrapper fills this in for URL inputs.
  pub input_contents: Option<String>,
  /// Banner / diagnostic display string. Required when `input_contents`
  /// is set; ignored when `input_path` is set (the existing path
  /// normalisation runs in that case).
  pub display_path: Option<String>,
  /// Decoder hint. Only honoured when `input_contents` is set; combining
  /// it with `input_path` is a shape error.
  pub input_format: Option<InputFormat>,
  /// Optional. When undefined, generation runs in-memory (no files written).
  /// Passing an empty string is rejected at option resolution.
  pub output_path: Option<String>,
  pub emit: Vec<EmitTarget>,
  pub mapped_types: Option<Vec<MappedType>>,
  /// Per-content-type override of the generated response-decoding kind
  /// (`json | blob | text | arrayBuffer`). Read by the normalize stage
  /// when picking how a successful response body is decoded.
  pub response_type_mapping: Option<Vec<ResponseTypeMapping>>,
  pub naming: Option<NamingOptions>,
}

/// Explicit decoder selection. Skips both extension-based detection and
/// the JSON-then-YAML sniff fallback. Honoured only with `input_contents`.
#[napi(string_enum = "lowercase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFormat {
  Json,
  Yaml,
}

#[napi(object)]
pub struct GenerateResult {
  pub summary: GenerateSummary,
  pub diagnostics: Vec<GeneratorDiagnostic>,
  pub artifacts: Vec<GeneratedArtifact>,
}

/// Payload attached to every fatal native throw. The JS wrapper in
/// `lib/index.js` upgrades the thrown plain Error into a `GenerateError`
/// (a real JS class that extends Error), copying these own-properties
/// across so consumers can `instanceof GenerateError` and still read
/// `code/subcode/message/path/warnings`.
///
/// The fatal sits at the top level (`code/subcode/message/path`); pre-fatal
/// warnings ride in `warnings`. `subcode` is set for `PolicyViolation`
/// codes; it is `null` for every other category.
#[napi(object)]
pub struct GenerateErrorPayload {
  pub code: String,
  pub subcode: Option<String>,
  pub message: String,
  pub path: String,
  pub warnings: Vec<GeneratorDiagnostic>,
}

/// Sentinel set on every thrown error so the JS wrapper can identify
/// them without leaking the marker into application code (consumers
/// guard with `err instanceof GenerateError`, not by inspecting this).
///
/// The value is read from `lib/error-marker.json` at compile time by
/// `build.rs`, the same file `lib/index.js` reads at module load. Single
/// source of truth — the two sides cannot drift.
const GENERATE_ERROR_MARKER: &str = env!("OPENAPI_NG_ERROR_MARKER");

/// Project a `catch_unwind` payload into the same `GenerateError` shape
/// that fatal diagnostics produce. The two common payload types are
/// `&'static str` (from `panic!("literal")`) and `String` (from
/// `panic!("{}", ...)` / `panic!(format!(...))`); everything else
/// collapses to a generic fallback message so the surface stays bounded.
///
/// The result is a `napi::Error` indistinguishable from the one a typed
/// fatal would produce, so the JS wrapper upgrades it to a real
/// `GenerateError` via the same path and consumers can write
/// `err.code === 'E_UNEXPECTED'`.
pub(crate) fn map_panic(panic: Box<dyn std::any::Any + Send>, env: Env) -> Error {
  let message = panic
    .downcast_ref::<&'static str>()
    .map(|s| (*s).to_string())
    .or_else(|| panic.downcast_ref::<String>().cloned())
    .unwrap_or_else(|| "openapi-ng: unexpected panic in native binding".to_string());
  let fatal = Diagnostic {
    code: DiagnosticCode::Unexpected,
    subcode: None,
    message: format!("unexpected panic in native binding: {message}"),
    path: std::rc::Rc::from(""),
  };
  map_failure(
    GenerateFailure {
      warnings: Vec::new(),
      fatal,
    },
    env,
  )
}

pub(crate) fn map_failure(failure: GenerateFailure, env: Env) -> Error {
  let GenerateFailure { warnings, fatal } = failure;
  let fatal = fatal.to_napi_error();
  let warnings: Vec<GeneratorDiagnostic> =
    warnings.iter().map(Diagnostic::to_napi_warning).collect();
  try_enrich_error(&fatal, &warnings, env).unwrap_or_else(|_| {
    // Boundary-side decoration failed (typically OOM during JS Object
    // construction): embed the diagnostic code and warning count into the
    // message so consumers branching on `err.code` still get a usable
    // signal instead of a bare Error.
    let dropped_suffix = if warnings.is_empty() {
      String::new()
    } else {
      format!(" ({} warning(s) dropped)", warnings.len())
    };
    Error::new(
      Status::GenericFailure,
      format!("[{}] {}{}", fatal.code, fatal.message, dropped_suffix),
    )
  })
}

fn try_enrich_error(
  fatal: &GeneratorDiagnostic,
  warnings: &[GeneratorDiagnostic],
  env: Env,
) -> napi::Result<Error> {
  // Build a plain JS Error and decorate it with the public own-properties.
  // The JS wrapper in `lib/index.js` then re-throws as a GenerateError
  // class instance so `err instanceof GenerateError` works while keeping
  // the native binding free of subclass-of-Error gymnastics that
  // `napi_is_error` doesn't honor for `#[napi]` classes.
  let global = env.get_global()?;
  let error_ctor: Function<String, Unknown> = global.get_named_property("Error")?;
  let unknown = error_ctor.new_instance(fatal.message.clone())?;
  // SAFETY: `unknown` was just constructed on the previous line via
  // `error_ctor.new_instance(...)` against the global `Error` constructor,
  // which always returns a JS `Object`. Downcasting back to `Object` therefore
  // cannot violate the napi-rs type invariant on `Unknown::cast`.
  let mut js_error: Object = unsafe { unknown.cast()? };
  js_error.set_named_property("code", fatal.code.clone())?;
  if let Some(subcode) = fatal.subcode.clone() {
    js_error.set_named_property("subcode", subcode)?;
  }
  js_error.set_named_property("path", fatal.path.clone())?;
  js_error.set_named_property("warnings", warnings.to_vec())?;
  js_error.set_named_property(GENERATE_ERROR_MARKER, true)?;
  Ok(Error::from(unknown))
}

/// Boundary projection: take the wire-shaped `GenerateOptions` from the
/// JS caller and lower it into the resolved `GenerateConfig` the domain
/// pipeline consumes. Lives in `bindings.rs` (not `options.rs`) so the
/// domain doesn't depend on the NAPI boundary types.
impl From<GenerateOptions> for GenerateConfig {
  fn from(value: GenerateOptions) -> Self {
    Self {
      input_path: value.input_path,
      input_contents: value.input_contents,
      display_path: value.display_path,
      input_format: value.input_format,
      output_path: value.output_path,
      emit: value.emit.into_iter().collect(),
      mapped_types: value.mapped_types.unwrap_or_default(),
      response_type_mapping: value.response_type_mapping.unwrap_or_default(),
      naming_options: value.naming,
      naming: crate::plan::naming::NamingConfig::default(),
    }
  }
}

pub(crate) fn map_generate_result(value: ApplicationGenerateResult) -> GenerateResult {
  GenerateResult {
    summary: value.summary,
    // Pipeline-collected diagnostics are warnings — fatals exit via the
    // `Err(GenerateFailure)` arm and are projected in `map_failure`.
    diagnostics: value
      .diagnostics
      .iter()
      .map(Diagnostic::to_napi_warning)
      .collect(),
    artifacts: value.artifacts,
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    bindings::{EmitTarget, GenerateOptions},
    error::{Diagnostic, DiagnosticCode},
    options::GenerateConfig,
    pipeline::GenerateResult as ApplicationGenerateResult,
    result::{GenerateSummary, GeneratedArtifact},
  };

  #[test]
  fn from_collects_emit_targets_into_the_resolved_set() {
    let config = GenerateConfig::from(GenerateOptions {
      input_path: Some("spec.yaml".to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: Some("out".to_string()),
      emit: vec![EmitTarget::Models, EmitTarget::Angular],
      mapped_types: None,
      response_type_mapping: None,
      naming: None,
    });

    assert!(config.emit.contains(&EmitTarget::Models));
    assert!(config.emit.contains(&EmitTarget::Angular));
  }

  #[test]
  fn from_deduplicates_repeated_emit_targets() {
    let config = GenerateConfig::from(GenerateOptions {
      input_path: Some("spec.yaml".to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: Some("out".to_string()),
      emit: vec![EmitTarget::Models, EmitTarget::Models, EmitTarget::Angular],
      mapped_types: None,
      response_type_mapping: None,
      naming: None,
    });

    assert_eq!(config.emit.len(), 2);
    assert!(config.emit.contains(&EmitTarget::Models));
    assert!(config.emit.contains(&EmitTarget::Angular));
  }

  #[test]
  fn map_generate_result_projects_domain_artifacts_to_napi_shape() {
    let result = super::map_generate_result(ApplicationGenerateResult {
      summary: GenerateSummary {
        normalized_source_path: "test/fixtures/petstore-minimal.openapi.yaml".to_string(),
        spec_version: "3.0.3".to_string(),
        title: "Petstore Minimal".to_string(),
        path_count: 1,
        operation_count: 1,
        schema_count: 1,
      },
      diagnostics: vec![Diagnostic::new(
        DiagnosticCode::UnsupportedSemantic,
        "Example warning",
        std::rc::Rc::from("spec.yaml"),
      )],
      artifacts: vec![GeneratedArtifact::new(
        "model.generated.ts".to_string(),
        "export interface Pet {}\n".to_string(),
      )],
    });

    assert_eq!(result.summary.title, "Petstore Minimal");
    assert_eq!(result.artifacts.len(), 1);
    assert_eq!(result.artifacts[0].path, "model.generated.ts");
    assert_eq!(result.artifacts[0].contents, "export interface Pet {}\n");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "E_UNSUPPORTED_SEMANTIC");
  }
}
