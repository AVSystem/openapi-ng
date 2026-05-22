use std::rc::Rc;

use napi_derive::napi;
use serde::Serialize;

const SEVERITY_WARNING: &str = "warning";
const SEVERITY_ERROR: &str = "error";

/// Compact diagnostic taxonomy. Six codes covering every fatal/warning
/// the pipeline emits:
///
/// * `InputInvalid` — read or decode failed (`E_INPUT_INVALID`).
/// * `UnsupportedSemantic` — accepted spec uses a shape outside the supported
///   subset (`E_UNSUPPORTED_SEMANTIC`).
/// * `InvalidReference` — `$ref` does not resolve (`E_INVALID_REFERENCE`).
/// * `InvalidOption` — caller-supplied option is invalid (`E_INVALID_OPTION`).
/// * `PolicyViolation` — IR-level rule (missing tag, missing operationId,
///   request-field collision, planner refusal) (`E_POLICY_VIOLATION`).
/// * `WriteFailed` — output file write failed (`E_WRITE_FAILED`).
/// * `Unexpected` — a panic crossed the NAPI boundary; surfaced by
///   `map_panic` so a Rust panic becomes an `E_UNEXPECTED` GenerateError
///   instead of aborting the host Node process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticCode {
  InputInvalid,
  UnsupportedSemantic,
  InvalidReference,
  InvalidOption,
  PolicyViolation,
  WriteFailed,
  Unexpected,
}

impl DiagnosticCode {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::InputInvalid => "E_INPUT_INVALID",
      Self::UnsupportedSemantic => "E_UNSUPPORTED_SEMANTIC",
      Self::InvalidReference => "E_INVALID_REFERENCE",
      Self::InvalidOption => "E_INVALID_OPTION",
      Self::PolicyViolation => "E_POLICY_VIOLATION",
      Self::WriteFailed => "E_WRITE_FAILED",
      Self::Unexpected => "E_UNEXPECTED",
    }
  }
}

/// Single internal diagnostic carried across the pipeline. Severity is
/// implicit (Err vs warnings-vec). `path` is an `Rc<str>` so the reporter
/// can attach the same display path to every diagnostic by bumping a
/// refcount, not allocating a fresh `String`.
///
/// Message convention: lead with a stage-gerund subject ("Failed to
/// decode input", "Unsupported OpenAPI semantic shape", "Failed to plan
/// services"), then state the detail, then append a sentence of
/// actionable advice when one exists ("Rename the colliding parameters
/// in the OpenAPI spec.", "Check for typos in the $ref..."). `subcode`
/// is set for `PolicyViolation` to let consumers route on a kebab-case
/// sub-class without parsing the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
  pub code: DiagnosticCode,
  pub subcode: Option<&'static str>,
  pub message: String,
  pub path: Rc<str>,
}

impl Diagnostic {
  pub(crate) fn new(code: DiagnosticCode, message: impl Into<String>, path: Rc<str>) -> Self {
    Self {
      code,
      subcode: None,
      message: message.into(),
      path,
    }
  }

  pub(crate) fn policy_violation(
    reporter: &Reporter<'_>,
    subcode: &'static str,
    message: impl Into<String>,
  ) -> Self {
    let mut diagnostic = reporter.error(DiagnosticCode::PolicyViolation, message);
    diagnostic.subcode = Some(subcode);
    diagnostic
  }

  pub(crate) fn to_napi_warning(&self) -> GeneratorDiagnostic {
    self.to_napi(SEVERITY_WARNING)
  }

  pub(crate) fn to_napi_error(&self) -> GeneratorDiagnostic {
    self.to_napi(SEVERITY_ERROR)
  }

  fn to_napi(&self, severity: &'static str) -> GeneratorDiagnostic {
    GeneratorDiagnostic {
      code: self.code.as_str().to_string(),
      subcode: self.subcode.map(str::to_string),
      severity: severity.to_string(),
      message: self.message.clone(),
      path: self.path.as_ref().to_string(),
    }
  }
}

impl std::fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for Diagnostic {}

/// Boundary projection of `Diagnostic` for the NAPI surface — string-typed
/// `code` is what JS consumers see and compare against. `severity` is
/// either `"warning"` or `"error"`; the TS surface narrows it to the
/// `'warning' | 'error'` union via `scripts/patch-types.mjs`. `subcode`
/// is populated only for `PolicyViolation` today; consumers route on it
/// when they need finer-grained remediation than `code` alone.
#[napi(object)]
#[derive(Clone, Debug, Serialize)]
pub struct GeneratorDiagnostic {
  pub code: String,
  pub subcode: Option<String>,
  pub severity: String,
  pub message: String,
  pub path: String,
}

/// Borrowed breadcrumb for diagnostic context messages used during schema/operation
/// normalization. Building a `Context` value is alloc-free; only `.render()` allocates,
/// and only on the error path when a diagnostic message is actually being constructed.
///
/// Each variant corresponds to one level of the recursive normalization walk.
/// `Copy` so inner call sites can take `context: &Context<'_>` and build a deeper
/// context by value without extra indirection.
#[derive(Clone, Copy)]
pub(crate) enum Context<'a> {
  /// Top-level named schema: renders as `"schema {name}"`.
  Schema(&'a str),
  /// A property inside an object schema: renders as `"{parent}.{name}"`.
  Property {
    parent: &'a Context<'a>,
    name: &'a str,
  },
  /// An `additionalProperties` sub-schema: renders as `"{parent} additionalProperties"`.
  AdditionalProperties { parent: &'a Context<'a> },
  /// One member of a oneOf/anyOf/allOf array (1-based index):
  /// renders as `"{parent} composition member {index}"`.
  CompositionMember {
    parent: &'a Context<'a>,
    index: usize,
  },
  /// A request parameter context for an operation: renders as `"parameter {method} {path}"`.
  Parameter { method: &'a str, path: &'a str },
  /// A request body context: renders as `"requestBody for {method} {path}"`.
  RequestBody { method: &'a str, path: &'a str },
  /// A response schema context: renders as `"response schema for {method} {path}"`.
  ResponseSchema { method: &'a str, path: &'a str },
}

