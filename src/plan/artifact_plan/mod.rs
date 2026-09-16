//! The shapes the emitters read: one plan per service, per operation and per request field.

mod collisions;
mod resolve;

use crate::api_model::canonical::{BodyFieldType, ErrorResponse, HttpMethod, ResponseContent};
use crate::api_model::schema::SchemaType;
use crate::identifier::{Identifier, MethodName, TypeName};
use crate::options::MappedType;

pub(crate) use resolve::{resolve_service_plans, validate_mapped_types_against_schemas};

/// A [`MappedType`] whose `schema` was found in the IR, borrowed from the model symbol that
/// matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedMappedType<'a> {
  pub(crate) schema: &'a str,
  pub(crate) import: Box<str>,
  pub(crate) type_name: Box<str>,
  pub(crate) alias: Option<Box<str>>,
}

impl<'a> ResolvedMappedType<'a> {
  #[must_use]
  pub(crate) fn new(schema: &'a str, source: &MappedType) -> Self {
    Self {
      schema,
      import: Box::from(source.import.as_str()),
      type_name: Box::from(source.type_name.as_str()),
      alias: source.alias.as_deref().map(Box::from),
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ServicePlan<'model> {
  pub(crate) group_name: String,
  pub(crate) class_name: TypeName,
  pub(crate) artifact_path: String,
  /// `rest/<group>/index.ts`; `Some` only when the `operations` layout is on.
  pub(crate) operations_barrel_path: Option<String>,
  pub(crate) operations: Vec<PlannedOperation<'model>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedOperation<'model> {
  pub(crate) operation_id: String,
  pub(crate) method_name: MethodName,
  pub(crate) method: HttpMethod,
  pub(crate) path: String,
  pub(crate) request: PlannedRequestContract<'model>,
  pub(crate) response: Option<&'model ResponseContent>,
  /// The operation's typed error responses, empty when it declared none.
  pub(crate) errors: &'model [ErrorResponse],
  /// Name of the `{Pascal}Params` interface, or `None` when the operation declares no path,
  /// query, header or body input and so emits none.
  pub(crate) request_interface: Option<TypeName>,
  /// Name of the `{Pascal}Error` interface, or `None` when the operation declares no 4xx/5xx
  /// response with a JSON schema.
  pub(crate) error_interface: Option<TypeName>,
  pub(crate) description: Option<String>,
  pub(crate) deprecated: bool,
  /// `rest/<group>/<method>.ts`; `Some` only when the `operations` layout is on.
  pub(crate) artifact_path: Option<String>,
}

/// Which slot of the HTTP request a [`PlannedRequestField`] fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestFieldKind {
  Path,
  Query,
  Body,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedRequestContract<'model> {
  /// Path and query parameters; a hoisted body property lives on [`PlannedRequestBody::FlatJson`].
  pub(crate) fields: Vec<PlannedRequestField<'model>>,
  /// Header parameters, empty when the operation declares none.
  pub(crate) headers: Vec<PlannedHeader<'model>>,
  /// The body's layout, `None` when the operation declares no body.
  pub(crate) body: Option<PlannedRequestBody<'model>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedRequestField<'model> {
  pub(crate) name: Box<str>,
  pub(crate) optional: bool,
  pub(crate) schema: &'model SchemaType,
  pub(crate) kind: RequestFieldKind,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedHeader<'model> {
  pub(crate) name: Box<str>,
  pub(crate) optional: bool,
  pub(crate) schema: &'model SchemaType,
}

/// One field of a multipart or urlencoded body.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedFormField<'model> {
  pub(crate) name: Identifier,
  pub(crate) optional: bool,
  pub(crate) field_type: &'model BodyFieldType,
}

/// How a request body is laid out on the request contract.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PlannedRequestBody<'model> {
  /// A top-level `$ref`, scalar, array or union, under one `body` key.
  Nested {
    schema: &'model SchemaType,
    optional: bool,
  },
  /// An inline JSON object body, its properties hoisted to top level.
  FlatJson {
    properties: Vec<PlannedRequestField<'model>>,
    required: bool,
  },
  /// A `multipart/form-data` body, its fields hoisted to top level.
  Multipart {
    fields: Vec<PlannedFormField<'model>>,
  },
  /// An `application/x-www-form-urlencoded` body, its fields hoisted to top level.
  UrlEncoded {
    fields: Vec<PlannedFormField<'model>>,
  },
}

#[cfg(test)]
mod tests {
  use super::{
    PlannedFormField, PlannedRequestBody, PlannedRequestContract, RequestFieldKind,
    resolve_service_plans, validate_mapped_types_against_schemas,
  };
  use crate::{
    api_model::{
      canonical::{
        ApiInfo, ApiModel, BodyContent, BodyFieldType, HttpMethod, ModelSymbol, OperationDef,
        RequestBodyDef, RequestDef, RequestInputDef, RequestInputSource, ResponseContent,
      },
      schema::{SchemaProperty, SchemaScalar, SchemaType},
    },
    options::MappedType,
  };

