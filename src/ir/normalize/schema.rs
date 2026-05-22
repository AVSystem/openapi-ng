use std::collections::BTreeMap;

use crate::error::{Context, Diagnostic, DiagnosticCode, Reporter};
use crate::ir::canonical::ModelSymbol;
use crate::ir::schema::{Discriminator, SchemaProperty, SchemaScalar, SchemaType};
use crate::parse::openapi_model::{AdditionalProperties, Schema};

use super::{MAX_NORMALIZE_DEPTH, check_unsupported_not, unsupported};

pub(super) fn normalize_schemas(
  schemas: &BTreeMap<String, Schema>,
  reporter: &mut Reporter<'_>,
) -> Result<Vec<ModelSymbol>, Diagnostic> {
  let mut normalized = Vec::with_capacity(schemas.len());

  for (name, schema) in schemas {
    normalized.push(normalize_named_schema(name, schema, reporter)?);
  }
  // `normalize_named_schema` starts each top-level schema walk at depth
  // 0 (see calls below); the recursive helpers carry the counter down so
  // a pathological spec is rejected by MAX_NORMALIZE_DEPTH before
  // overflowing the thread stack.

  // Discriminator narrowing happens in `normalize::semantic::finalize`
  // (the final step of `normalize_api_model`), not here. This file is a
  // pure "OpenAPI schema → canonical" stage; the discriminator patch is
  // emit-driven so it runs after operation lowering.

  Ok(normalized)
}

fn normalize_named_schema(
  schema_name: &str,
  schema: &Schema,
  reporter: &mut Reporter<'_>,
) -> Result<ModelSymbol, Diagnostic> {
  let context = Context::Schema(schema_name);
  check_unsupported_not(schema, &context, reporter)?;

  if let Some(values) = &schema.enum_ {
    validate_string_enum_type(schema, &context, reporter)?;
    return Ok(ModelSymbol {
      name: schema_name.into(),
      description: schema.description.clone(),
      deprecated: schema.deprecated,
      body: SchemaType::StringLiterals {
        values: normalize_string_enum(values, &context, reporter)?,
      },
    });
  }

  if schema.type_.as_deref() == Some("object")
    && schema.ref_.is_none()
    && !has_supported_composition(schema)
    && !is_additional_properties_constraint(schema)
    && !is_any_type_schema(schema)
  {
    return Ok(ModelSymbol {
      name: schema_name.into(),
      description: schema.description.clone(),
      deprecated: schema.deprecated,
      body: SchemaType::InlineObject {
        properties: normalize_object_properties(schema, schema_name, 0, reporter)?,
      },
    });
  }

  Ok(ModelSymbol {
    name: schema_name.into(),
    description: schema.description.clone(),
    deprecated: schema.deprecated,
    body: normalize_schema(schema, &context, 0, reporter)?,
  })
}

fn normalize_object_properties(
  schema: &Schema,
  schema_name: &str,
  depth: u16,
  reporter: &mut Reporter<'_>,
) -> Result<Vec<SchemaProperty>, Diagnostic> {
  normalize_properties(schema, &Context::Schema(schema_name), depth, reporter)
}

pub(super) fn normalize_properties(
  schema: &Schema,
  context: &Context<'_>,
  depth: u16,
  reporter: &mut Reporter<'_>,
) -> Result<Vec<SchemaProperty>, Diagnostic> {
  check_unsupported_not(schema, context, reporter)?;

  if is_additional_properties_constraint(schema) {
    return Err(unsupported(
      format!(
        "{} uses additionalProperties after composition, which remains outside the supported subset.",
        context.render()
      ),
      reporter,
      false,
    ));
  }

  let Some(properties) = &schema.properties else {
    return Ok(Vec::new());
  };

  let required: std::collections::HashSet<&str> =
    schema.required.iter().map(String::as_str).collect();
  let mut normalized = Vec::with_capacity(properties.len());

  for (name, property_schema) in properties {
    let required_flag = required.contains(name.as_str());
    let prop_context = Context::Property {
      parent: context,
      name,
    };
    let base_ty = normalize_schema_raw(property_schema, &prop_context, depth, reporter)?;
    let ty = apply_nullable_flag(base_ty, property_schema.nullable.unwrap_or(false));
    normalized.push(SchemaProperty {
      name: name.as_str().into(),
      required: required_flag,
      ty,
      description: property_schema.description.clone(),
      deprecated: property_schema.deprecated,
    });
  }

  Ok(normalized)
}

