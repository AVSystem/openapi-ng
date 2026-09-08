//! The semantic pass that runs once schema and operation lowering are
//! done: schema sorting, discriminator narrowing and `$ref` validation.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Diagnostic, DiagnosticCode, Reporter, bail, bail_policy};
use crate::ir::canonical::{ApiModel, BodyContent, ModelSymbol, ResponseContent};
use crate::ir::schema::{SchemaProperty, SchemaScalar, SchemaType, collect_type_references};

/// Sorts the schemas by name, narrows the discriminator properties, and
/// fails on a `$ref` that resolves to no declared schema. Mutates `model`
/// in place.
pub(super) fn finalize(model: &mut ApiModel, reporter: &Reporter) -> Result<(), Diagnostic> {
  model
    .schemas
    .sort_by(|left, right| left.name.cmp(&right.name));

  narrow_discriminator_properties(&mut model.schemas, reporter)?;
  validate_references(model, reporter)
}

/// Narrows each discriminated union member's discriminator property to a
/// single-value string literal, which is what lets the TypeScript compiler
/// narrow the union to a concrete member.
///
/// Fails with `missing-discriminator-property` when a member does not
/// declare the property, and `discriminator-property-must-be-string` when
/// it declares it with a non-string type.
fn narrow_discriminator_properties(
  symbols: &mut [ModelSymbol],
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  // Per member: the literal value to assign to each of its discriminator
  // properties.
  let mut narrowings: BTreeMap<Box<str>, BTreeMap<Box<str>, Box<str>>> = BTreeMap::new();
  for symbol in symbols.iter() {
    if let SchemaType::Union {
      members,
      discriminator: Some(discriminator),
      ..
    } = &symbol.body
    {
      for member in members {
        if let SchemaType::Ref(schema_name) = member {
          // A `discriminator.mapping` entry whose target is this member
          // supplies the wire value; without one it is the lowercased
          // schema name.
          let literal_value: Box<str> = discriminator
            .mapping
            .iter()
            .find(|(_, target)| target.as_ref() == schema_name.as_ref())
            .map_or_else(
              || schema_name.to_ascii_lowercase().into_boxed_str(),
              |(wire_value, _)| wire_value.clone(),
            );
          narrowings
            .entry(schema_name.clone())
            .or_default()
            .insert(discriminator.property_name.clone(), literal_value);
        }
      }
    }
  }

  if narrowings.is_empty() {
    return Ok(());
  }

  // Validate every member before mutating any of them, so a rejected
  // spec leaves the model untouched.
  let by_name: BTreeMap<&str, &SchemaType> = symbols
    .iter()
    .map(|symbol| (symbol.name.as_ref(), &symbol.body))
    .collect();

  for symbol in symbols.iter() {
    let Some(props) = narrowings.get(&symbol.name) else {
      continue;
    };
    for property_name in props.keys() {
      let Some(property) = find_property(&symbol.body, property_name, &by_name) else {
        bail_policy!(
          reporter,
          "missing-discriminator-property",
          "Failed to validate spec: oneOf member '{}' does not declare the discriminator property '{}'. Add the property to the member schema (typically as `type: string`) or remove the discriminator.",
          symbol.name,
          property_name
        );
      };
      if !is_string_discriminator_shape(&property.ty) {
        bail_policy!(
          reporter,
          "discriminator-property-must-be-string",
          "Failed to validate spec: oneOf member '{}' declares discriminator property '{}' with a non-string type. Discriminator properties must be `type: string` (optionally with an enum); change the property type or remove the discriminator.",
          symbol.name,
          property_name
        );
      }
    }
  }

  // Only inline objects are mutated: a `Ref` member is narrowed when the
  // symbol it names is reached by this same loop.
  for symbol in symbols.iter_mut() {
    let Some(props) = narrowings.get(&symbol.name) else {
      continue;
    };
    for (property_name, literal_value) in props {
      narrow_property_in_body(&mut symbol.body, property_name, literal_value.as_ref());
    }
  }

  Ok(())
}

/// Finds a property by name through the shapes that can carry one: an
/// inline object, an `allOf` part, a `$ref` target, or a nullable wrapper.
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

/// True for the property types a discriminator may declare: bare `string`
/// or a string-literal enum.
const fn is_string_discriminator_shape(ty: &SchemaType) -> bool {
  matches!(
    ty,
    SchemaType::Scalar(SchemaScalar::String) | SchemaType::StringLiterals { .. }
  )
}

