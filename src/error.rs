use std::cell::RefCell;
use std::rc::Rc;

use napi_derive::napi;
use serde::Serialize;

const SEVERITY_WARNING: &str = "warning";
const SEVERITY_ERROR: &str = "error";

/// Every code a fatal or a warning can carry:
///
/// * `InputInvalid` — read or decode failed (`E_INPUT_INVALID`).
/// * `UnsupportedSemantic` — accepted spec uses a shape outside the supported
///   subset (`E_UNSUPPORTED_SEMANTIC`).
/// * `InvalidReference` — `$ref` does not resolve (`E_INVALID_REFERENCE`).
/// * `InvalidOption` — caller-supplied option is invalid (`E_INVALID_OPTION`).
/// * `PolicyViolation` — IR-level rule (missing tag, missing operationId,
///   request-field collision, planner refusal) (`E_POLICY_VIOLATION`).
/// * `WriteFailed` — output file write failed (`E_WRITE_FAILED`).
/// * `Unexpected` — a panic crossed the NAPI boundary (`E_UNEXPECTED`).
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

/// One diagnostic. Severity is implicit: a fatal travels as `Err`, a
/// warning through [`Reporter::warning`].
///
/// `message` leads with a stage-gerund subject ("Failed to decode
/// input"), then the detail, then advice when there is any. `subcode`
/// is set for `PolicyViolation`.
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
    reporter: &Reporter,
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

/// Boundary projection of [`Diagnostic`] for the NAPI surface, where
/// `code` and `severity` are strings a JS consumer compares against.
///
/// `severity` is `"warning"` or `"error"`. `subcode` is set only for
/// `PolicyViolation`.
#[napi(object)]
#[derive(Clone, Debug, Serialize)]
pub struct GeneratorDiagnostic {
  pub code: String,
  pub subcode: Option<String>,
  pub severity: String,
  pub message: String,
  pub path: String,
}

/// Borrowed breadcrumb naming one position of a schema walk. Building one
/// is allocation-free; only [`Context::render`] allocates.
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
  /// Renders the full chain. Allocates.
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

/// Diagnostic sink for one run. Attaches the display path to every
/// diagnostic it builds and accumulates the pre-fatal warnings.
pub(crate) struct Reporter {
  path: Rc<str>,
  warnings: RefCell<Vec<Diagnostic>>,
}

impl Reporter {
  pub(crate) const fn new(path: Rc<str>) -> Self {
    Self {
      path,
      warnings: RefCell::new(Vec::new()),
    }
  }

  /// Builds a fatal diagnostic without recording it.
  pub(crate) fn error(&self, code: DiagnosticCode, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message, Rc::clone(&self.path))
  }

  /// Records a pre-fatal warning. `subcode` is a stable kebab-case tag,
  /// `None` when no subdivision applies.
  pub(crate) fn warning(
    &self,
    code: DiagnosticCode,
    subcode: Option<&'static str>,
    message: impl Into<String>,
  ) {
    let mut diagnostic = Diagnostic::new(code, message, Rc::clone(&self.path));
    diagnostic.subcode = subcode;
    self.warnings.borrow_mut().push(diagnostic);
  }

  pub(crate) fn into_warnings(self) -> Vec<Diagnostic> {
    self.warnings.into_inner()
  }
}

/// Returns a `PolicyViolation` from the enclosing function.
macro_rules! bail_policy {
  ($reporter:expr, $subcode:expr, $($message:tt)*) => {
    return ::core::result::Result::Err($crate::error::Diagnostic::policy_violation(
      $reporter,
      $subcode,
      ::std::format!($($message)*),
    ))
  };
}

/// Returns a fatal diagnostic of the given code from the enclosing function.
macro_rules! bail {
  ($reporter:expr, $code:expr, $($message:tt)*) => {
    return ::core::result::Result::Err(
      $reporter.error($code, ::std::format!($($message)*)),
    )
  };
}

pub(crate) use {bail, bail_policy};

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
    let ctx = crate::test_support::test_reporter();
    let diagnostic = Diagnostic::policy_violation(
      &ctx,
      "missing-tag",
      "Failed to plan services: operation missing tag.",
    );

    assert_eq!(diagnostic.subcode, Some("missing-tag"));
    let napi = diagnostic.to_napi_error();
    assert_eq!(napi.subcode.as_deref(), Some("missing-tag"));
  }

  #[test]
  fn warning_records_typed_diagnostic_carrying_path() {
    let reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"));

    reporter.warning(
      DiagnosticCode::UnsupportedSemantic,
      None,
      "Input used a fallback path.",
    );

    let warnings = reporter.into_warnings();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, DiagnosticCode::UnsupportedSemantic);
    assert_eq!(warnings[0].path.as_ref(), "fixtures/spec.yaml");
  }

  #[test]
  fn error_returns_a_fatal_diagnostic_without_recording_it() {
    let reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"));

    let fatal = reporter.error(DiagnosticCode::WriteFailed, "Failed to write artifact.");

    assert_eq!(fatal.code, DiagnosticCode::WriteFailed);
    assert_eq!(fatal.path.as_ref(), "fixtures/spec.yaml");
    assert!(reporter.into_warnings().is_empty());
  }

  #[test]
  fn warnings_accumulate_in_report_order() {
    let reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"));

    reporter.warning(DiagnosticCode::UnsupportedSemantic, None, "First warning.");
    reporter.warning(DiagnosticCode::UnsupportedSemantic, None, "Second warning.");

    let warnings = reporter.into_warnings();
    assert_eq!(warnings.len(), 2);
    assert_eq!(warnings[0].message, "First warning.");
    assert_eq!(warnings[1].message, "Second warning.");
  }

  #[test]
  fn reporting_composes_inside_a_fallible_iterator_chain() {
    let reporter = Reporter::new(std::rc::Rc::from("fixtures/spec.yaml"));

    let outcome = ["ok", "warn", "fatal"]
      .iter()
      .map(|token| match *token {
        "fatal" => Err(reporter.error(DiagnosticCode::InputInvalid, "Bad token.")),
        "warn" => {
          reporter.warning(DiagnosticCode::UnsupportedSemantic, None, "Odd token.");
          Ok(*token)
        }
        other => Ok(other),
      })
      .collect::<Result<Vec<_>, Diagnostic>>();

    assert_eq!(
      outcome.expect_err("chain short-circuits").code,
      DiagnosticCode::InputInvalid
    );
    assert_eq!(reporter.into_warnings().len(), 1);
  }
}
