//! Shared identifier validation used across normalize and emit.
//!
//! Normalize calls this to reject untrusted spec strings (form-field
//! names, path-template parameter names) before they reach emit and
//! land as bare JS identifiers; emit calls this when deciding whether
//! a property name needs quoting.

/// True when `name` is a valid bare JavaScript / TypeScript identifier
/// (restricted to the ASCII subset). Matches the production grammar
/// `[A-Za-z_$][A-Za-z0-9_$]*` — digits-first, kebab-case, dotted, or
/// whitespace-bearing names all reject.
pub(crate) fn is_valid_identifier(name: &str) -> bool {
  let mut chars = name.chars();
  let first_ok = chars
    .next()
    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$');
  first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}
