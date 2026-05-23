//! Compiled user-supplied `parse` regex. The NAPI boundary delivers the
//! source pattern and flags string (split out from the JS RegExp on the
//! wrapper side); we compile here once at config time so per-operation
//! evaluation is a cheap `regex.captures()` call.

use regex::{Regex, RegexBuilder};

#[derive(Debug, Clone)]
pub(crate) struct CompiledParseSpec {
  pub(crate) regex: Regex,
}

/// Why a `parse` spec could not be compiled. Surfaced at config-validation
/// time as an `E_INVALID_OPTION` diagnostic so the user sees the error
/// before any generation work runs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CompileError {
  UnsupportedFlag(char),
  InvalidPattern(String),
}

/// Compile a user `parse` regex. Supported flags: `i`, `m`, `s` (subset
/// of JS RegExp that maps cleanly to Rust's `regex` crate). Any other
/// flag — including `g`/`y`/`u` — is rejected loudly rather than
/// silently ignored, so JS authors don't get surprised when their JS
/// pattern relies on a flag the Rust engine cannot honour.
pub(crate) fn compile(source: &str, flags: &str) -> Result<CompiledParseSpec, CompileError> {
  let mut builder = RegexBuilder::new(source);
  for ch in flags.chars() {
    match ch {
      'i' => {
        builder.case_insensitive(true);
      }
      'm' => {
        builder.multi_line(true);
      }
      's' => {
        builder.dot_matches_new_line(true);
      }
      other => return Err(CompileError::UnsupportedFlag(other)),
    }
  }
  let regex = builder
    .build()
    .map_err(|err| CompileError::InvalidPattern(err.to_string()))?;
  Ok(CompiledParseSpec { regex })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compile_accepts_empty_flags() {
    let spec = compile(r"^(?<x>.+)$", "").expect("should compile");
    assert!(spec.regex.is_match("hello"));
  }

  #[test]
  fn compile_honours_case_insensitive_flag() {
    let spec = compile(r"^foo$", "i").expect("should compile");
    assert!(spec.regex.is_match("FOO"));
  }

  #[test]
  fn compile_rejects_unsupported_global_flag() {
    let err = compile(r".", "g").expect_err("`g` should be rejected");
    assert_eq!(err, CompileError::UnsupportedFlag('g'));
  }

  #[test]
  fn compile_rejects_unsupported_unicode_flag() {
    let err = compile(r".", "u").expect_err("`u` should be rejected");
    assert_eq!(err, CompileError::UnsupportedFlag('u'));
  }

  #[test]
  fn compile_propagates_invalid_pattern_error() {
    let err = compile("[", "").expect_err("unterminated class should fail");
    assert!(matches!(err, CompileError::InvalidPattern(_)));
  }

  #[test]
  fn compile_supports_named_captures() {
    let spec = compile(r"^(?<verb>[a-z]+)_(?<rest>.+)$", "").expect("should compile");
    let captures = spec.regex.captures("list_pets").expect("should match");
    assert_eq!(captures.name("verb").unwrap().as_str(), "list");
    assert_eq!(captures.name("rest").unwrap().as_str(), "pets");
  }
}
