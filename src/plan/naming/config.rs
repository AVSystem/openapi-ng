//! The validated, regex-compiled form of the caller's naming config.

use crate::plan::naming::parse_spec::CompiledParseSpec;

#[derive(Debug, Clone, Default)]
pub struct NamingConfig {
  pub(crate) method_name: Option<Naming>,
  pub(crate) group: Option<Naming>,
}

#[derive(Debug, Clone)]
pub(crate) enum Naming {
  Single(RuleEntry),
  Chain(Vec<RuleEntry>),
}

/// One entry of a fallback chain.
///
/// `Shorthand(s)` behaves as `Rule { format: Some(s), .. }`; the two stay
/// distinct so a config error can name the form the caller wrote.
#[derive(Debug, Clone)]
pub(crate) enum RuleEntry {
  Shorthand(String),
  Rule(Rule),
}

#[derive(Debug, Clone)]
pub(crate) struct Rule {
  pub(crate) from: Option<String>,
  pub(crate) parse: Option<CompiledParseSpec>,
  pub(crate) format: Option<String>,
  pub(crate) case: Option<Case>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
  Camel,
  Pascal,
  Snake,
  Kebab,
  Constant,
}

impl Case {
  pub(crate) fn parse(value: &str) -> Option<Self> {
    match value {
      "camel" => Some(Self::Camel),
      "pascal" => Some(Self::Pascal),
      "snake" => Some(Self::Snake),
      "kebab" => Some(Self::Kebab),
      "constant" => Some(Self::Constant),
      _ => None,
    }
  }
}

impl Case {
  /// Text inserted between adjacent tokens.
  pub(crate) const fn separator(self) -> &'static str {
    match self {
      Self::Camel | Self::Pascal => "",
      Self::Snake | Self::Constant => "_",
      Self::Kebab => "-",
    }
  }

  /// Appends `token` to `out` in the casing this style uses at `index`.
  pub(crate) fn write_token(self, out: &mut String, token: &str, index: usize) {
    match self {
      Self::Camel if index == 0 => push_lower(out, token),
      Self::Camel | Self::Pascal => push_title(out, token),
      Self::Snake | Self::Kebab => push_lower(out, token),
      Self::Constant => push_upper(out, token),
    }
  }
}

fn push_lower(out: &mut String, token: &str) {
  out.extend(token.chars().map(|ch| ch.to_ascii_lowercase()));
}

fn push_upper(out: &mut String, token: &str) {
  out.extend(token.chars().map(|ch| ch.to_ascii_uppercase()));
}

fn push_title(out: &mut String, token: &str) {
  let mut chars = token.chars();
  if let Some(first) = chars.next() {
    out.extend(first.to_uppercase());
    out.extend(chars.map(|ch| ch.to_ascii_lowercase()));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn case_parses_all_five_spec_values() {
    assert_eq!(Case::parse("camel"), Some(Case::Camel));
    assert_eq!(Case::parse("pascal"), Some(Case::Pascal));
    assert_eq!(Case::parse("snake"), Some(Case::Snake));
    assert_eq!(Case::parse("kebab"), Some(Case::Kebab));
    assert_eq!(Case::parse("constant"), Some(Case::Constant));
  }

  #[test]
  fn case_parse_rejects_unknown_values() {
    assert_eq!(Case::parse("upper"), None);
    assert_eq!(Case::parse(""), None);
    assert_eq!(Case::parse("Camel"), None);
  }
}
