mod imports;
mod request;
mod service;

pub(crate) use service::emit_service;

pub(crate) const REST_MODEL_PATH: &str = "rest.model.ts";
pub(crate) const REST_UTIL_PATH: &str = "rest.util.ts";
pub(crate) const REST_VALIDATE_PATH: &str = "rest.validate.ts";
pub(crate) const REST_MODEL_TEMPLATE: &str =
  include_str!("../../../templates/angular/rest.model.ts");
pub(crate) const REST_UTIL_TEMPLATE: &str = include_str!("../../../templates/angular/rest.util.ts");
pub(crate) const REST_VALIDATE_TEMPLATE: &str =
  include_str!("../../../templates/angular/rest.validate.ts");

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api_model::canonical::HttpMethod;
  use crate::api_model::schema::{SchemaScalar, SchemaType};
  use crate::identifier::TypeName;
  use crate::plan::artifact_plan::{
    PlannedRequestContract, PlannedRequestField, RequestFieldKind, ServicePlan,
  };
  use crate::test_support::{empty_request, op_with};

  #[test]
  fn rest_model_template_carries_common_request_definitions() {
    assert!(REST_MODEL_TEMPLATE.contains("CommonRequest"));
  }

  #[test]
  fn rest_util_template_carries_request_factory_helpers() {
    assert!(REST_UTIL_TEMPLATE.contains("requestFactory"));
  }

  #[test]
  fn rest_validate_template_carries_validate_rest_export() {
    assert!(REST_VALIDATE_TEMPLATE.contains("export function validateRest"));
  }

  #[test]
  fn emit_service_generates_injectable_class_with_operation_property() {
    let plan = ServicePlan {
      group_name: "pet".into(),
      class_name: TypeName::new("PetRest".to_string()),
      artifact_path: "rest/pet.rest.generated.ts".to_string(),
      operations: vec![op_with(
        "listPets",
        HttpMethod::Get,
        "/pets",
        empty_request(),
        None,
      )],
    };
    let content = emit_service(&plan);
    assert!(content.contains("@Injectable("));
    assert!(content.contains("export class PetRest"));
    assert!(content.contains("requestFactory"));
    assert!(content.contains("listPets"));
  }

  #[test]
  fn emit_service_includes_request_interface_when_operation_has_input_fields() {
    let schema = SchemaType::Scalar(SchemaScalar::String);
    let plan = ServicePlan {
      group_name: "pet".into(),
      class_name: TypeName::new("PetRest".to_string()),
      artifact_path: "rest/pet.rest.generated.ts".to_string(),
      operations: vec![op_with(
        "updatePet",
        HttpMethod::Put,
        "/pets/{id}",
        PlannedRequestContract {
          fields: vec![PlannedRequestField {
            name: "id".into(),
            optional: false,
            schema: &schema,
            kind: RequestFieldKind::Path,
          }],
          headers: vec![],
          body: None,
        },
        None,
      )],
    };
    let content = emit_service(&plan);
    assert!(content.contains("export interface UpdatePetParams"));
    assert!(content.contains("UpdatePetParams"));
    assert!(content.contains("id:"));
  }
}
