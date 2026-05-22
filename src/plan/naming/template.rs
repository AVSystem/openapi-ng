//! Template expander. Supports exactly three productions:
//! * `{fieldName}`         — context field by name
//! * `{arrayField[N]}`     — array index (negative allowed)
//! * `{capture.name}`      — regex named capture (explicit namespace)
//!
//! Unbound references and malformed templates surface as
//! `TemplateError`; the rule evaluator converts these into rule failures
//! per spec §"Failure modes".

use std::collections::HashMap;

use crate::plan::naming::context::OperationContext;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TemplateError {
  /// `{...}` referenced a name not in the context.
  Unbound(String),
  /// Malformed template: unclosed `{`, malformed index, etc.
  Malformed(String),
}

pub(crate) fn expand(
  template: &str,
  ctx: &OperationContext<'_>,
  captures: &HashMap<String, String>,
) -> Result<String, TemplateError> {
  let mut out = String::with_capacity(template.len());
  let chars: Vec<char> = template.chars().collect();
  let mut i = 0;
  while i < chars.len() {
    let ch = chars[i];
    if ch == '{' {
      let end = chars[i..]
        .iter()
        .position(|c| *c == '}')
        .ok_or_else(|| TemplateError::Malformed(format!("unclosed `{{` at offset {i}")))?;
      let token: String = chars[i + 1..i + end].iter().collect();
      out.push_str(&resolve_token(&token, ctx, captures)?);
      i += end + 1;
    } else {
      out.push(ch);
      i += 1;
    }
  }
  Ok(out)
}

fn resolve_token(
  token: &str,
  ctx: &OperationContext<'_>,
  captures: &HashMap<String, String>,
) -> Result<String, TemplateError> {
  if let Some(rest) = token.strip_prefix("capture.") {
    return captures
      .get(rest)
      .cloned()
      .ok_or_else(|| TemplateError::Unbound(token.to_string()));
  }
  if let Some((array_name, idx_str)) = parse_indexed(token) {
    let idx: i32 = idx_str
      .parse()
      .map_err(|_| TemplateError::Malformed(format!("invalid index in `{token}`")))?;
    return ctx
      .lookup_indexed(array_name, idx)
      .ok_or_else(|| TemplateError::Unbound(token.to_string()));
  }
  ctx
    .lookup(token)
    .ok_or_else(|| TemplateError::Unbound(token.to_string()))
}

fn parse_indexed(token: &str) -> Option<(&str, &str)> {
  let open = token.find('[')?;
  if !token.ends_with(']') {
    return None;
  }
  Some((&token[..open], &token[open + 1..token.len() - 1]))
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::*;
  use crate::ir::{
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
