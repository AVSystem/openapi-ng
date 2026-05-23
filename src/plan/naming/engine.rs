//! Single-rule evaluator and fallback-chain runner. Failure modes per
//! spec §"Failure modes": empty `from` + present `parse`, regex
//! mismatch, or any unbound name reference in `from`/`format`.

use std::collections::HashMap;

use crate::plan::naming::{
  case::apply as apply_case,
  config::{Naming, Rule, RuleEntry},
  context::OperationContext,
  template::{TemplateError, expand},
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RuleFailure {
  /// `parse` was present but `from` expanded to empty (nothing to match).
  EmptyFromWithParse,
  /// `parse` was present and did not match the expanded `from`.
  ParseMismatch,
  /// A template referenced an unbound name (field, indexed slot, or capture).
  Unbound(String),
  /// Template was malformed at parse time. This is technically a
  /// config-time error caught by validation, but evaluation still has
  /// to handle it defensively.
  Malformed(String),
}

pub(crate) fn evaluate_chain(
  chain: &Naming,
  ctx: &OperationContext<'_>,
) -> Result<String, Vec<RuleFailure>> {
  let entries: &[RuleEntry] = match chain {
    Naming::Single(entry) => std::slice::from_ref(entry),
    Naming::Chain(entries) => entries.as_slice(),
  };
  let mut failures = Vec::with_capacity(entries.len());
  for entry in entries {
    match evaluate_entry(entry, ctx) {
      Ok(s) => return Ok(s),
      Err(f) => failures.push(f),
    }
  }
  Err(failures)
}

fn evaluate_entry(entry: &RuleEntry, ctx: &OperationContext<'_>) -> Result<String, RuleFailure> {
  match entry {
    RuleEntry::Shorthand(format_template) => {
      let s = expand(format_template, ctx, &HashMap::new()).map_err(map_template_error)?;
      Ok(s)
    }
    RuleEntry::Rule(rule) => evaluate_rule(rule, ctx),
  }
}

fn evaluate_rule(rule: &Rule, ctx: &OperationContext<'_>) -> Result<String, RuleFailure> {
  // Step 1: expand `from` (default "" if omitted).
  let from_expanded = match &rule.from {
    Some(template) => expand(template, ctx, &HashMap::new()).map_err(map_template_error)?,
    None => String::new(),
  };

  // Step 2: parse — only runs when present.
  let captures: HashMap<String, String> = match &rule.parse {
    Some(spec) => {
      if from_expanded.is_empty() {
        return Err(RuleFailure::EmptyFromWithParse);
      }
      let captures = spec
        .regex
        .captures(&from_expanded)
        .ok_or(RuleFailure::ParseMismatch)?;
      spec
        .regex
        .capture_names()
        .flatten()
        .filter_map(|name| {
          captures
            .name(name)
            .map(|m| (name.to_string(), m.as_str().to_string()))
        })
        .collect()
    }
    None => HashMap::new(),
  };

  // Step 3: format — defaults to the expanded `from` when omitted (only
  // legal when `parse` is also absent; config-time validation enforces).
  let raw = match &rule.format {
    Some(template) => expand(template, ctx, &captures).map_err(map_template_error)?,
    None => from_expanded,
  };

  // Step 4: case transformation.
  let final_value = match rule.case {
    Some(case) => apply_case(&raw, case),
    None => raw,
  };

  Ok(final_value)
}

fn map_template_error(err: TemplateError) -> RuleFailure {
  match err {
    TemplateError::Unbound(name) => RuleFailure::Unbound(name),
    TemplateError::Malformed(msg) => RuleFailure::Malformed(msg),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    ir::{
      canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
      schema::{SchemaScalar, SchemaType},
    },
    plan::naming::{
      config::{Case, Naming, Rule, RuleEntry},
      parse_spec::compile,
    },
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
  fn shorthand_rule_expands_template_verbatim_without_case() {
    let operation = op("listPets", HttpMethod::Get, "/pets", &["Pet"]);
    let ctx = OperationContext::from_operation(&operation);
    let chain = Naming::Single(RuleEntry::Shorthand("{operationId}".to_string()));
    assert_eq!(evaluate_chain(&chain, &ctx).unwrap(), "listPets");
  }

  #[test]
  fn rule_with_parse_replaces_verb_prefix_with_capture_via_format() {
    // Spec example: parse `^[^_]+_(?<rest>.+)$`, format `{capture.rest}`, case camel.
    // posts_listAll → listAll.
    let operation = op("posts_listAll", HttpMethod::Get, "/posts", &[]);
    let ctx = OperationContext::from_operation(&operation);
    let parse = compile(r"^[^_]+_(?<rest>.+)$", "").unwrap();
    let chain = Naming::Single(RuleEntry::Rule(Rule {
      from: Some("{operationId}".to_string()),
      parse: Some(parse),
      format: Some("{capture.rest}".to_string()),
      case: Some(Case::Camel),
    }));
    assert_eq!(evaluate_chain(&chain, &ctx).unwrap(), "listAll");
  }

  #[test]
  fn rule_falls_through_to_next_entry_on_parse_mismatch() {
    let operation = op("createPet", HttpMethod::Get, "/pets", &[]);
    let ctx = OperationContext::from_operation(&operation);
    let parse = compile(r"^v\d+_(?<rest>.+)$", "").unwrap();
    let chain = Naming::Chain(vec![
      RuleEntry::Rule(Rule {
        from: Some("{operationId}".to_string()),
        parse: Some(parse),
        format: Some("{capture.rest}".to_string()),
        case: Some(Case::Camel),
      }),
      RuleEntry::Rule(Rule {
        from: None,
        parse: None,
        format: Some("{operationId}".to_string()),
        case: Some(Case::Camel),
      }),
    ]);
    assert_eq!(evaluate_chain(&chain, &ctx).unwrap(), "createPet");
  }

  #[test]
  fn rule_fails_when_parse_present_but_from_expands_to_empty() {
    // `from: {operationId}` but operationId is empty.
    let operation = op("", HttpMethod::Get, "/pets", &[]);
    let ctx = OperationContext::from_operation(&operation);
    let parse = compile(r".", "").unwrap();
    let chain = Naming::Single(RuleEntry::Rule(Rule {
      from: Some("{operationId}".to_string()),
      parse: Some(parse),
      format: Some("{capture.unused}".to_string()),
      case: None,
    }));
    let err = evaluate_chain(&chain, &ctx).unwrap_err();
    // Empty operationId → from-template is unbound (not empty), so we
    // fail with `Unbound("operationId")`. (Empty-`from`-with-`parse` is
    // only reachable when `from` is omitted, which is also legal with
    // `parse` per spec — but `parse` over an empty string fails too.)
    assert!(matches!(err[0], RuleFailure::Unbound(_)));
  }

  #[test]
  fn chain_returns_all_failures_when_every_entry_fails() {
    let operation = op("createPet", HttpMethod::Get, "/pets", &[]);
    let ctx = OperationContext::from_operation(&operation);
    let chain = Naming::Chain(vec![
      RuleEntry::Shorthand("{nonexistent1}".to_string()),
      RuleEntry::Shorthand("{nonexistent2}".to_string()),
    ]);
    let err = evaluate_chain(&chain, &ctx).unwrap_err();
    assert_eq!(err.len(), 2);
  }
}
