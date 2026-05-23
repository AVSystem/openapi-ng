use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SchemaProperty {
  pub(crate) name: Box<str>,
  pub(crate) required: bool,
  pub(crate) ty: SchemaType,
  /// Description carried over from `Schema.description` of the property's
  /// schema. Emitted as a JSDoc comment above the property declaration in
  /// named TypeScript interfaces. Not emitted inside inline-object types
  /// (where the multi-line comment would dominate the type expression).
  pub(crate) description: Option<String>,
  /// Source property schema's OpenAPI `deprecated: true`. Surfaces as
  /// `@deprecated` in the JSDoc above the property declaration in named
  /// interfaces — invisible inside inline-object positions (where no
  /// JSDoc is emitted) to keep parity with how `description` behaves.
  pub(crate) deprecated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SchemaType {
  /// OpenAPI "any" schema — a schema with no constraints (no `type`, no
  /// `$ref`, no composition). Renders as TS `unknown`.
  Any,
  Scalar(SchemaScalar),
  Array(Box<SchemaType>),
  Map(Box<SchemaType>),
  /// Literal-union vehicle (TS `'a' | 'b'`). Used both as a top-level
  /// `ModelSymbol.body` (renders as `export type X = 'a' | 'b'`) and as
  /// an anonymous in-place form inside compositions or for the synthetic
  /// single-value narrowings produced by `narrow_discriminator_properties`.
  StringLiterals {
    values: Vec<String>,
  },
  Ref(Box<str>),
  /// Type composition (`oneOf`/`anyOf`). `discriminator` is `Some(info)`
  /// when this comes from an OpenAPI `oneOf` with a discriminator; the
  /// semantic-finalize pass (`narrow_discriminator_properties`) reads it
  /// to rewrite each member's discriminator property to a single-value
  /// string literal so the TypeScript compiler can narrow the union to
  /// the concrete member type.
  Union {
    members: Vec<SchemaType>,
    discriminator: Option<Discriminator>,
  },
  Intersection(Vec<SchemaType>),
  InlineObject {
    properties: Vec<SchemaProperty>,
  },
  /// `nullable: true` carrier. Wraps any other variant; surfaces as
  /// ` | null` in TS. The single canonical representation for nullability —
  /// neither `SchemaProperty` nor `Union` carry a separate `nullable` flag.
  Nullable(Box<SchemaType>),
}

/// IR-side discriminator carrier. `property_name` is the OpenAPI
/// `discriminator.propertyName`. `mapping` is a pre-resolved
/// wire-value → bare schema-name map: OpenAPI mapping values may be a
/// full `#/components/schemas/X` ref or a bare name, but both shapes
/// are normalized to bare names at IR-build time so the semantic pass
/// can match against `SchemaType::Ref` payloads with a single
/// `mapping.iter().find(...)` lookup. Empty when the source spec omits
/// `mapping` — the fallback `schema_name.to_ascii_lowercase()` literal
/// then applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Discriminator {
  pub(crate) property_name: Box<str>,
  pub(crate) mapping: BTreeMap<Box<str>, Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SchemaScalar {
  String,
  Number,
  Boolean,
}

pub(crate) fn collect_type_references<'ir>(ty: &'ir SchemaType, imports: &mut BTreeSet<&'ir str>) {
  walk_refs(ty, imports);
}

fn walk_refs<'ir>(ty: &'ir SchemaType, refs: &mut BTreeSet<&'ir str>) {
  match ty {
    SchemaType::Any | SchemaType::Scalar(_) | SchemaType::StringLiterals { .. } => {}
    SchemaType::Array(items) | SchemaType::Map(items) | SchemaType::Nullable(items) => {
      walk_refs(items, refs);
    }
    SchemaType::Ref(name) => {
      refs.insert(name.as_ref());
    }
    SchemaType::Union { members, .. } | SchemaType::Intersection(members) => {
      for member in members {
        walk_refs(member, refs);
      }
    }
    SchemaType::InlineObject { properties } => {
      for property in properties {
        walk_refs(&property.ty, refs);
      }
    }
  }
}
