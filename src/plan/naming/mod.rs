//! Naming module — pre-emit derivation of `methodName` and `group` for
//! each operation, plus the formatting helpers (class name, file stem,
//! request-interface name, body-field inference) that consume those
//! resolved names.
//!
//! Submodules:
//! * `legacy`    — formatting helpers fixed by the project (not user-configurable).
//! * `config`, `context`, `template`, `case`, `parse_spec`, `engine`,
//!   `defaults` — the rule engine described in `docs/naming-spec.md`.
//!
//! Public surface (re-exported here): `NamingResolver`, the `NamingConfig`
//! / `Naming` / `Rule` / `RuleEntry` / `Case` types, the `compile_parse_spec`
//! helper, and the four legacy formatting helpers.

mod case;
mod config;
mod context;
mod defaults;
mod engine;
mod legacy;
mod parse_spec;
mod template;

pub use config::NamingConfig;
pub(crate) use config::{Case, Naming, Rule, RuleEntry};
pub(crate) use legacy::{
  error_interface_name, request_interface_name, service_class_name, service_file_stem,
};
pub(crate) use parse_spec::compile as compile_parse_spec;

use crate::{
  error::{Diagnostic, Reporter},
  ir::canonical::OperationDef,
};
use context::OperationContext;
use defaults::{default_group, default_method_name};
use engine::{RuleFailure, evaluate_chain};

impl Default for NamingConfig {
  fn default() -> Self {
    Self {
      method_name: None,
      group: None,
    }
  }
}

/// Resolved-naming entry point used by the planner. Holds the
/// user-supplied (validated, regex-compiled) config and exposes
/// per-operation lookups.
#[derive(Debug, Clone, Default)]
pub(crate) struct NamingResolver {
  pub(crate) config: NamingConfig,
}

impl NamingResolver {
  pub(crate) fn new(config: NamingConfig) -> Self {
    Self { config }
  }

  pub(crate) fn method_name(
    &self,
    operation: &OperationDef,
    reporter: &Reporter<'_>,
  ) -> Result<String, Diagnostic> {
    let ctx = OperationContext::from_operation(operation);
    match &self.config.method_name {
      Some(naming) => evaluate_chain(naming, &ctx).map_err(|failures| {
        naming_resolution_error(reporter, "methodName", operation, &failures)
      }),
      None => default_method_name(&ctx).map_err(|_| {
        Diagnostic::policy_violation(
          reporter,
          "naming-resolution",
          format!(
            "Could not derive a default methodName for operation {} {} (no operationId, and path produced no segments).",
            operation.method, operation.path,
          ),
        )
      }),
    }
  }

  pub(crate) fn group(
    &self,
    operation: &OperationDef,
    reporter: &Reporter<'_>,
  ) -> Result<String, Diagnostic> {
    let ctx = OperationContext::from_operation(operation);
    match &self.config.group {
      Some(naming) => evaluate_chain(naming, &ctx)
        .map_err(|failures| naming_resolution_error(reporter, "group", operation, &failures)),
      None => Ok(default_group(&ctx)),
    }
  }
}

fn naming_resolution_error(
  reporter: &Reporter<'_>,
  key: &str,
  operation: &OperationDef,
  failures: &[RuleFailure],
) -> Diagnostic {
  let formatted: String = failures
    .iter()
    .enumerate()
    .map(|(i, f)| format!("    [{}] {}", i, format_failure(f)))
    .collect::<Vec<_>>()
    .join("\n");
  Diagnostic::policy_violation(
    reporter,
    "naming-resolution",
    format!(
      "Failed to resolve `{}` for operation {} {} (operationId={}). All rules in the fallback chain failed:\n{}",
      key, operation.method, operation.path, operation.operation_id, formatted,
    ),
  )
}

fn format_failure(failure: &RuleFailure) -> String {
  match failure {
    RuleFailure::EmptyFromWithParse => {
      "expanded `from` was empty and `parse` is present (nothing to match)".to_string()
    }
    RuleFailure::ParseMismatch => "regex `parse` did not match the expanded `from`".to_string(),
    RuleFailure::Unbound(name) => format!("template referenced unbound name `{{{name}}}`"),
    RuleFailure::Malformed(msg) => format!("template malformed: {msg}"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    error::DiagnosticCode,
    ir::{
      canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
      schema::{SchemaScalar, SchemaType},
    },
    test_support::test_ctx,
  };

  fn op(id: &str, tags: &[&str], path: &str) -> OperationDef {
    OperationDef {
      operation_id: id.to_string(),
      tags: tags.iter().map(|s| s.to_string()).collect(),
      method: HttpMethod::Get,
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
  fn naming_resolver_returns_default_method_name_when_unconfigured() {
    let resolver = NamingResolver::default();
    let mut ctx = test_ctx();
    let operation = op("list_pets", &["Pet"], "/pets");
    assert_eq!(
      resolver.method_name(&operation, &ctx.reporter()).unwrap(),
      "listPets"
    );
  }

  #[test]
  fn naming_resolver_returns_default_group_when_unconfigured() {
    let resolver = NamingResolver::default();
    let mut ctx = test_ctx();
    let operation = op("x", &["pet-orders"], "/pets");
    assert_eq!(
      resolver.group(&operation, &ctx.reporter()).unwrap(),
      "PetOrders"
    );
  }

  #[test]
  fn naming_resolver_applies_user_supplied_method_name_chain() {
    let chain = Naming::Single(RuleEntry::Rule(Rule {
      from: Some("{operationId}".to_string()),
      parse: Some(compile_parse_spec(r"^[^_]+_(?<rest>.+)$", "").unwrap()),
      format: Some("{capture.rest}".to_string()),
      case: Some(Case::Camel),
    }));
    let resolver = NamingResolver::new(NamingConfig {
      method_name: Some(chain),
      group: None,
    });
    let mut ctx = test_ctx();
    let operation = op("posts_listAll", &["Posts"], "/posts");
    assert_eq!(
      resolver.method_name(&operation, &ctx.reporter()).unwrap(),
      "listAll"
    );
  }

  #[test]
  fn naming_resolver_emits_policy_violation_when_all_rules_fail() {
    let chain = Naming::Single(RuleEntry::Shorthand("{nonexistent}".to_string()));
    let resolver = NamingResolver::new(NamingConfig {
      method_name: Some(chain),
      group: None,
    });
    let mut ctx = test_ctx();
    let operation = op("x", &["Pet"], "/pets");
    let err = resolver
      .method_name(&operation, &ctx.reporter())
      .unwrap_err();
    assert_eq!(err.code, DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("naming-resolution"));
    assert!(err.message.contains("methodName"));
  }
}
