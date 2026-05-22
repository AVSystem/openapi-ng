//! Hardcoded defaults — applied when the user did not configure a
//! `Naming` for a given key. The spec says these are NOT expressed as
//! `Rule` chains, so they live as plain Rust here.
//!
//! Defaults:
//! * methodName: camelCase(operationId), else camelCase(method + '_' + path segments joined by `_`).
//!   Errors if both fail.
//! * group: pascalCase(tags[0]), else pascalCase(pathSegments[0]), else "Default".

use crate::plan::naming::{
  case::apply as apply_case, config::Case, context::OperationContext,
};

#[derive(Debug)]
pub(crate) enum DefaultMethodNameFailure {
  /// Neither operationId nor a usable path-segment fallback was available.
  NoSource,
}

pub(crate) fn default_method_name(
  ctx: &OperationContext<'_>,
) -> Result<String, DefaultMethodNameFailure> {
  if let Some(id) = ctx.operation_id
    && !id.is_empty()
  {
    return Ok(apply_case(id, Case::Camel));
  }
  if !ctx.path_segments.is_empty() {
    let suffix = ctx.path_segments.join("_");
    return Ok(apply_case(&format!("{}_{}", ctx.method, suffix), Case::Camel));
  }
  Err(DefaultMethodNameFailure::NoSource)
}

pub(crate) fn default_group(ctx: &OperationContext<'_>) -> String {
  if let Some(tag) = ctx.tags.first()
    && !tag.is_empty()
  {
    return apply_case(tag, Case::Pascal);
  }
  if let Some(segment) = ctx.path_segments.first()
    && !segment.is_empty()
  {
    return apply_case(segment, Case::Pascal);
  }
  "Default".to_string()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::{
    canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
    schema::{SchemaScalar, SchemaType},
  };

  fn op(id: &str, method: HttpMethod, path: &str, tags: &[&str]) -> OperationDef {
    OperationDef {
      operation_id: id.to_string(),
      tags: tags.iter().map(|s| s.to_string()).collect(),
      method,
      path: path.to_string(),
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
  fn default_method_name_uses_camel_case_of_operation_id_when_present() {
    let operation = op("list_pets", HttpMethod::Get, "/pets", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(default_method_name(&ctx).unwrap(), "listPets");
  }

  #[test]
  fn default_method_name_falls_back_to_method_plus_path_when_operation_id_missing() {
    let operation = op("", HttpMethod::Get, "/users/{id}/posts", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(default_method_name(&ctx).unwrap(), "getUsersIdPosts");
  }

  #[test]
  fn default_method_name_errors_when_no_operation_id_and_path_is_empty() {
    let operation = op("", HttpMethod::Get, "/", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert!(matches!(
      default_method_name(&ctx),
      Err(DefaultMethodNameFailure::NoSource)
    ));
  }

  #[test]
  fn default_group_uses_pascal_case_of_first_tag_when_present() {
    let operation = op("x", HttpMethod::Get, "/pets", &["pet-orders"]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(default_group(&ctx), "PetOrders");
  }

  #[test]
  fn default_group_falls_back_to_path_segment_when_tags_are_empty() {
    let operation = op("x", HttpMethod::Get, "/users/{id}", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(default_group(&ctx), "Users");
  }

  #[test]
  fn default_group_returns_default_when_both_sources_are_missing() {
    let operation = op("x", HttpMethod::Get, "/", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(default_group(&ctx), "Default");
  }
}
