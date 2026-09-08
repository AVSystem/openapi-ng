use std::collections::BTreeMap;

use crate::{
  error::{Diagnostic, DiagnosticCode, Reporter},
  ident::{Ident, MethodName, TypeName},
  ir::canonical::{
    ApiModel, BodyFieldType, ErrorResponse, HttpMethod, ModelSymbol, ResponseContent,
  },
  ir::schema::SchemaType,
  options::MappedType,
};

use super::{
  naming::{error_interface_name, request_interface_name, service_class_name, service_file_stem},
  services::plan_request_contract,
};

/// A [`MappedType`] whose `schema` was found in the IR, borrowed from the
/// model symbol that matched.
///
/// Only [`validate_mapped_types_against_schemas`] constructs one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedMappedType<'a> {
  pub(crate) schema: &'a str,
  pub(crate) import: Box<str>,
  pub(crate) ty: Box<str>,
  pub(crate) alias: Option<Box<str>>,
}

impl<'a> ResolvedMappedType<'a> {
  pub(crate) fn new(schema: &'a str, source: &MappedType) -> Self {
    Self {
      schema,
      import: Box::from(source.import.as_str()),
      ty: Box::from(source.ty.as_str()),
      alias: source.alias.as_deref().map(Box::from),
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ServicePlan<'ir> {
  pub(crate) group_name: String,
  pub(crate) class_name: TypeName,
  pub(crate) artifact_path: String,
  pub(crate) operations: Vec<PlannedOperation<'ir>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedOperation<'ir> {
  pub(crate) operation_id: String,
  pub(crate) method_name: MethodName,
  pub(crate) method: HttpMethod,
  pub(crate) path: String,
  pub(crate) request: PlannedRequestContract<'ir>,
  pub(crate) response: Option<&'ir ResponseContent>,
  /// The operation's typed error responses, empty when it declared
  /// none.
  pub(crate) errors: &'ir [ErrorResponse],
  /// Name of the `{Pascal}Params` interface, or `None` when the operation
  /// declares no path, query, header or body input and so emits none.
  pub(crate) request_interface: Option<TypeName>,
  /// Name of the `{Pascal}Error` interface, or `None` when the operation
  /// declares no 4xx/5xx response with a JSON schema.
  pub(crate) error_interface: Option<TypeName>,
  pub(crate) description: Option<String>,
  pub(crate) deprecated: bool,
}

/// Which slot of the HTTP request a [`PlannedRequestField`] fills.
///
/// `Body` marks a property hoisted out of an inline JSON body; a nested
/// body has no fields of this kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RequestFieldKind {
  Path,
  Query,
  Body,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedRequestContract<'ir> {
  /// Path and query parameters. A hoisted body property lives on
  /// [`PlannedRequestBody::FlatJson`], not here.
  pub(crate) fields: Vec<PlannedRequestField<'ir>>,
  /// Header parameters, empty when the operation declares none.
  pub(crate) headers: Vec<PlannedHeader<'ir>>,
  /// The body's layout, `None` when the operation declares no body.
  pub(crate) body: Option<PlannedRequestBody<'ir>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedRequestField<'ir> {
  pub(crate) name: Box<str>,
  pub(crate) optional: bool,
  pub(crate) ty: &'ir SchemaType,
  pub(crate) kind: RequestFieldKind,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedHeader<'ir> {
  pub(crate) name: Box<str>,
  pub(crate) optional: bool,
  pub(crate) ty: &'ir SchemaType,
}

/// One field of a multipart or urlencoded body.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlannedFormField<'ir> {
  pub(crate) name: Ident,
  pub(crate) optional: bool,
  pub(crate) ty: &'ir BodyFieldType,
}

/// How a request body is laid out on the request contract.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PlannedRequestBody<'ir> {
  /// A JSON body with no properties to hoist: a top-level `$ref`, or a
  /// scalar, array or union. Keeps the spec author's type under one
  /// `body` key.
  Nested { ty: &'ir SchemaType, optional: bool },
  /// An inline JSON object body, its properties hoisted to top level.
  ///
  /// Each `optional` already folds in the envelope's `required`: under a
  /// `required: false` body every property is optional.
  FlatJson {
    properties: Vec<PlannedRequestField<'ir>>,
    required: bool,
  },
  /// A `multipart/form-data` body, its fields hoisted to top level.
  Multipart { fields: Vec<PlannedFormField<'ir>> },
  /// An `application/x-www-form-urlencoded` body, its fields hoisted to
  /// top level.
  UrlEncoded { fields: Vec<PlannedFormField<'ir>> },
}

/// Resolves each mapped type against `model_symbols`, failing on the
/// first `schema` the IR does not declare.
pub(crate) fn validate_mapped_types_against_schemas<'ir>(
  model_symbols: &'ir [ModelSymbol],
  mapped_types: &[MappedType],
  reporter: &Reporter,
) -> Result<Vec<ResolvedMappedType<'ir>>, Diagnostic> {
  let by_name = model_symbols
    .iter()
    .map(|symbol| (symbol.name.as_ref(), symbol))
    .collect::<BTreeMap<&str, &ModelSymbol>>();

  mapped_types
    .iter()
    .map(|mapped_type| {
      let symbol = by_name.get(mapped_type.schema.as_str()).ok_or_else(|| {
        reporter.error(
          DiagnosticCode::InvalidOption,
          format!(
            "Failed to resolve generation options: mapped schema {} does not exist in the IR.",
            mapped_type.schema
          ),
        )
      })?;
      Ok(ResolvedMappedType::new(symbol.name.as_ref(), mapped_type))
    })
    .collect()
}

pub(crate) fn resolve_service_plans<'ir>(
  ir: &'ir ApiModel,
  resolver: &crate::plan::naming::NamingResolver,
  reporter: &Reporter,
) -> Result<Vec<ServicePlan<'ir>>, Diagnostic> {
  use super::services::group_operations;

