use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SchemaProperty {
  pub(crate) name: Box<str>,
  pub(crate) required: bool,
  pub(crate) ty: SchemaType,
  /// Emitted as JSDoc above the declaration in named interfaces only.
  pub(crate) description: Option<String>,
  /// Emitted as `@deprecated` in named interfaces only.
  pub(crate) deprecated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SchemaType {
  /// Schema with no `type`, `$ref` or composition. Renders as `unknown`.
  Any,
  Scalar(SchemaScalar),
  Array(Box<SchemaType>),
  Map(Box<SchemaType>),
  /// Literal union, `'a' | 'b'`.
  StringLiterals {
    values: Vec<String>,
  },
  Ref(Box<str>),
  /// `oneOf`/`anyOf`. `discriminator` is set only for a discriminated
  /// `oneOf`; `narrow_discriminator_properties` consumes it.
  Union {
    members: Vec<SchemaType>,
    discriminator: Option<Discriminator>,
  },
  Intersection(Vec<SchemaType>),
  InlineObject {
    properties: Vec<SchemaProperty>,
  },
  /// Wraps any other variant; renders as ` | null`. The only
  /// representation of nullability in the IR.
  Nullable(Box<SchemaType>),
}

/// `mapping` holds wire value → bare schema name; `#/components/schemas/X`
/// refs are reduced to bare names when the IR is built. Empty when the
/// spec omits `mapping`, and `schema_name.to_ascii_lowercase()` applies.
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
