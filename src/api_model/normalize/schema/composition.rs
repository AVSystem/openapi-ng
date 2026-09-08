//! `oneOf` / `anyOf` / `allOf` lowering, and the discriminator carried by a
//! discriminated `oneOf`.

use std::collections::BTreeMap;

use crate::api_model::schema::{Discriminator, SchemaType};
use crate::error::Diagnostic;
use crate::parse::openapi_model::{self, Schema};

use super::super::{SchemaWalk, bail_unsupported};
use super::{normalize_reference, normalize_schema};

/// Lowers whichever composition keyword `schema` declares, or `None` when it
/// declares none. Rejects a schema that mixes two of them.
pub(super) fn normalize_composition(
  schema: &Schema,
  walk: SchemaWalk<'_>,
) -> Result<Option<SchemaType>, Diagnostic> {
  let declared = [
    schema
      .one_of
      .as_deref()
      .map(|entries| (Kind::Union, entries)),
    schema
      .any_of
      .as_deref()
      .map(|entries| (Kind::Union, entries)),
    schema
      .all_of
      .as_deref()
      .map(|entries| (Kind::Intersection, entries)),
  ];
  let present = declared.iter().flatten().count();

  if present > 1 {
    bail_unsupported!(
      walk.reporter(),
      "{} must not combine multiple composition keywords.",
      walk.here()
    );
  }
  let Some((kind, entries)) = declared.into_iter().flatten().next() else {
    return Ok(None);
  };

  // Only `oneOf` carries a discriminator; `anyOf` and `allOf` ignore it.
  let discriminator = schema
    .discriminator
    .as_ref()
    .filter(|_| schema.one_of.is_some());
  normalize_entries(entries, kind, discriminator, walk).map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
  Union,
  Intersection,
}

fn normalize_entries(
  entries: &[Schema],
  kind: Kind,
  discriminator: Option<&openapi_model::Discriminator>,
  walk: SchemaWalk<'_>,
) -> Result<SchemaType, Diagnostic> {
  if entries.is_empty() {
    bail_unsupported!(
      walk.reporter(),
      "{} composition must contain at least one member.",
      walk.here()
    );
  }

  let mut members = entries
    .iter()
    .enumerate()
    .map(|(index, entry)| normalize_schema(entry, walk.composition_member(index + 1)))
    .collect::<Result<Vec<_>, Diagnostic>>()?;

  // A one-member composition is its member.
  if members.len() == 1 {
    return Ok(members.remove(0));
  }

  Ok(match kind {
    Kind::Union => SchemaType::Union {
      members,
      discriminator: discriminator
        .filter(|declared| !declared.property_name.is_empty())
        .map(|declared| resolve_discriminator(declared, walk))
        .transpose()?,
    },
    Kind::Intersection => SchemaType::Intersection(members),
  })
}

/// Resolves every `mapping` value to a bare schema name.
///
/// A value is either a bare name (`Cat`) or a full ref
/// (`#/components/schemas/Cat`); one containing `/` is resolved through
/// [`normalize_reference`] and fails on the shapes that rejects.
fn resolve_discriminator(
  declared: &openapi_model::Discriminator,
  walk: SchemaWalk<'_>,
) -> Result<Discriminator, Diagnostic> {
  let mapping = declared
    .mapping
    .iter()
    .map(|(wire_value, target)| {
      let resolved = if target.contains('/') {
        normalize_reference(target, walk)?
      } else {
        target.as_str().into()
      };
      Ok((wire_value.as_str().into(), resolved))
    })
    .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;

  Ok(Discriminator {
    property_name: declared.property_name.as_str().into(),
    mapping,
  })
}