  let mut services = group_operations(&ir.operations, resolver, reporter)?
    .into_iter()
    .map(|(group_name, group)| {
      let mut operations = group
        .into_iter()
        .map(|(operation, method_name)| plan_operation(operation, method_name, reporter))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
      operations.sort_by(|left, right| left.method_name.cmp(&right.method_name));

      Ok(ServicePlan {
        class_name: service_class_name(&group_name),
        artifact_path: format!("rest/{}.rest.generated.ts", service_file_stem(&group_name)),
        group_name,
        operations,
      })
    })
    .collect::<Result<Vec<_>, Diagnostic>>()?;
  services.sort_by(|left, right| left.class_name.cmp(&right.class_name));

  Ok(services)
}

fn plan_operation<'ir>(
  operation: &'ir crate::ir::canonical::OperationDef,
  method_name: MethodName,
  reporter: &Reporter,
) -> Result<PlannedOperation<'ir>, Diagnostic> {
  let request = plan_request_contract(operation, reporter)?;
  Ok(PlannedOperation {
    operation_id: operation.operation_id.clone(),
    request_interface: takes_input(&request).then(|| request_interface_name(&method_name)),
    error_interface: (!operation.errors.is_empty()).then(|| error_interface_name(&method_name)),
    method_name,
    method: operation.method,
    path: operation.path.clone(),
    request,
    response: operation.response.as_ref(),
    errors: operation.errors.as_slice(),
    description: operation.description.clone(),
    deprecated: operation.deprecated,
  })
}

/// True when the operation declares any path, query, header or body
/// input.
const fn takes_input(request: &PlannedRequestContract<'_>) -> bool {
  !request.fields.is_empty() || request.body.is_some() || !request.headers.is_empty()
}

#[cfg(test)]
mod tests {
  use super::{
    PlannedFormField, PlannedRequestBody, PlannedRequestContract, RequestFieldKind,
    resolve_service_plans, validate_mapped_types_against_schemas,
  };
  use crate::{
    ir::{
      canonical::{
        ApiInfo, ApiModel, BodyContent, BodyFieldType, HttpMethod, ModelSymbol, OperationDef,
        RequestBodyDef, RequestDef, RequestInputDef, RequestInputSource, ResponseContent,
      },
      schema::{SchemaProperty, SchemaScalar, SchemaType},
    },
    options::MappedType,
  };

