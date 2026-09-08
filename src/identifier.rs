//! Name types checked at construction, so a holder may interpolate one
//! into generated TypeScript without quoting or escaping.

/// An ASCII JavaScript identifier: `[A-Za-z_$][A-Za-z0-9_$]*`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Identifier(Box<str>);

impl Identifier {
  /// Returns `None` when `name` is not a bare identifier — digits-first,
  /// kebab-case, dotted, empty, or whitespace-bearing names all reject.
  pub(crate) fn parse(name: &str) -> Option<Self> {
    is_identifier(name).then(|| Self(Box::from(name)))
  }

  pub(crate) fn as_str(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Display for Identifier {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

/// True when `name` is a bare identifier. Prefer [`Identifier::parse`] where
/// the validated name is kept.
pub(crate) fn is_identifier(name: &str) -> bool {
  let mut chars = name.chars();
  chars
    .next()
    .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
    && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

/// An operation's method name after the naming rules have run.
///
/// Not the spec's `operationId`: a rule may rewrite `Pet_listPets` into
/// `listPets`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MethodName(String);

impl MethodName {
  pub(crate) const fn new(name: String) -> Self {
    Self(name)
  }

  pub(crate) fn as_str(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Display for MethodName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

/// A PascalCase TypeScript type name emitted by the generator, derived from
/// a [`MethodName`] or a service group.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TypeName(String);

impl TypeName {
  pub(crate) const fn new(name: String) -> Self {
    Self(name)
  }

  pub(crate) fn as_str(&self) -> &str {
    &self.0
  }
}

impl std::fmt::Display for TypeName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.0)
  }
}

#[cfg(test)]
mod tests {
  use super::{Identifier, is_identifier};

  #[test]
  fn accepts_the_bare_identifier_grammar() {
    for name in ["pet", "_pet", "$pet", "Pet2", "a_b$c9"] {
      assert!(Identifier::parse(name).is_some(), "{name} must parse");
    }
  }

  #[test]
  fn rejects_names_that_need_quoting() {
    for name in ["", "2pet", "pet-name", "pet.name", "pet name", "pét"] {
      assert!(Identifier::parse(name).is_none(), "{name} must reject");
      assert!(!is_identifier(name));
    }
  }

  #[test]
  fn parsed_identifier_round_trips_its_source() {
    let ident = Identifier::parse("listPets").expect("bare identifier");
    assert_eq!(ident.as_str(), "listPets");
    assert_eq!(ident.to_string(), "listPets");
  }
}
