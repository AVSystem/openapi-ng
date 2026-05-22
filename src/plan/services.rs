//! Planning services: operation grouping, body-layout resolution, and
//! per-operation request-contract construction.

use std::collections::{BTreeSet, HashMap};

use crate::{
  error::{Diagnostic, Reporter},
  ir::{
    canonical::{BodyContent, BodyField, OperationDef, RequestBodyDef},
    schema::SchemaType,
  },
  plan::artifact_plan::{
    PlannedFormField, PlannedHeader, PlannedRequestBody, PlannedRequestContract,
    PlannedRequestField, RequestFieldKind,
  },
};

// ---------------------------------------------------------------------------
// Section 1: Operation grouper
// ---------------------------------------------------------------------------

/// Groups operations by their resolved `group` name. Returns
/// `(group_name, Vec<(operation, method_name)>)` pairs in
/// operation-discovery order; downstream sorting is the planner's job
/// (`resolve_service_plans`). Both `group` and `method_name` are
/// resolved up-front via the `NamingResolver` so each operation is
/// touched exactly once.
pub(crate) fn group_operations<'a>(
  operations: &'a [OperationDef],
  resolver: &crate::plan::naming::NamingResolver,
  reporter: &Reporter<'_>,
) -> Result<Vec<(String, Vec<(&'a OperationDef, String)>)>, Diagnostic> {
  let mut groups = Vec::<(String, Vec<(&'a OperationDef, String)>)>::new();
  let mut group_indexes = HashMap::<String, usize>::new();

  for operation in operations {
    let group_name = resolver.group(operation, reporter)?;
    let method_name = resolver.method_name(operation, reporter)?;

    let group_index = if let Some(index) = group_indexes.get(&group_name) {
      *index
    } else {
      let index = groups.len();
      let key = group_name.clone();
      groups.push((group_name, Vec::new()));
      group_indexes.insert(key, index);
      index
    };

    groups[group_index].1.push((operation, method_name));
  }

  Ok(groups)
}

// ---------------------------------------------------------------------------
// Section 2: Body resolution
// ---------------------------------------------------------------------------

/// Translate an IR `RequestBodyDef` into the planner's `PlannedRequestBody`,
/// applying the smart-flatten rule:
///
/// - Inline JSON `type: object` → `FlatJson` with the body's properties
///   hoisted to top-level request fields. The body envelope's `required`
///   flag is propagated to each hoisted property's `optional` marker so an
///   `required: false` body never produces required fields on the params
///   interface.
/// - JSON `$ref` (named schema) or any non-object JSON shape → `Nested`,
///   preserving the spec author's type as a single `body: T` field.
/// - Form bodies (multipart / urlencoded) always flatten their fields to
///   top-level, since `BodyFieldType` (`Blob | File`, …) can't compose
///   back under the source schema name. Form fields are sorted
///   alphabetically by name so the emitted interface stays stable across
///   spec re-orderings.
fn plan_request_body<'ir>(body: Option<&'ir RequestBodyDef>) -> Option<PlannedRequestBody<'ir>> {
  let body = body?;
  match &body.content {
    BodyContent::Json(SchemaType::InlineObject { properties }) => {
      let envelope_required = body.required;
      let hoisted = properties
        .iter()
        .map(|property| PlannedRequestField {
          name: property.name.clone(),
          optional: !envelope_required || !property.required,
          ty: &property.ty,
          kind: RequestFieldKind::Body,
        })
        .collect();
      Some(PlannedRequestBody::FlatJson {
        properties: hoisted,
        required: envelope_required,
      })
    }
    BodyContent::Json(ty) => Some(PlannedRequestBody::Nested {
      ty,
      optional: !body.required,
    }),
    BodyContent::Multipart { fields, .. } => Some(PlannedRequestBody::Multipart {
      fields: plan_form_fields(fields),
    }),
    BodyContent::UrlEncoded { fields, .. } => Some(PlannedRequestBody::UrlEncoded {
      fields: plan_form_fields(fields),
    }),
  }
}

fn plan_form_fields<'ir>(fields: &'ir [BodyField]) -> Vec<PlannedFormField<'ir>> {
  let mut out: Vec<PlannedFormField<'ir>> = fields
    .iter()
    .map(|f| PlannedFormField {
      name: f.name.clone(),
      optional: !f.required,
      ty: &f.ty,
    })
    .collect();
  out.sort_by(|a, b| a.name.cmp(&b.name));
  out
}

