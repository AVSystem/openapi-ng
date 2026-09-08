//! `in: path` / `in: query` / `in: header` parameter lowering.

use crate::error::{Diagnostic, DiagnosticCode};
use crate::ir::canonical::{HeaderDef, RequestInputDef, RequestInputSource};
use crate::ir::schema::SchemaType;

use super::super::schema::normalize_schema;
use super::super::{SchemaWalk, bail_unsupported, unsupported};
use crate::error::Context;

use super::{OperationCx, request_input_sort_key};

/// Which slot of the request contract a parameter lands in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
  Input(RequestInputSource),
  Header,
}

/// Lowers an operation's parameters into its path/query inputs and its
/// header list, each sorted by name.
///
/// A `cookie` parameter is dropped with a warning; any other unsupported
/// location fails.
pub(super) fn normalize_request_inputs(
  parameters: &[crate::parse::openapi_model::Parameter],
  operation_id: &str,
  cx: OperationCx<'_>,
) -> Result<(Vec<RequestInputDef>, Vec<HeaderDef>), Diagnostic> {
  let (method, path, reporter) = (cx.method(), cx.path(), cx.reporter());
  let mut inputs = Vec::with_capacity(parameters.len());
  let mut headers = Vec::new();

  for parameter in parameters {
    let name = &parameter.name;
    let destination = match parameter.location.as_str() {
      "path" => Destination::Input(RequestInputSource::Path),
      "query" => Destination::Input(RequestInputSource::Query),
      "header" => Destination::Header,
      "cookie" => {
        reporter.warning(
          DiagnosticCode::UnsupportedSemantic,
          Some("unsupported-parameter-location"),
          format!(
            "operationId '{operation_id}': parameter '{name}' uses location 'cookie', which is not supported in the generated service contract and will be omitted.",
          ),
        );
        continue;
      }
      other => {
        bail_unsupported!(
          reporter,
          "parameter {name} for {method} {path} uses unsupported location {other}."
        );
      }
    };

    let required = parameter.required;

    if destination == Destination::Input(RequestInputSource::Path) && !required {
      bail_unsupported!(
        reporter,
        "path parameter {name} for {method} {path} must be required."
      );
    }

    if parameter.content.is_some() {
      bail_unsupported!(
        reporter,
        "parameter {name} for {method} {path} must use schema, not content."
      );
    }

    let schema = parameter.schema.as_ref().ok_or_else(|| {
      unsupported(
        reporter,
        format!("parameter {name} for {method} {path} must define schema."),
      )
    })?;

    let walk = SchemaWalk::root(Context::Parameter { method, path }, reporter);
    let ty = normalize_schema(schema, walk)?;
    match ty {
      SchemaType::InlineObject { .. } => {
        bail_unsupported!(
          reporter,
          "parameter {name} for {method} {path} uses an inline object schema, which is outside the supported subset."
        );
      }
      SchemaType::Any => {
        bail_unsupported!(
          reporter,
          "parameter {name} for {method} {path} uses an empty schema, which is outside the supported subset."
        );
      }
      _ => {}
    }

    match destination {
      Destination::Input(source) => inputs.push(RequestInputDef {
        name: name.as_str().into(),
        source,
        required,
        ty,
      }),
      Destination::Header => headers.push(HeaderDef {
        name: name.as_str().into(),
        required,
        ty,
      }),
    }
  }

  inputs.sort_by(|left, right| request_input_sort_key(left).cmp(&request_input_sort_key(right)));
  headers.sort_by(|left, right| left.name.cmp(&right.name));
  Ok((inputs, headers))
}
