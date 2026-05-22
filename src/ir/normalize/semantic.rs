//! Final semantic step of `normalize_api_model`: sort schemas, narrow
//! discriminator member properties, and validate `$ref` resolution.
//!
//! These transforms run after schema and operation lowering. They are
//! kept in a sibling file (rather than inlined into `mod.rs`) so the
//! discriminator-narrowing / reference-validation invariants are easy
//! to find and edit independently — but they are not a separate
//! pipeline stage.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Diagnostic, DiagnosticCode, Reporter};
use crate::ir::canonical::{ApiModel, BodyContent, ModelSymbol, ResponseContent};
use crate::ir::schema::{SchemaProperty, SchemaScalar, SchemaType, collect_type_references};

/// Sort the schema list alphabetically (stable iteration order), narrow
/// discriminator member properties to single-value enums, and validate
/// every `$ref` resolves to a declared top-level schema. Mutates the
/// model in place.
pub(super) fn finalize(model: &mut ApiModel, reporter: &Reporter<'_>) -> Result<(), Diagnostic> {
  model
    .schemas
    .sort_by(|left, right| left.name.cmp(&right.name));

  narrow_discriminator_properties(&mut model.schemas, reporter)?;
  validate_references(model, reporter)
}

/// Pre-emit transform: for each discriminated union, patches the
/// discriminator property on every member interface to a single-value
/// string literal type. Lets the TypeScript compiler narrow the union to
/// the concrete member type.
///
/// Before patching, validates that every member interface actually
/// declares the discriminator property. A member that omits it would
/// otherwise be patched with a synthetic single-value literal that
/// never existed on the source schema — producing TS that does not
/// narrow correctly and silently diverges from the original spec. Emit
/// `E_POLICY_VIOLATION` with subcode `missing-discriminator-property`
/// so consumers see the gap loudly.
fn narrow_discriminator_properties(
  symbols: &mut [ModelSymbol],
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  // Per member: map from discriminator property name to the literal
  // value to assign. Building a map lets the per-property pass below do
  // an O(log K) lookup instead of scanning K (prop_name, value) pairs
  // per property — the original shape was O(P · K).
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
          // Honor OpenAPI `discriminator.mapping`: when an entry's
          // value (pre-resolved to the bare schema name at IR-build
          // time) matches this member, use the entry's key as the
          // wire-value literal. Fall back to a lowercased schema name
          // so unmapped specs keep their previous narrowing shape.
          let literal_value: Box<str> = discriminator
            .mapping
            .iter()
            .find(|(_, target)| target.as_ref() == schema_name.as_ref()).map_or_else(|| schema_name.to_ascii_lowercase().into_boxed_str(), |(wire_value, _)| wire_value.clone());
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

  // First pass: validate that each member that needs a discriminator
  // narrowing actually declares the property — walking InlineObject,
  // Intersection (the canonical `allOf: [Base, {kind: '…'}]` shape),
  // Ref, and Nullable so allOf-composed variants don't silently skip.
  // Also confirms the existing property type is string-shaped before any
  // mutation happens, so an integer discriminator surfaces a loud
  // diagnostic instead of being coerced into a synthetic string literal.
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
        return Err(Diagnostic::policy_violation(
          reporter,
          "missing-discriminator-property",
          format!(
            "Failed to validate spec: oneOf member '{}' does not declare the discriminator property '{}'. Add the property to the member schema (typically as `type: string`) or remove the discriminator.",
            symbol.name, property_name
          ),
        ));
      };
      if !is_string_discriminator_shape(&property.ty) {
        return Err(Diagnostic::policy_violation(
          reporter,
          "discriminator-property-must-be-string",
          format!(
            "Failed to validate spec: oneOf member '{}' declares discriminator property '{}' with a non-string type. Discriminator properties must be `type: string` (optionally with an enum); change the property type or remove the discriminator.",
            symbol.name, property_name
          ),
        ));
      }
    }
  }

  // Second pass: mutate. Only mutates InlineObject members directly
  // (either as a symbol body, or as an inline part of an Intersection).
  // Ref-shaped members inherit narrowing from the referenced symbol's
  // own mutation — no double-write needed. An Intersection of only
  // Refs is left alone (the referenced symbols mutate themselves if
  // they're also union members).
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