/// Reject a contract whose top-level body field names (from `FlatJson`
/// properties, `Multipart` fields, or `UrlEncoded` fields) clash with the
/// path/query parameter names already on `fields`. Nested-body operations
/// have nothing to check here — their body sits on the dedicated `body`
/// slot under the literal key `body`.
///
/// The diagnostic mirrors the original (pre-smart-flatten) message and
/// nudges the spec author toward the natural escape hatch: hoist the
/// inline body to a top-level `$ref` so it lands on the `body` slot
/// instead of flattening.
fn check_body_field_collisions(
  fields: &[PlannedRequestField],
  body: Option<&PlannedRequestBody>,
  operation_id: &str,
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  let path_query_names: std::collections::BTreeSet<&str> =
    fields.iter().map(|f| f.name.as_ref()).collect();
  if path_query_names.is_empty() {
    return Ok(());
  }
  let body_names: Vec<&str> = match body {
    Some(PlannedRequestBody::FlatJson { properties, .. }) => {
      properties.iter().map(|p| p.name.as_ref()).collect()
    }
    Some(PlannedRequestBody::Multipart { fields })
    | Some(PlannedRequestBody::UrlEncoded { fields }) => {
      fields.iter().map(|f| f.name.as_ref()).collect()
    }
    _ => return Ok(()),
  };
  let colliding: Vec<&str> = body_names
    .into_iter()
    .filter(|n| path_query_names.contains(n))
    .collect();
  if colliding.is_empty() {
    return Ok(());
  }
  let names = colliding.join(", ");
  Err(Diagnostic::policy_violation(
    reporter,
    "field-collision",
    format!(
      "operationId '{operation_id}': body fields [{names}] duplicate path/query parameter names. \
       Rename the colliding fields in the OpenAPI spec, or hoist the body schema to a named `$ref` so it nests under `body`."
    ),
  ))
}

// ---------------------------------------------------------------------------
// Section 3: Request-contract planning
// ---------------------------------------------------------------------------

fn check_path_query_collisions(
  fields: &[PlannedRequestField],
  operation_id: &str,
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  let path_set: BTreeSet<&str> = fields
    .iter()
    .filter(|f| f.kind == RequestFieldKind::Path)
    .map(|f| f.name.as_ref())
    .collect();
  let colliding: Vec<&str> = fields
    .iter()
    .filter(|f| f.kind == RequestFieldKind::Query && path_set.contains(f.name.as_ref()))
    .map(|f| f.name.as_ref())
    .collect();
  if !colliding.is_empty() {
    let names = colliding.join(", ");
    return Err(Diagnostic::policy_violation(
      reporter,
      "field-collision",
      format!(
        "operationId '{operation_id}': path and query parameters share names [{names}], \
         which would produce duplicate fields in the generated request contract. \
         Rename the colliding parameters in the OpenAPI spec."
      ),
    ));
  }
  Ok(())
}

