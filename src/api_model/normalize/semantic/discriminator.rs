//! Narrows each discriminated union member's discriminator property to the single literal that
//! selects it.

use std::collections::BTreeMap;

use crate::api_model::canonical::ModelSymbol;
use crate::api_model::schema::{Discriminator, SchemaProperty, SchemaScalar, SchemaType};
use crate::error::{Diagnostic, Reporter, bail_policy};
use crate::subcode;

/// Member schema name, then property name, to the literal it narrows to.
type Narrowings = BTreeMap<Box<str>, BTreeMap<Box<str>, Box<str>>>;

pub(super) fn narrow(symbols: &mut [ModelSymbol], reporter: &Reporter) -> Result<(), Diagnostic> {
  let narrowings = collect_narrowings(symbols);
  if narrowings.is_empty() {
    return Ok(());
  }
  validate_narrowings(symbols, &narrowings, reporter)?;
  apply_narrowings(symbols, &narrowings);
  Ok(())
}

#[must_use]
fn collect_narrowings(symbols: &[ModelSymbol]) -> Narrowings {
  symbols
    .iter()
    .filter_map(|symbol| match &symbol.body {
      SchemaType::Union {
        members,
        discriminator: Some(discriminator),
        ..
      } => Some((members, discriminator)),
      _ => None,
    })
    .flat_map(|(members, discriminator)| {
      members.iter().filter_map(move |member| match member {
        SchemaType::Ref(schema_name) => Some((schema_name, discriminator)),
        _ => None,
      })
    })
    .fold(
      Narrowings::new(),
      |mut narrowings, (schema_name, discriminator)| {
        narrowings.entry(schema_name.clone()).or_default().insert(
          discriminator.property_name.clone(),
          wire_value(discriminator, schema_name),
        );
        narrowings
      },
    )
}

/// The wire value a `mapping` entry gives this member, or its lowercased schema name.
#[must_use]
fn wire_value(discriminator: &Discriminator, schema_name: &str) -> Box<str> {
  discriminator
    .mapping
    .iter()
    .find(|(_, target)| target.as_ref() == schema_name)
    .map_or_else(
      || schema_name.to_ascii_lowercase().into_boxed_str(),
      |(value, _)| value.clone(),
    )
}

/// Runs before any mutation, so a rejected spec leaves the model untouched.
fn validate_narrowings(
  symbols: &[ModelSymbol],
  narrowings: &Narrowings,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  let by_name: BTreeMap<&str, &SchemaType> = symbols
    .iter()
    .map(|symbol| (symbol.name.as_ref(), &symbol.body))
    .collect();

  symbols
    .iter()
    .filter_map(|symbol| narrowings.get(&symbol.name).map(|names| (symbol, names)))
    .flat_map(|(symbol, names)| names.keys().map(move |name| (symbol, name)))
    .try_for_each(|(symbol, property_name)| {
      validate_discriminator_property(symbol, property_name, &by_name, reporter)
    })
}

fn validate_discriminator_property(
  symbol: &ModelSymbol,
  property_name: &str,
  by_name: &BTreeMap<&str, &SchemaType>,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  let Some(property) = find_property(&symbol.body, property_name, by_name) else {
    bail_policy!(
      reporter,
      subcode::MISSING_DISCRIMINATOR_PROPERTY,
      "Failed to validate spec: oneOf member '{}' does not declare the discriminator property '{}'. Add the property to the member schema (typically as `type: string`) or remove the discriminator.",
      symbol.name,
      property_name
    );
  };

  if !is_string_discriminator_shape(&property.schema) {
    bail_policy!(
      reporter,
      subcode::DISCRIMINATOR_PROPERTY_MUST_BE_STRING,
      "Failed to validate spec: oneOf member '{}' declares discriminator property '{}' with a non-string type. Discriminator properties must be `type: string` (optionally with an enum); change the property type or remove the discriminator.",
      symbol.name,
      property_name
    );
  }
  Ok(())
}

/// Only inline objects are mutated; a `Ref` member is narrowed when this reaches the symbol it
/// names.
fn apply_narrowings(symbols: &mut [ModelSymbol], narrowings: &Narrowings) {
  symbols.iter_mut().for_each(|symbol| {
    let Some(names) = narrowings.get(&symbol.name) else {
      return;
    };
    names.iter().for_each(|(property_name, literal_value)| {
      narrow_property_in_body(&mut symbol.body, property_name, literal_value.as_ref());
    });
  });
}

/// Finds a property by name through the shapes that can carry one: an inline object, an `allOf`
/// part, a `$ref` target, or a nullable wrapper.
#[must_use]
fn find_property<'a>(
  body: &'a SchemaType,
  name: &str,
  by_name: &BTreeMap<&str, &'a SchemaType>,
) -> Option<&'a SchemaProperty> {
  match body {
    SchemaType::InlineObject { properties } => properties
      .iter()
      .find(|property| property.name.as_ref() == name),
    SchemaType::Intersection(parts) => parts
      .iter()
      .find_map(|part| find_property(part, name, by_name)),
    SchemaType::Ref(target) => by_name
      .get(target.as_ref())
      .and_then(|inner| find_property(inner, name, by_name)),
    SchemaType::Nullable(inner) => find_property(inner, name, by_name),
    _ => None,
  }
}

/// True for the property types a discriminator may declare: bare `string` or a string-literal enum.
#[must_use]
const fn is_string_discriminator_shape(schema: &SchemaType) -> bool {
  matches!(
    schema,
    SchemaType::Scalar(SchemaScalar::String) | SchemaType::StringLiterals { .. }
  )
}

/// Narrows the named property to a single literal, reporting whether it was found; one
/// reachable only through a `$ref` keeps its `string` type.
fn narrow_property_in_body(body: &mut SchemaType, name: &str, literal_value: &str) -> bool {
  match body {
    SchemaType::InlineObject { properties } => {
      if let Some(property) = properties
        .iter_mut()
        .find(|property| property.name.as_ref() == name)
      {
        property.schema = SchemaType::StringLiterals {
          values: vec![literal_value.to_owned()],
        };
        return true;
      }
      false
    }
    SchemaType::Intersection(parts) => parts
      .iter_mut()
      .any(|part| narrow_property_in_body(part, name, literal_value)),
    SchemaType::Nullable(inner) => narrow_property_in_body(inner, name, literal_value),
    _ => false,
  }
}
