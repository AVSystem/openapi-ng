//! Unified emit-layer writer.
//!
//! `Writer` is the single output engine for every emitter (TS models,
//! Angular services). It owns the `String` buffer, tracks indent
//! depth, and exposes both raw output primitives (`push`, `line`,
//! `block`) and TypeScript-shaped helpers (`jsdoc`,
//! `interface_block`, `type_alias`, `string_union`, `import_block`,
//! `render_type`). Folding everything into one module collapses the
//! prior split across `CodeBuffer` (buffer engine), `primitives`
//! (interface/type-alias/import helpers), and `ts_renderer`
//! (type-expression rendering).
//!
//! Two policies that used to leak across modules now live exactly once:
//!   * **Parenthesization** — `render_type` consults `needs_parens` with
//!     a single `Position` enum (standalone / composition / array-item).
//!   * **Indentation** — `Writer.indent_cache` is the only ratchet.
//!
//! Most renderers take `&mut Writer` and append directly. The free
//! functions below (`safe_property_name`, `render_type_reference`)
//! exist for the few callers that need a standalone `String` without
//! owning a `Writer`.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::ir::canonical::BodyFieldType;
use crate::ir::identifier::is_valid_identifier;
use crate::ir::schema::{SchemaProperty, SchemaScalar, SchemaType};
use crate::wln;

/// Width budget below which a top-level string union renders on a single line.
/// Counts the joined `'a' | 'b' | 'c'` form, not the `export type X = ` prefix.
/// Matches prettier's default `printWidth: 80`.
const ENUM_INLINE_WIDTH: usize = 80;

/// Width budget for a single-line `import { ... } from '...';` statement.
/// Lines that would exceed this when joined fall back to a multi-line
/// form (one identifier per indented line) so subsequent formatter runs
/// don't re-wrap the file and produce non-empty diffs on every regen.
const IMPORT_INLINE_WIDTH: usize = 100;

// ── Buffer engine ────────────────────────────────────────────────────────────

/// Indent-aware string writer used by every emit target. Tracks line-start
/// state so consecutive `push` calls share an indent prefix without the
/// caller threading it explicitly.
#[derive(Debug, Default)]
pub(crate) struct Writer {
  buf: String,
  indent_cache: String,
  indent_level: usize,
  line_start: bool,
  last_was_blank: bool,
}

impl Writer {
  pub(crate) fn with_capacity(capacity: usize) -> Self {
    Self {
      buf: String::with_capacity(capacity),
      indent_cache: String::new(),
      indent_level: 0,
      line_start: true,
      last_was_blank: false,
    }
  }

  pub(crate) fn push(&mut self, value: &str) {
    // Fast path: mid-line, no embedded newline — the common case during
    // type/expression emission ("(", ", ", identifier tokens). Skips the
    // indent bookkeeping and `find('\n')` loop below. Slow path still
    // handles every state transition (indent emission, blank-line tracking,
    // multi-line literals).
    if !self.line_start && !value.contains('\n') {
      if !value.is_empty() {
        self.buf.push_str(value);
        self.last_was_blank = false;
      }
      return;
    }

    let mut rest = value;
    while !rest.is_empty() {
      if self.line_start {
        if let Some(stripped) = rest.strip_prefix('\n') {
          self.buf.push('\n');
          self.last_was_blank = true;
          rest = stripped;
          continue;
        }
        self.write_indent();
      }

      if let Some(pos) = rest.find('\n') {
        self.buf.push_str(&rest[..=pos]);
        let line_had_content = pos > 0;
        rest = &rest[pos + 1..];
        self.line_start = true;
        if line_had_content {
          self.last_was_blank = false;
        }
      } else {
        self.buf.push_str(rest);
        self.line_start = false;
        self.last_was_blank = false;
        break;
      }
    }
  }

  pub(crate) fn line(&mut self, value: &str) {
    let was_empty = value.is_empty() && self.line_start;
    self.push(value);
    self.buf.push('\n');
    self.line_start = true;
    if was_empty {
      self.last_was_blank = true;
    }
  }