pub(crate) fn plan_request_contract<'ir>(
  operation: &'ir OperationDef,
  reporter: &Reporter<'_>,
) -> Result<PlannedRequestContract<'ir>, Diagnostic> {
  let mut fields: Vec<PlannedRequestField<'ir>> = Vec::new();

  for input in &operation.request.inputs {
    let kind = match input.source {
      crate::ir::canonical::RequestInputSource::Path => RequestFieldKind::Path,
      crate::ir::canonical::RequestInputSource::Query => RequestFieldKind::Query,
    };
    fields.push(PlannedRequestField {
      name: input.name.clone(),
      optional: !input.required,
      ty: &input.ty,
      kind,
    });
  }

  let headers: Vec<PlannedHeader<'ir>> = operation
    .request
    .headers
    .iter()
    .map(|header| PlannedHeader {
      name: header.name.clone(),
      optional: !header.required,
      ty: &header.ty,
    })
    .collect();

  check_path_query_collisions(&fields, &operation.operation_id, reporter)?;

  let body = plan_request_body(operation.request.body.as_ref());

  check_body_field_collisions(&fields, body.as_ref(), &operation.operation_id, reporter)?;

  Ok(PlannedRequestContract {
    fields,
    headers,
    body,
  })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  mod grouper {
    use crate::{
      ir::{
        canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
        schema::{SchemaScalar, SchemaType},
      },
      plan::{naming::NamingResolver, services::group_operations},
      test_support::test_ctx,
    };

    fn operation(id: &str, tags: Vec<&str>) -> OperationDef {
      OperationDef {
        operation_id: id.to_string(),
        tags: tags.into_iter().map(str::to_string).collect(),
        method: HttpMethod::Get,
        path: format!("/{id}"),
        request: RequestDef::default(),
        response: Some(ResponseContent::Json(Some(SchemaType::Scalar(
          SchemaScalar::Boolean,
        )))),
        errors: Vec::new(),
        description: None,
        deprecated: false,
      }
    }

    #[test]
    fn tag_first_operation_grouper_preserves_group_and_operation_discovery_order() {
      let operations = [
        operation("listPets", vec!["Pet"]),
        operation("listAdoptions", vec!["Adoption"]),
        operation("getPet", vec!["Pet"]),
      ];
      let mut ctx = test_ctx();
      let resolver = NamingResolver::default();
      let groups =
        group_operations(&operations, &resolver, &ctx.reporter()).expect("grouping succeeds");

      assert_eq!(
        groups
          .iter()
          .map(|(name, _)| name.as_str())
          .collect::<Vec<_>>(),
        vec!["Pet", "Adoption"]
      );
      assert_eq!(
        groups[0]
          .1
          .iter()
          .map(|(operation, _method_name)| operation.operation_id.as_str())
          .collect::<Vec<_>>(),
        vec!["listPets", "getPet"]
      );
    }

    #[test]
    fn tagless_operations_fall_back_to_path_derived_group_with_default_resolver() {
      // The previous `tag_first_operation_grouper_rejects_tagless_operations`
      // test asserted a policy violation; with the configurable naming
      // engine, the default `group` rule falls back to
      // `pascalCase(pathSegments[0])` when tags are missing.
      let mut ctx = test_ctx();
      let resolver = NamingResolver::default();
      let ops = [operation("listPets", Vec::new())];
      let groups = group_operations(&ops, &resolver, &ctx.reporter())
        .expect("default resolver groups by path segment when tags are absent");
      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].0, "ListPets");
    }
  }

  mod body {
    use super::super::plan_request_body;
    use crate::{
      ir::{
        canonical::{BodyContent, RequestBodyDef},
        schema::{SchemaProperty, SchemaScalar, SchemaType},
      },
      plan::artifact_plan::PlannedRequestBody,
    };

    #[test]
    fn returns_none_when_body_is_absent() {
      assert!(plan_request_body(None).is_none());
    }

    #[test]
    fn ref_body_stays_nested_with_named_schema_preserved() {
      let body = RequestBodyDef {
        required: true,
        content: BodyContent::Json(SchemaType::Ref("CreatePetRequest".into())),
      };
      match plan_request_body(Some(&body)).expect("body present") {
        PlannedRequestBody::Nested { ty, optional } => {
          assert!(!optional);
          assert!(matches!(ty, SchemaType::Ref(name) if name.as_ref() == "CreatePetRequest"));
        }
        other => panic!("expected nested ref body, got {other:?}"),
      }
    }

    #[test]
    fn inline_object_body_hoists_properties_with_required_flag_propagated() {
      let body = RequestBodyDef {
        required: false,
        content: BodyContent::Json(SchemaType::InlineObject {
          properties: vec![SchemaProperty {
            name: "status".into(),
            required: true,
            ty: SchemaType::Scalar(SchemaScalar::String),
            description: None,
            deprecated: false,
          }],
        }),
      };
      match plan_request_body(Some(&body)).expect("body present") {
        PlannedRequestBody::FlatJson {
          properties,
          required,
        } => {
          assert!(!required, "envelope marked optional in fixture");
          assert_eq!(properties.len(), 1);
          assert_eq!(properties[0].name.as_ref(), "status");
          // Required property under an optional envelope ⇒ field is optional.
          assert!(properties[0].optional);
        }
        other => panic!("expected FlatJson, got {other:?}"),
      }
    }

    #[test]
    fn non_object_json_body_stays_nested() {
      let body = RequestBodyDef {
        required: true,
        content: BodyContent::Json(SchemaType::Scalar(SchemaScalar::String)),
      };
      assert!(matches!(
        plan_request_body(Some(&body)),
        Some(PlannedRequestBody::Nested { .. })
      ));
    }
  }

  mod contract {
    use crate::{
      ir::{
        canonical::{
          BodyContent, HeaderDef, HttpMethod, OperationDef, RequestBodyDef, RequestDef,
          RequestInputDef, RequestInputSource,
        },
        schema::{SchemaProperty, SchemaScalar, SchemaType},
      },
      plan::artifact_plan::{PlannedRequestBody, RequestFieldKind},
      plan::services::plan_request_contract,
      test_support::test_ctx,
    };

    #[test]
    fn request_contract_planner_nests_ref_body_under_dedicated_slot() {
      let mut ctx = test_ctx();
      let operation = OperationDef {
        operation_id: "updatePet".to_string(),
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
            content: BodyContent::Json(SchemaType::Ref("UpdatePetPayload".into())),
          }),
        },
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      };
      let request =
        plan_request_contract(&operation, &ctx.reporter()).expect("request contract resolves");

      let path_fields: Vec<&str> = request
        .fields
        .iter()
        .filter(|f| f.kind == RequestFieldKind::Path)
        .map(|f| f.name.as_ref())
        .collect();
      assert_eq!(path_fields, vec!["petId"]);
      match &request.body {
        Some(PlannedRequestBody::Nested { ty, optional }) => {
          assert!(!optional);
          assert!(matches!(ty, SchemaType::Ref(name) if name.as_ref() == "UpdatePetPayload"));
        }
        other => panic!("expected nested ref body, got {other:?}"),
      }
      assert!(request.headers.is_empty());
    }

    #[test]
    fn request_contract_planner_flattens_inline_object_body_to_top_level() {
      // Smart-flatten: an inline `type: object` body is hoisted onto the
      // request interface alongside path/query — call sites match the
      // spec's authorial intent (loose parameter bag, not a named DTO).
      let mut ctx = test_ctx();
      let operation = OperationDef {
        operation_id: "decide".to_string(),
        tags: vec!["AssetCsvImport".to_string()],
        method: HttpMethod::Post,
        path: "/decide".to_string(),
        request: RequestDef {
          inputs: Vec::new(),
          headers: Vec::new(),
          body: Some(RequestBodyDef {
            required: true,
            content: BodyContent::Json(SchemaType::InlineObject {
              properties: vec![
                SchemaProperty {
                  name: "csvImportId".into(),
                  required: true,
                  ty: SchemaType::Ref("CsvImportId".into()),
                  description: None,
                  deprecated: false,
                },
                SchemaProperty {
                  name: "doImport".into(),
                  required: true,
                  ty: SchemaType::Scalar(SchemaScalar::Boolean),
                  description: None,
                  deprecated: false,
                },
              ],
            }),
          }),
        },
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      };
      let request =
        plan_request_contract(&operation, &ctx.reporter()).expect("request contract resolves");

      // Path/query field list stays empty — flattened properties live on
      // the FlatJson variant, not in `fields`.
      assert!(request.fields.is_empty());
      let Some(PlannedRequestBody::FlatJson {
        properties,
        required,
      }) = &request.body
      else {
        panic!("expected FlatJson body, got {:?}", request.body);
      };
      assert!(required, "envelope required in fixture");
      assert_eq!(
        properties
          .iter()
          .map(|p| p.name.as_ref())
          .collect::<Vec<_>>(),
        vec!["csvImportId", "doImport"]
      );
      assert!(properties.iter().all(|p| !p.optional));
    }

    #[test]
    fn request_contract_planner_lifts_headers_into_dedicated_list() {
      let mut ctx = test_ctx();
      let operation = OperationDef {
        operation_id: "tracedGet".to_string(),
        tags: vec!["Pet".to_string()],
        method: HttpMethod::Get,
        path: "/pets".to_string(),
        request: RequestDef {
          inputs: Vec::new(),
          headers: vec![HeaderDef {
            name: "x-trace".into(),
            required: false,
            ty: SchemaType::Scalar(SchemaScalar::String),
          }],
          body: None,
        },
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      };
      let request =
        plan_request_contract(&operation, &ctx.reporter()).expect("request contract resolves");

      assert!(request.fields.is_empty());
      assert_eq!(request.headers.len(), 1);
      assert_eq!(request.headers[0].name.as_ref(), "x-trace");
      assert!(request.headers[0].optional);
    }

    #[test]
    fn request_contract_planner_rejects_flattened_body_property_colliding_with_path_param() {
      // Smart-flatten hoists inline-object body properties to top-level,
      // so a body property named the same as a path parameter would
      // produce a duplicate field on the request interface. The planner
      // rejects the spec; the author can recover by either renaming or
      // by hoisting the body schema to a top-level `$ref` (which nests
      // it under the `body` slot instead).
      let operation = OperationDef {
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
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      };
      let mut ctx = test_ctx();
      let err = plan_request_contract(&operation, &ctx.reporter())
        .expect_err("should fail on flattened body property colliding with path");

      use crate::error::DiagnosticCode;
      assert_eq!(err.code, DiagnosticCode::PolicyViolation);
      assert_eq!(err.subcode, Some("field-collision"));
      assert!(err.message.contains("petId"));
    }

    #[test]
    fn request_contract_planner_accepts_ref_body_property_sharing_path_param_name() {
      // The same property collision is fine when the body is a top-level
      // `$ref` — it nests under `body` rather than flattening, so the
      // property name doesn't appear at the top of the request interface.
      let operation = OperationDef {
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
      };
      let mut ctx = test_ctx();
      let request = plan_request_contract(&operation, &ctx.reporter())
        .expect("ref body nests under `body`, no top-level collision");
      assert!(matches!(
        request.body,
        Some(PlannedRequestBody::Nested { .. })
      ));
    }

    #[test]
    fn request_contract_planner_errors_when_path_and_query_param_share_a_name() {
      let mut ctx = test_ctx();
      let err = plan_request_contract(
        &OperationDef {
          operation_id: "searchUsers".to_string(),
          tags: vec!["User".to_string()],
          method: HttpMethod::Get,
          path: "/users/{id}".to_string(),
          request: RequestDef {
            inputs: vec![
              RequestInputDef {
                name: "id".into(),
                source: RequestInputSource::Path,
                required: true,
                ty: SchemaType::Scalar(SchemaScalar::String),
              },
              RequestInputDef {
                name: "id".into(),
                source: RequestInputSource::Query,
                required: false,
                ty: SchemaType::Scalar(SchemaScalar::String),
              },
            ],
            headers: Vec::new(),
            body: None,
          },
          response: None,
          errors: Vec::new(),
          description: None,
          deprecated: false,
        },
        &ctx.reporter(),
      )
      .expect_err("should fail on path/query collision");

      use crate::error::DiagnosticCode;
      assert_eq!(err.code, DiagnosticCode::PolicyViolation);
      assert!(err.message.contains("id"));
    }
  }

  mod form_body {
    use crate::{
      ir::{
        canonical::{
          ApiInfo, ApiModel, BodyContent, BodyField, BodyFieldType, HttpMethod, ModelSymbol,
          OperationDef, RequestBodyDef, RequestDef, RequestInputDef, RequestInputSource,
        },
        schema::{SchemaScalar, SchemaType},
      },
      plan::{
        artifact_plan::{PlannedRequestBody, resolve_service_plans},
        naming::NamingResolver,
      },
      test_support::test_ctx,
    };

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

    fn multipart_operation(
      operation_id: &str,
      path: &str,
      inputs: Vec<RequestInputDef>,
      body_ref: Option<&str>,
      fields: Vec<BodyField>,
    ) -> OperationDef {
      OperationDef {
        operation_id: operation_id.to_string(),
        tags: vec!["Upload".to_string()],
        method: HttpMethod::Post,
        path: path.to_string(),
        request: RequestDef {
          inputs,
          headers: Vec::new(),
          body: Some(RequestBodyDef {
            required: true,
            content: BodyContent::Multipart {
              body_ref: body_ref.map(Box::from),
              fields,
            },
          }),
        },
        response: None,
        errors: Vec::new(),
        description: None,
        deprecated: false,
      }
    }

    fn api_model_with_multipart_op() -> ApiModel {
      api_model(
        Vec::new(),
        vec![multipart_operation(
          "uploadAvatar",
          "/avatar",
          Vec::new(),
          None,
          vec![
            BodyField {
              name: "avatar".into(),
              required: true,
              ty: BodyFieldType::Binary,
            },
            BodyField {
              name: "caption".into(),
              required: false,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
          ],
        )],
      )
    }

    fn api_model_with_multipart_unsorted_fields() -> ApiModel {
      api_model(
        Vec::new(),
        vec![multipart_operation(
          "uploadAssets",
          "/assets",
          Vec::new(),
          None,
          vec![
            BodyField {
              name: "zeta".into(),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
            BodyField {
              name: "alpha".into(),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
            BodyField {
              name: "mu".into(),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
          ],
        )],
      )
    }

    fn api_model_with_form_collision() -> ApiModel {
      api_model(
        Vec::new(),
        vec![multipart_operation(
          "uploadByFileName",
          "/files/{fileName}",
          vec![RequestInputDef {
            name: "fileName".into(),
            source: RequestInputSource::Path,
            required: true,
            ty: SchemaType::Scalar(SchemaScalar::String),
          }],
          None,
          vec![
            BodyField {
              name: "fileName".into(),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
            BodyField {
              name: "blob".into(),
              required: true,
              ty: BodyFieldType::Binary,
            },
          ],
        )],
      )
    }

    fn api_model_with_multipart_ref_body(body_ref: &str) -> ApiModel {
      api_model(
        Vec::new(),
        vec![multipart_operation(
          "uploadForm",
          "/form",
          Vec::new(),
          Some(body_ref),
          vec![BodyField {
            name: "file".into(),
            required: true,
            ty: BodyFieldType::Binary,
          }],
        )],
      )
    }

    #[test]
    fn plans_multipart_body_with_fields_hoisted_to_form_collection() {
      let ir = api_model_with_multipart_op();
      let mut ctx = test_ctx();
      let services =
        resolve_service_plans(&ir, &NamingResolver::default(), &ctx.reporter()).expect("ok");
      let op = &services[0].operations[0];
      match &op.request.body {
        Some(PlannedRequestBody::Multipart { fields }) => {
          assert!(fields.iter().any(|f| f.name.as_ref() == "avatar"));
        }
        other => panic!("expected multipart body, got {other:?}"),
      }
      // Path/query field list stays empty in this fixture; form fields
      // hoist to top-level via the body slot, not via `fields`.
      assert!(op.request.fields.is_empty());
    }

    #[test]
    fn plans_form_fields_sorted_alphabetically() {
      let ir = api_model_with_multipart_unsorted_fields();
      let mut ctx = test_ctx();
      let services =
        resolve_service_plans(&ir, &NamingResolver::default(), &ctx.reporter()).expect("ok");
      let Some(PlannedRequestBody::Multipart { fields }) = &services[0].operations[0].request.body
      else {
        panic!("expected multipart body");
      };
      let names: Vec<&str> = fields.iter().map(|f| f.name.as_ref()).collect();
      let mut sorted = names.clone();
      sorted.sort_unstable();
      assert_eq!(names, sorted);
    }

    #[test]
    fn form_field_name_collision_with_path_param_emits_field_collision() {
      // Path has {fileName} and the multipart body has a `fileName` field;
      // smart-flatten hoists form fields to top-level so the duplicate
      // surfaces on the request interface — reject at planning time.
      let ir = api_model_with_form_collision();
      let mut ctx = test_ctx();
      let err = resolve_service_plans(&ir, &NamingResolver::default(), &ctx.reporter())
        .expect_err("hoisted form fields collide with path param");
      assert_eq!(err.subcode, Some("field-collision"));
      assert!(err.message.contains("fileName"));
    }

    #[test]
    fn multipart_ref_body_still_flattens_fields_under_smart_rule() {
      // Even when the multipart body carries a named source schema, we
      // can't render the schema's name as a TS type — `BodyFieldType`
      // (Blob | File, …) does not compose into the source `SchemaType`.
      // So multipart bodies always flatten regardless of `body_ref`.
      let ir = api_model_with_multipart_ref_body("UploadForm");
      let mut ctx = test_ctx();
      let services =
        resolve_service_plans(&ir, &NamingResolver::default(), &ctx.reporter()).expect("ok");
      assert!(matches!(
        services[0].operations[0].request.body,
        Some(PlannedRequestBody::Multipart { .. })
      ));
    }
  }
}
