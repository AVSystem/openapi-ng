use std::collections::BTreeMap;

use crate::{
  error::{Diagnostic, DiagnosticCode, Reporter},
  parse::{
    input::{max_operations, max_schemas},
    openapi_model::OpenApiDocument,
  },
};

pub(crate) fn validate_openapi_version(
  document: &OpenApiDocument,
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  if !document.openapi.starts_with("3.") {
    return Err(reporter.error(
      DiagnosticCode::UnsupportedSemantic,
      format!(
        "Unsupported OpenAPI document shape: only OpenAPI 3.x documents are supported, found {}.",
        document.openapi
      ),
    ));
  }
  Ok(())
}

pub(crate) fn validate_generation_policy(
  document: &OpenApiDocument,
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  // Per-document caps. These are sized to forestall accidental
  // pathological inputs (e.g. a fanned-out anchor expansion) before any
  // O(n²)-ish normalize/emit work runs. The defaults are deliberately
  // generous (10k each) — real specs are several orders of magnitude
  // below — and overridable via env so downstream consumers can opt out
  // without recompiling.
  let schema_count = document.components.schemas.len();
  let cap_schemas = max_schemas();
  if schema_count > cap_schemas {
    return Err(Diagnostic::policy_violation(
      reporter,
      "schema-cap-exceeded",
      format!(
        "Failed to plan services: OpenAPI document declares {schema_count} schemas under components.schemas; \
         the per-document cap is {cap_schemas}. Set OPENAPI_NG_MAX_SCHEMAS to override.",
      ),
    ));
  }

  let operation_count: usize = document
    .paths
    .values()
    .map(|path_item| path_item.operations().count())
    .sum();
  let cap_operations = max_operations();
  if operation_count > cap_operations {
    return Err(Diagnostic::policy_violation(
      reporter,
      "operation-cap-exceeded",
      format!(
        "Failed to plan services: OpenAPI document declares {operation_count} operations across paths; \
         the per-document cap is {cap_operations}. Set OPENAPI_NG_MAX_OPERATIONS to override.",
      ),
    ));
  }

  // Maps operationId → (method, path) for duplicate detection.
  let mut seen_operation_ids: BTreeMap<&str, (&'static str, &str)> = BTreeMap::new();

  for (path, path_item) in &document.paths {
    for (method, operation) in path_item.operations() {
      if operation.operation_id.is_none() {
        return Err(Diagnostic::policy_violation(
          reporter,
          "missing-operation-id",
          format!(
            "Failed to plan services: operation {} {} must define operationId when service generation is enabled.",
            method.to_ascii_uppercase(),
            path
          ),
        ));
      }

      if let Some(ref op_id) = operation.operation_id {
        if let Some(&(prev_method, prev_path)) = seen_operation_ids.get(op_id.as_str()) {
          return Err(Diagnostic::policy_violation(
            reporter,
            "duplicate-operation-id",
            format!(
              "Failed to plan services: operationId '{}' is defined on both {} {} and {} {}. \
               operationIds must be globally unique.",
              op_id,
              prev_method.to_ascii_uppercase(),
              prev_path,
              method.to_ascii_uppercase(),
              path,
            ),
          ));
        }
        seen_operation_ids.insert(op_id.as_str(), (method, path.as_str()));
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::{path::Path, rc::Rc};

  use crate::{parse::input::decode_openapi_input, test_support::test_ctx};

  use super::{validate_generation_policy, validate_openapi_version};

  fn decode(json: &str) -> crate::parse::openapi_model::OpenApiDocument {
    let display: Rc<str> = Rc::from("fixture.json");
    decode_openapi_input(Path::new("fixture.json"), json, &display).expect("decode should succeed")
  }

  #[test]
  fn validate_openapi_version_accepts_documents_without_operation_id() {
    let document = decode(
      r#"{"openapi":"3.0.3","info":{"title":"Missing OperationId","version":"1.0.0"},
         "paths":{"/pets":{"get":{"responses":{"200":{"description":"ok"}}}}}}"#,
    );

    let mut ctx = test_ctx();
    validate_openapi_version(&document, &ctx.reporter()).expect("version check should pass");
  }

  #[test]
  fn validate_openapi_version_rejects_non_3x_openapi_version() {
    let document =
      decode(r#"{"openapi":"2.0.0","info":{"title":"Old","version":"1.0.0"},"paths":{}}"#);

    let mut ctx = test_ctx();
    let Err(error) = validate_openapi_version(&document, &ctx.reporter()) else {
      panic!("old version should fail")
    };

    assert_eq!(
      error.code,
      crate::error::DiagnosticCode::UnsupportedSemantic
    );
    assert!(error.message.contains("3.x"));
  }

  #[test]
  fn validate_generation_policy_rejects_missing_operation_id() {
    let document = decode(
      r#"{"openapi":"3.0.3","info":{"title":"Missing OperationId","version":"1.0.0"},
         "paths":{"/pets":{"get":{"responses":{"200":{"description":"ok"}}}}}}"#,
    );
    let mut ctx = test_ctx();

    let Err(error) = validate_generation_policy(&document, &ctx.reporter()) else {
      panic!("missing operationId should fail")
    };

    assert_eq!(error.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(error.subcode, Some("missing-operation-id"));
    assert!(
      error
        .message
        .contains("must define operationId when service generation is enabled")
    );
  }

  #[test]
  fn validate_generation_policy_accepts_operations_with_operation_ids() {
    let document = decode(
      r#"{"openapi":"3.0.3","info":{"title":"Has OperationId","version":"1.0.0"},
         "paths":{"/pets":{"get":{"operationId":"listPets","responses":{"200":{"description":"ok"}}}}}}"#,
    );
    let mut ctx = test_ctx();

    validate_generation_policy(&document, &ctx.reporter())
      .expect("operation with operationId should pass");
  }

  #[test]
  fn duplicate_operation_id_is_rejected() {
    let yaml = include_str!("../../test/fixtures/duplicate-operation-id.openapi.yaml");
    let display: Rc<str> = Rc::from("fixture.yaml");
    let document = decode_openapi_input(Path::new("fixture.yaml"), yaml, &display)
      .expect("decode should succeed");
    let mut ctx = test_ctx();
    let err = validate_generation_policy(&document, &ctx.reporter())
      .expect_err("should reject duplicate operationId");
    assert_eq!(err.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("duplicate-operation-id"));
  }
}

#[cfg(test)]
mod cap_tests {
  use std::rc::Rc;

  use crate::{
    parse::input::{
      DEFAULT_MAX_OPERATIONS, DEFAULT_MAX_SCHEMAS, max_operations_from, max_schemas_from,
    },
    test_support::test_ctx,
  };

  use super::validate_generation_policy;

  // Pure-function tests for the cap helpers — not affected by OnceLock state.

  #[test]
  fn schemas_cap_helper_default() {
    assert_eq!(max_schemas_from(None), DEFAULT_MAX_SCHEMAS);
    assert_eq!(max_schemas_from(None), 10_000);
  }

  #[test]
  fn operations_cap_helper_default() {
    assert_eq!(max_operations_from(None), DEFAULT_MAX_OPERATIONS);
    assert_eq!(max_operations_from(None), 10_000);
  }

  #[test]
  fn schemas_cap_helper_respects_valid_env() {
    assert_eq!(max_schemas_from(Some("1")), 1);
    assert_eq!(max_schemas_from(Some("0")), 0);
  }

  #[test]
  fn operations_cap_helper_respects_valid_env() {
    assert_eq!(max_operations_from(Some("1")), 1);
    assert_eq!(max_operations_from(Some("0")), 0);
  }

  #[test]
  fn schemas_cap_helper_rejects_invalid_env_uses_default() {
    assert_eq!(max_schemas_from(Some("not-a-number")), DEFAULT_MAX_SCHEMAS);
    assert_eq!(max_schemas_from(Some("")), DEFAULT_MAX_SCHEMAS);
    assert_eq!(max_schemas_from(Some("-1")), DEFAULT_MAX_SCHEMAS);
  }

  #[test]
  fn operations_cap_helper_rejects_invalid_env_uses_default() {
    assert_eq!(
      max_operations_from(Some("not-a-number")),
      DEFAULT_MAX_OPERATIONS
    );
    assert_eq!(max_operations_from(Some("")), DEFAULT_MAX_OPERATIONS);
    assert_eq!(max_operations_from(Some("-1")), DEFAULT_MAX_OPERATIONS);
  }

  // Build an OpenAPI YAML document on the fly with N empty-object schemas
  // under components.schemas. Used to assert the schema-cap fires at the
  // configured boundary.
  fn build_doc_with_schemas(n: usize) -> String {
    let mut s = String::from(
      "openapi: 3.0.3\ninfo:\n  title: Bulk\n  version: 1.0.0\npaths: {}\ncomponents:\n  schemas:\n",
    );
    for i in 0..n {
      s.push_str(&format!("    S{i}:\n      type: object\n"));
    }
    s
  }

  // Build an OpenAPI YAML document with N total operations distributed
  // across paths (up to 8 operations per path, alphabetical methods).
  fn build_doc_with_operations(n: usize) -> String {
    let methods = [
      "delete", "get", "head", "options", "patch", "post", "put", "trace",
    ];
    let mut s = String::from("openapi: 3.0.3\ninfo:\n  title: Bulk\n  version: 1.0.0\npaths:\n");
    let mut remaining = n;
    let mut path_idx = 0usize;
    while remaining > 0 {
      s.push_str(&format!("  /p{path_idx}:\n"));
      let chunk = remaining.min(methods.len());
      for (mi, method) in methods.iter().take(chunk).enumerate() {
        let op_id = format!("op_{path_idx}_{mi}");
        s.push_str(&format!(
          "    {method}:\n      operationId: {op_id}\n      tags: [t]\n      responses:\n        '200':\n          description: ok\n",
        ));
      }
      remaining -= chunk;
      path_idx += 1;
    }
    s
  }

  fn decode(yaml: &str) -> crate::parse::openapi_model::OpenApiDocument {
    let display: Rc<str> = Rc::from("fixture.yaml");
    crate::parse::input::decode_openapi_input(std::path::Path::new("fixture.yaml"), yaml, &display)
      .expect("decode should succeed")
  }

  #[test]
  fn schemas_cap_rejects_oversize() {
    let yaml = build_doc_with_schemas(DEFAULT_MAX_SCHEMAS + 1);
    let document = decode(&yaml);

    let mut ctx = test_ctx();
    let err = validate_generation_policy(&document, &ctx.reporter())
      .expect_err("should reject oversize schemas");

    assert_eq!(err.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("schema-cap-exceeded"));
    assert!(
      err.message.contains("OPENAPI_NG_MAX_SCHEMAS"),
      "expected env-var hint in message: {}",
      err.message,
    );
  }

  #[test]
  fn operations_cap_rejects_oversize() {
    let yaml = build_doc_with_operations(DEFAULT_MAX_OPERATIONS + 1);
    let document = decode(&yaml);

    let mut ctx = test_ctx();
    let err = validate_generation_policy(&document, &ctx.reporter())
      .expect_err("should reject oversize operations");

    assert_eq!(err.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("operation-cap-exceeded"));
    assert!(
      err.message.contains("OPENAPI_NG_MAX_OPERATIONS"),
      "expected env-var hint in message: {}",
      err.message,
    );
  }
}
