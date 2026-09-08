//! Type-expression rendering.

use crate::api_model::canonical::BodyFieldType;
use crate::api_model::schema::{SchemaProperty, SchemaScalar, SchemaType};

use super::literal::safe_property_name;
use super::writer::{Writer, write_separated};

/// Syntactic position of a rendered type, which decides whether a composite
/// needs parentheses so the surrounding operator binds correctly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Position {
  /// A type-alias right-hand side, a property type, a generic argument —
  /// nothing binds tighter, so nothing is parenthesized.
  Standalone,
  /// A composition member (`A | B`, `A & B`) or an array element (`X[]`),
  /// where a composite child must be parenthesized.
  ///
  /// An inline object never needs it: both `A & { x: T }` and `{ x: T }[]`
  /// parse unambiguously.
  Wrapped,
}

/// A value with a TypeScript type expression.
pub(crate) trait Render {
  /// Appends the type expression, parenthesized if `at` requires it.
  fn render(&self, out: &mut Writer, at: Position);
}

impl Render for SchemaType {
  fn render(&self, out: &mut Writer, at: Position) {
    if at == Position::Wrapped && self.is_composite() {
      out.push("(");
      self.render_inner(out);
      out.push(")");
    } else {
      self.render_inner(out);
    }
  }
}

impl Render for BodyFieldType {
  /// Binary parts surface as `Blob | File`, the union
  /// `FormData.append` accepts. Arrays of binary keep the parentheses so
  /// `(Blob | File)[]` does not read as `Blob | File[]`.
  fn render(&self, out: &mut Writer, _at: Position) {
    match self {
      Self::Scalar(scalar) => out.push(scalar_keyword(scalar)),
      Self::ArrayOfScalar(scalar) => {
        out.push(scalar_keyword(scalar));
        out.push("[]");
      }
      Self::Binary => out.push("Blob | File"),
      Self::ArrayOfBinary => out.push("(Blob | File)[]"),
    }
  }
}

/// Lets a reference stand in for the value it points at.
impl<T: Render + ?Sized> Render for &T {
  fn render(&self, out: &mut Writer, at: Position) {
    (**self).render(out, at);
  }
}

impl SchemaType {
  const fn is_composite(&self) -> bool {
    matches!(
      self,
      Self::Union { .. } | Self::Intersection(_) | Self::Nullable(_)
    )
  }

  fn render_inner(&self, out: &mut Writer) {
    match self {
      Self::Any => out.push("unknown"),
      Self::Scalar(scalar) => out.push(scalar_keyword(scalar)),
      Self::Array(items) => {
        items.render(out, Position::Wrapped);
        out.push("[]");
      }
      Self::Map(values) => {
        out.push("Record<string, ");
        values.render(out, Position::Standalone);
        out.push(">");
      }
      Self::StringLiterals { values } => render_literal_union(out, values),
      Self::Ref(name) => out.push(name),
      Self::Union { members, .. } => {
        if members.is_empty() {
          out.push("never");
        } else {
          render_composition(out, members, " | ");
        }
      }
      Self::Intersection(members) => render_composition(out, members, " & "),
      Self::InlineObject { properties } => render_inline_object(out, properties),
      Self::Nullable(inner) => {
        // `A | B | null`, not `(A | B) | null`: the flat form mirrors
        // OpenAPI 3.1's `oneOf: [A, B, null]`.
        let at = if matches!(inner.as_ref(), Self::Union { .. }) {
          Position::Standalone
        } else {
          Position::Wrapped
        };
        inner.render(out, at);
        out.push(" | null");
      }
    }
  }
}

/// Appends `name`, its optional marker, and its type as an interface member
/// — without the trailing `;`.
fn property_declaration(out: &mut Writer, name: &str, optional: bool, type_expr: &impl Render) {
  out.push(&safe_property_name(name));
  if optional {
    out.push("?");
  }
  out.push(": ");
  type_expr.render(out, Position::Standalone);
}

/// Writes `name{?}: {type};` and ends the line.
pub(crate) fn member_declaration(
  out: &mut Writer,
  name: &str,
  optional: bool,
  type_expr: &impl Render,
) {
  property_declaration(out, name, optional, type_expr);
  out.push(";\n");
}

const fn scalar_keyword(scalar: &SchemaScalar) -> &'static str {
  match scalar {
    SchemaScalar::String => "string",
    SchemaScalar::Number => "number",
    SchemaScalar::Boolean => "boolean",
  }
}

fn render_composition(out: &mut Writer, members: &[SchemaType], separator: &str) {
  write_separated(out, members, separator, |out, member| {
    member.render(out, Position::Wrapped);
  });
}

fn render_literal_union(out: &mut Writer, values: &[String]) {
  write_separated(out, values, " | ", |out, value| {
    out.push(&super::literal::quoted(value));
  });
}

fn render_inline_object(out: &mut Writer, properties: &[SchemaProperty]) {
  if properties.is_empty() {
    out.push("Record<string, never>");
    return;
  }
  out.inline_block(|out| {
    properties.iter().for_each(|property| {
      member_declaration(out, &property.name, !property.required, &property.schema);
    });
  });
}

/// Renders `value` into a fresh `String`.
#[cfg(test)]
pub(crate) fn render_to_string(value: &impl Render) -> String {
  let mut out = Writer::with_capacity(128);
  value.render(&mut out, Position::Standalone);
  out.into_string()
}
