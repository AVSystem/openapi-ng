//! String-literal and property-name escaping.

use std::borrow::Cow;

use crate::ident::is_ident;

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

/// Quotes `name` when it is not a bare identifier.
///
/// Reserved words such as `class` or `default` are legal in property
/// position — an interface member is a `PropertyName`, which accepts any
/// `IdentifierName` — so only names outside the
/// `[A-Za-z_$][A-Za-z0-9_$]*` shape get quoted.
pub(crate) fn safe_property_name(name: &str) -> Cow<'_, str> {
  if is_ident(name) {
    Cow::Borrowed(name)
  } else {
    Cow::Owned(quoted(name))
  }
}
