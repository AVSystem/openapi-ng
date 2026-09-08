//! Per-operation request-contract planning.
//!
//! `plan_request_contract` is the entry point; the submodules own the
//! grouping and the body layout it composes.

mod body;
mod grouping;

use std::collections::BTreeSet;

use crate::{
  error::{Diagnostic, Reporter, bail_policy},
  ir::canonical::{OperationDef, RequestInputSource},
  plan::artifact_plan::{
    PlannedHeader, PlannedRequestContract, PlannedRequestField, RequestFieldKind,
  },
};

pub(crate) use grouping::group_operations;

use body::{check_body_field_collisions, plan_request_body};

fn check_path_query_collisions(
  fields: &[PlannedRequestField],
  operation_id: &str,
  reporter: &Reporter,
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
    bail_policy!(
      reporter,
      "field-collision",
      "operationId '{operation_id}': path and query parameters share names [{names}], \
         which would produce duplicate fields in the generated request contract. \
         Rename the colliding parameters in the OpenAPI spec."
    );
  }
  Ok(())
}

pub(crate) fn plan_request_contract<'ir>(
  operation: &'ir OperationDef,
  reporter: &Reporter,
) -> Result<PlannedRequestContract<'ir>, Diagnostic> {
  let fields: Vec<PlannedRequestField<'ir>> = operation
    .request
    .inputs
    .iter()
    .map(|input| PlannedRequestField {
      name: input.name.clone(),
      optional: !input.required,
      ty: &input.ty,
      kind: match input.source {
        RequestInputSource::Path => RequestFieldKind::Path,
        RequestInputSource::Query => RequestFieldKind::Query,
      },
    })
    .collect();

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

#[cfg(test)]
mod tests {
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
      test_support::test_reporter,
    };

    #[test]
    fn request_contract_planner_nests_ref_body_under_dedicated_slot() {
      let ctx = test_reporter();
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
      let request = plan_request_contract(&operation, &ctx).expect("request contract resolves");

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
      let ctx = test_reporter();
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
      let request = plan_request_contract(&operation, &ctx).expect("request contract resolves");

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
      let ctx = test_reporter();
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
      let request = plan_request_contract(&operation, &ctx).expect("request contract resolves");

      assert!(request.fields.is_empty());
      assert_eq!(request.headers.len(), 1);
      assert_eq!(request.headers[0].name.as_ref(), "x-trace");
      assert!(request.headers[0].optional);
    }

    #[test]
    fn request_contract_planner_rejects_flattened_body_property_colliding_with_path_param() {
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
      let ctx = test_reporter();
      let err = plan_request_contract(&operation, &ctx)
        .expect_err("should fail on flattened body property colliding with path");

      use crate::error::DiagnosticCode;
      assert_eq!(err.code, DiagnosticCode::PolicyViolation);
      assert_eq!(err.subcode, Some("field-collision"));
      assert!(err.message.contains("petId"));
    }

    #[test]
    fn request_contract_planner_accepts_ref_body_property_sharing_path_param_name() {
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
      let ctx = test_reporter();
      let request = plan_request_contract(&operation, &ctx)
        .expect("ref body nests under `body`, no top-level collision");
      assert!(matches!(
        request.body,
        Some(PlannedRequestBody::Nested { .. })
      ));
    }

    #[test]
    fn request_contract_planner_errors_when_path_and_query_param_share_a_name() {
      let ctx = test_reporter();
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
        &ctx,
      )
      .expect_err("should fail on path/query collision");

      use crate::error::DiagnosticCode;
      assert_eq!(err.code, DiagnosticCode::PolicyViolation);
      assert!(err.message.contains("id"));
    }
  }
}
