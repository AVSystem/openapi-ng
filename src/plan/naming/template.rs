//! Template expander for `Rule.from` and `Rule.format`.
//!
//! Three productions, and nothing else:
//! * `{fieldName}`     — a context field
//! * `{arrayField[N]}` — an array element, negative indexes counting from
//!   the tail
//! * `{capture.name}`  — a named capture from the rule's `parse`

use std::collections::HashMap;

use crate::plan::naming::context::OperationContext;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TemplateError {
  /// A `{...}` named something the context does not bind.
  Unbound(String),
  /// Unclosed `{`, or an index that is not an integer.
  Malformed(String),
}

/// Expands every `{...}` in `template`.
pub(crate) fn expand(
  template: &str,
  ctx: &OperationContext<'_>,
  captures: &HashMap<String, String>,
) -> Result<String, TemplateError> {
  let mut out = String::with_capacity(template.len());
  let mut rest = template;

  while let Some(open) = rest.find('{') {
    out.push_str(&rest[..open]);
    let after = &rest[open + 1..];
    let Some(close) = after.find('}') else {
      return Err(TemplateError::Malformed(format!(
        "unclosed `{{` in `{template}`"
      )));
    };
    out.push_str(&resolve(&after[..close], ctx, captures)?);
    rest = &after[close + 1..];
  }

  out.push_str(rest);
  Ok(out)
}

fn resolve(
  token: &str,
  ctx: &OperationContext<'_>,
  captures: &HashMap<String, String>,
) -> Result<String, TemplateError> {
  if let Some(name) = token.strip_prefix("capture.") {
    return captures
      .get(name)
      .cloned()
      .ok_or_else(|| TemplateError::Unbound(token.to_string()));
  }

  if let Some((array, index)) = split_index(token) {
    let index: i32 = index
      .parse()
      .map_err(|_| TemplateError::Malformed(format!("invalid index in `{token}`")))?;
    return ctx
      .lookup_indexed(array, index)
      .map(str::to_string)
      .ok_or_else(|| TemplateError::Unbound(token.to_string()));
  }

  ctx
    .lookup(token)
    .map(str::to_string)
    .ok_or_else(|| TemplateError::Unbound(token.to_string()))
}

/// Splits `tags[-1]` into `("tags", "-1")`, or `None` when the token is not
/// an indexed reference.
fn split_index(token: &str) -> Option<(&str, &str)> {
  let open = token.find('[')?;
  let index = token[open + 1..].strip_suffix(']')?;
  Some((&token[..open], index))
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::*;
  use crate::api_model::{
    canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
    schema::{SchemaScalar, SchemaType},
  };

  fn ctx<'a>(operation: &'a OperationDef) -> OperationContext<'a> {
    OperationContext::from_operation(operation)
  }

  fn op() -> OperationDef {
    OperationDef {
      operation_id: "listPets".to_string(),
      tags: vec!["Pet".to_string()],
      method: HttpMethod::Get,
      path: "/pets/{id}".to_string(),
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
  fn expand_substitutes_plain_field_reference() {
    let operation = op();
    let result = expand("{operationId}", &ctx(&operation), &HashMap::new()).unwrap();
    assert_eq!(result, "listPets");
  }

  #[test]
  fn expand_substitutes_indexed_path_segment() {
    let operation = op();
    let result = expand(
      "{method}_{pathSegments[0]}",
      &ctx(&operation),
      &HashMap::new(),
    )
    .unwrap();
    assert_eq!(result, "get_pets");
  }

  #[test]
  fn expand_substitutes_named_capture_in_capture_namespace() {
    let operation = op();
    let mut captures = HashMap::new();
    captures.insert("rest".to_string(), "listAll".to_string());
    let result = expand("{capture.rest}", &ctx(&operation), &captures).unwrap();
    assert_eq!(result, "listAll");
  }

  #[test]
  fn expand_returns_unbound_for_unknown_field() {
    let operation = op();
    let err = expand("{whatever}", &ctx(&operation), &HashMap::new()).unwrap_err();
    assert_eq!(err, TemplateError::Unbound("whatever".to_string()));
  }

  #[test]
  fn expand_returns_unbound_for_out_of_range_index() {
    let operation = op();
    let err = expand("{pathSegments[7]}", &ctx(&operation), &HashMap::new()).unwrap_err();
    assert_eq!(err, TemplateError::Unbound("pathSegments[7]".to_string()));
  }

  #[test]
  fn expand_returns_unbound_for_missing_capture() {
    let operation = op();
    let err = expand("{capture.missing}", &ctx(&operation), &HashMap::new()).unwrap_err();
    assert_eq!(err, TemplateError::Unbound("capture.missing".to_string()));
  }

  #[test]
  fn expand_returns_malformed_for_unclosed_brace() {
    let operation = op();
    let err = expand("{operationId", &ctx(&operation), &HashMap::new()).unwrap_err();
    assert!(matches!(err, TemplateError::Malformed(_)));
  }

  #[test]
  fn expand_returns_malformed_for_non_numeric_index() {
    let operation = op();
    let err = expand("{pathSegments[abc]}", &ctx(&operation), &HashMap::new()).unwrap_err();
    assert!(matches!(err, TemplateError::Malformed(_)));
  }
}
