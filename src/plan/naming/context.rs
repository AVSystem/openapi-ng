//! The read-only values a naming rule's template can reference for one
//! operation.

use std::collections::BTreeMap;

use crate::api_model::canonical::OperationDef;

#[derive(Debug)]
pub(crate) struct OperationContext<'a> {
  operation_id: Option<&'a str>,
  /// Lower-case method name, as the spec writes it.
  method: &'static str,
  path: &'a str,
  /// Path split on `/`, empty segments dropped, `{name}` unwrapped to
  /// `name`.
  path_segments: Vec<&'a str>,
  tags: &'a [String],
  /// `x-<name>` vendor extensions. Empty, since `OperationDef` does not
  /// carry them: an `{x-foo}` reference stays unbound.
  extensions: BTreeMap<String, String>,
}

impl<'a> OperationContext<'a> {
  pub(crate) fn from_operation(operation: &'a OperationDef) -> Self {
    Self {
      operation_id: Some(operation.operation_id.as_str()).filter(|id| !id.is_empty()),
      method: operation.method.as_lowercase(),
      path: operation.path.as_str(),
      path_segments: clean_path_segments(operation.path.as_str()),
      tags: operation.tags.as_slice(),
      extensions: BTreeMap::new(),
    }
  }

  /// Looks up a bare field name. `None` means unbound, which the caller
  /// turns into a rule failure.
  pub(crate) fn lookup(&self, name: &str) -> Option<&str> {
    match name {
      "operationId" => self.operation_id,
      "method" => Some(self.method),
      "path" => Some(self.path),
      _ if name.starts_with("x-") => self.extensions.get(name).map(String::as_str),
      _ => None,
    }
  }

  /// Looks up an array element: `pathSegments[0]`, `tags[-1]`. A negative
  /// index counts from the tail; out of bounds is unbound.
  pub(crate) fn lookup_indexed(&self, array: &str, index: i32) -> Option<&str> {
    match array {
      "pathSegments" => element(&self.path_segments, index).copied(),
      "tags" => element(self.tags, index).map(String::as_str),
      _ => None,
    }
  }

  /// Path segments joined with `_`, for the default method name.
  pub(crate) fn path_segments_joined(&self) -> String {
    self.path_segments.join("_")
  }

  pub(crate) const fn tags(&self) -> &'a [String] {
    self.tags
  }

  pub(crate) const fn method(&self) -> &'static str {
    self.method
  }

  pub(crate) const fn operation_id(&self) -> Option<&'a str> {
    self.operation_id
  }
}

/// Resolves a possibly-negative index against `items`.
fn element<T>(items: &[T], index: i32) -> Option<&T> {
  if index >= 0 {
    return items.get(usize::try_from(index).ok()?);
  }
  let from_tail = usize::try_from(-index).ok()?;
  items
    .len()
    .checked_sub(from_tail)
    .and_then(|i| items.get(i))
}

fn clean_path_segments(path: &str) -> Vec<&str> {
  path
    .split('/')
    .filter(|segment| !segment.is_empty())
    .map(|segment| {
      segment
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(segment)
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api_model::{
    canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
    schema::{SchemaScalar, SchemaType},
  };

  fn op(operation_id: &str, method: HttpMethod, path: &str, tags: &[&str]) -> OperationDef {
    OperationDef {
      operation_id: operation_id.to_string(),
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
  fn clean_path_segments_drops_leading_and_trailing_slashes() {
    assert_eq!(
      clean_path_segments("/users/{id}/posts/"),
      vec!["users", "id", "posts"]
    );
  }

  #[test]
  fn clean_path_segments_unwraps_path_params_literally() {
    assert_eq!(
      clean_path_segments("/api/v1/{resource}"),
      vec!["api", "v1", "resource"]
    );
  }

  #[test]
  fn lookup_returns_operation_id_method_and_path() {
    let operation = op("listPets", HttpMethod::Get, "/pets", &["Pet"]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(ctx.lookup("operationId"), Some("listPets"));
    assert_eq!(ctx.lookup("method"), Some("get"));
    assert_eq!(ctx.lookup("path"), Some("/pets"));
  }

  #[test]
  fn lookup_returns_none_for_missing_operation_id() {
    let operation = op("", HttpMethod::Get, "/pets", &["Pet"]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(ctx.lookup("operationId"), None);
  }

  #[test]
  fn lookup_indexed_supports_positive_and_negative_path_indexes() {
    let operation = op("x", HttpMethod::Get, "/users/{id}/posts", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(ctx.lookup_indexed("pathSegments", 0), Some("users"));
    assert_eq!(ctx.lookup_indexed("pathSegments", -1), Some("posts"));
    assert_eq!(ctx.lookup_indexed("pathSegments", 5), None);
  }

  #[test]
  fn lookup_indexed_returns_none_for_unknown_array_name() {
    let operation = op("x", HttpMethod::Get, "/pets", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(ctx.lookup_indexed("nothing", 0), None);
  }
}