  pub(crate) fn blank_line(&mut self) {
    if self.buf.is_empty() {
      return;
    }

    if self.last_was_blank {
      return;
    }

    if !self.buf.ends_with('\n') {
      self.buf.push('\n');
    }

    self.buf.push('\n');
    self.line_start = true;
    self.last_was_blank = true;
  }

  pub(crate) fn open_block(&mut self, header: &str) {
    if header.is_empty() {
      self.line("{");
    } else {
      // Infallible writer; sidestep `std::fmt::Write` so we don't drag in
      // an `.unwrap()` for an error the buffer never produces.
      self.push(header);
      self.push(" {");
      self.buf.push('\n');
      self.line_start = true;
      self.last_was_blank = false;
    }
    self.indent();
  }

  pub(crate) fn close_block(&mut self, suffix: &str) {
    self.dedent();
    if suffix.is_empty() {
      self.line("}");
    } else {
      self.push("}");
      self.push(suffix);
      self.buf.push('\n');
      self.line_start = true;
      self.last_was_blank = false;
    }
  }

  pub(crate) fn into_string(self) -> String {
    self.buf
  }

  pub(crate) fn indent(&mut self) {
    self.indent_level += 1;
    self.indent_cache.push_str("  ");
  }

  pub(crate) fn dedent(&mut self) {
    self.indent_level = self
      .indent_level
      .checked_sub(1)
      .expect("over-dedent in emitter");
    let new_len = self.indent_level * 2;
    self.indent_cache.truncate(new_len);
  }

  fn write_indent(&mut self) {
    self.buf.push_str(&self.indent_cache);
    self.line_start = false;
  }
}

impl std::fmt::Write for Writer {
  fn write_str(&mut self, s: &str) -> std::fmt::Result {
    self.push(s);
    Ok(())
  }
}

// ── Identifier escaping ──────────────────────────────────────────────────────

/// Quote a property name when it isn't a valid bare TS identifier.
///
/// Reserved words like `class` or `default` are valid in property position in
/// TypeScript (an interface property is `PropertyName`, which accepts any
/// `IdentifierName`), so we only quote when the source name contains
/// characters that would make it syntactically invalid bare: anything other
/// than the `[A-Za-z_$][A-Za-z0-9_$]*` shape (digits-first, kebab-case,
/// dotted, whitespace, etc.). Quoting uses single quotes with backslash
/// escapes, matching `write_string_literal`.
pub(crate) fn safe_property_name(name: &str) -> Cow<'_, str> {
  if is_valid_identifier(name) {
    Cow::Borrowed(name)
  } else {
    let mut out = String::with_capacity(name.len() + 2);
    write_string_literal(&mut out, name);
    Cow::Owned(out)
  }
}

// ── TS-shape helpers ─────────────────────────────────────────────────────────

/// Emit a JSDoc block for the given description, if any. The description
/// may be multi-line (e.g. when OpenAPI `summary` and `description` are
/// merged with a blank-line separator); each line becomes ` * <line>`.
/// Emits nothing when description is None or empty after trimming AND
/// `deprecated` is false. When `deprecated` is true an `@deprecated` tag
/// line is added (after the description body if both are present) so the
/// emitted output surfaces the deprecation marker to IDE tooltips and
/// linters at the call site.
pub(crate) fn jsdoc(out: &mut Writer, description: Option<&str>, deprecated: bool) {
  let trimmed = description.map(str::trim_end).filter(|s| !s.is_empty());
  if trimmed.is_none() && !deprecated {
    return;
  }
  out.line("/**");
  if let Some(text) = trimmed {
    for line in text.lines() {
      let body = line.trim_end();
      if body.is_empty() {
        out.line(" *");
      } else {
        let escaped = body.replace("*/", "*\\/");
        wln!(out, " * {escaped}");
      }
    }
  }
  if deprecated {
    out.line(" * @deprecated");
  }
  out.line(" */");
}

