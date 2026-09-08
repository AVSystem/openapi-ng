//! Request-body layout.

use crate::{
  error::{Diagnostic, Reporter},
  ir::{
    canonical::{BodyContent, BodyField, RequestBodyDef},
    schema::SchemaType,
  },
  plan::artifact_plan::{
    PlannedFormField, PlannedRequestBody, PlannedRequestField, RequestFieldKind,
  },
};

/// Chooses a body's layout:
///
/// - an inline JSON object → `FlatJson`, its properties hoisted and each
///   `optional` folding in the envelope's `required`;
/// - any other JSON shape → `Nested`, under one `body` key;
/// - a form body → `Multipart` / `UrlEncoded`, its fields hoisted and
///   sorted by name.
pub(super) fn plan_request_body<'ir>(
  body: Option<&'ir RequestBodyDef>,
) -> Option<PlannedRequestBody<'ir>> {
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
    .map(|field| PlannedFormField {
      name: field.name.clone(),
      optional: !field.required,
      ty: &field.ty,
    })
    .collect();
  out.sort_by(|a, b| a.name.cmp(&b.name));
  out
}

/// Fails when a hoisted body field name clashes with a path or query
/// parameter already on `fields`.
///
/// A nested body has nothing to clash: it occupies the single `body` key.
pub(super) fn check_body_field_collisions(
  fields: &[PlannedRequestField],
  body: Option<&PlannedRequestBody>,
  operation_id: &str,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  let path_query_names: std::collections::BTreeSet<&str> =
    fields.iter().map(|field| field.name.as_ref()).collect();
  if path_query_names.is_empty() {
    return Ok(());
  }
  let body_names: Vec<&str> = match body {
    Some(PlannedRequestBody::FlatJson { properties, .. }) => {
      properties.iter().map(|p| p.name.as_ref()).collect()
    }
    Some(PlannedRequestBody::Multipart { fields } | PlannedRequestBody::UrlEncoded { fields }) => {
      fields.iter().map(|field| field.name.as_str()).collect()
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

#[cfg(test)]
mod tests {
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
      test_support::test_reporter,
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
              name: crate::ident::Ident::parse("avatar").expect("identifier"),
              required: true,
              ty: BodyFieldType::Binary,
            },
            BodyField {
              name: crate::ident::Ident::parse("caption").expect("identifier"),
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
              name: crate::ident::Ident::parse("zeta").expect("identifier"),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
            BodyField {
              name: crate::ident::Ident::parse("alpha").expect("identifier"),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
            BodyField {
              name: crate::ident::Ident::parse("mu").expect("identifier"),
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
              name: crate::ident::Ident::parse("fileName").expect("identifier"),
              required: true,
              ty: BodyFieldType::Scalar(SchemaScalar::String),
            },
            BodyField {
              name: crate::ident::Ident::parse("blob").expect("identifier"),
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
            name: crate::ident::Ident::parse("file").expect("identifier"),
            required: true,
            ty: BodyFieldType::Binary,
          }],
        )],
      )
    }

    #[test]
    fn plans_multipart_body_with_fields_hoisted_to_form_collection() {
      let ir = api_model_with_multipart_op();
      let ctx = test_reporter();
      let services = resolve_service_plans(&ir, &NamingResolver::default(), &ctx).expect("ok");
      let op = &services[0].operations[0];
      match &op.request.body {
        Some(PlannedRequestBody::Multipart { fields }) => {
          assert!(fields.iter().any(|f| f.name.as_str() == "avatar"));
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
      let ctx = test_reporter();
      let services = resolve_service_plans(&ir, &NamingResolver::default(), &ctx).expect("ok");
      let Some(PlannedRequestBody::Multipart { fields }) = &services[0].operations[0].request.body
      else {
        panic!("expected multipart body");
      };
      let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
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
      let ctx = test_reporter();
      let err = resolve_service_plans(&ir, &NamingResolver::default(), &ctx)
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
      let ctx = test_reporter();
      let services = resolve_service_plans(&ir, &NamingResolver::default(), &ctx).expect("ok");
      assert!(matches!(
        services[0].operations[0].request.body,
        Some(PlannedRequestBody::Multipart { .. })
      ));
    }
  }
}
