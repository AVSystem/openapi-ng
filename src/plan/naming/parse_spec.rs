//! A caller's `parse` regex, compiled once so evaluating a rule is a `captures` call.

use std::borrow::Cow;

use regex::{Regex, RegexBuilder};

use crate::{
  bindings::NamingParseSpec,
  error::{Diagnostic, DiagnosticCode, Reporter},
};

#[derive(Debug, Clone)]
pub(crate) struct CompiledParseSpec {
  pub(crate) regex: Regex,
}

/// Why a `parse` spec could not be compiled.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CompileError {
  UnsupportedFlag(char),
  InvalidPattern(String),
}

impl std::fmt::Display for CompileError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::UnsupportedFlag(flag) => {
        write!(
          formatter,
          "unsupported flag `{flag}`; only `i`, `m` and `s` change matching (`g`, `y`, `d` and \
           `u` are ignored)"
        )
      }
      Self::InvalidPattern(message) => formatter.write_str(message),
    }
  }
}

impl std::error::Error for CompileError {}

/// Compiles a JS `RegExp`'s `source` and `flags`. `i`, `m` and `s` map to the engine's own; `g`,
/// `y`, `d` and `u` are ignored (one match is tested, and the engine is always Unicode).
pub(crate) fn compile(source: &str, flags: &str) -> Result<CompiledParseSpec, CompileError> {
  let source = escape_literal_braces(source);
  let builder = flags
    .chars()
    .try_fold(RegexBuilder::new(&source), |mut builder, flag| {
      match flag {
        'i' => builder.case_insensitive(true),
        'm' => builder.multi_line(true),
        's' => builder.dot_matches_new_line(true),
        'g' | 'y' | 'd' | 'u' => &mut builder,
        unsupported => return Err(CompileError::UnsupportedFlag(unsupported)),
      };
      Ok(builder)
    })?;
  let regex = builder
    .build()
    .map_err(|err| CompileError::InvalidPattern(err.to_string()))?;
  Ok(CompiledParseSpec { regex })
}

/// Compiles an option's `{ source, flags }`, reporting a failure under `field`, its config path.
pub(crate) fn compile_option_pattern(
  spec: &NamingParseSpec,
  field: &str,
  reporter: &Reporter,
) -> Result<CompiledParseSpec, Diagnostic> {
  compile(&spec.source, &spec.flags).map_err(|error| {
    reporter.error(
      DiagnosticCode::InvalidOption,
      format!(
        "{field}: failed to compile regex `{}` (flags=`{}`): {error}",
        spec.source, spec.flags,
      ),
    )
  })
}

/// Escapes each `{` that opens no `{n}`, `{n,}` or `{n,m}` quantifier: JS reads it as a literal,
/// the engine rejects it (`/^\/pets\/{id}$/`). Escapes, classes and `\p{..}`, `\x{..}`, `\u{..}`
/// pass as is.
fn escape_literal_braces(source: &str) -> Cow<'_, str> {
  if !source.contains('{') {
    return Cow::Borrowed(source);
  }
  let mut escaped = String::with_capacity(source.len());
  let mut in_class = false;
  let mut chars = source.char_indices().peekable();
  while let Some((index, current)) = chars.next() {
    match current {
      '\\' => {
        escaped.push(current);
        if let Some((_, next)) = chars.next() {
          escaped.push(next);
          if matches!(next, 'p' | 'P' | 'x' | 'u' | 'U')
            && chars.peek().is_some_and(|&(_, brace)| brace == '{')
          {
            for (_, inner) in chars.by_ref() {
              escaped.push(inner);
              if inner == '}' {
                break;
              }
            }
          }
        }
      }
      '[' if !in_class => {
        in_class = true;
        escaped.push(current);
      }
      ']' if in_class => {
        in_class = false;
        escaped.push(current);
      }
      '{' if !in_class && !opens_quantifier(&source[index + 1..]) => escaped.push_str("\\{"),
      _ => escaped.push(current),
    }
  }
  // Escaping only lengthens the source, so an equal length means nothing was escaped.
  if escaped.len() == source.len() {
    Cow::Borrowed(source)
  } else {
    Cow::Owned(escaped)
  }
}

/// Whether `rest`, the text after a `{`, is `n}`, `n,}` or `n,m}`.
fn opens_quantifier(rest: &str) -> bool {
  let Some((body, _)) = rest.split_once('}') else {
    return false;
  };
  let (min, max) = body.split_once(',').unwrap_or((body, ""));
  let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
  !min.is_empty() && digits(min) && digits(max)
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
  fn compile_ignores_flags_that_do_not_change_one_match() {
    let spec = compile(r"^foo$", "dgiuy").expect("should compile");
    assert!(spec.regex.is_match("FOO"));
  }

  #[test]
  fn compile_rejects_the_unicode_sets_flag() {
    let err = compile(r".", "v").expect_err("`v` should be rejected");
    assert_eq!(err, CompileError::UnsupportedFlag('v'));
  }

  #[test]
  fn compile_reads_a_brace_that_opens_no_quantifier_as_a_literal() {
    let spec = compile(r"^\/pets\/{id}$", "").expect("should compile");
    assert!(spec.regex.is_match("/pets/{id}"));
    let spec = compile(r"^a{,2}{$", "").expect("should compile");
    assert!(spec.regex.is_match("a{,2}{"));
  }

  #[test]
  fn compile_keeps_quantifiers_escapes_and_classes() {
    let spec = compile(r"^a{2}b{1,}c{1,2}$", "").expect("should compile");
    assert!(spec.regex.is_match("aabbbcc"));
    assert!(!spec.regex.is_match("abc"));
    let spec = compile(r"^\{[{]\p{Lu}\x{41}$", "").expect("should compile");
    assert!(spec.regex.is_match("{{ÉA"));
    let spec = compile(r"^\u{1F600}$", "u").expect("should compile");
    assert!(spec.regex.is_match("😀"));
  }

  #[test]
  fn compile_leaves_a_source_without_literal_braces_borrowed() {
    assert!(matches!(escape_literal_braces(r"a{2}"), Cow::Borrowed(_)));
    assert!(matches!(escape_literal_braces(r"{id}"), Cow::Owned(_)));
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