  use crate::test_support::{empty_request, op_with, test_reporter};

  fn api_model(schemas: Vec<ModelSymbol>, operations: Vec<OperationDef>) -> ApiModel {
    ApiModel {
      info: ApiInfo {
        spec_version: "3.0.3".to_string(),
        title: "Test".to_string(),
      },
      schemas,
      operations,
    }
  }

  fn test_model_symbols() -> Vec<ModelSymbol> {
    vec![
      ModelSymbol {
        name: "UserId".into(),
        description: None,
        deprecated: false,
        body: SchemaType::Ref("string".into()),
      },
      ModelSymbol {
        name: "User".into(),
        description: None,
        deprecated: false,
        body: SchemaType::InlineObject {
          properties: Vec::new(),
        },
      },
    ]
  }

  fn service_test_ir() -> ApiModel {
    let model_symbols = vec![
      ModelSymbol {
        name: "PetId".into(),
        description: None,
        deprecated: false,
        body: SchemaType::Scalar(SchemaScalar::String),
      },
      ModelSymbol {
        name: "PetStatus".into(),
        description: None,
        deprecated: false,
        body: SchemaType::StringLiterals {
          values: vec!["available".to_string(), "pending".to_string()],
        },
      },
      ModelSymbol {
        name: "UpdatePetPayload".into(),
        description: None,
        deprecated: false,
        body: SchemaType::InlineObject {
          properties: vec![
            SchemaProperty {
              name: "status".into(),
              required: true,
              schema: SchemaType::Ref("PetStatus".into()),
              description: None,
              deprecated: false,
            },
            SchemaProperty {
              name: "tagIds".into(),
              required: true,
              schema: SchemaType::Array(Box::new(SchemaType::Scalar(SchemaScalar::Number))),
              description: None,
              deprecated: false,
            },
            SchemaProperty {
              name: "nickname".into(),
              required: false,
              schema: SchemaType::Nullable(Box::new(SchemaType::Scalar(SchemaScalar::String))),
              description: None,
              deprecated: false,
            },
          ],
        },
      },
      ModelSymbol {
        name: "Pet".into(),
        description: None,
        deprecated: false,
        body: SchemaType::InlineObject {
          properties: Vec::new(),
        },
      },
      ModelSymbol {
        name: "PetList".into(),
        description: None,
        deprecated: false,
        body: SchemaType::InlineObject {
          properties: Vec::new(),
        },
      },
    ];
    let operations = vec![
      OperationDef {
        operation_id: "listPets".to_string(),
        tags: vec!["Pet".to_string()],
        method: HttpMethod::Get,
        path: "/pets".to_string(),
        request: RequestDef::default(),
        response: Some(ResponseContent::Json(Some(SchemaType::Ref(
          "PetList".into(),
        )))),
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
      OperationDef {
        operation_id: "updatePet".to_string(),
        tags: vec!["Pet".to_string()],
        method: HttpMethod::Post,
        path: "/pets/{petId}".to_string(),
        request: RequestDef {
          inputs: vec![
            RequestInputDef {
              name: "petId".into(),
              source: RequestInputSource::Path,
              required: true,
              schema: SchemaType::Ref("PetId".into()),
            },
            RequestInputDef {
              name: "includeHistory".into(),
              source: RequestInputSource::Query,
              required: false,
              schema: SchemaType::Scalar(SchemaScalar::Boolean),
            },
          ],
          headers: Vec::new(),
          body: Some(RequestBodyDef {
            required: true,
            content: BodyContent::Json(SchemaType::Ref("UpdatePetPayload".into())),
          }),
        },
        response: Some(ResponseContent::Json(Some(SchemaType::Ref("Pet".into())))),
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
      OperationDef {
        operation_id: "createAdoptionRequest".to_string(),
        tags: vec!["AdoptionRequest".to_string()],
        method: HttpMethod::Post,
        path: "/adoption-requests".to_string(),
        request: RequestDef {
          inputs: Vec::new(),
          headers: Vec::new(),
          body: Some(RequestBodyDef {
            required: true,
            content: BodyContent::Json(SchemaType::InlineObject {
              properties: vec![SchemaProperty {
                name: "petId".into(),
                required: true,
                schema: SchemaType::Ref("PetId".into()),
                description: None,
                deprecated: false,
              }],
            }),
          }),
        },
        response: Some(ResponseContent::Json(Some(SchemaType::Ref("Pet".into())))),
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
    ];
    api_model(model_symbols, operations)
  }

  #[test]
  fn model_symbol_name_returns_variant_name() {
    let symbols = test_model_symbols();

    assert_eq!(
      symbols
        .iter()
        .map(|symbol| symbol.name.as_ref())
        .collect::<Vec<_>>(),
      vec!["UserId", "User"]
    );
  }

  #[test]
  fn validate_mapped_types_accepts_schemas_that_exist_in_the_ir() {
    let ctx = test_reporter();
    let symbols = test_model_symbols();
    let resolved = validate_mapped_types_against_schemas(
      &symbols,
      &[MappedType {
        schema: "UserId".to_string(),
        import: "./shared/user-id".to_string(),
        type_name: "ExternalUserId".to_string(),
        alias: Some("UserId".to_string()),
      }],
      &ctx,
    )
    .expect("mapped types validate against IR");

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].schema, "UserId");
    assert_eq!(resolved[0].import.as_ref(), "./shared/user-id");
    assert_eq!(resolved[0].type_name.as_ref(), "ExternalUserId");
    assert_eq!(resolved[0].alias.as_deref(), Some("UserId"));
  }

  #[test]
  fn validate_mapped_types_rejects_schemas_missing_from_the_ir() {
    let ctx = test_reporter();
    let err = validate_mapped_types_against_schemas(
      &test_model_symbols(),
      &[MappedType {
        schema: "Missing".to_string(),
        import: "./missing".to_string(),
        type_name: "Missing".to_string(),
        alias: None,
      }],
      &ctx,
    )
    .expect_err("missing schema should fail validation");

    assert_eq!(err.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(err.message.contains("Missing"));
  }

  #[test]
  fn resolve_service_plans_rejects_a_group_name_with_no_letters_or_digits() {
    // The kebab stem would be empty, leaving `rest/.rest.ts` and, under the
    // operations layout, `rest//index.ts`.
    let model = service_test_ir();
    let ctx = test_reporter();
    let resolver = crate::plan::naming::NamingResolver::new(crate::plan::naming::NamingConfig {
      group: Some(crate::plan::naming::Naming::Single(
        crate::plan::naming::RuleEntry::Shorthand("---".to_string()),
      )),
      ..Default::default()
    });

    let error = resolve_service_plans(&model, &resolver, &ctx, false)
      .expect_err("an all-punctuation group names no file");
    assert_eq!(error.subcode, Some("naming-resolution"));
    assert!(error.message.contains("no letters or digits"));
  }

  #[test]
  fn resolve_service_plans_groups_operations_and_builds_request_contracts() {
    let model = service_test_ir();
    let ctx = test_reporter();
    let services = resolve_service_plans(
      &model,
      &crate::plan::naming::NamingResolver::default(),
      &ctx,
      false,
    )
    .expect("service plan resolves");

    assert_eq!(services.len(), 2);
    // Services sort by `class_name`, not by discovery order.
    assert_eq!(
      services
        .iter()
        .map(|service| service.group_name.as_str())
        .collect::<Vec<_>>(),
      vec!["AdoptionRequest", "Pet"]
    );

    let pet_service = &services[1];
    assert_eq!(pet_service.class_name.to_string(), "PetRest");
    assert_eq!(pet_service.artifact_path, "rest/pet.rest.ts");
    assert_eq!(
      pet_service
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<Vec<_>>(),
      vec!["listPets", "updatePet"]
    );

    let update_pet = &pet_service.operations[1];
    assert!(!update_pet.request.fields.is_empty());
    assert_eq!(
      update_pet
        .request
        .fields
        .iter()
        .map(|field| field.name.as_ref())
        .collect::<Vec<_>>(),
      vec!["petId", "includeHistory"]
    );
    let kinds: Vec<RequestFieldKind> = update_pet
      .request
      .fields
      .iter()
      .map(|field| field.kind)
      .collect();
    assert_eq!(kinds, vec![RequestFieldKind::Path, RequestFieldKind::Query]);
    match &update_pet.request.body {
      Some(PlannedRequestBody::Nested { schema, optional }) => {
        assert!(!optional, "body marked required in fixture");
        assert!(
          matches!(schema, SchemaType::Ref(name) if name.as_ref() == "UpdatePetPayload"),
          "expected body schema to remain the ref, got {schema:?}"
        );
      }
      other => panic!("expected nested ref body, got {other:?}"),
    }
  }

  #[test]
  fn resolve_service_plans_keeps_ref_bodies_nested_under_smart_flatten() {
    // A body authored as a `$ref` stays nested even when the ref resolves to an `InlineObject`.
    let model_symbols = vec![
      ModelSymbol {
        name: "PetId".into(),
        description: None,
        deprecated: false,
        body: SchemaType::Scalar(SchemaScalar::String),
      },
      ModelSymbol {
        name: "CreatePetRequest".into(),
        description: None,
        deprecated: false,
        body: SchemaType::InlineObject {
          properties: vec![SchemaProperty {
            name: "petId".into(),
            required: true,
            schema: SchemaType::Ref("PetId".into()),
            description: None,
            deprecated: false,
          }],
        },
      },
    ];
    let model = api_model(
      model_symbols,
      vec![OperationDef {
        operation_id: "createPet".to_string(),
        tags: vec!["Pet".to_string()],
        method: HttpMethod::Post,
        path: "/pets/{petId}".to_string(),
        request: RequestDef {
          inputs: vec![RequestInputDef {
            name: "petId".into(),
            source: RequestInputSource::Path,
            required: true,
            schema: SchemaType::Ref("PetId".into()),
          }],
          headers: Vec::new(),
          body: Some(RequestBodyDef {
            required: true,
            content: BodyContent::Json(SchemaType::Ref("CreatePetRequest".into())),
          }),
        },
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      }],
    );

    let ctx = test_reporter();
    let services = resolve_service_plans(
      &model,
      &crate::plan::naming::NamingResolver::default(),
      &ctx,
      false,
    )
    .expect("ref body stays nested even when it resolves to an inline object");
    let create_pet = &services[0].operations[0];
    assert!(matches!(
      create_pet.request.body,
      Some(PlannedRequestBody::Nested { .. })
    ));
  }

  #[test]
  fn resolve_service_plans_sorts_services_and_operations_alphabetically() {
    let operations = vec![
      OperationDef {
        operation_id: "zebraInZoo".to_string(),
        tags: vec!["Zoo".to_string()],
        method: HttpMethod::Get,
        path: "/zoo/zebra".to_string(),
        request: RequestDef::default(),
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
      OperationDef {
        operation_id: "adoptPet".to_string(),
        tags: vec!["Adoption".to_string()],
        method: HttpMethod::Post,
        path: "/adoptions".to_string(),
        request: RequestDef::default(),
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
      OperationDef {
        operation_id: "antInZoo".to_string(),
        tags: vec!["Zoo".to_string()],
        method: HttpMethod::Get,
        path: "/zoo/ant".to_string(),
        request: RequestDef::default(),
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
      OperationDef {
        operation_id: "abandonPet".to_string(),
        tags: vec!["Adoption".to_string()],
        method: HttpMethod::Post,
        path: "/abandonments".to_string(),
        request: RequestDef::default(),
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      },
    ];
    let model = api_model(Vec::new(), operations);

    let ctx = test_reporter();
    let services = resolve_service_plans(
      &model,
      &crate::plan::naming::NamingResolver::default(),
      &ctx,
      false,
    )
    .expect("plans resolve");

    assert_eq!(
      services
        .iter()
        .map(|service| service.class_name.to_string())
        .collect::<Vec<_>>(),
      vec!["AdoptionRest", "ZooRest"]
    );

    for service in &services {
      let ids: Vec<&str> = service
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect();
      let mut sorted = ids.clone();
      sorted.sort_unstable();
      assert_eq!(
        ids, sorted,
        "operations must be alphabetical by method_name (== operation_id for already-camelCase ids)"
      );
    }

    let adoption = &services[0];
    assert_eq!(
      adoption
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<Vec<_>>(),
      vec!["abandonPet", "adoptPet"]
    );
  }

  #[test]
  fn planned_request_body_multipart_carries_form_fields_collection() {
    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let contract = PlannedRequestContract {
      fields: vec![],
      headers: vec![],
      body: Some(PlannedRequestBody::Multipart {
        fields: vec![PlannedFormField {
          name: crate::identifier::Identifier::parse("status").expect("identifier"),
          optional: false,
          field_type: &scalar,
        }],
      }),
    };
    let Some(PlannedRequestBody::Multipart { fields }) = &contract.body else {
      panic!("expected multipart body");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name.as_str(), "status");
  }

  #[test]
  fn planned_request_body_carries_smart_flatten_variants() {
    let schema = SchemaType::Scalar(SchemaScalar::String);
    let _: PlannedRequestBody<'_> = PlannedRequestBody::Nested {
      schema: &schema,
      optional: false,
    };
    let _: PlannedRequestBody<'_> = PlannedRequestBody::FlatJson {
      properties: vec![],
      required: true,
    };
    let _: PlannedRequestBody<'_> = PlannedRequestBody::Multipart { fields: vec![] };
    let _: PlannedRequestBody<'_> = PlannedRequestBody::UrlEncoded { fields: vec![] };
  }

  fn operation_def(operation_id: &str, method: HttpMethod, path: &str) -> OperationDef {
    OperationDef {
      operation_id: operation_id.to_string(),
      tags: vec!["Pet".to_string()],
      method,
      path: path.to_string(),
      request: RequestDef::default(),
      response: None,
      errors: Vec::new(),
      description: None,
      deprecated: false,
    }
  }

  #[test]
  fn index_method_name_is_rejected_under_the_operations_layout() {
    let model = api_model(
      Vec::new(),
      vec![operation_def("index", HttpMethod::Get, "/pets")],
    );
    let ctx = test_reporter();
    let err = resolve_service_plans(
      &model,
      &crate::plan::naming::NamingResolver::default(),
      &ctx,
      true,
    )
    .expect_err("an operation file named index.ts would overwrite the barrel");
    assert!(
      err.message.contains("methodName 'index'"),
      "{}",
      err.message
    );
    assert!(
      err.message.contains("rest/pet/index.ts is the barrel"),
      "{}",
      err.message
    );
  }

  #[test]
  fn index_method_name_is_a_plain_property_under_the_services_layout() {
    let model = api_model(
      Vec::new(),
      vec![operation_def("index", HttpMethod::Get, "/pets")],
    );
    let ctx = test_reporter();
    let services = resolve_service_plans(
      &model,
      &crate::plan::naming::NamingResolver::default(),
      &ctx,
      false,
    )
    .expect("no barrel, no collision");
    assert_eq!(services[0].operations[0].method_name.as_str(), "index");
    assert_eq!(services[0].operations[0].artifact_path, None);
  }

  #[test]
  fn method_name_without_letters_or_digits_is_rejected_under_the_operations_layout() {
    let model = api_model(
      Vec::new(),
      vec![operation_def("$", HttpMethod::Get, "/pets")],
    );
    let ctx = test_reporter();
    let err = resolve_service_plans(
      &model,
      &crate::plan::naming::NamingResolver::new(crate::plan::naming::NamingConfig {
        method_name: Some(crate::plan::naming::Naming::Single(
          crate::plan::naming::RuleEntry::Rule(crate::plan::naming::Rule {
            from: Some("{operationId}".to_string()),
            parse: None,
            format: None,
            case: None,
          }),
        )),
        group: None,
      }),
      &ctx,
      true,
    )
    .expect_err("an empty file stem would produce rest/pet/.ts");
    assert_eq!(err.subcode, Some("naming-resolution"));
    assert!(err.message.contains("methodName '$'"), "{}", err.message);
  }

  #[test]
  fn groups_whose_files_share_a_stem_are_rejected() {
    let mut list = operation_def("listPets", HttpMethod::Get, "/pets");
    list.tags = vec!["Pet".to_string()];
    let mut get = operation_def("getPet", HttpMethod::Get, "/pets/{id}");
    get.tags = vec!["pet".to_string()];
    let model = api_model(Vec::new(), vec![list, get]);
    // A group rule without a case transform keeps `Pet` and `pet` distinct.
    let resolver = crate::plan::naming::NamingResolver::new(crate::plan::naming::NamingConfig {
      method_name: None,
      group: Some(crate::plan::naming::Naming::Single(
        crate::plan::naming::RuleEntry::Rule(crate::plan::naming::Rule {
          from: Some("{tags[0]}".to_string()),
          parse: None,
          format: None,
          case: None,
        }),
      )),
    });
    for standalone in [false, true] {
      let ctx = test_reporter();
      let err = resolve_service_plans(&model, &resolver, &ctx, standalone)
        .expect_err("both groups plan rest/pet.rest.ts");
      assert_eq!(err.subcode, Some("naming-resolution"));
      assert!(
        err
          .message
          .contains("groups 'Pet' and 'pet' both map to the file rest/pet.rest.ts"),
        "{}",
        err.message
      );
    }
  }

  #[test]
  fn operations_whose_files_share_a_stem_are_rejected() {
    // `delete` and `delete_` are distinct method names that both kebab-case to `delete.ts`.
    let mut operations = vec![
      op_with(
        "delete",
        HttpMethod::Delete,
        "/pets/{id}",
        empty_request(),
        None,
      ),
      op_with(
        "delete_",
        HttpMethod::Post,
        "/pets/purge",
        empty_request(),
        None,
      ),
    ];
    for operation in &mut operations {
      operation.artifact_path = Some(format!(
        "rest/pet/{}.ts",
        crate::plan::naming::operation_file_stem(operation.method_name.as_str())
      ));
    }
    let ctx = test_reporter();
    let err = super::collisions::reject_artifact_path_collisions(
      "pet",
      "rest/pet/index.ts",
      &operations,
      &ctx,
    )
    .expect_err("two operation files at one path");
    assert!(
      err
        .message
        .contains("'delete' (DELETE /pets/{id}, operationId=delete) and 'delete_'"),
      "{}",
      err.message
    );
    assert!(
      err.message.contains("rest/pet/delete.ts"),
      "{}",
      err.message
    );
  }
}