impl<'a> Context<'a> {
  /// Render the full breadcrumb chain into a `String`. This allocates —
  /// call only when actually constructing a diagnostic message.
  pub(crate) fn render(&self) -> String {
    match self {
      Context::Schema(name) => format!("schema {name}"),
      Context::Property { parent, name } => format!("{}.{name}", parent.render()),
      Context::AdditionalProperties { parent } => {
        format!("{} additionalProperties", parent.render())
      }
      Context::CompositionMember { parent, index } => {
        format!("{} composition member {index}", parent.render())
      }
      Context::Parameter { method, path } => format!("parameter {method} {path}"),
      Context::RequestBody { method, path } => format!("requestBody for {method} {path}"),
      Context::ResponseSchema { method, path } => {
        format!("response schema for {method} {path}")
      }
    }
  }
}

/// Single reporter type carried through every pipeline stage. Holds the
/// display path (shared via `Rc<str>` across every diagnostic it builds)
/// and a borrow into the boundary-owned warnings vec.
///
/// Stages take `&Reporter<'_>` when they only emit fatals via
/// `.error(...)`; they take `&mut Reporter<'_>` when they also need to
/// push pre-fatal warnings via `.warning(...)`.
pub(crate) struct Reporter<'a> {
  path: Rc<str>,
  warnings: &'a mut Vec<Diagnostic>,
}

impl<'a> Reporter<'a> {
  pub(crate) const fn new(path: Rc<str>, warnings: &'a mut Vec<Diagnostic>) -> Self {
    Self { path, warnings }
  }

  pub(crate) fn error(&self, code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message, Rc::clone(&self.path))
  }

  /// Push a pre-fatal warning. `subcode` is an optional stable
  /// kebab-case tag that lets consumers route on a finer-grained class
  /// than `code` alone; pass `None` when no such subdivision applies.
  pub(crate) fn warning(
    &mut self,
    code: DiagnosticCode,
    subcode: Option<&'static str>,
    message: impl Into<String>,
  ) {
    let mut diagnostic = Diagnostic::new(code, message, Rc::clone(&self.path));
    diagnostic.subcode = subcode;
    self.warnings.push(diagnostic);
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::{Diagnostic, DiagnosticCode, Reporter};

  #[test]
  fn fatal_projects_typed_metadata_into_napi_boundary_strings() {
    let diagnostic = Diagnostic::new(
      DiagnosticCode::InputInvalid,
      "Failed to decode input.",
      std::rc::Rc::from("spec.yaml"),
    );
    let napi = diagnostic.to_napi_error();

    assert_eq!(napi.code, "E_INPUT_INVALID");
    assert_eq!(napi.severity, "error");

    let serialized = serde_json::to_value(&napi).expect("diagnostic serializes");

    assert_eq!(
      serialized,
      json!({
        "code": "E_INPUT_INVALID",
        "subcode": null,
        "severity": "error",
        "message": "Failed to decode input.",
        "path": "spec.yaml",
      })
    );
  }

  #[test]
  fn warning_projects_to_warning_severity_at_the_boundary() {
    let diagnostic = Diagnostic::new(
      DiagnosticCode::UnsupportedSemantic,
      "Shape is deprecated but accepted.",
      std::rc::Rc::from("spec.yaml"),
    );
    let napi = diagnostic.to_napi_warning();

    assert_eq!(napi.code, "E_UNSUPPORTED_SEMANTIC");
    assert_eq!(napi.severity, "warning");
  }

  #[test]
  fn subcode_threads_through_the_napi_projection() {
    let mut ctx = crate::test_support::test_ctx();
    let diagnostic = Diagnostic::policy_violation(
      &ctx.reporter(),
      "missing-tag",
      "Failed to plan services: operation missing tag.",
    );

    assert_eq!(diagnostic.subcode, Some("missing-tag"));
    let napi = diagnostic.to_napi_error();
    assert_eq!(napi.subcode.as_deref(), Some("missing-tag"));
  }

  #[test]
  fn warning_pushes_typed_diagnostic_carrying_path() {
    let mut warnings = Vec::new();
    let mut reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"), &mut warnings);

    reporter.warning(
      DiagnosticCode::UnsupportedSemantic,
      None,
      "Input used a fallback path.",
    );

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, DiagnosticCode::UnsupportedSemantic);
    assert_eq!(warnings[0].path.as_ref(), "fixtures/spec.yaml");
  }

  #[test]
  fn error_returns_a_fatal_diagnostic_without_pushing() {
    let mut warnings = Vec::new();
    let reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"), &mut warnings);

    let fatal = reporter.error(DiagnosticCode::WriteFailed, "Failed to write artifact.");

    assert_eq!(fatal.code, DiagnosticCode::WriteFailed);
    assert_eq!(fatal.path.as_ref(), "fixtures/spec.yaml");
    assert!(warnings.is_empty());
  }

  #[test]
  fn warnings_accumulate_in_order_on_the_caller_owned_vec() {
    let mut warnings = Vec::new();
    {
      let mut reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"), &mut warnings);
      reporter.warning(DiagnosticCode::UnsupportedSemantic, None, "First warning.");
      reporter.warning(DiagnosticCode::UnsupportedSemantic, None, "Second warning.");
    }

    assert_eq!(warnings.len(), 2);
    assert_eq!(warnings[0].message, "First warning.");
    assert_eq!(warnings[1].message, "Second warning.");
  }
}
