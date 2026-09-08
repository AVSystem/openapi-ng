//! `in: path` / `in: query` / `in: header` parameter lowering.

use crate::api_model::canonical::{HeaderDef, RequestInputDef, RequestInputSource};
use crate::api_model::schema::SchemaType;
use crate::error::{Diagnostic, DiagnosticCode};

use super::super::schema::normalize_schema;
use super::super::{SchemaWalk, bail_unsupported, unsupported};
use crate::error::Context;

use super::{LoweringContext, request_input_sort_key};

/// Which slot of the request contract a parameter lands in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
  Input(RequestInputSource),
  Header,
}

impl Destination {
  /// The slot `location` names. `Ok(None)` is a `cookie`, which the
  /// contract omits.
  fn parse(
    location: &str,
    name: &str,
    operation_id: &str,
    context: LoweringContext<'_>,
  ) -> Result<Option<Self>, Diagnostic> {
    let reporter = context.reporter();
    match location {
      "path" => Ok(Some(Self::Input(RequestInputSource::Path))),
      "query" => Ok(Some(Self::Input(RequestInputSource::Query))),
      "header" => Ok(Some(Self::Header)),
      "cookie" => {
        reporter.warning(
          DiagnosticCode::UnsupportedSemantic,
          Some("unsupported-parameter-location"),
          format!(
            "operationId '{operation_id}': parameter '{name}' uses location 'cookie', which is not supported in the generated service contract and will be omitted.",
          ),
        );
        Ok(None)
      }
      other => bail_unsupported!(
        reporter,
        "parameter {name} for {} {} uses unsupported location {other}.",
        context.method(),
        context.path()
      ),
    }
  }
}

/// A normalized schema shape that a parameter position cannot carry.
#[derive(Clone, Copy)]
enum UnsupportedShape {
  InlineObject,
  Empty,
}

impl UnsupportedShape {
  /// The shape `schema` presents, or `None` when the position accepts it.
  const fn of(schema: &SchemaType) -> Option<Self> {
    match schema {
      SchemaType::InlineObject { .. } => Some(Self::InlineObject),
      SchemaType::Any => Some(Self::Empty),
      _ => None,
    }
  }

  const fn label(self) -> &'static str {
    match self {
      Self::InlineObject => "an inline object schema",
      Self::Empty => "an empty schema",
    }
  }
}

/// One parameter reduced to the slot it fills and the type it carries.
struct Parameter {
  destination: Destination,
  name: Box<str>,
  required: bool,
  schema: SchemaType,
}

impl Parameter {
  fn into_input(self, source: RequestInputSource) -> RequestInputDef {
    RequestInputDef {
      name: self.name,
      source,
      required: self.required,
      schema: self.schema,
    }
  }

  fn into_header(self) -> HeaderDef {
    HeaderDef {
      name: self.name,
      required: self.required,
      schema: self.schema,
    }
  }
}

/// Lowers an operation's parameters into its path/query inputs and its
/// header list, each sorted by name.
pub(super) fn normalize_request_inputs(
  parameters: &[crate::parse::openapi_model::Parameter],
  operation_id: &str,
  context: LoweringContext<'_>,
) -> Result<(Vec<RequestInputDef>, Vec<HeaderDef>), Diagnostic> {
  let lowered = parameters
    .iter()
    .map(|parameter| lower(parameter, operation_id, context))
    .collect::<Result<Vec<_>, Diagnostic>>()?;

  let (mut inputs, mut headers) = lowered.into_iter().flatten().fold(
    (Vec::with_capacity(parameters.len()), Vec::new()),
    |(mut inputs, mut headers), parameter| {
      match parameter.destination {
        Destination::Input(source) => inputs.push(parameter.into_input(source)),
        Destination::Header => headers.push(parameter.into_header()),
      }
      (inputs, headers)
    },
  );

  inputs.sort_by(|left, right| request_input_sort_key(left).cmp(&request_input_sort_key(right)));
  headers.sort_by(|left, right| left.name.cmp(&right.name));
  Ok((inputs, headers))
}

/// Lowers one parameter, or `None` when the contract omits it.
fn lower(
  parameter: &crate::parse::openapi_model::Parameter,
  operation_id: &str,
  context: LoweringContext<'_>,
) -> Result<Option<Parameter>, Diagnostic> {
  let name = &parameter.name;
  let Some(destination) =
    Destination::parse(parameter.location.as_str(), name, operation_id, context)?
  else {
    return Ok(None);
  };

  reject_unsupported_declaration(parameter, destination, context)?;

  Ok(Some(Parameter {
    destination,
    name: name.as_str().into(),
    required: parameter.required,
    schema: normalize_parameter_schema(parameter, context)?,
  }))
}

/// Rejects an optional path parameter and one declared with `content`.
fn reject_unsupported_declaration(
  parameter: &crate::parse::openapi_model::Parameter,
  destination: Destination,
  context: LoweringContext<'_>,
) -> Result<(), Diagnostic> {
  let (method, path, reporter) = (context.method(), context.path(), context.reporter());
  let name = &parameter.name;

  if destination == Destination::Input(RequestInputSource::Path) && !parameter.required {
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

  Ok(())
}

/// Normalizes a parameter's declared schema, rejecting a shape the position
/// cannot carry.
fn normalize_parameter_schema(
  parameter: &crate::parse::openapi_model::Parameter,
  context: LoweringContext<'_>,
) -> Result<SchemaType, Diagnostic> {
  let (method, path, reporter) = (context.method(), context.path(), context.reporter());
  let name = &parameter.name;

  let declared = parameter.schema.as_ref().ok_or_else(|| {
    unsupported(
      reporter,
      format!("parameter {name} for {method} {path} must define schema."),
    )
  })?;

  let walk = SchemaWalk::root(Context::Parameter { method, path }, reporter);
  let schema = normalize_schema(declared, walk)?;

  if let Some(shape) = UnsupportedShape::of(&schema) {
    bail_unsupported!(
      reporter,
      "parameter {name} for {method} {path} uses {}, which is outside the supported subset.",
      shape.label()
    );
  }
  Ok(schema)
}
