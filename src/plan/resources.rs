//! Which operations get `.resource()`: the caller's `resources` rules, overridden per operation
//! by `x-openapi-ng-resource`.

use std::sync::LazyLock;

use regex::Regex;

use crate::{
  api_model::canonical::{HttpMethod, OperationDef},
  bindings::{NamingParseSpec, ResourceRuleEntry},
  error::{Diagnostic, DiagnosticCode, Reporter, bail},
  plan::naming::{compile_option_pattern, compile_pattern},
};

// A POST whose operationId reads as a query: `searchPets`, `query_orders`, `FindUsers`, but not
// `findingsCreate` or `findOrCreateUser`. The `regex` crate has no lookahead, so the word
// boundary and the `Or` + capital exclusion are spelled out.
const QUERY_OPERATION_ID: &str =
  "^([sS]earch|[qQ]uery|[fF]ind|[lL]ookup)([A-NP-Z0-9_-]|O($|[^r])|Or($|[^A-Z])|$)";

// Compiled once and cloned cheaply (`Regex` is `Arc`-backed): every `GenerateConfig` starts from
// the default before its options are lowered.
static QUERY: LazyLock<Regex> = LazyLock::new(|| {
  compile_pattern(QUERY_OPERATION_ID, "")
    .expect("the default operationId pattern compiles")
    .regex
});

/// One rule: every field it sets must match.
#[derive(Clone, Debug)]
struct ResourceRule {
  method: Option<HttpMethod>,
  operation_id: Option<Regex>,
  path: Option<Regex>,
}

impl ResourceRule {
  fn matches(&self, operation: &OperationDef) -> bool {
    self.method.is_none_or(|method| method == operation.method)
      && self
        .operation_id
        .as_ref()
        .is_none_or(|pattern| pattern.is_match(&operation.operation_id))
      && self
        .path
        .as_ref()
        .is_none_or(|pattern| pattern.is_match(&operation.path))
  }
}

/// An operation gets `.resource()` when any rule matches.
#[derive(Clone, Debug)]
pub(crate) struct ResourceRules(Vec<ResourceRule>);

impl Default for ResourceRules {
  fn default() -> Self {
    Self(vec![
      ResourceRule {
        method: Some(HttpMethod::Get),
        operation_id: None,
        path: None,
      },
      ResourceRule {
        method: Some(HttpMethod::Post),
        operation_id: Some(QUERY.clone()),
        path: None,
      },
    ])
  }
}

impl ResourceRules {
  #[must_use]
  pub(crate) fn admits(&self, operation: &OperationDef) -> bool {
    operation
      .resource
      .unwrap_or_else(|| self.0.iter().any(|rule| rule.matches(operation)))
  }
}

/// Lowers the caller's `resources`; absent means the default rules, and an empty list means no
/// operation gets `.resource()`.
pub(crate) fn lower(
  entries: Option<Vec<ResourceRuleEntry>>,
  reporter: &Reporter,
) -> Result<ResourceRules, Diagnostic> {
  let Some(entries) = entries else {
    return Ok(ResourceRules::default());
  };
  entries
    .into_iter()
    .enumerate()
    .map(|(index, entry)| lower_rule(index, entry, reporter))
    .collect::<Result<Vec<_>, _>>()
    .map(ResourceRules)
}

fn lower_rule(
  index: usize,
  entry: ResourceRuleEntry,
  reporter: &Reporter,
) -> Result<ResourceRule, Diagnostic> {
  let ResourceRuleEntry {
    method,
    operation_id,
    path,
  } = entry;
  if let (None, None, None) = (&method, &operation_id, &path) {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "resources[{index}] must set at least one of method, operationId or path."
    );
  }
  let pattern = |spec: Option<NamingParseSpec>, field: &str| {
    spec
      .map(|spec| {
        compile_option_pattern(&spec, &format!("resources[{index}].{field}"), reporter)
          .map(|compiled| compiled.regex)
      })
      .transpose()
  };
  Ok(ResourceRule {
    method: method
      .map(|method| parse_method(&method, index, reporter))
      .transpose()?,
    operation_id: pattern(operation_id, "operationId")?,
    path: pattern(path, "path")?,
  })
}