pub(super) fn normalize_schema(
  schema: &Schema,
  context: &Context<'_>,
  depth: u16,
  reporter: &mut Reporter<'_>,
) -> Result<SchemaType, Diagnostic> {
  let base = normalize_schema_raw(schema, context, depth, reporter)?;
  Ok(apply_nullable_flag(base, schema.nullable.unwrap_or(false)))
}

fn normalize_schema_raw(
  schema: &Schema,
  context: &Context<'_>,
  depth: u16,
  reporter: &mut Reporter<'_>,
) -> Result<SchemaType, Diagnostic> {
  // Single chokepoint for the recursion guard: every schema-shape branch
  // below either bottoms out (scalar / ref / enum / Any) or routes back
  // through one of the recursive helpers, all of which forward
  // `depth + 1`. Checking here keeps the bound enforceable from one
  // place rather than scattered across each recursive call.
  if depth >= MAX_NORMALIZE_DEPTH {
    return Err(unsupported(
      format!(
        "{} nesting exceeds {MAX_NORMALIZE_DEPTH} levels (likely cyclic or pathological spec).",
        context.render()
      ),
      reporter,
      false,
    ));
  }

  // OpenAPI `format` hints (e.g. `uuid`, `date-time`, `int32`) carry
  // semantic information that the current IR does not preserve — the
  // generator emits the base type without format-specific narrowing.
  // Surface every occurrence as a warning so spec authors see what's
  // being dropped instead of the field being silently ignored.
  if let Some(format) = &schema.format {
    reporter.warning(
      DiagnosticCode::UnsupportedSemantic,
      Some("format-dropped"),
      format!(
        "{} declares format '{format}', which is currently dropped — the generator emits the base type without format-specific narrowing.",
        context.render()
      ),
    );
  }

  check_unsupported_not(schema, context, reporter)?;

  if is_additional_properties_constraint(schema) {
    // Safe to unwrap-via-match: is_additional_properties_constraint is true
    // only when `additional_properties` is `Some(Schema)` or `Some(Boolean(true))`.
    if let Some(ap) = &schema.additional_properties {
      return normalize_additional_properties(schema, ap, context, depth, reporter);
    }
  }

  if let Some(composition) = normalize_composition(schema, context, depth, reporter)? {
    return Ok(composition);
  }

  if is_any_type_schema(schema) {
    return Ok(SchemaType::Any);
  }

  if let Some(reference) = &schema.ref_ {
    return Ok(SchemaType::Ref(normalize_reference(
      reference, context, reporter,
    )?));
  }

  if let Some(values) = &schema.enum_ {
    validate_string_enum_type(schema, context, reporter)?;
    return Ok(SchemaType::StringLiterals {
      values: normalize_string_enum(values, context, reporter)?,
    });
  }

  match schema.type_.as_deref() {
    Some("string") => Ok(SchemaType::Scalar(SchemaScalar::String)),
    Some("integer" | "number") => Ok(SchemaType::Scalar(SchemaScalar::Number)),
    Some("boolean") => Ok(SchemaType::Scalar(SchemaScalar::Boolean)),
    Some("array") => {
      let items = schema.items.as_deref().ok_or_else(|| {
        unsupported(
          format!("{} array schemas must define items.", context.render()),
          reporter,
          true,
        )
      })?;
      Ok(SchemaType::Array(Box::new(normalize_schema(
        items,
        context,
        depth + 1,
        reporter,
      )?)))
    }
    Some("object") => Ok(SchemaType::InlineObject {
      properties: normalize_properties(schema, context, depth + 1, reporter)?,
    }),
    Some(other) => Err(unsupported(
      format!("{} uses unsupported type {other}.", context.render()),
      reporter,
      true,
    )),
    None => Err(unsupported(
      format!(
        "{} must define a supported type, $ref, or supported composition.",
        context.render()
      ),
      reporter,
      true,
    )),
  }
}

