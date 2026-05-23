use std::rc::Rc;

use crate::{
  error::{Diagnostic, Reporter},
  ir::{
    canonical::{BodyFieldType, HttpMethod, ResponseContent},
    schema::{SchemaProperty, SchemaScalar, SchemaType},
  },
  plan::artifact_plan::{
    PlannedFormField, PlannedHeader, PlannedOperation, PlannedRequestBody, PlannedRequestContract,
    PlannedRequestField, RequestFieldKind,
  },
};

/// Self-contained reporter scaffolding for tests. Owns the warnings vec so
/// individual tests don't have to manage the borrow themselves; expose
/// the warnings borrow through `.reporter()` so tests can either ignore
/// warnings (treat as `&Reporter<'_>`) or push warnings via
/// `&mut Reporter<'_>`.
pub(crate) struct TestReporter {
  pub(crate) path: Rc<str>,
  pub(crate) warnings: Vec<Diagnostic>,
}

impl TestReporter {
  pub(crate) fn new(path: impl Into<Rc<str>>) -> Self {
    Self {
      path: path.into(),
      warnings: Vec::new(),
    }
  }

  pub(crate) fn reporter(&mut self) -> Reporter<'_> {
    Reporter::new(Rc::clone(&self.path), &mut self.warnings)
  }
}

pub(crate) fn test_ctx() -> TestReporter {
  TestReporter::new("test")
}

pub(crate) fn property(name: &str, required: bool, ty: SchemaType) -> SchemaProperty {
  SchemaProperty {
    name: name.into(),
    required,
    ty,
    description: None,
    deprecated: false,
  }
}

pub(crate) fn nullable_property(name: &str, required: bool, ty: SchemaType) -> SchemaProperty {
  SchemaProperty {
    name: name.into(),
    required,
    ty: SchemaType::Nullable(Box::new(ty)),
    description: None,
    deprecated: false,
  }
}

// ── Request-field / operation fixture builders ────────────────────────────────

/// A plain `string` scalar — the most common field type used in test fixtures.
pub(crate) fn string_ty() -> SchemaType {
  SchemaType::Scalar(SchemaScalar::String)
}

pub(crate) fn path_field<'a>(name: &str, ty: &'a SchemaType) -> PlannedRequestField<'a> {
  PlannedRequestField {
    name: name.into(),
    optional: false,
    ty,
    kind: RequestFieldKind::Path,
  }
}

pub(crate) fn query_field<'a>(
  name: &str,
  optional: bool,
  ty: &'a SchemaType,
) -> PlannedRequestField<'a> {
  PlannedRequestField {
    name: name.into(),
    optional,
    ty,
    kind: RequestFieldKind::Query,
  }
}

/// Build a `PlannedRequestField` of kind `Body` for tests that exercise the
/// FlatJson body layout (inline JSON object body whose properties hoisted to
/// top-level).
pub(crate) fn body_field<'a>(
  name: &str,
  optional: bool,
  ty: &'a SchemaType,
) -> PlannedRequestField<'a> {
  PlannedRequestField {
    name: name.into(),
    optional,
    ty,
    kind: RequestFieldKind::Body,
  }
}

/// A `PlannedRequestBody::Nested` carrier with the given `ty` and optionality.
pub(crate) fn nested_body(ty: &SchemaType, optional: bool) -> PlannedRequestBody<'_> {
  PlannedRequestBody::Nested { ty, optional }
}

/// A `PlannedRequestBody::FlatJson` carrier whose properties are the given
/// `Body`-kinded fields. `required` records whether the envelope was
/// `requestBody.required: true`.
pub(crate) fn flat_json_body<'a>(
  properties: Vec<PlannedRequestField<'a>>,
  required: bool,
) -> PlannedRequestBody<'a> {
  PlannedRequestBody::FlatJson {
    properties,
    required,
  }
}

/// Returns a `PlannedRequestContract` with no fields, headers, or body.
/// Useful in tests that care about operation structure but not request shape.
pub(crate) fn empty_request() -> PlannedRequestContract<'static> {
  PlannedRequestContract {
    fields: vec![],
    headers: vec![],
    body: None,
  }
}

/// Constructs a minimal `PlannedOperation` with the given parameters.
/// `response` is `None` for operations without a typed response.
pub(crate) fn op_with<'a>(
  operation_id: &str,
  method: HttpMethod,
  path: &str,
  request: PlannedRequestContract<'a>,
  response: Option<&'a ResponseContent>,
) -> PlannedOperation<'a> {
  PlannedOperation {
    operation_id: operation_id.to_string(),
    method_name: operation_id.to_string(),
    method,
    path: path.to_string(),
    request,
    response,
    errors: &[],
    description: None,
    deprecated: false,
  }
}

/// Variant of `op_with` that attaches an error-response slice. Borrows
/// the slice from the caller — typical use is `&[ErrorResponse{...},
/// ...]` constructed in the test body.
pub(crate) fn op_with_errors<'a>(
  operation_id: &str,
  errors: &'a [crate::ir::canonical::ErrorResponse],
) -> PlannedOperation<'a> {
  PlannedOperation {
    operation_id: operation_id.to_string(),
    method_name: operation_id.to_string(),
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
    .map(|(name, optional, ty)| PlannedFormField {
      name: name.into(),
      optional,
      ty,
    })
    .collect()
}

/// Constructs a `PlannedOperation` whose request body is a multipart form,
/// populated with the supplied form fields. `fields` and `headers` on the
/// contract are empty. Each tuple is `(name, optional, ty)` where `ty` is
/// borrowed from the caller (matching the IR-borrowing convention of
/// `PlannedFormField`).
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

/// Constructs a `PlannedOperation` whose request body is a multipart form
/// alongside non-empty `path/query` fields and/or `headers`. Mirror of
/// `op_with_multipart_fields` but lets a caller supply path/query fields
/// (typically `path_field(...)` / `query_field(...)`) and a list of
/// `PlannedHeader`s in addition to the form fields.
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

/// Constructs a `PlannedOperation` whose request body is a url-encoded form,
/// populated with the supplied form fields. Mirror of
/// `op_with_multipart_fields` for the urlencoded variant.
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
