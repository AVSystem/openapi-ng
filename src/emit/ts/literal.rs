//! String-literal and property-name escaping.

use std::borrow::Cow;

use crate::identifier::is_identifier;

/// Appends `value` to `out` as a single-quoted TypeScript string literal,
/// quotes included.
pub(crate) fn escape_into(out: &mut String, value: &str) {
  out.reserve(value.len() + 2);
  out.push('\'');
  value.chars().for_each(|ch| match escape_sequence(ch) {
    Some(sequence) => out.push_str(sequence),
    None => out.push(ch),
  });
  out.push('\'');
}

/// The escape `ch` needs, or `None` when it stands for itself.
const fn escape_sequence(ch: char) -> Option<&'static str> {
  match ch {
    '\\' => Some("\\\\"),
    '\'' => Some("\\'"),
    '\n' => Some("\\n"),
    '\r' => Some("\\r"),
    '\t' => Some("\\t"),
    _ => None,
  }
}

/// `value` as a single-quoted TypeScript string literal.
pub(crate) fn quoted(value: &str) -> String {
  let mut out = String::with_capacity(value.len() + 2);
  escape_into(&mut out, value);
  out
}

/// Quotes `name` when it falls outside `[A-Za-z_$][A-Za-z0-9_$]*`, which
/// leaves a reserved word like `class` unquoted in property position.
pub(crate) fn safe_property_name(name: &str) -> Cow<'_, str> {
  if is_identifier(name) {
    Cow::Borrowed(name)
  } else {
    Cow::Owned(quoted(name))
  }
}