/// Narrows the named property to a single-value string literal, returning
/// whether it was found.
///
/// A property reachable only through a `$ref` is left alone: it keeps its
/// declared `string` type, which still compiles but narrows only partly.
fn narrow_property_in_body(body: &mut SchemaType, name: &str, literal_value: &str) -> bool {
  match body {
    SchemaType::InlineObject { properties } => {
      if let Some(property) = properties
        .iter_mut()
        .find(|property| property.name.as_ref() == name)
      {
        property.ty = SchemaType::StringLiterals {
          values: vec![literal_value.to_owned()],
        };
        return true;
      }
      false
    }
    SchemaType::Intersection(parts) => {
      for part in parts {
        if narrow_property_in_body(part, name, literal_value) {
          return true;
        }
      }
      false
    }
    SchemaType::Nullable(inner) => narrow_property_in_body(inner, name, literal_value),
    _ => false,
  }
}

fn validate_references(document: &ApiModel, reporter: &Reporter) -> Result<(), Diagnostic> {
  let symbol_index: BTreeSet<&str> = document
    .schemas
    .iter()
    .map(|symbol| symbol.name.as_ref())
    .collect();
  let mut refs: BTreeSet<&str> = BTreeSet::new();

  for symbol in &document.schemas {
    collect_type_references(&symbol.body, &mut refs);
  }

  for operation in &document.operations {
    for input in &operation.request.inputs {
      collect_type_references(&input.ty, &mut refs);
    }
    for header in &operation.request.headers {
      collect_type_references(&header.ty, &mut refs);
    }
    if let Some(body) = &operation.request.body {
      match &body.content {
        BodyContent::Json(ty) => collect_type_references(ty, &mut refs),
        // A form body's fields are typed by `BodyFieldType`, which
        // carries no schema reference; its `body_ref` was resolved
        // against the schema index at lowering time.
        BodyContent::Multipart { .. } | BodyContent::UrlEncoded { .. } => {}
      }
    }
    if let Some(response) = &operation.response {
      match response {
        ResponseContent::Json(Some(ty)) => collect_type_references(ty, &mut refs),
        // `Json(None)` carries no schema, and the other variants carry
        // no payload.
        ResponseContent::Json(None)
        | ResponseContent::Blob
        | ResponseContent::Text
        | ResponseContent::ArrayBuffer => {}
      }
    }
  }

  for name in refs {
    if !symbol_index.contains(name) {
      bail!(
        reporter,
        DiagnosticCode::InvalidReference,
        "Failed to validate spec: unresolved schema reference {name}. Check for typos in the $ref and confirm that components.schemas defines a top-level entry named '{name}'."
      );
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::narrow_discriminator_properties;
  use crate::ir::canonical::ModelSymbol;
  use crate::ir::schema::{Discriminator, SchemaProperty, SchemaScalar, SchemaType};
  use crate::test_support::test_reporter;
  use std::collections::BTreeMap;

  fn property(name: &str, ty: SchemaType) -> SchemaProperty {
    SchemaProperty {
      name: name.into(),
      required: true,
      ty,
      description: None,
      deprecated: false,
    }
  }

  fn symbol(name: &str, body: SchemaType) -> ModelSymbol {
    ModelSymbol {
      name: name.into(),
      description: None,
      deprecated: false,
      body,
    }
  }

  fn pet_union(members: Vec<&str>) -> SchemaType {
    SchemaType::Union {
      members: members
        .into_iter()
        .map(|n| SchemaType::Ref(n.into()))
        .collect(),
      discriminator: Some(Discriminator {
        property_name: "kind".into(),
        mapping: BTreeMap::new(),
      }),
    }
  }

  #[test]
  fn narrows_discriminator_property_on_intersection_member() {
    // Cat: allOf: [Animal, {kind: string, whiskers: number}]
    let cat_inline = SchemaType::InlineObject {
      properties: vec![
        property("kind", SchemaType::Scalar(SchemaScalar::String)),
        property("whiskers", SchemaType::Scalar(SchemaScalar::Number)),
      ],
    };
    let mut symbols = vec![
      symbol(
        "Animal",
        SchemaType::InlineObject {
          properties: vec![property("name", SchemaType::Scalar(SchemaScalar::String))],
        },
      ),
      symbol(
        "Cat",
        SchemaType::Intersection(vec![SchemaType::Ref("Animal".into()), cat_inline]),
      ),
      symbol("Pet", pet_union(vec!["Cat"])),
    ];

    let ctx = test_reporter();
    narrow_discriminator_properties(&mut symbols, &ctx).expect("ok");

    let cat = symbols.iter().find(|s| s.name.as_ref() == "Cat").unwrap();
    let SchemaType::Intersection(parts) = &cat.body else {
      panic!("Cat body should remain Intersection");
    };
    // The InlineObject part should now have kind narrowed to 'cat'.
    let kind_ty = parts
      .iter()
      .find_map(|part| match part {
        SchemaType::InlineObject { properties } => properties
          .iter()
          .find(|p| p.name.as_ref() == "kind")
          .map(|p| &p.ty),
        _ => None,
      })
      .expect("kind property present on inline part of Intersection");
    assert_eq!(
      kind_ty,
      &SchemaType::StringLiterals {
        values: vec!["cat".into()]
      }
    );
  }

  #[test]
  fn validates_discriminator_via_ref_in_intersection() {
    // Cat: allOf: [Animal] where only Animal declares 'kind'. Validation
    // walks into the referenced Animal and finds 'kind' there — no
    // missing-property diagnostic. (Mutation is partial in this shape;
    // the validation pass is the security-relevant guarantee.)
    let mut symbols = vec![
      symbol(
        "Animal",
        SchemaType::InlineObject {
          properties: vec![property("kind", SchemaType::Scalar(SchemaScalar::String))],
        },
      ),
      symbol(
        "Cat",
        SchemaType::Intersection(vec![SchemaType::Ref("Animal".into())]),
      ),
      symbol("Pet", pet_union(vec!["Cat"])),
    ];
    let ctx = test_reporter();
    narrow_discriminator_properties(&mut symbols, &ctx)
      .expect("Ref-shaped intersection should validate via the referenced base");
  }

  #[test]
  fn rejects_member_missing_discriminator_property_in_intersection() {
    // Cat: allOf: [Animal, {whiskers}] — neither part declares 'kind'.
    let mut symbols = vec![
      symbol(
        "Animal",
        SchemaType::InlineObject {
          properties: vec![property("name", SchemaType::Scalar(SchemaScalar::String))],
        },
      ),
      symbol(
        "Cat",
        SchemaType::Intersection(vec![
          SchemaType::Ref("Animal".into()),
          SchemaType::InlineObject {
            properties: vec![property(
              "whiskers",
              SchemaType::Scalar(SchemaScalar::Number),
            )],
          },
        ]),
      ),
      symbol("Pet", pet_union(vec!["Cat"])),
    ];
    let ctx = test_reporter();
    let err = narrow_discriminator_properties(&mut symbols, &ctx)
      .expect_err("missing kind anywhere must reject");
    assert_eq!(err.subcode, Some("missing-discriminator-property"));
  }

  #[test]
  fn rejects_integer_discriminator_property() {
    let mut symbols = vec![
      symbol(
        "Cat",
        SchemaType::InlineObject {
          properties: vec![property("kind", SchemaType::Scalar(SchemaScalar::Number))],
        },
      ),
      symbol("Pet", pet_union(vec!["Cat"])),
    ];
    let ctx = test_reporter();
    let err = narrow_discriminator_properties(&mut symbols, &ctx)
      .expect_err("integer discriminator must reject");
    assert_eq!(err.subcode, Some("discriminator-property-must-be-string"));
  }

  #[test]
  fn rejects_nullable_string_discriminator_property() {
    let mut symbols = vec![
      symbol(
        "Cat",
        SchemaType::InlineObject {
          properties: vec![property(
            "kind",
            SchemaType::Nullable(Box::new(SchemaType::Scalar(SchemaScalar::String))),
          )],
        },
      ),
      symbol("Pet", pet_union(vec!["Cat"])),
    ];
    let ctx = test_reporter();
    let err = narrow_discriminator_properties(&mut symbols, &ctx)
      .expect_err("nullable string discriminator must reject");
    assert_eq!(err.subcode, Some("discriminator-property-must-be-string"));
  }

  #[test]
  fn accepts_string_literals_discriminator_property() {
    // A spec that already constrains the discriminator to a single
    // literal is fine — the mutation simply rewrites to the canonical
    // single-value form.
    let mut symbols = vec![
      symbol(
        "Cat",
        SchemaType::InlineObject {
          properties: vec![property(
            "kind",
            SchemaType::StringLiterals {
              values: vec!["cat".into()],
            },
          )],
        },
      ),
      symbol("Pet", pet_union(vec!["Cat"])),
    ];
    let ctx = test_reporter();
    narrow_discriminator_properties(&mut symbols, &ctx).expect("ok");
  }
}
