//! The name tokenizer and case renderers.

use crate::plan::naming::config::Case;

/// Splits `name` into its casing tokens, each borrowed from `name`.
pub(crate) const fn tokenize(name: &str) -> Tokens<'_> {
  Tokens { rest: name }
}

pub(crate) struct Tokens<'a> {
  rest: &'a str,
}

impl<'a> Iterator for Tokens<'a> {
  type Item = &'a str;

  fn next(&mut self) -> Option<&'a str> {
    let start = self.rest.find(char::is_alphanumeric)?;
    let token = &self.rest[start..];
    let end = token_len(token);
    self.rest = &token[end..];
    Some(&token[..end])
  }
}

/// Byte length of the token at the start of `token`.
fn token_len(token: &str) -> usize {
  let current = token.char_indices().skip(1);
  let previous = token.chars();
  let following = token.chars().skip(2).map(Some).chain(std::iter::once(None));

  current
    .zip(previous)
    .zip(following)
    .find(|(((_, current), previous), following)| {
      !current.is_alphanumeric() || splits_before(*previous, *current, *following)
    })
    .map_or(token.len(), |(((offset, _), _), _)| offset)
}

/// True when a token boundary falls immediately before `current`, which a
/// run of alphanumerics reaches at two case transitions: after a lowercase
/// or digit (`listPets`), and at the last uppercase of a run followed by a
/// lowercase (`URLPath`).
fn splits_before(previous: char, current: char, following: Option<char>) -> bool {
  let starts_after_lower = previous.is_ascii_lowercase() || previous.is_ascii_digit();
  let ends_upper_run = previous.is_ascii_uppercase()
    && current.is_ascii_uppercase()
    && following.is_some_and(|ch| ch.is_ascii_lowercase());
  starts_after_lower && current.is_ascii_uppercase() || ends_upper_run
}

/// Renders `name`'s tokens joined in the given case.
pub(crate) fn apply(name: &str, case: Case) -> String {
  tokenize(name).enumerate().fold(
    String::with_capacity(name.len()),
    |mut out, (index, token)| {
      if index > 0 {
        out.push_str(case.separator());
      }
      case.write_token(&mut out, token, index);
      out
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tokenize_splits_on_underscore_hyphen_space() {
    assert_eq!(
      tokenize("get_some_thing").collect::<Vec<_>>(),
      vec!["get", "some", "thing"]
    );
    assert_eq!(
      tokenize("get-some-thing").collect::<Vec<_>>(),
      vec!["get", "some", "thing"]
    );
    assert_eq!(
      tokenize("get some thing").collect::<Vec<_>>(),
      vec!["get", "some", "thing"]
    );
  }

  #[test]
  fn tokenize_splits_on_any_non_alphanumeric_punctuation() {
    assert_eq!(
      tokenize("get.some/thing").collect::<Vec<_>>(),
      vec!["get", "some", "thing"]
    );
    assert_eq!(
      tokenize("get!some@thing").collect::<Vec<_>>(),
      vec!["get", "some", "thing"]
    );
    assert_eq!(
      tokenize("a__b---c").collect::<Vec<_>>(),
      vec!["a", "b", "c"]
    );
  }

  #[test]
  fn tokenize_splits_on_camel_case_transition() {
    assert_eq!(
      tokenize("getSomeThing").collect::<Vec<_>>(),
      vec!["get", "Some", "Thing"]
    );
  }

  #[test]
  fn tokenize_treats_consecutive_uppercase_as_single_token() {
    // From the spec example.
    assert_eq!(
      tokenize("getURLPath").collect::<Vec<_>>(),
      vec!["get", "URL", "Path"]
    );
  }

  #[test]
  fn tokenize_handles_trailing_uppercase_run() {
    assert_eq!(
      tokenize("parseURL").collect::<Vec<_>>(),
      vec!["parse", "URL"]
    );
  }

  #[test]
  fn tokenize_handles_leading_uppercase_run() {
    assert_eq!(tokenize("URLPath").collect::<Vec<_>>(), vec!["URL", "Path"]);
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
