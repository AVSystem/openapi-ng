//! Tokenizer + case transformations. Tokens are split on any
//! non-alphanumeric character (covers `_`, `-`, whitespace, punctuation)
//! and on case transitions. A run of consecutive uppercase letters is
//! treated as a single token; downstream cases title-case that token,
//! so `getURLPath` → `getUrlPath` for camelCase.
//!
//! This is the single tokenizer used both by the user-facing `case` rule
//! engine and by the project-fixed legacy helpers (`service_class_name`,
//! `service_file_stem`, `request_interface_name`, `infer_body_field_name`),
//! so all naming-side case conversions agree on edge cases.

use crate::plan::naming::config::Case;

pub(crate) fn tokenize(s: &str) -> Vec<String> {
  let mut tokens: Vec<String> = Vec::new();
  let mut current = String::new();
  let chars: Vec<char> = s.chars().collect();
  let mut i = 0;
  while i < chars.len() {
    let ch = chars[i];
    if !ch.is_alphanumeric() {
      if !current.is_empty() {
        tokens.push(std::mem::take(&mut current));
      }
      i += 1;
      continue;
    }
    // Case transition: lowercase/digit → uppercase starts a new token.
    if let Some(prev) = current.chars().last() {
      let prev_lower_or_digit = prev.is_ascii_lowercase() || prev.is_ascii_digit();
      if prev_lower_or_digit && ch.is_ascii_uppercase() {
        tokens.push(std::mem::take(&mut current));
        current.push(ch);
        i += 1;
        continue;
      }
    }
    // Uppercase run followed by lowercase: the last uppercase belongs to
    // the next token. e.g. "URLPath" → ["URL", "Path"]: when reading
    // 'P' we know 'L' was the last upper, and the next char would be
    // lower — but we only see the lower one char later. So at lowercase,
    // if the previous two chars were upper+upper, peel the trailing
    // upper into a new token.
    if ch.is_ascii_lowercase() && current.len() >= 2 {
      let last_two: Vec<char> = current.chars().rev().take(2).collect();
      if last_two[0].is_ascii_uppercase() && last_two[1].is_ascii_uppercase() {
        let peeled = current.pop().unwrap();
        tokens.push(std::mem::take(&mut current));
        current.push(peeled);
      }
    }
    current.push(ch);
    i += 1;
  }
  if !current.is_empty() {
    tokens.push(current);
  }
  tokens
}

pub(crate) fn apply(s: &str, case: Case) -> String {
  let tokens = tokenize(s);
  if tokens.is_empty() {
    return String::new();
  }
  match case {
    Case::Camel => {
      let mut out = String::new();
      for (i, t) in tokens.iter().enumerate() {
        if i == 0 {
          out.push_str(&t.to_ascii_lowercase());
        } else {
          out.push_str(&title_case(t));
        }
      }
      out
    }
    Case::Pascal => tokens.iter().map(|t| title_case(t)).collect(),
    Case::Snake => tokens
      .iter()
      .map(|t| t.to_ascii_lowercase())
      .collect::<Vec<_>>()
      .join("_"),
    Case::Kebab => tokens
      .iter()
      .map(|t| t.to_ascii_lowercase())
      .collect::<Vec<_>>()
      .join("-"),
    Case::Constant => tokens
      .iter()
      .map(|t| t.to_ascii_uppercase())
      .collect::<Vec<_>>()
      .join("_"),
  }
}

fn title_case(t: &str) -> String {
  let mut chars = t.chars();
  chars.next().map_or_else(String::new, |first| {
    let mut out = String::new();
    out.extend(first.to_uppercase());
    out.push_str(&chars.as_str().to_ascii_lowercase());
    out
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tokenize_splits_on_underscore_hyphen_space() {
    assert_eq!(tokenize("get_some_thing"), vec!["get", "some", "thing"]);
    assert_eq!(tokenize("get-some-thing"), vec!["get", "some", "thing"]);
    assert_eq!(tokenize("get some thing"), vec!["get", "some", "thing"]);
  }

  #[test]
  fn tokenize_splits_on_any_non_alphanumeric_punctuation() {
    assert_eq!(tokenize("get.some/thing"), vec!["get", "some", "thing"]);
    assert_eq!(tokenize("get!some@thing"), vec!["get", "some", "thing"]);
    assert_eq!(tokenize("a__b---c"), vec!["a", "b", "c"]);
  }

  #[test]
  fn tokenize_splits_on_camel_case_transition() {
    assert_eq!(tokenize("getSomeThing"), vec!["get", "Some", "Thing"]);
  }

  #[test]
  fn tokenize_treats_consecutive_uppercase_as_single_token() {
    // From the spec example.
    assert_eq!(tokenize("getURLPath"), vec!["get", "URL", "Path"]);
  }

  #[test]
  fn tokenize_handles_trailing_uppercase_run() {
    assert_eq!(tokenize("parseURL"), vec!["parse", "URL"]);
  }

  #[test]
  fn tokenize_handles_leading_uppercase_run() {
    assert_eq!(tokenize("URLPath"), vec!["URL", "Path"]);
  }

  #[test]
  fn apply_camel_matches_spec_example_table_row() {
    // spec: `get_someThing` → camel → `getSomeThing`
    assert_eq!(apply("get_someThing", Case::Camel), "getSomeThing");
  }

  #[test]
  fn apply_pascal_matches_spec_example_table_row() {
    assert_eq!(apply("get_someThing", Case::Pascal), "GetSomeThing");
  }

  #[test]
  fn apply_snake_matches_spec_example_table_row() {
    assert_eq!(apply("get_someThing", Case::Snake), "get_some_thing");
  }

  #[test]
  fn apply_kebab_matches_spec_example_table_row() {
    assert_eq!(apply("get_someThing", Case::Kebab), "get-some-thing");
  }

  #[test]
  fn apply_constant_matches_spec_example_table_row() {
    assert_eq!(apply("get_someThing", Case::Constant), "GET_SOME_THING");
  }

  #[test]
  fn apply_camel_handles_consecutive_uppercase_run_per_spec() {
    // spec: `getURLPath` → camelCase → `getUrlPath`
    assert_eq!(apply("getURLPath", Case::Camel), "getUrlPath");
  }
}
