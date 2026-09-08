use std::rc::Rc;

use crate::{
  api_model::{
    canonical::{BodyFieldType, HttpMethod, ResponseContent},
    schema::{SchemaProperty, SchemaScalar, SchemaType},
  },
  error::Reporter,
  identifier::{Identifier, MethodName},
  plan::{
    artifact_plan::{
      PlannedFormField, PlannedHeader, PlannedOperation, PlannedRequestBody,
      PlannedRequestContract, PlannedRequestField, RequestFieldKind,
    },
    naming::{error_interface_name, request_interface_name},
  },
};

/// Reporter over a placeholder display path.
pub(crate) fn test_reporter() -> Reporter {
  reporter_for("test")
}

/// Reporter whose diagnostics carry `path`.
pub(crate) fn reporter_for(path: &str) -> Reporter {
  Reporter::new(Rc::from(path))
}

pub(crate) fn property(name: &str, required: bool, schema: SchemaType) -> SchemaProperty {
  SchemaProperty {
    name: name.into(),
    required,
    schema,
    description: None,
    deprecated: false,
  }
}

pub(crate) fn nullable_property(name: &str, required: bool, schema: SchemaType) -> SchemaProperty {
  SchemaProperty {
    name: name.into(),
    required,
    schema: SchemaType::Nullable(Box::new(schema)),
    description: None,
    deprecated: false,
  }
}

pub(crate) fn string_schema() -> SchemaType {
  SchemaType::Scalar(SchemaScalar::String)
}

pub(crate) fn path_field<'a>(name: &str, schema: &'a SchemaType) -> PlannedRequestField<'a> {
  PlannedRequestField {
    name: name.into(),
    optional: false,
    schema,
    kind: RequestFieldKind::Path,
  }
}

pub(crate) fn query_field<'a>(
  name: &str,
  optional: bool,
  schema: &'a SchemaType,
) -> PlannedRequestField<'a> {
  PlannedRequestField {
    name: name.into(),
    optional,
    schema,
    kind: RequestFieldKind::Query,
  }
}

/// A field hoisted out of an inline JSON body.
pub(crate) fn body_field<'a>(
  name: &str,
  optional: bool,
  schema: &'a SchemaType,
) -> PlannedRequestField<'a> {
  PlannedRequestField {
    name: name.into(),
    optional,
    schema,
    kind: RequestFieldKind::Body,
  }
}

pub(crate) fn nested_body(schema: &SchemaType, optional: bool) -> PlannedRequestBody<'_> {
  PlannedRequestBody::Nested { schema, optional }
}

/// A hoisted JSON body, `required` being the envelope's own flag.
pub(crate) fn flat_json_body<'a>(
  properties: Vec<PlannedRequestField<'a>>,
  required: bool,
) -> PlannedRequestBody<'a> {
  PlannedRequestBody::FlatJson {
    properties,
    required,
  }
}

/// A contract with no fields, headers or body.
pub(crate) fn empty_request() -> PlannedRequestContract<'static> {
  PlannedRequestContract {
    fields: vec![],
    headers: vec![],
    body: None,
  }
}

/// An operation whose interface names follow from `operation_id`.
pub(crate) fn op_with<'a>(
  operation_id: &str,
  method: HttpMethod,
  path: &str,
  request: PlannedRequestContract<'a>,
  response: Option<&'a ResponseContent>,
) -> PlannedOperation<'a> {
  let method_name = MethodName::new(operation_id.to_string());
  let takes_input =
    !request.fields.is_empty() || request.body.is_some() || !request.headers.is_empty();
  PlannedOperation {
    operation_id: operation_id.to_string(),
    request_interface: takes_input.then(|| request_interface_name(&method_name)),
    error_interface: None,
    method_name,
    method,
    path: path.to_string(),
    request,
    response,
    errors: &[],
    description: None,
    deprecated: false,
  }
}

/// An operation carrying `errors` and nothing else.
pub(crate) fn op_with_errors<'a>(
  operation_id: &str,
  errors: &'a [crate::api_model::canonical::ErrorResponse],
) -> PlannedOperation<'a> {
  let method_name = MethodName::new(operation_id.to_string());
  PlannedOperation {
    operation_id: operation_id.to_string(),
    request_interface: None,
    error_interface: (!errors.is_empty()).then(|| error_interface_name(&method_name)),
    method_name,
    method: HttpMethod::Post,
    path: "/x".to_string(),
    request: empty_request(),
    response: None,
    errors,
    description: None,
    deprecated: false,
  }
}

fn build_form_fields<'a>(
  fields: Vec<(&str, bool, &'a BodyFieldType)>,
) -> Vec<PlannedFormField<'a>> {
  fields
    .into_iter()
    .map(|(name, optional, field_type)| PlannedFormField {
      name: Identifier::parse(name).expect("test form-field name is an identifier"),
      optional,
      field_type,
    })
    .collect()
}

/// An operation whose body is a multipart form of `(name, optional, schema)`
/// fields.
pub(crate) fn op_with_multipart_fields<'a>(
  fields: Vec<(&str, bool, &'a BodyFieldType)>,
) -> PlannedOperation<'a> {
  op_with(
    "op",
    HttpMethod::Post,
    "/op",
    PlannedRequestContract {
      fields: vec![],
      headers: vec![],
      body: Some(PlannedRequestBody::Multipart {
        fields: build_form_fields(fields),
      }),
    },
    None,
  )
}

/// [`op_with_multipart_fields`] with path/query fields and headers too.
pub(crate) fn op_with_multipart_fields_full<'a>(
  path_fields: Vec<PlannedRequestField<'a>>,
  headers: Vec<PlannedHeader<'a>>,
  form_fields: Vec<(&str, bool, &'a BodyFieldType)>,
) -> PlannedOperation<'a> {
  op_with(
    "op",
    HttpMethod::Post,
    "/op",
    PlannedRequestContract {
      fields: path_fields,
      headers,
      body: Some(PlannedRequestBody::Multipart {
        fields: build_form_fields(form_fields),
      }),
    },
    None,
  )
}

/// [`op_with_multipart_fields`] for the urlencoded flavour.
pub(crate) fn op_with_urlencoded_fields<'a>(
  fields: Vec<(&str, bool, &'a BodyFieldType)>,
) -> PlannedOperation<'a> {
  op_with(
    "op",
    HttpMethod::Post,
    "/op",
    PlannedRequestContract {
      fields: vec![],
      headers: vec![],
      body: Some(PlannedRequestBody::UrlEncoded {
        fields: build_form_fields(fields),
      }),
    },
    None,
  )
}
