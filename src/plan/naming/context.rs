//! `OperationContext` — the read-only bag of values a `Rule.from` /
//! `Rule.format` template can reference for one operation. Fields map
//! 1:1 to the spec's "Context fields" table.

use std::collections::BTreeMap;

use crate::ir::canonical::OperationDef;

#[derive(Debug)]
pub(crate) struct OperationContext<'a> {
  pub(crate) operation_id: Option<&'a str>,
  pub(crate) method: String, // lowercased
  pub(crate) path: &'a str,
  pub(crate) path_segments: Vec<String>,
  pub(crate) tags: &'a [String],
  pub(crate) extensions: BTreeMap<String, String>, // x-<name> → string
                                                   // contentType / statusCode are unbound here
                                                   // until those carriers exist on OperationDef.
}

impl<'a> OperationContext<'a> {
  pub(crate) fn from_operation(operation: &'a OperationDef) -> Self {
    Self {
      operation_id: if operation.operation_id.is_empty() {
        None
      } else {
        Some(operation.operation_id.as_str())
      },
      method: operation.method.as_str().to_ascii_lowercase(),
      path: operation.path.as_str(),
      path_segments: clean_path_segments(operation.path.as_str()),
      tags: operation.tags.as_slice(),
      // `vendor_extensions` does not yet exist on `OperationDef`; an
      // empty map keeps `{x-foo}` references unbound (triggering
      // fallback) and the carrier can be plumbed through normalize
      // later without touching the engine.
      extensions: BTreeMap::new(),
    }
  }

  /// Lookup by template name. Returns `None` for unbound names — the
  /// caller turns that into a rule failure.
  pub(crate) fn lookup(&self, name: &str) -> Option<String> {
    match name {
      "operationId" => self.operation_id.map(str::to_string),
      "method" => Some(self.method.clone()),
      "path" => Some(self.path.to_string()),
      _ if name.starts_with("x-") => self.extensions.get(name).cloned(),
      _ => None,
    }
  }

  /// Lookup with array indexing: `pathSegments[0]`, `tags[-1]`, etc.
  /// Negative indexes count from the tail. Out-of-bounds is unbound.
  pub(crate) fn lookup_indexed(&self, array_name: &str, index: i32) -> Option<String> {
    let slice: Vec<&str> = match array_name {
      "pathSegments" => self.path_segments.iter().map(String::as_str).collect(),
      "tags" => self.tags.iter().map(String::as_str).collect(),
      _ => return None,
    };
    resolve_index(slice.len(), index).map(|i| slice[i].to_string())
  }
}

fn resolve_index(len: usize, index: i32) -> Option<usize> {
  if index >= 0 {
    let i = index as usize;
    if i < len { Some(i) } else { None }
  } else {
    let from_tail = (-index) as usize;
    if from_tail == 0 || from_tail > len {
      None
    } else {
      Some(len - from_tail)
    }
  }
}

/// Clean a path per spec:
/// * drop leading empty segment (leading `/`)
/// * drop trailing empty segment (trailing `/`)
/// * unwrap `{name}` → `name` (literal content between braces)
fn clean_path_segments(path: &str) -> Vec<String> {
  path
    .split('/')
    .filter(|s| !s.is_empty())
    .map(|s| {
      if s.starts_with('{') && s.ends_with('}') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
      } else {
        s.to_string()
      }
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::{
    canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
    schema::{SchemaScalar, SchemaType},
  };

  fn op(operation_id: &str, method: HttpMethod, path: &str, tags: &[&str]) -> OperationDef {
    OperationDef {
      operation_id: operation_id.to_string(),
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
    assert_eq!(ctx.lookup("operationId").as_deref(), Some("listPets"));
    assert_eq!(ctx.lookup("method").as_deref(), Some("get"));
    assert_eq!(ctx.lookup("path").as_deref(), Some("/pets"));
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
    assert_eq!(
      ctx.lookup_indexed("pathSegments", 0).as_deref(),
      Some("users")
    );
    assert_eq!(
      ctx.lookup_indexed("pathSegments", -1).as_deref(),
      Some("posts")
    );
    assert_eq!(ctx.lookup_indexed("pathSegments", 5), None);
  }

  #[test]
  fn lookup_indexed_returns_none_for_unknown_array_name() {
    let operation = op("x", HttpMethod::Get, "/pets", &[]);
    let ctx = OperationContext::from_operation(&operation);
    assert_eq!(ctx.lookup_indexed("nothing", 0), None);
  }
}
