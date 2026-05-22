//! Normalize tests. Each test parses a YAML/JSON fixture, runs
//! `normalize_document`, and asserts on the resulting `ApiModel`
//! shape. The final semantic step (discriminator narrowing + `$ref`
//! validation) is exercised through `normalize_document` since it runs
//! inside `normalize_api_model`. Two-stage tests that pair normalize
//! with `render_type_reference` live in `crate::ir::tests` instead.

use serde_json::Value;

use crate::error::DiagnosticCode;
use crate::ir::canonical::{
  ApiInfo, ApiModel, BodyContent, HeaderDef, HttpMethod, ModelSymbol, OperationDef, RequestBodyDef,
  RequestDef, RequestInputDef, RequestInputSource, ResponseContent,
};
use crate::ir::normalize::normalize_document;
use crate::ir::normalize::semantic;
use crate::ir::schema::SchemaType;
use crate::test_support::{TestReporter, test_ctx};

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
fn normalize_lowers_oneof_anyof_and_collapses_single_entry_composition() {
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/oneof-anyof-composition.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/oneof-anyof-composition.openapi.yaml");
  let normalized = normalize_document(&document, &mut sink.reporter())
    .expect("normalize succeeds for supported oneOf/anyOf fixture");

  let pet_union = find_symbol(&normalized.schemas, "PetUnion");
  match &pet_union.body {
    SchemaType::Union { members, .. } => {
      assert_eq!(members.len(), 2);
      assert!(matches!(&members[0], SchemaType::Ref(name) if name.as_ref() == "Cat"));
      assert!(matches!(&members[1], SchemaType::Ref(name) if name.as_ref() == "Dog"));
    }
    other => panic!("expected union alias, got {other:?}"),
  }

  let adoption_request = find_symbol(&normalized.schemas, "AdoptionRequest");
  match &adoption_request.body {
    SchemaType::InlineObject { properties } => {
      let contact = properties
        .iter()
        .find(|property| property.name.as_ref() == "contact")
        .expect("contact property exists");
      assert!(matches!(contact.ty, SchemaType::Union { .. }));
    }
    other => panic!("expected object schema, got {other:?}"),
  }

  let single_entry = parse_fixture(include_str!(
    "../../../test/fixtures/single-entry-composition.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/single-entry-composition.openapi.yaml");
  let normalized_single = normalize_document(&single_entry, &mut sink.reporter())
    .expect("normalize succeeds for single-entry composition fixture");

  let animal_view = find_symbol(&normalized_single.schemas, "AnimalView");
  assert!(matches!(
    &animal_view.body,
    SchemaType::Ref(name) if name.as_ref() == "AnimalBase"
  ));
}

#[test]
fn normalize_supports_inline_object_allof_members_and_preserves_additional_properties_boundary() {
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/allof-composition.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/allof-composition.openapi.yaml");
  let normalized = normalize_document(&document, &mut sink.reporter())
    .expect("normalize succeeds for supported allOf fixture");

  let adopter_profile = find_symbol(&normalized.schemas, "AdopterProfile");
  match &adopter_profile.body {
    SchemaType::Intersection(members) => {
      assert_eq!(members.len(), 3);
      assert!(matches!(&members[0], SchemaType::Ref(name) if name.as_ref() == "AuditFields"));
      assert!(matches!(&members[1], SchemaType::Ref(name) if name.as_ref() == "ContactFields"));
      assert!(matches!(&members[2], SchemaType::InlineObject { .. }));
    }
    other => panic!("expected intersection alias, got {other:?}"),
  }

  let unsupported = parse_fixture(include_str!(
    "../../../test/fixtures/unsupported-semantic.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/unsupported-semantic.openapi.yaml");
  let error = normalize_document(&unsupported, &mut sink.reporter())
    .expect_err("unsupported fixture should fail at additionalProperties boundary");

  assert_eq!(error.code, DiagnosticCode::UnsupportedSemantic);
  assert!(error.message.contains("additionalProperties"));
  assert!(!error.message.contains("allOf"));
}

#[test]
fn normalize_supports_inline_object_model_shapes_outside_allof() {
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/inline-model.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/inline-model.openapi.yaml");
  let normalized = normalize_document(&document, &mut sink.reporter())
    .expect("normalize succeeds for inline model fixture");

  let pet_profile = find_symbol(&normalized.schemas, "PetProfile");

  let properties = match &pet_profile.body {
    SchemaType::InlineObject { properties } => properties,
    other => panic!("expected object schema, got {other:?}"),
  };

  let details = properties
    .iter()
    .find(|property| property.name.as_ref() == "details")
    .expect("details property exists");
  match &details.ty {
    SchemaType::InlineObject { properties } => {
      assert!(
        properties
          .iter()
          .any(|property| property.name.as_ref() == "displayName")
      );
      let address = properties
        .iter()
        .find(|property| property.name.as_ref() == "address")
        .expect("address property exists");
      assert!(matches!(address.ty, SchemaType::InlineObject { .. }));
    }
    other => panic!("expected inline object property, got {other:?}"),
  }

  let labels_by_locale = properties
    .iter()
    .find(|property| property.name.as_ref() == "labelsByLocale")
    .expect("labelsByLocale property exists");
  match &labels_by_locale.ty {
    SchemaType::Map(values) => {
      assert!(matches!(values.as_ref(), SchemaType::InlineObject { .. }));
    }
    other => panic!("expected map with inline object values, got {other:?}"),
  }

  let visits = properties
    .iter()
    .find(|property| property.name.as_ref() == "visits")
    .expect("visits property exists");
  match &visits.ty {
    SchemaType::Array(items) => {
      assert!(matches!(items.as_ref(), SchemaType::InlineObject { .. }));
    }
    other => panic!("expected array of inline objects, got {other:?}"),
  }
}

#[test]
fn normalize_supports_typed_additional_properties_for_nested_and_named_object_maps() {
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/additional-properties.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/additional-properties.openapi.yaml");
  let normalized = normalize_document(&document, &mut sink.reporter())
    .expect("normalize succeeds for typed additionalProperties fixture");

  let pet_catalog = find_symbol(&normalized.schemas, "PetCatalog");
  match &pet_catalog.body {
    SchemaType::InlineObject { properties } => {
      let scope = properties
        .iter()
        .find(|property| property.name.as_ref() == "scope")
        .expect("scope property exists");

      assert!(matches!(
        &scope.ty,
        SchemaType::StringLiterals { values }
          if values == &vec![
            "available".to_string(),
            "adopted".to_string(),
            "foster".to_string()
          ]
      ));

      let pets_by_breed = properties
        .iter()
        .find(|property| property.name.as_ref() == "petsByBreed")
        .expect("petsByBreed property exists");
      match &pets_by_breed.ty {
        SchemaType::Map(values) => match values.as_ref() {
          SchemaType::Array(items) => {
            assert!(matches!(items.as_ref(), SchemaType::Ref(name) if name.as_ref() == "Pet"));
          }
          other => panic!("expected map values to be arrays, got {other:?}"),
        },
        other => panic!("expected typed object map, got {other:?}"),
      }
    }
    other => panic!("expected object schema, got {other:?}"),
  }

  let pet_metadata_by_tag = find_symbol(&normalized.schemas, "PetMetadataByTag");
  match &pet_metadata_by_tag.body {
    SchemaType::Map(values) => {
      assert!(matches!(values.as_ref(), SchemaType::Ref(name) if name.as_ref() == "PetMetadata"));
    }
    other => panic!("expected map alias, got {other:?}"),
  }
}

#[test]
fn normalize_marks_only_listed_properties_as_required() {
  let document = parse_fixture(
    r#"
openapi: 3.0.3
info:
  title: Required Fields
  version: 1.0.0
paths: {}
components:
  schemas:
    RequiredExample:
      type: object
      required:
        - id
        - name
      properties:
        id:
          type: string
        optionalNote:
          type: string
        name:
          type: string
"#,
  );

  let mut sink = TestReporter::new("test/fixtures/required-fields.yaml");
  let normalized = normalize_document(&document, &mut sink.reporter())
    .expect("normalize succeeds for required fields fixture");

  let schema = find_symbol(&normalized.schemas, "RequiredExample");

  let properties = match &schema.body {
    SchemaType::InlineObject { properties } => properties,
    other => panic!("expected object schema, got {other:?}"),
  };

  assert_eq!(
    properties
      .iter()
      .map(|property| (property.name.as_ref(), property.required))
      .collect::<std::collections::BTreeMap<_, _>>(),
    std::collections::BTreeMap::from([("id", true), ("name", true), ("optionalNote", false),])
  );
}

#[test]
fn normalize_rejects_non_string_enums_and_invalid_enum_values() {
  let non_string_enum = parse_fixture(include_str!(
    "../../../test/fixtures/invalid-enum-type.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/invalid-enum-type.openapi.yaml");
  let non_string_error = normalize_document(&non_string_enum, &mut sink.reporter())
    .expect_err("non-string enum should fail normalization");

  assert_eq!(non_string_error.code, DiagnosticCode::UnsupportedSemantic);
  assert!(non_string_error.message.contains("enum"));
  assert!(non_string_error.message.contains("string"));

  let invalid_enum_value: Value = serde_json::from_str(include_str!(
    "../../../test/fixtures/invalid-enum-value.openapi.json"
  ))
  .expect("fixture parses as JSON");
  let mut sink = TestReporter::new("test/fixtures/invalid-enum-value.openapi.json");
  let invalid_value_error = normalize_document(&invalid_enum_value, &mut sink.reporter())
    .expect_err("enum value with null byte should fail normalization");

  assert_eq!(
    invalid_value_error.code,
    DiagnosticCode::UnsupportedSemantic
  );
  assert!(invalid_value_error.message.contains("enum"));
  assert!(invalid_value_error.message.contains("null"));
}

#[test]
fn normalize_rejects_empty_schema_parameters_outside_model_generation_scope() {
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/empty-parameter.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/empty-parameter.openapi.yaml");
  let error = normalize_document(&document, &mut sink.reporter())
    .expect_err("empty parameter schema should fail normalization");

  assert_eq!(error.code, DiagnosticCode::UnsupportedSemantic);
  assert!(error.message.contains("parameter"));
}

#[test]
fn normalize_rejects_ref_with_empty_target_name() {
  let document = parse_fixture(
    r#"
openapi: 3.0.3
info:
  title: Empty Ref Target
  version: 1.0.0
paths: {}
components:
  schemas:
    Wrapper:
      $ref: '#/components/schemas/'
"#,
  );

  let mut sink = TestReporter::new("test/fixtures/empty-ref-target.yaml");
  let error = normalize_document(&document, &mut sink.reporter())
    .expect_err("$ref with empty target name should fail normalization");

  assert_eq!(error.code, DiagnosticCode::UnsupportedSemantic);
  assert!(error.message.contains("empty"));
  assert!(error.message.contains("$ref"));
}

#[test]
fn normalize_rejects_trace_operations_with_specific_diagnostic() {
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/unsupported-trace.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/unsupported-trace.openapi.yaml");
  let error = normalize_document(&document, &mut sink.reporter())
    .expect_err("trace operations should fail normalization");

  assert_eq!(error.code, DiagnosticCode::UnsupportedSemantic);
  assert!(error.message.contains("TRACE"));
}

#[test]
fn normalize_rejects_paths_with_unbalanced_braces() {
  // Mirrors the `write_path_template_into` precondition: emit assumes
  // every `{` in a path has a matching `}`. Without this guard, a path
  // like `/pets/{id` would yield a broken TS template (`url:
  // `/pets/id`` with no `${encodeURIComponent(id)}` expansion).
  let document = parse_fixture(include_str!(
    "../../../test/fixtures/unbalanced-path-template.openapi.yaml"
  ));
  let mut sink = TestReporter::new("test/fixtures/unbalanced-path-template.openapi.yaml");
  let error = normalize_document(&document, &mut sink.reporter())
    .expect_err("unbalanced path template should fail normalization");

  assert_eq!(error.code, DiagnosticCode::UnsupportedSemantic);
  assert!(error.message.contains("unbalanced"));
  assert!(error.message.contains("/pets/{id"));
}

// ── semantic finalize (discriminator narrowing + ref validation) ─────────

#[test]
fn semantic_finalize_lowers_operations_with_inputs_body_and_response() {
  let mut model = ApiModel {
    info: ApiInfo {
      spec_version: "3.0.3".to_string(),
      title: "Example".to_string(),
    },
    schemas: vec![ModelSymbol {
      name: "Example".into(),
      description: None,
      deprecated: false,
      body: SchemaType::InlineObject {
        properties: Vec::new(),
      },
    }],
    operations: vec![OperationDef {
      operation_id: "createExample".to_string(),
      tags: vec!["Example".to_string(), "Ignored".to_string()],
      method: HttpMethod::Post,
      path: "/examples/{id}".to_string(),
      request: RequestDef {
        inputs: vec![
          RequestInputDef {
            name: "id".into(),
            source: RequestInputSource::Path,
            required: true,
            ty: SchemaType::Scalar(crate::ir::schema::SchemaScalar::String),
          },
          RequestInputDef {
            name: "includeInactive".into(),
            source: RequestInputSource::Query,
            required: false,
            ty: SchemaType::Scalar(crate::ir::schema::SchemaScalar::Boolean),
          },
        ],
        headers: vec![HeaderDef {
          name: "xTrace".into(),
          required: false,
          ty: SchemaType::Scalar(crate::ir::schema::SchemaScalar::String),
        }],
        body: Some(RequestBodyDef {
          required: true,
          content: BodyContent::Json(SchemaType::Array(Box::new(SchemaType::Ref(
            "Example".into(),
          )))),
        }),
      },
      response: Some(ResponseContent::Json(Some(SchemaType::Ref("Example".into())))),
      errors: Vec::new(),
      description: None,
      deprecated: false,
    }],
  };

  let mut ctx = test_ctx();
  semantic::finalize(&mut model, &ctx.reporter()).expect("semantic finalize succeeds");
  let operation = model.operations.first().expect("operation lowered");

  assert_eq!(operation.operation_id, "createExample");
  assert_eq!(operation.tags, vec!["Example", "Ignored"]);
  assert_eq!(operation.method, HttpMethod::Post);
  assert_eq!(operation.path, "/examples/{id}");
  assert_eq!(operation.request.inputs.len(), 2);
  assert!(matches!(
    operation.request.inputs[0].source,
    RequestInputSource::Path
  ));
  assert!(matches!(
    operation.request.inputs[1].source,
    RequestInputSource::Query
  ));
  assert_eq!(operation.request.headers.len(), 1);
  assert_eq!(operation.request.headers[0].name.as_ref(), "xTrace");
  assert!(matches!(
    operation.request.body.as_ref().expect("body present").content,
    BodyContent::Json(SchemaType::Array(_))
  ));
  assert!(matches!(
    operation.response.as_ref().expect("response present"),
    ResponseContent::Json(Some(SchemaType::Ref(name))) if name.as_ref() == "Example"
  ));
  assert_eq!(model.schemas.len(), 1);
  assert_eq!(model.schemas[0].name.as_ref(), "Example");
}

#[test]
fn semantic_finalize_rejects_unresolved_schema_reference() {
  let mut model = ApiModel {
    info: ApiInfo {
      spec_version: "3.0.3".to_string(),
      title: "Example".to_string(),
    },
    schemas: vec![ModelSymbol {
      name: "Wrapper".into(),
      description: None,
      deprecated: false,
      body: SchemaType::Ref("Missing".into()),
    }],
    operations: Vec::new(),
  };

  let mut ctx = test_ctx();
  let err = semantic::finalize(&mut model, &ctx.reporter()).expect_err("unresolved ref must error");
  assert_eq!(err.code, crate::error::DiagnosticCode::InvalidReference);
  assert!(err.message.contains("Missing"));
}