/// Emit an `interface` block with the given properties.
/// Properties are tuples of `(name, optional, ty, description, deprecated)`.
/// Nullability is folded into `ty` (`SchemaType::Nullable(...)`) rather than
/// carried alongside it — one carrier for the whole IR. The trailing
/// `deprecated` flag emits a `@deprecated` JSDoc tag above the property
/// declaration when the source schema's `deprecated: true` is set.
pub(crate) fn interface_block<'a>(
  out: &mut Writer,
  name: &str,
  description: Option<&str>,
  deprecated: bool,
  properties: impl IntoIterator<Item = (&'a str, bool, &'a SchemaType, Option<&'a str>, bool)>,
  exported: bool,
) {
  jsdoc(out, description, deprecated);
  let keyword = if exported {
    "export interface "
  } else {
    "interface "
  };
  out.open_block(&format!("{keyword}{name}"));
  for (prop_name, optional, ty, prop_description, prop_deprecated) in properties {
    jsdoc(out, prop_description, prop_deprecated);
    write_property_declaration(out, prop_name, optional, ty);
    out.push(";\n");
  }
  out.close_block("");
}

/// Emit `export type {name} = {rhs};` with an optional JSDoc header.
pub(crate) fn type_alias(
  out: &mut Writer,
  name: &str,
  description: Option<&str>,
  deprecated: bool,
  rhs: &str,
) {
  jsdoc(out, description, deprecated);
  wln!(out, "export type {name} = {rhs};");
}

/// Emit a string-literal union type, collapsing to one line when short.
pub(crate) fn string_union(
  out: &mut Writer,
  name: &str,
  description: Option<&str>,
  deprecated: bool,
  values: &[String],
) {
  jsdoc(out, description, deprecated);

  // Cheap upper bound on the joined `'a' | 'b' | 'c'` length: each value
  // contributes its byte length plus the two quote characters, separated
  // by ` | `. UTF-8 byte length over-counts visual width for non-ASCII
  // identifiers, which is fine — the budget exists to keep lines short,
  // and over-counting only ever forces an additional wrap.
  let separator_total = values.len().saturating_sub(1) * " | ".len();
  let quoted_total: usize = values.iter().map(|v| v.len() + 2).sum();
  let joined_width = separator_total + quoted_total;

  if joined_width <= ENUM_INLINE_WIDTH {
    let mut inline = String::with_capacity(joined_width);
    for (index, value) in values.iter().enumerate() {
      if index > 0 {
        inline.push_str(" | ");
      }
      write_string_literal(&mut inline, value);
    }
    wln!(out, "export type {name} = {inline};");
    return;
  }

  wln!(out, "export type {name} =");
  out.indent();
  let last = values.len().saturating_sub(1);
  for (index, value) in values.iter().enumerate() {
    let suffix = if index == last { ";" } else { "" };
    let mut literal = String::with_capacity(value.len() + 2);
    write_string_literal(&mut literal, value);
    wln!(out, "| {literal}{suffix}");
  }
  out.dedent();
}

/// Emit one `import [type] { ... } from '...';` line per path entry.
/// Names within each path are emitted in iteration order (callers should
/// pass a `BTreeSet` for stable output).
pub(crate) fn import_block(
  out: &mut Writer,
  by_path: &BTreeMap<&str, BTreeSet<&str>>,
  type_only: bool,
) {
  for (path, names) in by_path {
    write_import_line(out, names.iter().map(|n| (*n, None)), path, type_only);
  }
}

