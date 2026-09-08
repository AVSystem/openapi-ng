//! OpenAPI paths → canonical `OperationDef`s.
//!
//! Each submodule owns one slot of an operation: the path template, the
//! parameters, the request body, and the responses.

mod body;
mod form;
mod parameters;
mod path_template;
mod responses;

use std::collections::BTreeMap;

use crate::error::{Diagnostic, Reporter};
use crate::ir::canonical::{
  HttpMethod, OperationDef, RequestDef, RequestInputDef, RequestInputSource,
};
use crate::ir::schema::SchemaType;
use crate::options::ResponseTypeMapping;
use crate::parse::openapi_model::{Operation, PathItem};

use super::unsupported;
use body::normalize_request_body;
use parameters::normalize_request_inputs;
use path_template::validate_path_template;
use responses::{normalize_error_responses, normalize_success_response};

/// Everything an operation's lowering needs besides the operation itself:
/// where it sits, the schemas its `$ref`s may resolve to, the caller's
/// response-kind overrides, and the diagnostic sink.
///
/// `method` is the canonical upper-case name.
#[derive(Clone, Copy)]
pub(super) struct OperationCx<'a> {
  method: &'a str,
  path: &'a str,
  schemas: &'a BTreeMap<&'a str, &'a SchemaType>,
  response_types: &'a [ResponseTypeMapping],
  reporter: &'a Reporter,
}

impl<'a> OperationCx<'a> {
  pub(super) const fn new(
    method: &'a str,
    path: &'a str,
    schemas: &'a BTreeMap<&'a str, &'a SchemaType>,
    response_types: &'a [ResponseTypeMapping],
    reporter: &'a Reporter,
  ) -> Self {
    Self {
      method,
      path,
      schemas,
      response_types,
      reporter,
    }
  }

  pub(super) const fn method(&self) -> &'a str {
    self.method
  }

  pub(super) const fn path(&self) -> &'a str {
    self.path
  }

  pub(super) const fn schemas(&self) -> &'a BTreeMap<&'a str, &'a SchemaType> {
    self.schemas
  }

  pub(super) const fn response_types(&self) -> &'a [ResponseTypeMapping] {
    self.response_types
  }

  pub(super) const fn reporter(&self) -> &'a Reporter {
    self.reporter
  }
}

pub(super) fn normalize_operations(
  paths: &BTreeMap<String, PathItem>,
  schemas: &BTreeMap<&str, &SchemaType>,
  response_types: &[ResponseTypeMapping],
  reporter: &Reporter,
) -> Result<Vec<OperationDef>, Diagnostic> {
  paths
    .iter()
    .map(|(path, path_item)| {
      validate_path_template(path, reporter)?;
      path_item
        .operations()
        .map(|(method, operation)| {
          normalize_operation(path, method, operation, schemas, response_types, reporter)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
    })
    .collect::<Result<Vec<_>, Diagnostic>>()
    .map(|per_path| per_path.into_iter().flatten().collect())
}

fn normalize_operation(
  path: &str,
  declared_method: &str,
  operation: &Operation,
  schemas: &BTreeMap<&str, &SchemaType>,
  response_types: &[ResponseTypeMapping],
  reporter: &Reporter,
) -> Result<OperationDef, Diagnostic> {
  let method = HttpMethod::from_lowercase(declared_method)
    .ok_or_else(|| unsupported(reporter, unsupported_method_detail(declared_method, path)))?;

  let operation_id = operation
    .operation_id
    .clone()
    .unwrap_or_else(|| format!("{declared_method}_{}", path.replace(['/', '{', '}'], "_")));

  let context = OperationCx::new(method.as_str(), path, schemas, response_types, reporter);

  Ok(OperationDef {
    request: normalize_request(operation, &operation_id, context)?,
    response: normalize_success_response(operation.responses.as_deref(), context)?,
    errors: normalize_error_responses(operation.responses.as_deref(), context)?,
    operation_id,
    tags: operation.tags.clone(),
    method,
    path: path.to_string(),
    description: operation.merged_description(),
    deprecated: operation.deprecated,
  })
}

fn unsupported_method_detail(declared_method: &str, path: &str) -> String {
  if declared_method == "trace" {
    format!(
      "HTTP method TRACE for {path} is not supported; remove the trace operation or split it into a non-generated client."
    )
  } else {
    format!("unknown HTTP method {declared_method} for {path}.")
  }
}

fn normalize_request(
  operation: &Operation,
  operation_id: &str,
  cx: OperationCx<'_>,
) -> Result<RequestDef, Diagnostic> {
  let (inputs, headers) = normalize_request_inputs(&operation.parameters, operation_id, cx)?;
  Ok(RequestDef {
    inputs,
    headers,
    body: normalize_request_body(operation.request_body.as_ref(), cx)?,
  })
}

pub(super) fn request_input_sort_key(value: &RequestInputDef) -> (u8, &str) {
  let weight = match value.source {
    RequestInputSource::Path => 0,
    RequestInputSource::Query => 1,
  };

  (weight, &value.name)
}
