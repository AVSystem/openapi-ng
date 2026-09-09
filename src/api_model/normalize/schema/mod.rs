//! OpenAPI schema to canonical `SchemaType`, entered through
//! [`normalize_schemas`], [`normalize_schema`] and
//! [`normalize_properties`].

mod composition;
mod enums;
mod map;
mod reference;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet};

use crate::api_model::canonical::ModelSymbol;
use crate::api_model::schema::{SchemaProperty, SchemaScalar, SchemaType};
use crate::error::{Context, Diagnostic, Reporter};
use crate::parse::openapi_model::{AdditionalProperties, Schema};

use super::{SchemaWalk, bail_unsupported, bail_unsupported_rule, check_unsupported_not};
use composition::normalize_composition;
use enums::{normalize_string_enum, validate_string_enum_type};
use map::normalize_additional_properties;
use reference::normalize_reference;

pub(super) fn normalize_schemas(
  schemas: &BTreeMap<String, Schema>,
  reporter: &Reporter,
) -> Result<Vec<ModelSymbol>, Diagnostic> {
  schemas
    .iter()
    .map(|(name, schema)| normalize_named_schema(name, schema, reporter))
    .collect()
}

fn normalize_named_schema(
  name: &str,
  schema: &Schema,
  reporter: &Reporter,
) -> Result<ModelSymbol, Diagnostic> {
  let walk = SchemaWalk::root(Context::Schema(name), reporter);
  check_unsupported_not(schema, walk)?;

  let body = if let Some(values) = &schema.enum_ {
    validate_string_enum_type(schema, walk)?;
    SchemaType::StringLiterals {
      values: normalize_string_enum(values, walk)?,
    }
  } else if declares_a_plain_object(schema) {
    SchemaType::InlineObject {
      properties: normalize_properties(schema, walk)?,
    }
  } else {
    normalize_schema(schema, walk)?
  };

  Ok(ModelSymbol {
    name: name.into(),
    description: schema.description.clone(),
    deprecated: schema.deprecated,
    body,
  })
}

/// True when the schema is an object declared by its `properties` alone, so
/// it emits as `export interface` rather than a type alias.
fn declares_a_plain_object(schema: &Schema) -> bool {
  schema.type_.as_deref() == Some("object")
    && schema.ref_.is_none()
    && !has_supported_composition(schema)
    && !is_additional_properties_constraint(schema)
    && !is_any_type_schema(schema)
}

pub(super) fn normalize_properties(
  schema: &Schema,
  walk: SchemaWalk<'_>,
) -> Result<Vec<SchemaProperty>, Diagnostic> {
  check_unsupported_not(schema, walk)?;

  if is_additional_properties_constraint(schema) {
    bail_unsupported_rule!(
      walk.reporter(),
      "{} uses additionalProperties after composition, which remains outside the supported subset.",
      walk.here()
    );
  }

  let Some(properties) = &schema.properties else {
    return Ok(Vec::new());
  };

  let required: HashSet<&str> = schema.required.iter().map(String::as_str).collect();

  properties
    .iter()
    .map(|(name, property)| {
      let base = normalize_type(property, walk.property(name))?;
      Ok(SchemaProperty {
        name: name.as_str().into(),
        required: required.contains(name.as_str()),
        schema: apply_nullable_flag(base, property.nullable.unwrap_or(false)),
        description: property.description.clone(),
        deprecated: property.deprecated,
      })
    })
    .collect()
}

/// Normalizes `schema` at `walk`, folding its own `nullable: true` into the
/// result.
pub(super) fn normalize_schema(
  schema: &Schema,
  walk: SchemaWalk<'_>,
) -> Result<SchemaType, Diagnostic> {
  let base = normalize_type(schema, walk)?;
  Ok(apply_nullable_flag(base, schema.nullable.unwrap_or(false)))
}

