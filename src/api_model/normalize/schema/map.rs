//! `additionalProperties` lowering into `Record<string, T>`.

use crate::api_model::schema::SchemaType;
use crate::error::Diagnostic;
use crate::parse::openapi_model::{AdditionalProperties, Schema};

use super::super::{SchemaWalk, bail_unsupported_rule};
use super::normalize_schema;

/// Lowers a schema whose `additionalProperties` constrains emission into
/// [`SchemaType::Map`].
///
/// The supported shape is `additionalProperties` alone. Combining it with
/// `properties`, `required`, `$ref`, a composition keyword or a non-object
/// `type` fails, naming the rule it broke.
pub(super) fn normalize_additional_properties(
  schema: &Schema,
  additional: &AdditionalProperties,
  walk: SchemaWalk<'_>,
) -> Result<SchemaType, Diagnostic> {
  if super::has_supported_composition(schema) {
    bail_unsupported_rule!(
      walk.reporter(),
      "{} must not combine additionalProperties with composition keywords.",
      walk.here()
    );
  }
  if schema.properties.is_some() || !schema.required.is_empty() {
    bail_unsupported_rule!(
      walk.reporter(),
      "{} combines additionalProperties with named object properties, which remains outside the supported subset.",
      walk.here()
    );
  }
  if schema.ref_.is_some() {
    bail_unsupported_rule!(
      walk.reporter(),
      "{} must not combine additionalProperties with $ref.",
      walk.here()
    );
  }
  if let Some(declared) = &schema.type_
    && declared != "object"
  {
    bail_unsupported_rule!(
      walk.reporter(),
      "{} uses additionalProperties with non-object type {declared}.",
      walk.here()
    );
  }

  let AdditionalProperties::Schema(values) = additional else {
    bail_unsupported_rule!(
      walk.reporter(),
      "{} must define additionalProperties as a schema object.",
      walk.here()
    );
  };

  Ok(SchemaType::Map(Box::new(normalize_schema(
    values,
    walk.additional_properties(),
  )?)))
}
