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

/// Payload returned inside `GenerateOutcome.error`. The JS wrapper
/// constructs a `GenerateError` (a real JS class that extends Error)
/// from these fields, so consumers can `instanceof GenerateError` and
/// read `code/subcode/message/path/warnings`.
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

/// Return shape of the native export. Exactly one field is set. The JS
/// wrapper turns `error` into a thrown `GenerateError`; returning data
/// instead of throwing keeps native and WASI runtimes identical.
#[napi(object)]
pub struct GenerateOutcome {
  pub result: Option<GenerateResult>,
  pub error: Option<GenerateErrorPayload>,
}

/// Project a `catch_unwind` payload into the same payload shape a typed
/// fatal produces. `&'static str` and `String` are the two common panic
/// payload types; anything else collapses to a generic message.
pub(crate) fn map_panic(panic: Box<dyn std::any::Any + Send>) -> GenerateErrorPayload {
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
  map_failure(GenerateFailure {
    warnings: Vec::new(),
    fatal,
  })
}

pub(crate) fn map_failure(failure: GenerateFailure) -> GenerateErrorPayload {
  let GenerateFailure { warnings, fatal } = failure;
  let fatal = fatal.to_napi_error();
  GenerateErrorPayload {
    code: fatal.code,
    subcode: fatal.subcode,
    message: fatal.message,
    path: fatal.path,
    warnings: warnings.iter().map(Diagnostic::to_napi_warning).collect(),
  }
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
    pipeline::{GenerateFailure, GenerateResult as ApplicationGenerateResult},
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

  #[test]
  fn map_failure_projects_fatal_and_warnings_into_payload() {
    let failure = GenerateFailure {
      warnings: vec![Diagnostic::new(
        DiagnosticCode::UnsupportedSemantic,
        "warned",
        std::rc::Rc::from("spec.yaml"),
      )],
      fatal: Diagnostic {
        code: DiagnosticCode::PolicyViolation,
        subcode: Some("missing-operation-id"),
        message: "no operationId".to_string(),
        path: std::rc::Rc::from("spec.yaml"),
      },
    };

    let payload = super::map_failure(failure);

    assert_eq!(payload.code, "E_POLICY_VIOLATION");
    assert_eq!(payload.subcode.as_deref(), Some("missing-operation-id"));
    assert_eq!(payload.message, "no operationId");
    assert_eq!(payload.path, "spec.yaml");
    assert_eq!(payload.warnings.len(), 1);
    assert_eq!(payload.warnings[0].code, "E_UNSUPPORTED_SEMANTIC");
    assert_eq!(payload.warnings[0].severity, "warning");
  }

  #[test]
  fn map_panic_projects_string_payloads_into_e_unexpected() {
    let payload = super::map_panic(Box::new("boom"));
    assert_eq!(payload.code, "E_UNEXPECTED");
    assert!(payload.message.contains("boom"));
    assert!(payload.warnings.is_empty());

    let payload = super::map_panic(Box::new(String::from("owned boom")));
    assert!(payload.message.contains("owned boom"));

    let payload = super::map_panic(Box::new(42_u8));
    assert!(payload.message.contains("unexpected panic"));
  }
}