  use crate::test_support::test_reporter;

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
              ty: SchemaType::Ref("PetStatus".into()),
              description: None,
              deprecated: false,
            },
            SchemaProperty {
              name: "tagIds".into(),
              required: true,
              ty: SchemaType::Array(Box::new(SchemaType::Scalar(SchemaScalar::Number))),
              description: None,
              deprecated: false,
            },
            SchemaProperty {
              name: "nickname".into(),
              required: false,
              ty: SchemaType::Nullable(Box::new(SchemaType::Scalar(SchemaScalar::String))),
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
              ty: SchemaType::Ref("PetId".into()),
            },
            RequestInputDef {
              name: "includeHistory".into(),
              source: RequestInputSource::Query,
              required: false,
              ty: SchemaType::Scalar(SchemaScalar::Boolean),
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
                ty: SchemaType::Ref("PetId".into()),
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
        ty: "ExternalUserId".to_string(),
        alias: Some("UserId".to_string()),
      }],
      &ctx,
    )
    .expect("mapped types validate against IR");

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].schema, "UserId");
    assert_eq!(resolved[0].import.as_ref(), "./shared/user-id");
    assert_eq!(resolved[0].ty.as_ref(), "ExternalUserId");
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
        ty: "Missing".to_string(),
        alias: None,
      }],
      &ctx,
    )
    .expect_err("missing schema should fail validation");

    assert_eq!(err.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(err.message.contains("Missing"));
  }

  #[test]
  fn resolve_service_plans_groups_operations_and_builds_request_contracts() {
    let ir = service_test_ir();
    let ctx = test_reporter();
    let services =
      resolve_service_plans(&ir, &crate::plan::naming::NamingResolver::default(), &ctx)
        .expect("service plan resolves");

    assert_eq!(services.len(), 2);
    // Services are sorted alphabetically by class_name (AdoptionRequestRest
    // sorts before PetRest), regardless of the discovery order in the spec.
    assert_eq!(
      services
        .iter()
        .map(|service| service.group_name.as_str())
        .collect::<Vec<_>>(),
      vec!["AdoptionRequest", "Pet"]
    );

    let pet_service = &services[1];
    assert_eq!(pet_service.class_name.to_string(), "PetRest");
    assert_eq!(pet_service.artifact_path, "rest/pet.rest.generated.ts");
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
      Some(PlannedRequestBody::Nested { ty, optional }) => {
        assert!(!optional, "body marked required in fixture");
        assert!(
          matches!(ty, SchemaType::Ref(name) if name.as_ref() == "UpdatePetPayload"),
          "expected body ty to remain the ref, got {ty:?}"
        );
      }
      other => panic!("expected nested ref body, got {other:?}"),
    }
  }

  #[test]
  fn resolve_service_plans_keeps_ref_bodies_nested_under_smart_flatten() {
    // Smart-flatten preserves a body that's authored as a `$ref` even when
    // that ref resolves to an `InlineObject` schema — the spec author's
    // named type is the signal we honor.
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
            ty: SchemaType::Ref("PetId".into()),
            description: None,
            deprecated: false,
          }],
        },
      },
    ];
    let ir = api_model(
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
            ty: SchemaType::Ref("PetId".into()),
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
    let services =
      resolve_service_plans(&ir, &crate::plan::naming::NamingResolver::default(), &ctx)
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
    let ir = api_model(Vec::new(), operations);

    let ctx = test_reporter();
    let services =
      resolve_service_plans(&ir, &crate::plan::naming::NamingResolver::default(), &ctx)
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
        .map(|op| op.operation_id.as_str())
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
        .map(|op| op.operation_id.as_str())
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
          name: crate::ident::Ident::parse("status").expect("identifier"),
          optional: false,
          ty: &scalar,
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
    let ty = SchemaType::Scalar(SchemaScalar::String);
    let _: PlannedRequestBody<'_> = PlannedRequestBody::Nested {
      ty: &ty,
      optional: false,
    };
    let _: PlannedRequestBody<'_> = PlannedRequestBody::FlatJson {
      properties: vec![],
      required: true,
    };
    let _: PlannedRequestBody<'_> = PlannedRequestBody::Multipart { fields: vec![] };
    let _: PlannedRequestBody<'_> = PlannedRequestBody::UrlEncoded { fields: vec![] };
  }
}