/// Write a single `import [type] { name [as alias], ... } from 'path';`
/// statement. Folds the 2 sites that duplicate this `format!` shape
/// (mapped-type imports, service type imports).
///
/// When the single-line form would exceed `IMPORT_INLINE_WIDTH`, the
/// statement is emitted multi-line — one identifier per indented line,
/// closing `} from '...';` on its own line — so that prettier-shaped
/// consumer formatters don't re-wrap on the first save and regenerate
/// always produces an empty diff.
pub(crate) fn write_import_line<'a>(
  out: &mut Writer,
  names: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
  path: &str,
  type_only: bool,
) {
  let prefix = if type_only {
    "import type { "
  } else {
    "import { "
  };
  let suffix_len = " } from '".len() + path.len() + "';".len();
  // Buffer the (name, alias) pairs so we can measure the joined width
  // before committing to inline vs multi-line. Names are short
  // identifiers; the allocation is tiny in practice.
  let entries: Vec<(&'a str, Option<&'a str>)> = names.into_iter().collect();

  let names_width: usize = entries
    .iter()
    .map(|(name, alias)| name.len() + alias.map_or(0, |a| " as ".len() + a.len()))
    .sum();
  let separators_width = entries.len().saturating_sub(1) * ", ".len();
  let joined_width = prefix.len() + names_width + separators_width + suffix_len;

  if joined_width <= IMPORT_INLINE_WIDTH || entries.len() <= 1 {
    out.push(prefix);
    let mut first = true;
    for (name, alias) in &entries {
      if !first {
        out.push(", ");
      }
      first = false;
      out.push(name);
      if let Some(alias) = alias {
        out.push(" as ");
        out.push(alias);
      }
    }
    out.push(" } from '");
    out.push(path);
    out.push("';\n");
    return;
  }

  // Multi-line form: one identifier per indented line, trailing comma
  // on every entry (matches prettier's wrap style so the first
  // formatter pass on a consumer's checkout is a no-op).
  out.push(if type_only {
    "import type {\n"
  } else {
    "import {\n"
  });
  out.indent();
  for (name, alias) in &entries {
    out.push(name);
    if let Some(alias) = alias {
      out.push(" as ");
      out.push(alias);
    }
    out.push(",\n");
  }
  out.dedent();
  out.push("} from '");
  out.push(path);
  out.push("';\n");
}

// ── Type-expression rendering ────────────────────────────────────────────────

/// Syntactic position of a `SchemaType` reference. The position decides
/// whether a composite child needs to be parenthesized so that the
/// surrounding operator binds correctly. Centralising this in one place
/// removes the 3-way duplication we used to keep across separate
/// `render_type_reference` / `render_wrapped_type_reference` /
/// `render_array_item_reference` functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Position {
  /// Standalone reference (top-level type alias RHS, property type, etc.).
  /// Never parenthesizes the value.
  Standalone,
  /// Inside a context where composites (`A | B`, `A & B`, `T | null`) must
  /// be parenthesized so the surrounding operator binds correctly — both
  /// composition lists (`A | B`, `A & B`) and array-item position (`X[]`).
  /// Inline objects don't need wrapping in either context: `A & { x: T }`
  /// parses unambiguously, and `{a: T}[]` likewise.
  Wrapped,
}

/// Render `value` to `out` at the given syntactic position.
pub(crate) fn render_type(out: &mut Writer, value: &SchemaType, position: Position) {
  if needs_parens(value, position) {
    out.push("(");
    render_type_inner(out, value);
    out.push(")");
  } else {
    render_type_inner(out, value);
  }
}

/// Test-only string-returning shim around [`render_type`]. Production code
/// streams type references straight into the active `Writer`; the only
/// callers that need a `String` back are unit tests that compare rendered
/// fragments.
#[cfg(test)]
pub(crate) fn render_type_reference(value: &SchemaType) -> String {
  let mut buf = Writer::with_capacity(128);
  render_type(&mut buf, value, Position::Standalone);
  buf.into_string()
}

pub(crate) fn write_property_declaration(
  out: &mut Writer,
  name: &str,
  optional: bool,
  ty: &SchemaType,
) {
  out.push(&safe_property_name(name));
  if optional {
    out.push("?");
  }
  out.push(": ");
  render_type(out, ty, Position::Standalone);
}

const fn needs_parens(value: &SchemaType, position: Position) -> bool {
  let is_composite = matches!(
    value,
    SchemaType::Union { .. } | SchemaType::Intersection(_) | SchemaType::Nullable(_)
  );
  match position {
    Position::Standalone => false,
    // Inline objects don't need wrapping in `A & { x: T }` — `&` binds
    // lower than the property-block, and TS parses the form
    // unambiguously. Composites (`|`, `&`, `T | null`) still need
    // wrapping so the surrounding operator binds correctly.
    Position::Wrapped => is_composite,
  }
}