/// Resolve a property by name across the shapes that can carry one
/// after normalization. Used by the validation pass so a discriminator
/// property hidden behind `allOf` (Intersection) or a base-class `$ref`
/// is still found.
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

/// Predicate for the validation pass: the existing property type must
/// already be string-shaped — bare `string`, or a `'a' | 'b'` enum.
/// Anything else (integer, nullable, ref to another schema, …) is
/// rejected as `discriminator-property-must-be-string`.
const fn is_string_discriminator_shape(ty: &SchemaType) -> bool {
  matches!(
    ty,
    SchemaType::Scalar(SchemaScalar::String) | SchemaType::StringLiterals { .. }
  )
}

/// Narrow the named property in `body` to a single-value string literal.
/// Recurses into Intersection so a property declared on an inline part
/// of an `allOf` is mutated in place. Returns silently when the property
/// can't be reached through inline shapes — the validation pass has
/// already confirmed it exists somewhere reachable; for a Ref-only
/// intersection that points at a non-union-member base, the type just
/// stays as its original `string` shape (TS narrowing is partial in
/// that case but the surface still compiles).
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

fn validate_references(document: &ApiModel, reporter: &Reporter<'_>) -> Result<(), Diagnostic> {
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
        // Multipart / UrlEncoded bodies are not yet produced by
        // normalize; their field-type references will be collected
        // when the walkers land in a later phase.
        BodyContent::Multipart { .. } | BodyContent::UrlEncoded { .. } => {}
      }
    }
    if let Some(response) = &operation.response {
      match response {
        ResponseContent::Json(Some(ty)) => collect_type_references(ty, &mut refs),
        // `Json(None)` carries no schema, and the non-JSON variants
        // have fixed TS surfaces (`Blob` / `string` / `ArrayBuffer`)
        // that never reference user-declared schemas.
        ResponseContent::Json(None)
        | ResponseContent::Blob
        | ResponseContent::Text
        | ResponseContent::ArrayBuffer => {}
      }
    }
  }

  for name in refs {
    if !symbol_index.contains(name) {
      return Err(reporter.error(
        DiagnosticCode::InvalidReference,
        format!(
          "Failed to validate spec: unresolved schema reference {name}. Check for typos in the $ref and confirm that components.schemas defines a top-level entry named '{name}'."
        ),
      ));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::narrow_discriminator_properties;
  use crate::ir::canonical::ModelSymbol;
  use crate::ir::schema::{Discriminator, SchemaProperty, SchemaScalar, SchemaType};
  use crate::test_support::test_ctx;
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

  // ── Issue 2a: Intersection walk ─────────────────────────────────────────

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

    let mut ctx = test_ctx();
    narrow_discriminator_properties(&mut symbols, &ctx.reporter()).expect("ok");

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
    let mut ctx = test_ctx();
    narrow_discriminator_properties(&mut symbols, &ctx.reporter())
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
    let mut ctx = test_ctx();
    let err = narrow_discriminator_properties(&mut symbols, &ctx.reporter())
      .expect_err("missing kind anywhere must reject");
    assert_eq!(err.subcode, Some("missing-discriminator-property"));
  }

  // ── Issue 2b: type-check before clobbering ──────────────────────────────

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
    let mut ctx = test_ctx();
    let err = narrow_discriminator_properties(&mut symbols, &ctx.reporter())
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
    let mut ctx = test_ctx();
    let err = narrow_discriminator_properties(&mut symbols, &ctx.reporter())
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
    let mut ctx = test_ctx();
    narrow_discriminator_properties(&mut symbols, &ctx.reporter()).expect("ok");
  }
}