fn normalize_additional_properties(
  schema: &Schema,
  ap: &AdditionalProperties,
  context: &Context<'_>,
  depth: u16,
  reporter: &mut Reporter<'_>,
) -> Result<SchemaType, Diagnostic> {
  if has_supported_composition(schema) {
    return Err(unsupported(
      format!(
        "{} must not combine additionalProperties with composition keywords.",
        context.render()
      ),
      reporter,
      false,
    ));
  }

  if schema.properties.is_some() || !schema.required.is_empty() {
    return Err(unsupported(
      format!(
        "{} combines additionalProperties with named object properties, which remains outside the supported subset.",
        context.render()
      ),
      reporter,
      false,
    ));
  }

  if schema.ref_.is_some() {
    return Err(unsupported(
      format!(
        "{} must not combine additionalProperties with $ref.",
        context.render()
      ),
      reporter,
      false,
    ));
  }

  if let Some(type_) = &schema.type_
    && type_ != "object"
  {
    return Err(unsupported(
      format!(
        "{} uses additionalProperties with non-object type {type_}.",
        context.render()
      ),
      reporter,
      false,
    ));
  }

  let ap_schema = match ap {
    AdditionalProperties::Schema(s) => s.as_ref(),
    AdditionalProperties::Boolean(_) => {
      return Err(unsupported(
        format!(
          "{} must define additionalProperties as a schema object.",
          context.render()
        ),
        reporter,
        false,
      ));
    }
  };

  let ap_context = Context::AdditionalProperties { parent: context };
  Ok(SchemaType::Map(Box::new(normalize_schema(
    ap_schema,
    &ap_context,
    depth + 1,
    reporter,
  )?)))
}

