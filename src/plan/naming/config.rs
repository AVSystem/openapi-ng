//! Internal representation of the user-facing `NamingConfig`. The NAPI
//! boundary projects `bindings::NamingOptions` into this shape after
//! flag-validating each parse spec and unwrapping the JS RegExp into
//! `{ source, flags }`.

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

/// A single entry in a chain — either a bare format-string shorthand or
/// a full `Rule`. The shorthand is equivalent to `Rule { format:
/// Some(s), case: None, .. }`; we keep them distinct so config-time
/// error messages can name the source form precisely.
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
  pub(crate) fn parse(s: &str) -> Option<Self> {
    match s {
      "camel" => Some(Self::Camel),
      "pascal" => Some(Self::Pascal),
      "snake" => Some(Self::Snake),
      "kebab" => Some(Self::Kebab),
      "constant" => Some(Self::Constant),
      _ => None,
    }
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
