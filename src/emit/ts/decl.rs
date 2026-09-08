//! Declaration-level emit: JSDoc, interfaces, type aliases, literal unions.

use super::literal::quoted;
use super::types::{Render, property_declaration};
use super::writer::{Writer, wln};

/// Width below which a top-level literal union stays on one line. Counts
/// the joined `'a' | 'b'` form, not the `export type X = ` prefix, and
/// matches prettier's default `printWidth`.
const UNION_INLINE_WIDTH: usize = 80;

/// The JSDoc a declaration carries.
#[derive(Clone, Copy, Default)]
pub(crate) struct Doc<'a> {
  pub(crate) description: Option<&'a str>,
  /// The source declared `deprecated: true`; renders as an `@deprecated`
  /// tag so IDEs mark the reference site.
  pub(crate) deprecated: bool,
}

impl<'a> Doc<'a> {
  pub(crate) const fn new(description: Option<&'a str>, deprecated: bool) -> Self {
    Self {
      description,
      deprecated,
    }
  }

  /// Prose with trailing whitespace trimmed, or `None` when it is empty.
  fn prose(self) -> Option<&'a str> {
    self
      .description
      .map(str::trim_end)
      .filter(|text| !text.is_empty())
  }

  fn is_empty(self) -> bool {
    self.prose().is_none() && !self.deprecated
  }
}

/// Emits `doc` as a JSDoc block, or nothing when it carries neither prose
/// nor a deprecation.
pub(crate) fn jsdoc(out: &mut Writer, doc: Doc<'_>) {
  if doc.is_empty() {
    return;
  }
  out.line("/**");
  if let Some(text) = doc.prose() {
    for line in text.lines() {
      let body = line.trim_end();
      if body.is_empty() {
        out.line(" *");
      } else {
        wln!(out, " * {}", body.replace("*/", "*\\/"));
      }
    }
  }
  if doc.deprecated {
    out.line(" * @deprecated");
  }
  out.line(" */");
}

/// One member of an emitted interface.
pub(crate) struct Member<'a> {
  pub(crate) name: &'a str,
  pub(crate) optional: bool,
  pub(crate) ty: &'a dyn Render,
  pub(crate) doc: Doc<'a>,
}

/// Emits `interface {name} { ... }`, exported unless `exported` is false.
pub(crate) fn interface_block<'a>(
  out: &mut Writer,
  name: &str,
  doc: Doc<'_>,
  members: impl IntoIterator<Item = Member<'a>>,
  exported: bool,
) {
  jsdoc(out, doc);
  let keyword = if exported {
    "export interface "
  } else {
    "interface "
  };
  out.open_block(&format!("{keyword}{name}"));
  members.into_iter().for_each(|member| {
    jsdoc(out, member.doc);
    property_declaration(out, member.name, member.optional, &member.ty);
    out.push(";\n");
  });
  out.close_block("");
}

/// Emits `export type {name} = {rhs};`.
pub(crate) fn type_alias(out: &mut Writer, name: &str, doc: Doc<'_>, rhs: &str) {
  jsdoc(out, doc);
  wln!(out, "export type {name} = {rhs};");
}

/// Emits a string-literal union, collapsing to one line when it fits
/// [`UNION_INLINE_WIDTH`].
pub(crate) fn string_union(out: &mut Writer, name: &str, doc: Doc<'_>, values: &[String]) {
  jsdoc(out, doc);

  // Upper bound on the joined width. Byte length over-counts the visual
  // width of non-ASCII values, which can only force an extra wrap.
  let separators = values.len().saturating_sub(1) * " | ".len();
  let quotes: usize = values.iter().map(|value| value.len() + 2).sum();

  if separators + quotes <= UNION_INLINE_WIDTH {
    let inline = values
      .iter()
      .map(|value| quoted(value))
      .collect::<Vec<_>>()
      .join(" | ");
    wln!(out, "export type {name} = {inline};");
    return;
  }

  wln!(out, "export type {name} =");
  out.indent();
  let last = values.len().saturating_sub(1);
  for (index, value) in values.iter().enumerate() {
    let terminator = if index == last { ";" } else { "" };
    wln!(out, "| {}{terminator}", quoted(value));
  }
  out.dedent();
}
