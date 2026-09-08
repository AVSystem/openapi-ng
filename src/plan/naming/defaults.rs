//! The naming applied when the caller configured no rule for a key.

use crate::plan::naming::{case::apply as apply_case, config::Case, context::OperationContext};

/// The method name has no source: neither an `operationId` nor a usable
/// path segment.
#[derive(Debug)]
pub(crate) struct NoMethodNameSource;

/// camelCase of `operationId`; failing that, camelCase of the method joined
/// with the path segments.
pub(crate) fn default_method_name(
  ctx: &OperationContext<'_>,
) -> Result<String, NoMethodNameSource> {
  if let Some(id) = ctx.operation_id() {
    return Ok(apply_case(id, Case::Camel));
  }
  let segments = ctx.path_segments_joined();
  if segments.is_empty() {
    return Err(NoMethodNameSource);
  }
  Ok(apply_case(
    &format!("{}_{segments}", ctx.method()),
    Case::Camel,
  ))
}

/// PascalCase of the first tag; failing that, of the first path segment;
/// failing that, `Default`.
pub(crate) fn default_group(ctx: &OperationContext<'_>) -> String {
  ctx
    .tags()
    .first()
    .map(String::as_str)
    .or_else(|| ctx.lookup_indexed("pathSegments", 0))
    .filter(|source| !source.is_empty())
    .map_or_else(
      || "Default".to_string(),
      |source| apply_case(source, Case::Pascal),
    )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api_model::{
    canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
    schema::{SchemaScalar, SchemaType},
  };

  fn op(id: &str, method: HttpMethod, path: &str, tags: &[&str]) -> OperationDef {
    OperationDef {
      operation_id: id.to_string(),
      tags: tags.iter().map(ToString::to_string).collect(),
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
    assert!(matches!(default_method_name(&ctx), Err(NoMethodNameSource)));
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