fn render_type_inner(out: &mut Writer, value: &SchemaType) {
  match value {
    SchemaType::Any => out.push("unknown"),
    SchemaType::Scalar(scalar) => out.push(scalar_keyword(scalar)),
    SchemaType::Array(items) => {
      render_type(out, items, Position::Wrapped);
      out.push("[]");
    }
    SchemaType::Map(items) => {
      out.push("Record<string, ");
      render_type(out, items, Position::Standalone);
      out.push(">");
    }
    SchemaType::StringLiterals { values } => render_string_literal_union(out, values),
    SchemaType::Ref(name) => out.push(name),
    SchemaType::Union { members, .. } => {
      if members.is_empty() {
        out.push("never");
      } else {
        render_composition(out, members, " | ");
      }
    }
    SchemaType::Intersection(members) => render_composition(out, members, " & "),
    SchemaType::InlineObject { properties } => render_inline_object(out, properties),
    SchemaType::Nullable(inner) => {
      // Flatten `Nullable(Union { A, B })` into `A | B | null` rather
      // than `(A | B) | null` — both are equivalent TS but the flat form
      // mirrors OpenAPI 3.1's `oneOf: [A, B, null]` semantics. Other
      // inner shapes (Intersection, InlineObject) still need parens to
      // preserve precedence.
      let inner_position = if matches!(inner.as_ref(), SchemaType::Union { .. }) {
        Position::Standalone
      } else {
        Position::Wrapped
      };
      render_type(out, inner, inner_position);
      out.push(" | null");
    }
  }
}

const fn scalar_keyword(scalar: &SchemaScalar) -> &'static str {
  match scalar {
    SchemaScalar::String => "string",
    SchemaScalar::Number => "number",
    SchemaScalar::Boolean => "boolean",
  }
}

/// Render a form-body field type (multipart/urlencoded) as TS source.
///
/// Binary parts surface as `Blob | File` (the runtime union the fetch
/// `FormData.append` overload accepts); scalar parts reuse the same
/// keywords as `SchemaType::Scalar`. Arrays wrap binary unions in
/// parentheses so `(Blob | File)[]` parses as an array of unions rather
/// than the precedence trap `Blob | File[]`.
pub(crate) fn render_body_field_type(ty: &BodyFieldType) -> String {
  match ty {
    BodyFieldType::Scalar(scalar) => scalar_keyword(scalar).to_string(),
    BodyFieldType::ArrayOfScalar(scalar) => format!("{}[]", scalar_keyword(scalar)),
    BodyFieldType::Binary => "Blob | File".to_string(),
    BodyFieldType::ArrayOfBinary => "(Blob | File)[]".to_string(),
  }
}

fn render_composition(out: &mut Writer, members: &[SchemaType], separator: &str) {
  let mut first = true;
  for member in members {
    if !first {
      out.push(separator);
    }
    first = false;
    render_type(out, member, Position::Wrapped);
  }
}

fn render_string_literal_union(out: &mut Writer, values: &[String]) {
  let mut first = true;
  for value in values {
    if !first {
      out.push(" | ");
    }
    first = false;
    write_string_literal(out, value);
  }
}

fn render_inline_object(out: &mut Writer, properties: &[SchemaProperty]) {
  if properties.is_empty() {
    out.push("Record<string, never>");
    return;
  }
  out.push("{\n");
  out.indent();
  for property in properties {
    write_property_declaration(out, &property.name, !property.required, &property.ty);
    out.push(";\n");
  }
  out.dedent();
  out.push("}");
}

pub(crate) fn write_string_literal<W: std::fmt::Write>(out: &mut W, value: &str) {
  out.write_char('\'').unwrap();
  for ch in value.chars() {
    match ch {
      '\\' => out.write_str("\\\\").unwrap(),
      '\'' => out.write_str("\\'").unwrap(),
      '\n' => out.write_str("\\n").unwrap(),
      '\r' => out.write_str("\\r").unwrap(),
      '\t' => out.write_str("\\t").unwrap(),
      _ => out.write_char(ch).unwrap(),
    }
  }
  out.write_char('\'').unwrap();
}
