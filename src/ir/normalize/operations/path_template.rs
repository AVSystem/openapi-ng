//! Path-template validation.

use crate::error::{Diagnostic, Reporter, bail_policy};
use crate::ident::is_ident;

use super::super::bail_unsupported;

/// Fails when `path`'s braces are unbalanced or nested, or when a
/// placeholder wraps a name that is not a bare identifier.
pub(super) fn validate_path_template(path: &str, reporter: &Reporter) -> Result<(), Diagnostic> {
  let mut rest = path;
  while let Some(open) = rest.find('{') {
    let after_open = &rest[open + 1..];
    if let Some(stray) = after_open.find('{') {
      let close = after_open.find('}');
      if close.is_none_or(|c| stray < c) {
        bail_unsupported!(
          reporter,
          "path template {path} contains nested '{{' which is not a valid OpenAPI parameter placeholder."
        );
      }
    }
    let Some(close) = after_open.find('}') else {
      bail_unsupported!(
        reporter,
        "path template {path} has an unbalanced '{{' with no matching '}}'."
      );
    };
    let name = &after_open[..close];
    if !is_ident(name) {
      bail_policy!(
        reporter,
        "invalid-path-parameter-name",
        "path template {path}: parameter name '{name}' is not a valid JavaScript identifier. Rename the parameter or split this path into a non-generated client."
      );
    }
    rest = &after_open[close + 1..];
  }
  if let Some(stray) = rest.find('}') {
    let _ = stray;
    bail_unsupported!(
      reporter,
      "path template {path} has an unbalanced '}}' with no matching '{{'."
    );
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::validate_path_template;
  use crate::test_support::test_reporter;

  #[test]
  fn validate_path_template_accepts_well_formed_paths() {
    let ctx = test_reporter();
    for path in [
      "/pets",
      "/pets/{id}",
      "/users/{userId}/pets/{petId}",
      "/_internal/{$ref}",
    ] {
      validate_path_template(path, &ctx)
        .unwrap_or_else(|err| panic!("path {path} should validate, got: {err:?}"));
    }
  }

  #[test]
  fn validate_path_template_rejects_invalid_identifier_parameter_name() {
    let ctx = test_reporter();
    let err =
      validate_path_template("/pets/{it's}", &ctx).expect_err("invalid identifier must reject");
    assert_eq!(err.subcode, Some("invalid-path-parameter-name"));
  }

  #[test]
  fn validate_path_template_rejects_digits_first_parameter_name() {
    let ctx = test_reporter();
    let err = validate_path_template("/pets/{1foo}", &ctx).expect_err("digits-first must reject");
    assert_eq!(err.subcode, Some("invalid-path-parameter-name"));
  }

  #[test]
  fn validate_path_template_rejects_kebab_case_parameter_name() {
    let ctx = test_reporter();
    let err = validate_path_template("/pets/{pet-id}", &ctx).expect_err("kebab-case must reject");
    assert_eq!(err.subcode, Some("invalid-path-parameter-name"));
  }

  #[test]
  fn validate_path_template_still_rejects_unbalanced_braces() {
    let ctx = test_reporter();
    let err = validate_path_template("/pets/{id", &ctx).expect_err("unbalanced { must reject");
    // unsupported() uses code, not subcode; just confirm it's an error.
    assert_eq!(err.code, crate::error::DiagnosticCode::UnsupportedSemantic);
  }

  #[test]
  fn validate_path_template_still_rejects_stray_close_brace() {
    let ctx = test_reporter();
    let err = validate_path_template("/pets/id}", &ctx).expect_err("stray } must reject");
    assert_eq!(err.code, crate::error::DiagnosticCode::UnsupportedSemantic);
  }
}
