//! Two-stage tests that drive `normalize_document` →
//! `render_type_reference`. They exercise the IR-shape invariants the
//! emit layer relies on. (Discriminator narrowing and `$ref`
//! validation run inside `normalize_api_model`, so a single
//! `normalize_document` call now returns the finalized IR.) Pure
//! normalize tests with no render step live next to the normalize
//! stage in `ir::normalize::tests`.

use serde_json::Value;

use crate::emit::typescript::render_type_reference;
use crate::ir::canonical::ModelSymbol;
use crate::ir::normalize::normalize_document;
use crate::ir::schema::SchemaType;
use crate::test_support::TestReporter;

fn parse_fixture(source: &str) -> Value {
  serde_yml::from_str(source).expect("fixture parses as YAML")
}

fn find_symbol<'a>(symbols: &'a [ModelSymbol], name: &str) -> &'a ModelSymbol {
  symbols
    .iter()
    .find(|symbol| symbol.name.as_ref() == name)
    .unwrap_or_else(|| panic!("{name} schema exists"))
}

#[test]
fn normalize_supports_empty_schema_any_type_and_empty_object_shapes() {
  let document = parse_fixture(include_str!(
    "../../test/fixtures/empty-shapes.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/empty-shapes.openapi.yaml");
  let ir = normalize_document(&document, &mut sink.reporter())
    .expect("normalize succeeds for empty schema fixture");

  let any_value = find_symbol(&ir.schemas, "AnyValue");
  assert!(!matches!(&any_value.body, SchemaType::Ref(_)));
  assert_eq!(render_type_reference(&any_value.body), "unknown");

  for schema_name in ["EmptyObject", "EmptyObjectWithProperties"] {
    let empty_object = find_symbol(&ir.schemas, schema_name);
    match &empty_object.body {
      SchemaType::InlineObject { properties } => {
        assert!(
          properties.is_empty(),
          "{schema_name} should have no properties"
        );
      }
      other => panic!("expected object schema for {schema_name}, got {other:?}"),
    }
  }

  let shape_container = find_symbol(&ir.schemas, "ShapeContainer");
  let properties = match &shape_container.body {
    SchemaType::InlineObject { properties } => properties,
    other => panic!("expected object schema, got {other:?}"),
  };

  let anything = properties
    .iter()
    .find(|property| property.name.as_ref() == "anything")
    .expect("anything property exists");
  assert_eq!(render_type_reference(&anything.ty), "unknown");

  let empty_inline = properties
    .iter()
    .find(|property| property.name.as_ref() == "emptyInline")
    .expect("emptyInline property exists");
  let empty_inline_with_properties = properties
    .iter()
    .find(|property| property.name.as_ref() == "emptyInlineWithProperties")
    .expect("emptyInlineWithProperties property exists");
  let empty_array = properties
    .iter()
    .find(|property| property.name.as_ref() == "emptyArray")
    .expect("emptyArray property exists");
  let empty_map = properties
    .iter()
    .find(|property| property.name.as_ref() == "emptyMap")
    .expect("emptyMap property exists");

  for property in [empty_inline, empty_inline_with_properties] {
    match &property.ty {
      SchemaType::InlineObject { properties } => assert!(properties.is_empty()),
      other => panic!("expected empty inline object, got {other:?}"),
    }
  }

  match &empty_array.ty {
    SchemaType::Array(items) => {
      assert_eq!(render_type_reference(&empty_array.ty), "unknown[]");
      assert!(!matches!(items.as_ref(), SchemaType::Ref(_)));
    }
    other => panic!("expected array, got {other:?}"),
  }

  match &empty_map.ty {
    SchemaType::Map(values) => {
      assert_eq!(
        render_type_reference(&empty_map.ty),
        "Record<string, unknown>"
      );
      assert!(!matches!(values.as_ref(), SchemaType::Ref(_)));
    }
    other => panic!("expected map, got {other:?}"),
  }
}

#[test]
fn ir_renders_union_and_intersection_type_fragments_from_normalized_composition() {
  let oneof_document = parse_fixture(include_str!(
    "../../test/fixtures/oneof-anyof-composition.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/oneof-anyof-composition.openapi.yaml");
  let oneof_ir =
    normalize_document(&oneof_document, &mut sink.reporter()).expect("normalize succeeds");

  let pet_union = oneof_ir
    .schemas
    .iter()
    .find_map(|symbol| {
      if symbol.name.as_ref() == "PetUnion" {
        Some(&symbol.body)
      } else {
        None
      }
    })
    .expect("PetUnion alias exists in IR");
  assert_eq!(render_type_reference(pet_union), "Cat | Dog");

  let allof_document = parse_fixture(include_str!(
    "../../test/fixtures/allof-composition.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/allof-composition.openapi.yaml");
  let allof_ir =
    normalize_document(&allof_document, &mut sink.reporter()).expect("normalize succeeds");

  let adopter_profile = allof_ir
    .schemas
    .iter()
    .find_map(|symbol| {
      if symbol.name.as_ref() == "AdopterProfile" {
        Some(&symbol.body)
      } else {
        None
      }
    })
    .expect("AdopterProfile alias exists in IR");

  match adopter_profile {
    SchemaType::Intersection(members) => {
      assert_eq!(members.len(), 3);
      let rendered = render_type_reference(adopter_profile);
      assert!(rendered.contains("AuditFields & ContactFields & {"));
      assert!(rendered.contains("nickname?: string | null;"));
    }
    other => panic!("expected IR intersection, got {other:?}"),
  }

  let additional_properties_document = parse_fixture(include_str!(
    "../../test/fixtures/additional-properties.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/additional-properties.openapi.yaml");
  let additional_properties_ir =
    normalize_document(&additional_properties_document, &mut sink.reporter())
      .expect("normalize succeeds");

  let pet_catalog_pets_by_breed = additional_properties_ir
    .schemas
    .iter()
    .find_map(|symbol| match &symbol.body {
      SchemaType::InlineObject { properties } if symbol.name.as_ref() == "PetCatalog" => properties
        .iter()
        .find(|property| property.name.as_ref() == "petsByBreed")
        .map(|property| &property.ty),
      _ => None,
    })
    .expect("PetCatalog.petsByBreed exists in IR");
  assert_eq!(
    render_type_reference(pet_catalog_pets_by_breed),
    "Record<string, Pet[]>"
  );

  let pet_catalog_scope = additional_properties_ir
    .schemas
    .iter()
    .find_map(|symbol| match &symbol.body {
      SchemaType::InlineObject { properties } if symbol.name.as_ref() == "PetCatalog" => properties
        .iter()
        .find(|property| property.name.as_ref() == "scope")
        .map(|property| &property.ty),
      _ => None,
    })
    .expect("PetCatalog.scope exists in IR");
  assert_eq!(
    render_type_reference(pet_catalog_scope),
    "'available' | 'adopted' | 'foster'"
  );

  let pet_metadata_by_tag = additional_properties_ir
    .schemas
    .iter()
    .find_map(|symbol| {
      if symbol.name.as_ref() == "PetMetadataByTag" {
        Some(&symbol.body)
      } else {
        None
      }
    })
    .expect("PetMetadataByTag alias exists in IR");
  assert_eq!(
    render_type_reference(pet_metadata_by_tag),
    "Record<string, PetMetadata>"
  );
}
