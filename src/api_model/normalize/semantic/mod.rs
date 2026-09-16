//! Schema sorting, discriminator narrowing and `$ref` validation, run once schema and operation
//! lowering are done.

mod discriminator;
mod references;

use crate::api_model::canonical::ApiModel;
use crate::error::{Diagnostic, Reporter};

pub(super) fn finalize(model: &mut ApiModel, reporter: &Reporter) -> Result<(), Diagnostic> {
  model
    .schemas
    .sort_by(|left, right| left.name.cmp(&right.name));

  discriminator::narrow(&mut model.schemas, reporter)?;
  references::validate(model, reporter)
}

#[cfg(test)]
mod tests {
  use super::discriminator::narrow as narrow_discriminator_properties;
  use crate::api_model::canonical::ModelSymbol;
  use crate::api_model::schema::{Discriminator, SchemaProperty, SchemaScalar, SchemaType};
  use crate::test_support::test_reporter;
  use std::collections::BTreeMap;

  fn property(name: &str, schema: SchemaType) -> SchemaProperty {
    SchemaProperty {
      name: name.into(),
      required: true,
      schema,
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
        .map(|name| SchemaType::Ref(name.into()))
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
    let kind_ty = parts
      .iter()
      .find_map(|part| match part {
        SchemaType::InlineObject { properties } => properties
          .iter()
          .find(|p| p.name.as_ref() == "kind")
          .map(|p| &p.schema),
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
    // Cat: allOf: [Animal], where only Animal declares 'kind'.
    // Validation walks into Animal and finds it; mutation stays partial.
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
    // An already single-literal discriminator rewrites to the same shape.
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
