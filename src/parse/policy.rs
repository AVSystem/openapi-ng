use std::collections::BTreeMap;

use crate::{
  error::{Diagnostic, DiagnosticCode, Reporter, bail, bail_policy},
  parse::{
    limits::{MAX_OPERATIONS, MAX_SCHEMAS},
    openapi_model::OpenApiDocument,
  },
};

pub(crate) fn validate_openapi_version(
  document: &OpenApiDocument,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  if !document.openapi.starts_with("3.") {
    bail!(
      reporter,
      DiagnosticCode::UnsupportedSemantic,
      "Unsupported OpenAPI document shape: only OpenAPI 3.x documents are supported, found {}.",
      document.openapi
    );
  }
  Ok(())
}

pub(crate) fn validate_generation_policy(
  document: &OpenApiDocument,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  check_schema_cap(document, reporter)?;
  check_operation_cap(document, reporter)?;
  check_operation_ids_are_unique(document, reporter)
}

fn check_schema_cap(document: &OpenApiDocument, reporter: &Reporter) -> Result<(), Diagnostic> {
  let schema_count = document.components.schemas.len();
  let cap_schemas = MAX_SCHEMAS.get();
  if schema_count > cap_schemas {
    bail_policy!(
      reporter,
      "schema-cap-exceeded",
      "Failed to plan services: OpenAPI document declares {schema_count} schemas under components.schemas; \
         the per-document cap is {cap_schemas}. Set OPENAPI_NG_MAX_SCHEMAS to override.",
    );
  }
  Ok(())
}

fn check_operation_cap(document: &OpenApiDocument, reporter: &Reporter) -> Result<(), Diagnostic> {
  let operation_count: usize = document
    .paths
    .values()
    .map(|path_item| path_item.operations().count())
    .sum();
  let cap_operations = MAX_OPERATIONS.get();
  if operation_count > cap_operations {
    bail_policy!(
      reporter,
      "operation-cap-exceeded",
      "Failed to plan services: OpenAPI document declares {operation_count} operations across paths; \
         the per-document cap is {cap_operations}. Set OPENAPI_NG_MAX_OPERATIONS to override.",
    );
  }
  Ok(())
}

/// Fails on the second operation to declare an `operationId`, naming the
/// first, and on an operation that declares none.
fn check_operation_ids_are_unique(
  document: &OpenApiDocument,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  document
    .paths
    .iter()
    .flat_map(|(path, path_item)| {
      path_item
        .operations()
        .map(move |(method, operation)| (path.as_str(), method, operation))
    })
    .try_fold(BTreeMap::new(), |declared, (path, method, operation)| {
      let Some(operation_id) = operation.operation_id.as_deref() else {
        bail_policy!(
          reporter,
          "missing-operation-id",
          "Failed to plan services: operation {} {} must define operationId when service generation is enabled.",
          method.to_ascii_uppercase(),
          path
        );
      };
      claim_operation_id(declared, operation_id, method, path, reporter)
    })
    .map(|_| ())
}

/// Records `operation_id` against `method` and `path`, failing when another
/// operation already claimed it.
fn claim_operation_id<'a>(
  mut declared: BTreeMap<&'a str, (&'static str, &'a str)>,
  operation_id: &'a str,
  method: &'static str,
  path: &'a str,
  reporter: &Reporter,
) -> Result<BTreeMap<&'a str, (&'static str, &'a str)>, Diagnostic> {
  if let Some(&(first_method, first_path)) = declared.get(operation_id) {
    bail_policy!(
      reporter,
      "duplicate-operation-id",
      "Failed to plan services: operationId '{}' is defined on both {} {} and {} {}. \
         operationIds must be globally unique.",
      operation_id,
      first_method.to_ascii_uppercase(),
      first_path,
      method.to_ascii_uppercase(),
      path,
    );
  }
  declared.insert(operation_id, (method, path));
  Ok(declared)
}

#[cfg(test)]
mod tests {
  use std::{path::Path, rc::Rc};

  use crate::{parse::input::decode_openapi_input, test_support::test_reporter};

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

    let ctx = test_reporter();
    validate_openapi_version(&document, &ctx).expect("version check should pass");
  }

  #[test]
  fn validate_openapi_version_rejects_non_3x_openapi_version() {
    let document =
      decode(r#"{"openapi":"2.0.0","info":{"title":"Old","version":"1.0.0"},"paths":{}}"#);

    let ctx = test_reporter();
    let Err(error) = validate_openapi_version(&document, &ctx) else {
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
    let ctx = test_reporter();

    let Err(error) = validate_generation_policy(&document, &ctx) else {
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
    let ctx = test_reporter();

    validate_generation_policy(&document, &ctx).expect("operation with operationId should pass");
  }

  #[test]
  fn duplicate_operation_id_is_rejected() {
    let yaml = include_str!("../../test/fixtures/duplicate-operation-id.openapi.yaml");
    let display: Rc<str> = Rc::from("fixture.yaml");
    let document = decode_openapi_input(Path::new("fixture.yaml"), yaml, &display)
      .expect("decode should succeed");
    let ctx = test_reporter();
    let err =
      validate_generation_policy(&document, &ctx).expect_err("should reject duplicate operationId");
    assert_eq!(err.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("duplicate-operation-id"));
  }
}

#[cfg(test)]
mod cap_tests {
  use std::rc::Rc;

  use crate::parse::limits::{MAX_OPERATIONS, MAX_SCHEMAS};
  use crate::test_support::test_reporter;

  use super::validate_generation_policy;

  // An OpenAPI document with N empty-object schemas.
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
    let yaml = build_doc_with_schemas(MAX_SCHEMAS.get() + 1);
    let document = decode(&yaml);

    let ctx = test_reporter();
    let err =
      validate_generation_policy(&document, &ctx).expect_err("should reject oversize schemas");

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
    let yaml = build_doc_with_operations(MAX_OPERATIONS.get() + 1);
    let document = decode(&yaml);

    let ctx = test_reporter();
    let err =
      validate_generation_policy(&document, &ctx).expect_err("should reject oversize operations");

    assert_eq!(err.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("operation-cap-exceeded"));
    assert!(
      err.message.contains("OPENAPI_NG_MAX_OPERATIONS"),
      "expected env-var hint in message: {}",
      err.message,
    );
  }
}