fn normalize_composition(
  schema: &Schema,
  context: &Context<'_>,
  depth: u16,
  reporter: &mut Reporter<'_>,
) -> Result<Option<SchemaType>, Diagnostic> {
  let composition_count = [
    schema.one_of.is_some(),
    schema.any_of.is_some(),
    schema.all_of.is_some(),
  ]
  .into_iter()
  .filter(|&present| present)
  .count();

  if composition_count == 0 {
    return Ok(None);
  }

  if composition_count > 1 {
    return Err(unsupported(
      format!(
        "{} must not combine multiple composition keywords.",
        context.render()
      ),
      reporter,
      true,
    ));
  }

  if let Some(entries) = &schema.one_of {
    return normalize_composition_entries(
      entries,
      context,
      depth,
      reporter,
      CompositionKind::Union,
      schema.discriminator.as_ref(),
    )
    .map(Some);
  }

  if let Some(entries) = &schema.any_of {
    return normalize_composition_entries(
      entries,
      context,
      depth,
      reporter,
      CompositionKind::Union,
      None,
    )
    .map(Some);
  }

  let Some(entries) = schema.all_of.as_deref() else {
    // Defensive guard: today's invariant is that `composition_count > 0`
    // with one_of/any_of None implies all_of is Some, since
    // `composition_count` is the population count of exactly those three
    // booleans. A future refactor that adds a fourth composition keyword
    // without updating this proof would otherwise crash the host Node
    // process via `unreachable!`. Surfacing a typed error keeps such a
    // regression user-visible.
    return Err(unsupported(
      format!(
        "{} internal: composition counted {composition_count} keywords but none matched.",
        context.render()
      ),
      reporter,
      false,
    ));
  };
  normalize_composition_entries(
    entries,
    context,
    depth,
    reporter,
    CompositionKind::Intersection,
    None,
  )
  .map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompositionKind {
  Union,
  Intersection,
}

fn normalize_composition_entries(
  entries: &[Schema],
  context: &Context<'_>,
  depth: u16,
  reporter: &mut Reporter<'_>,
  kind: CompositionKind,
  discriminator: Option<&crate::parse::openapi_model::Discriminator>,
) -> Result<SchemaType, Diagnostic> {
  if entries.is_empty() {
    return Err(unsupported(
      format!(
        "{} composition must contain at least one member.",
        context.render()
      ),
      reporter,
      true,
    ));
  }

  let mut normalized = Vec::with_capacity(entries.len());
  for (index, entry) in entries.iter().enumerate() {
    let member_context = Context::CompositionMember {
      parent: context,
      index: index + 1,
    };
    normalized.push(normalize_schema(
      entry,
      &member_context,
      depth + 1,
      reporter,
    )?);
  }

  if normalized.len() == 1 {
    return Ok(normalized.remove(0));
  }

  Ok(match kind {
    CompositionKind::Union => {
      let discriminator = match discriminator.filter(|d| !d.property_name.is_empty()) {
        None => None,
        Some(d) => Some(resolve_discriminator(d, context, reporter)?),
      };
      SchemaType::Union {
        members: normalized,
        discriminator,
      }
    }
    CompositionKind::Intersection => SchemaType::Intersection(normalized),
  })
}

/// Build the IR-side `Discriminator` from the parse-stage one, resolving
/// every `mapping` value to a bare schema name.
///
/// Per the OpenAPI spec, a `discriminator.mapping` value is either a bare
/// schema name (`Cat`) or a full `$ref` (`#/components/schemas/Cat`). A
/// value that contains a `/` is treated as ref-shaped and routed through
/// `normalize_reference`, which is the single source of truth for `$ref`
/// validation across this crate — that gives external refs
/// (`http://...`), sibling-file refs (`./other.yaml#/...`), and other
/// unsupported shapes the same `E_UNSUPPORTED_SEMANTIC` diagnostic the
/// rest of the pipeline emits, instead of silently passing the literal
/// through where it would never match a union member.
///
/// Bare names (no `/`) are accepted as-is so the common spec idiom keeps
/// working without forcing authors to write the full ref form.
fn resolve_discriminator(
  parsed: &crate::parse::openapi_model::Discriminator,
  context: &Context<'_>,
  reporter: &Reporter<'_>,
) -> Result<Discriminator, Diagnostic> {
  let mut mapping = std::collections::BTreeMap::new();
  for (wire_value, schema_ref) in &parsed.mapping {
    let resolved = if schema_ref.contains('/') {
      normalize_reference(schema_ref, context, reporter)?
    } else {
      schema_ref.as_str().into()
    };
    mapping.insert(wire_value.as_str().into(), resolved);
  }
  Ok(Discriminator {
    property_name: parsed.property_name.as_str().into(),
    mapping,
  })
}

/// Wrap `base` in `SchemaType::Nullable` when the OpenAPI `nullable: true`
/// flag is set. Idempotent over already-`Nullable` types.
fn apply_nullable_flag(base: SchemaType, nullable: bool) -> SchemaType {
  if !nullable || matches!(base, SchemaType::Nullable(_)) {
    return base;
  }
  SchemaType::Nullable(Box::new(base))
}

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

/// True when `additionalProperties` actually constrains emission (a schema
/// object or `Boolean(true)`). `Boolean(false)` is treated as a no-op —
/// OpenAPI semantics are "no extras beyond the declared `properties`",
/// which is structurally the same as not setting the field at all for our
/// emit purposes (TS interfaces with declared properties don't accept
/// arbitrary extras by default).
const fn is_additional_properties_constraint(schema: &Schema) -> bool {
  matches!(
    schema.additional_properties,
    Some(AdditionalProperties::Schema(_) | AdditionalProperties::Boolean(true))
  )
}

fn normalize_reference(
  reference: &str,
  context: &Context<'_>,
  reporter: &Reporter<'_>,
) -> Result<Box<str>, Diagnostic> {
  let name = reference
    .strip_prefix("#/components/schemas/")
    .ok_or_else(|| {
      unsupported(
        format!(
          "{} uses unsupported reference {reference}.",
          context.render()
        ),
        reporter,
        true,
      )
    })?;

  // Reject `$ref: '#/components/schemas/'` (trailing slash, empty target
  // name) before the empty `Box<str>` flows downstream — every downstream
  // consumer of a ref name (naming helpers, emit-side identifier checks)
  // assumes a non-empty token, so failing here keeps the host process
  // alive with a clean diagnostic instead of producing nameless output.
  if name.is_empty() {
    return Err(unsupported(
      format!(
        "{} $ref target name is empty (reference {reference}).",
        context.render()
      ),
      reporter,
      true,
    ));
  }

  Ok(Box::from(name))
}

fn normalize_string_enum(
  values: &[serde_json::Value],
  context: &Context<'_>,
  reporter: &Reporter<'_>,
) -> Result<Vec<String>, Diagnostic> {
  let mut result = Vec::with_capacity(values.len());
  for entry in values {
    let Some(s) = entry.as_str() else {
      return Err(unsupported(
        format!("{} enum must contain only strings.", context.render()),
        reporter,
        true,
      ));
    };

    if s.contains('\u{0000}') {
      return Err(unsupported(
        format!(
          "{} enum values must not contain null bytes.",
          context.render()
        ),
        reporter,
        true,
      ));
    }

    result.push(s.to_string());
  }

  Ok(result)
}

fn validate_string_enum_type(
  schema: &Schema,
  context: &Context<'_>,
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  match schema.type_.as_deref() {
    Some("string") | None => Ok(()),
    Some(other) => Err(unsupported(
      format!(
        "{} enum is supported only for string schemas, found type {other}.",
        context.render()
      ),
      reporter,
      true,
    )),
  }
}

#[cfg(test)]
mod proptests {
  use std::rc::Rc;

  use proptest::prelude::*;

  use super::normalize_named_schema;
  use crate::error::{DiagnosticCode, Reporter};
  use crate::parse::openapi_model::Schema;

  fn arb_schema(max_depth: u32) -> impl Strategy<Value = Schema> {
    let leaf = Just(Schema::default_string());
    leaf.prop_recursive(max_depth, 32, 4, |inner| {
      prop_oneof![
        inner.clone().prop_map(Schema::wrap_array),
        proptest::collection::vec(inner.clone(), 0..3).prop_map(Schema::wrap_one_of),
        inner.prop_map(Schema::wrap_nullable),
      ]
    })
  }

  proptest! {
    #![proptest_config(ProptestConfig {
      cases: 128,
      ..ProptestConfig::default()
    })]

    #[test]
    fn normalize_named_schema_never_panics(schema in arb_schema(40)) {
      let mut warnings = Vec::new();
      let path: Rc<str> = Rc::from("test");
      let mut reporter = Reporter::new(path, &mut warnings);
      let result = normalize_named_schema("Root", &schema, &mut reporter);

      if let Err(diag) = result {
        prop_assert!(
          matches!(
            diag.code,
            DiagnosticCode::UnsupportedSemantic | DiagnosticCode::PolicyViolation,
          ),
          "unexpected diagnostic code: {:?}", diag.code,
        );
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::normalize_named_schema;
  use crate::error::Reporter;
  use crate::parse::openapi_model::Schema;

  #[test]
  fn depth_exceeded_diagnostic_includes_breadcrumb_chain() {
    // Build a 40-level-deep schema by wrapping in array; MAX_NORMALIZE_DEPTH is 32.
    let mut schema = Schema::default_string();
    for _ in 0..40 {
      schema = Schema::wrap_array(schema);
    }

    let mut warnings = Vec::new();
    let path: Rc<str> = Rc::from("test");
    let mut reporter = Reporter::new(path, &mut warnings);
    let err = normalize_named_schema("Root", &schema, &mut reporter)
      .expect_err("should fail with depth exceeded");

    assert!(
      err.message.contains("32"),
      "expected depth limit in message: {}",
      err.message,
    );
    assert!(
      err.message.contains("Root"),
      "expected root breadcrumb in message: {}",
      err.message,
    );
  }
}
