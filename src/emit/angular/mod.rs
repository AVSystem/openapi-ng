mod imports;
mod request;
mod service;

pub(crate) use service::emit_service;

pub(crate) const REST_MODEL_PATH: &str = "rest.model.ts";
pub(crate) const REST_UTIL_PATH: &str = "rest.util.ts";
pub(crate) const REST_MODEL_TEMPLATE: &str =
  include_str!("../../../templates/angular/rest.model.ts");
pub(crate) const REST_UTIL_TEMPLATE: &str = include_str!("../../../templates/angular/rest.util.ts");

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::canonical::HttpMethod;
  use crate::ir::schema::{SchemaScalar, SchemaType};
  use crate::plan::artifact_plan::{
    PlannedOperation, PlannedRequestContract, PlannedRequestField, RequestFieldKind, ServicePlan,
  };
  use crate::test_support::empty_request;

  #[test]
  fn rest_model_template_carries_common_request_definitions() {
    assert!(REST_MODEL_TEMPLATE.contains("CommonRequest"));
  }

  #[test]
  fn rest_util_template_carries_request_factory_helpers() {
    assert!(REST_UTIL_TEMPLATE.contains("requestFactory"));
  }

  #[test]
  fn emit_service_generates_injectable_class_with_operation_property() {
    let plan = ServicePlan {
      group_name: "pet".into(),
      class_name: "PetRest".into(),
      artifact_path: "rest/pet.rest.generated.ts".to_string(),
      operations: vec![PlannedOperation {
        operation_id: "listPets".to_string(),
        method_name: "listPets".to_string(),
        method: HttpMethod::Get,
        path: "/pets".to_string(),
        request: empty_request(),
        response: None,
        errors: &[],
        description: None,
        deprecated: false,
      }],
    };
    let content = emit_service(&plan);
    assert!(content.contains("@Injectable("));
    assert!(content.contains("export class PetRest"));
    assert!(content.contains("requestFactory"));
    assert!(content.contains("listPets"));
  }

  #[test]
  fn emit_service_includes_request_interface_when_operation_has_input_fields() {
    let ty = SchemaType::Scalar(SchemaScalar::String);
    let plan = ServicePlan {
      group_name: "pet".into(),
      class_name: "PetRest".into(),
      artifact_path: "rest/pet.rest.generated.ts".to_string(),
      operations: vec![PlannedOperation {
        operation_id: "updatePet".to_string(),
        method_name: "updatePet".to_string(),
        method: HttpMethod::Put,
        path: "/pets/{id}".to_string(),
        request: PlannedRequestContract {
          fields: vec![PlannedRequestField {
            name: "id".into(),
            optional: false,
            ty: &ty,
            kind: RequestFieldKind::Path,
          }],
          headers: vec![],
          body: None,
        },
        response: None,
        errors: &[],
        description: None,
        deprecated: false,
      }],
    };
    let content = emit_service(&plan);
    assert!(content.contains("export interface UpdatePetParams"));
    assert!(content.contains("UpdatePetParams"));
    assert!(content.contains("id:"));
  }
}