// Upper case only, as the spec's methods are printed everywhere else.
fn parse_method(value: &str, index: usize, reporter: &Reporter) -> Result<HttpMethod, Diagnostic> {
  HttpMethod::from_uppercase(value).ok_or_else(|| {
    let allowed = HttpMethod::ALL
      .map(|method| format!("'{method}'"))
      .join(", ");
    reporter.error(
      DiagnosticCode::InvalidOption,
      format!("resources[{index}].method: '{value}' is not one of {allowed}."),
    )
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api_model::canonical::RequestDef;
  use crate::error::DiagnosticCode;
  use crate::test_support::test_reporter;

  fn operation(operation_id: &str, method: HttpMethod, path: &str) -> OperationDef {
    OperationDef {
      operation_id: operation_id.to_string(),
      tags: Vec::new(),
      method,
      path: path.to_string(),
      request: RequestDef::default(),
      response: None,
      errors: Vec::new(),
      description: None,
      deprecated: false,
      resource: None,
    }
  }

  fn pattern(source: &str) -> Option<NamingParseSpec> {
    Some(NamingParseSpec {
      source: source.to_string(),
      flags: String::new(),
    })
  }

  fn rule(method: Option<&str>) -> ResourceRuleEntry {
    ResourceRuleEntry {
      method: method.map(str::to_string),
      operation_id: None,
      path: None,
    }
  }

  fn rules(entries: Vec<ResourceRuleEntry>) -> ResourceRules {
    lower(Some(entries), &test_reporter()).expect("rules lower")
  }

  #[test]
  fn hand_written_copies_spell_the_default_pattern() {
    for (file, contents) in [
      ("index.d.ts.in", include_str!("../../index.d.ts.in")),
      ("bin/openapi-ng.js", include_str!("../../bin/openapi-ng.js")),
      (
        "configuration.md",
        include_str!("../../website/src/content/docs/guides/configuration.md"),
      ),
    ] {
      assert!(
        contents.contains(&format!("operationId: /{QUERY_OPERATION_ID}/")),
        "{file}"
      );
    }
  }

  #[test]
  fn default_admits_get_and_query_named_post_only() {
    let rules = ResourceRules::default();
    assert!(rules.admits(&operation("getPet", HttpMethod::Get, "/pets/{id}")));
    assert!(rules.admits(&operation("searchPets", HttpMethod::Post, "/pets/search")));
    assert!(rules.admits(&operation("QueryOrders", HttpMethod::Post, "/orders/query")));
    assert!(!rules.admits(&operation("createPet", HttpMethod::Post, "/pets")));
    assert!(!rules.admits(&operation("deletePet", HttpMethod::Delete, "/pets/{id}")));
    assert!(!rules.admits(&operation("searchPets", HttpMethod::Put, "/pets/search")));
  }

  #[test]
  fn default_query_prefix_ends_at_a_word_boundary() {
    let rules = ResourceRules::default();
    for admitted in [
      "search",
      "find_pets",
      "lookup-pets",
      "findOrders",
      "findOr",
      "findORM",
      "query2",
    ] {
      assert!(
        rules.admits(&operation(admitted, HttpMethod::Post, "/x")),
        "{admitted}"
      );
    }
    for refused in [
      "findingsCreate",
      "searchindexRebuild",
      "SEARCHPETS",
      "lookups",
      "findOrCreateUser",
    ] {
      assert!(
        !rules.admits(&operation(refused, HttpMethod::Post, "/x")),
        "{refused}"
      );
    }
  }

  #[test]
  fn absent_option_lowers_to_the_default() {
    let lowered = lower(None, &test_reporter()).expect("absent lowers");
    assert!(lowered.admits(&operation("findPets", HttpMethod::Post, "/pets/find")));
  }

  #[test]
  fn empty_list_admits_nothing() {
    let rules = rules(Vec::new());
    assert!(!rules.admits(&operation("getPet", HttpMethod::Get, "/pets/{id}")));
  }

  #[test]
  fn explicit_list_replaces_the_default() {
    let rules = rules(vec![rule(Some("GET"))]);
    assert!(rules.admits(&operation("getPet", HttpMethod::Get, "/pets")));
    assert!(!rules.admits(&operation("searchPets", HttpMethod::Post, "/pets/search")));
  }

  #[test]
  fn every_field_of_a_rule_must_match() {
    let rules = rules(vec![ResourceRuleEntry {
      method: Some("POST".to_string()),
      operation_id: pattern("^search"),
      path: pattern("/search$"),
    }]);
    assert!(rules.admits(&operation("searchPets", HttpMethod::Post, "/pets/search")));
    assert!(!rules.admits(&operation("searchPets", HttpMethod::Post, "/pets")));
    assert!(!rules.admits(&operation("searchPets", HttpMethod::Get, "/pets/search")));
  }

  #[test]
  fn a_rule_without_method_matches_any_method() {
    let rules = rules(vec![ResourceRuleEntry {
      method: None,
      operation_id: None,
      path: pattern("^/reports/"),
    }]);
    assert!(rules.admits(&operation("buildReport", HttpMethod::Put, "/reports/x")));
  }

  #[test]
  fn extension_overrides_the_rules_both_ways() {
    let rules = ResourceRules::default();
    let mut delete = operation("deletePet", HttpMethod::Delete, "/pets/{id}");
    delete.resource = Some(true);
    assert!(rules.admits(&delete));
    let mut logout = operation("logout", HttpMethod::Get, "/logout");
    logout.resource = Some(false);
    assert!(!rules.admits(&logout));
  }

  #[test]
  fn extension_overrides_an_explicit_list_both_ways() {
    let mut delete = operation("deletePet", HttpMethod::Delete, "/pets/{id}");
    delete.resource = Some(true);
    assert!(rules(Vec::new()).admits(&delete));
    let mut list = operation("listPets", HttpMethod::Get, "/pets");
    list.resource = Some(false);
    assert!(!rules(vec![rule(Some("GET"))]).admits(&list));
  }

  #[test]
  fn a_pattern_honours_its_flags() {
    let rules = rules(vec![ResourceRuleEntry {
      method: None,
      operation_id: Some(NamingParseSpec {
        source: "^SEARCH".to_string(),
        flags: "gi".to_string(),
      }),
      path: None,
    }]);
    assert!(rules.admits(&operation("searchPets", HttpMethod::Post, "/pets")));
    assert!(!rules.admits(&operation("researchPets", HttpMethod::Post, "/pets")));
  }

  #[test]
  fn a_path_pattern_matches_template_braces_literally() {
    let rules = rules(vec![ResourceRuleEntry {
      method: None,
      operation_id: None,
      path: pattern(r"^\/pets\/{id}\/preview$"),
    }]);
    assert!(rules.admits(&operation(
      "previewPet",
      HttpMethod::Put,
      "/pets/{id}/preview"
    )));
    assert!(!rules.admits(&operation("previewPet", HttpMethod::Put, "/pets/7/preview")));
  }

  #[test]
  fn rejects_a_rule_that_sets_nothing() {
    let error = lower(Some(vec![rule(None)]), &test_reporter()).expect_err("empty rule");
    assert_eq!(error.code, DiagnosticCode::InvalidOption);
    assert!(
      error
        .message
        .contains("resources[0] must set at least one of")
    );
  }

  #[test]
  fn rejects_a_lower_case_or_unknown_method() {
    for method in ["get", "TRACE", "FETCH"] {
      let error = lower(Some(vec![rule(Some(method))]), &test_reporter()).expect_err(method);
      assert_eq!(error.code, DiagnosticCode::InvalidOption);
      assert!(
        error.message.contains(&format!(
          "resources[0].method: '{method}' is not one of 'GET'"
        )),
        "{}",
        error.message
      );
    }
  }

  #[test]
  fn rejects_an_invalid_pattern_naming_its_field() {
    let entries = vec![
      rule(Some("GET")),
      ResourceRuleEntry {
        method: None,
        operation_id: None,
        path: pattern("["),
      },
    ];
    let error = lower(Some(entries), &test_reporter()).expect_err("bad pattern");
    assert_eq!(error.code, DiagnosticCode::InvalidOption);
    assert!(
      error
        .message
        .starts_with("resources[1].path: failed to compile regex")
    );
  }
}