/// Dispatches on the schema's shape; the caller folds in `nullable`. The
/// single chokepoint for the depth guard.
fn normalize_type(schema: &Schema, walk: SchemaWalk<'_>) -> Result<SchemaType, Diagnostic> {
  walk.check_depth()?;
  warn_dropped_format(schema, walk);
  check_unsupported_not(schema, walk)?;

  if is_additional_properties_constraint(schema)
    && let Some(additional) = &schema.additional_properties
  {
    return normalize_additional_properties(schema, additional, walk);
  }

  if let Some(composition) = normalize_composition(schema, walk)? {
    return Ok(composition);
  }

  if is_any_type_schema(schema) {
    return Ok(SchemaType::Any);
  }

  if let Some(reference) = &schema.ref_ {
    return Ok(SchemaType::Ref(normalize_reference(reference, walk)?));
  }

  if let Some(values) = &schema.enum_ {
    validate_string_enum_type(schema, walk)?;
    return Ok(SchemaType::StringLiterals {
      values: normalize_string_enum(values, walk)?,
    });
  }

  normalize_declared_type(schema, walk)
}

fn normalize_declared_type(
  schema: &Schema,
  walk: SchemaWalk<'_>,
) -> Result<SchemaType, Diagnostic> {
  match schema.type_.as_deref() {
    Some("string") => Ok(SchemaType::Scalar(SchemaScalar::String)),
    Some("integer" | "number") => Ok(SchemaType::Scalar(SchemaScalar::Number)),
    Some("boolean") => Ok(SchemaType::Scalar(SchemaScalar::Boolean)),
    Some("array") => {
      let Some(items) = schema.items.as_deref() else {
        bail_unsupported!(
          walk.reporter(),
          "{} array schemas must define items.",
          walk.here()
        );
      };
      Ok(SchemaType::Array(Box::new(normalize_schema(
        items,
        walk.item(),
      )?)))
    }
    Some("object") => Ok(SchemaType::InlineObject {
      properties: normalize_properties(schema, walk.item())?,
    }),
    Some(other) => bail_unsupported!(
      walk.reporter(),
      "{} uses unsupported type {other}.",
      walk.here()
    ),
    None => bail_unsupported!(
      walk.reporter(),
      "{} must define a supported type, $ref, or supported composition.",
      walk.here()
    ),
  }
}

/// Reports every `format` the IR drops.
fn warn_dropped_format(schema: &Schema, walk: SchemaWalk<'_>) {
  if let Some(format) = &schema.format {
    walk.reporter().warning(
      crate::error::DiagnosticCode::UnsupportedSemantic,
      Some("format-dropped"),
      format!(
        "{} declares format '{format}', which is currently dropped — the generator emits the base type without format-specific narrowing.",
        walk.here()
      ),
    );
  }
}

/// Wraps `base` in [`SchemaType::Nullable`] when `nullable` is set.
/// Idempotent over an already-nullable type.
fn apply_nullable_flag(base: SchemaType, nullable: bool) -> SchemaType {
  if !nullable || matches!(base, SchemaType::Nullable(_)) {
    return base;
  }
  SchemaType::Nullable(Box::new(base))
}

/// True for a schema with no constraints at all, which renders as
/// `unknown`.
const fn is_any_type_schema(schema: &Schema) -> bool {
  schema.type_.is_none()
    && schema.ref_.is_none()
    && schema.enum_.is_none()
    && !has_supported_composition(schema)
    && schema.additional_properties.is_none()
}

const fn has_supported_composition(schema: &Schema) -> bool {
  schema.one_of.is_some() || schema.any_of.is_some() || schema.all_of.is_some()
}

/// True when `additionalProperties` constrains emission — a schema object,
/// or literal `true`.
///
/// `additionalProperties: false` is a no-op here: "no members beyond
/// `properties`" is what a TypeScript interface already means.
const fn is_additional_properties_constraint(schema: &Schema) -> bool {
  matches!(
    schema.additional_properties,
    Some(AdditionalProperties::Schema(_) | AdditionalProperties::Boolean(true))
  )
}
