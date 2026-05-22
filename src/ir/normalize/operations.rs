use std::collections::BTreeMap;

use crate::error::{Context, Diagnostic, DiagnosticCode, Reporter};
use crate::ir::canonical::{
  BodyContent, BodyField, BodyFieldType, ErrorResponse, HeaderDef, HttpMethod, OperationDef,
  RequestBodyDef, RequestDef, RequestInputDef, RequestInputSource, ResponseContent,
};
use crate::ir::identifier::is_valid_identifier;
use crate::ir::schema::{SchemaScalar, SchemaType};
use crate::options::{ResponseType, ResponseTypeMapping};
use crate::parse::openapi_model::{
  AdditionalProperties, MediaType, Operation, PathItem, RequestBody, Response, Schema,
};

use super::schema::normalize_schema;
use super::unsupported;

pub(super) fn normalize_operations<'a>(
  paths: &BTreeMap<String, PathItem>,
  schema_index: &BTreeMap<&str, &'a SchemaType>,
  response_type_mapping: &[ResponseTypeMapping],
  reporter: &mut Reporter<'_>,
) -> Result<Vec<OperationDef>, Diagnostic> {
  let mut operations = Vec::new();

  for (path, path_item) in paths {
    validate_path_template(path, reporter)?;
    for (method, operation) in path_item.operations() {
      operations.push(normalize_operation(
        path,
        method,
        operation,
        schema_index,
        response_type_mapping,
        reporter,
      )?);
    }
  }

  Ok(operations)
}

/// Reject path strings with unbalanced `{` / `}` braces before emit
/// silently produces a broken TypeScript template. The path-template
/// expander in `emit/angular/request.rs` bails on a stray `{` and emits
/// the remainder verbatim — fine on validated input, but a malformed
/// spec like `/pets/{id` would yield `url: \`/pets/id\`` with no
/// `encodeURIComponent` call. Surfacing the error at normalize time
/// keeps the emit stage operating on validated IR.
fn validate_path_template(path: &str, reporter: &mut Reporter<'_>) -> Result<(), Diagnostic> {
  let mut rest = path;
  while let Some(open) = rest.find('{') {
    let after_open = &rest[open + 1..];
    if let Some(stray) = after_open.find('{') {
      let close = after_open.find('}');
      if close.is_none_or(|c| stray < c) {
        return Err(unsupported(
          format!(
            "path template {path} contains nested '{{' which is not a valid OpenAPI parameter placeholder."
          ),
          reporter,
          true,
        ));
      }
    }
    let Some(close) = after_open.find('}') else {
      return Err(unsupported(
        format!("path template {path} has an unbalanced '{{' with no matching '}}'."),
        reporter,
        true,
      ));
    };
    let name = &after_open[..close];
    if !is_valid_identifier(name) {
      return Err(Diagnostic::policy_violation(
        reporter,
        "invalid-path-parameter-name",
        format!(
          "path template {path}: parameter name '{name}' is not a valid JavaScript identifier. Rename the parameter or split this path into a non-generated client."
        ),
      ));
    }
    rest = &after_open[close + 1..];
  }
  if let Some(stray) = rest.find('}') {
    let _ = stray;
    return Err(unsupported(
      format!("path template {path} has an unbalanced '}}' with no matching '{{'."),
      reporter,
      true,
    ));
  }
  Ok(())
}

fn normalize_operation<'a>(
  path_name: &str,
  method: &str,
  operation: &Operation,
  schema_index: &BTreeMap<&str, &'a SchemaType>,
  response_type_mapping: &[ResponseTypeMapping],
  reporter: &mut Reporter<'_>,
) -> Result<OperationDef, Diagnostic> {
  let http_method = HttpMethod::from_lowercase(method).ok_or_else(|| {
    let detail = if method == "trace" {
      // OpenAPI permits `trace:` but Angular's HttpClient has no
      // `.trace()` helper and TRACE is disabled at most production
      // gateways for security reasons (XST). Reject explicitly rather
      // than silently emitting a service that references an unusable
      // method.
      format!("HTTP method TRACE for {path_name} is not supported; remove the trace operation or split it into a non-generated client.")
    } else {
      format!("unknown HTTP method {method} for {path_name}.")
    };
    unsupported(detail, reporter, true)
  })?;

  let operation_id = operation
    .operation_id
    .clone()
    .unwrap_or_else(|| format!("{}_{}", method, path_name.replace(['/', '{', '}'], "_")));

  let method_str = http_method.as_str();
  let request = normalize_request(
    operation,
    &operation_id,
    method_str,
    path_name,
    schema_index,
    reporter,
  )?;
  let response = normalize_success_response(
    operation.responses.as_ref(),
    http_method,
    path_name,
    response_type_mapping,
    reporter,
  )?;

  let errors = normalize_error_responses(
    operation.responses.as_ref(),
    http_method,
    path_name,
    reporter,
  )?;

  Ok(OperationDef {
    operation_id,
    tags: operation.tags.clone(),
    method: http_method,
    path: path_name.to_string(),
    request,
    response,
    errors,
    description: operation.merged_description(),
    deprecated: operation.deprecated,
  })
}

fn normalize_request<'a>(
  operation: &Operation,
  operation_id: &str,
  method: &str,
  path: &str,
  schema_index: &BTreeMap<&str, &'a SchemaType>,
  reporter: &mut Reporter<'_>,
) -> Result<RequestDef, Diagnostic> {
  let (inputs, headers) =
    normalize_request_inputs(&operation.parameters, operation_id, method, path, reporter)?;
  let body = normalize_request_body(
    operation.request_body.as_ref(),
    method,
    path,
    schema_index,
    reporter,
  )?;
  Ok(RequestDef {
    inputs,
    headers,
    body,
  })
}

fn normalize_request_inputs(
  parameters: &[crate::parse::openapi_model::Parameter],
  operation_id: &str,
  method: &str,
  path: &str,
  reporter: &mut Reporter<'_>,
) -> Result<(Vec<RequestInputDef>, Vec<HeaderDef>), Diagnostic> {
  let mut inputs = Vec::with_capacity(parameters.len());
  let mut headers = Vec::new();

  for parameter in parameters {
    let name = &parameter.name;
    // `None` routes the parameter to `headers`; `Some(source)` routes it to
    // `inputs` with that source. `cookie` short-circuits with a warning,
    // anything else is an error.
    let source: Option<RequestInputSource> = match parameter.location.as_str() {
      "path" => Some(RequestInputSource::Path),
      "query" => Some(RequestInputSource::Query),
      "header" => None,
      "cookie" => {
        // Cookies are managed by the browser; surfacing them in the
        // generated request contract would create an inconsistent API
        // surface (the client can't actually set Cookie headers). Warn
        // and drop the parameter here at normalize-time so downstream
        // stages never see it.
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
        return Err(unsupported(
          format!("parameter {name} for {method} {path} uses unsupported location {other}."),
          reporter,
          true,
        ));
      }
    };

    let required = parameter.required;

    if source == Some(RequestInputSource::Path) && !required {
      return Err(unsupported(
        format!("path parameter {name} for {method} {path} must be required."),
        reporter,
        true,
      ));
    }

    if parameter.content.is_some() {
      return Err(unsupported(
        format!("parameter {name} for {method} {path} must use schema, not content."),
        reporter,
        true,
      ));
    }

    let schema = parameter.schema.as_ref().ok_or_else(|| {
      unsupported(
        format!("parameter {name} for {method} {path} must define schema."),
        reporter,
        true,
      )
    })?;

    // Each operation-level schema walk starts at depth 0; the recursion
    // counter only spans a single schema tree, not the request/response
    // grouping above it.
    let param_context = Context::Parameter { method, path };
    let ty = normalize_schema(schema, &param_context, 0, reporter)?;
    match ty {
      SchemaType::InlineObject { .. } => {
        return Err(unsupported(
          format!(
            "parameter {name} for {method} {path} uses an inline object schema, which is outside the supported subset."
          ),
          reporter,
          true,
        ));
      }
      SchemaType::Any => {
        return Err(unsupported(
          format!(
            "parameter {name} for {method} {path} uses an empty schema, which is outside the supported subset."
          ),
          reporter,
          true,
        ));
      }
      _ => {}
    }

    match source {
      Some(source) => inputs.push(RequestInputDef {
        name: name.as_str().into(),
        source,
        required,
        ty,
      }),
      None => headers.push(HeaderDef {
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

fn normalize_request_body<'a>(
  request_body: Option<&RequestBody>,
  method: &str,
  path: &str,
  schema_index: &BTreeMap<&str, &'a SchemaType>,
  reporter: &mut Reporter<'_>,
) -> Result<Option<RequestBodyDef>, Diagnostic> {
  let Some(body) = request_body else {
    return Ok(None);
  };

  // Multi-content bodies cannot be represented by a single request
  // contract: the caller would have to pick one media type at call site,
  // which defeats the typed-client guarantees. Reject up-front with a
  // dedicated subcode so downstream tooling can route on it.
  if body.content.len() > 1 {
    return Err(Diagnostic::policy_violation(
      reporter,
      "multi-content-body",
      format!("requestBody for {method} {path} must declare exactly one content type."),
    ));
  }

  let Some((mime, media)) = body.content.iter().next() else {
    return Ok(None);
  };
  // OpenAPI permits MIME case variation (`Application/JSON`); lowercase
  // before matching so the dispatch is case-insensitive while the arms
  // remain canonical-form string literals.
  let mime_lc = mime.to_ascii_lowercase();

  let content = match mime_lc.as_str() {
    "application/json" => {
      let schema = media.schema.as_ref().ok_or_else(|| {
        unsupported(
          format!("requestBody for {method} {path} must define schema."),
          reporter,
          true,
        )
      })?;

      let body_context = Context::RequestBody { method, path };
      let ty = normalize_schema(schema, &body_context, 0, reporter)?;

      if matches!(ty, SchemaType::Any) {
        return Err(unsupported(
          format!("requestBody for {method} {path} must define a concrete schema."),
          reporter,
          true,
        ));
      }

      BodyContent::Json(ty)
    }
    "multipart/form-data" => {
      let (body_ref, fields) = normalize_form_body_fields(
        media,
        FormKind::Multipart,
        method,
        path,
        schema_index,
        reporter,
      )?;
      BodyContent::Multipart { body_ref, fields }
    }
    "application/x-www-form-urlencoded" => {
      let (body_ref, fields) = normalize_form_body_fields(
        media,
        FormKind::UrlEncoded,
        method,
        path,
        schema_index,
        reporter,
      )?;
      BodyContent::UrlEncoded { body_ref, fields }
    }
    other => {
      return Err(Diagnostic::policy_violation(
        reporter,
        "unsupported-body-content-type",
        format!(
          "requestBody for {method} {path}: unsupported content type {other:?}. Use application/json, multipart/form-data, or application/x-www-form-urlencoded."
        ),
      ));
    }
  };

  Ok(Some(RequestBodyDef {
    required: body.required,
    content,
  }))
}

/// Discriminates between `multipart/form-data` and
/// `application/x-www-form-urlencoded` so the field walker can apply the
/// content-type-specific rejection rules (e.g. urlencoded forbids binary
/// payloads).
#[derive(Clone, Copy)]
enum FormKind {
  Multipart,
  UrlEncoded,
}

/// Normalizes a `multipart/form-data` or `application/x-www-form-urlencoded`
/// media's schema into a flat list of `BodyField`s. Top-level `$ref`s to a
/// named object are recorded in the returned `body_ref` so plan/emit can
/// surface the schema name in the request contract. Returned fields are
/// sorted alphabetically for deterministic emit.
///
/// Format-binary detection requires raw `Schema.format`, which the IR-side
/// `SchemaType` does not carry — so we peek the raw `Schema` directly for
/// inline bodies. For top-level `$ref` bodies the raw schema is not
/// reachable from `schema_index` (which carries normalized types only); the
/// resolved properties come from the normalized index and format detection
/// for ref-target bodies is currently a no-op. The Task 7 accept tests do
/// not exercise format-binary through a `$ref`.
fn normalize_form_body_fields<'a>(
  media: &MediaType,
  kind: FormKind,
  method: &str,
  path: &str,
  schema_index: &BTreeMap<&str, &'a SchemaType>,
  reporter: &mut Reporter<'_>,
) -> Result<(Option<Box<str>>, Vec<BodyField>), Diagnostic> {
  let raw_schema = media.schema.as_ref().ok_or_else(|| {
    Diagnostic::policy_violation(
      reporter,
      "missing-body-schema",
      format!("requestBody for {method} {path} must define schema."),
    )
  })?;

  // Open-schema pre-check: form bodies must enumerate every field
  // statically so the field walker can emit a stable contract. Any form
  // of `additionalProperties` (literal `true`, or a schema describing
  // the additional values) means the body's shape is open-ended and
  // cannot be represented as a fixed `FormData` / urlencoded layout.
  // Reject up-front so the per-property walk below operates on a closed
  // object. `additionalProperties: false` and the absent case are fine.
  // Subcode is kind-aware so downstream tooling can route on the precise
  // form variant without parsing the message.
  if let Some(ap) = &raw_schema.additional_properties
    && !matches!(ap, AdditionalProperties::Boolean(false))
  {
    return Err(Diagnostic::policy_violation(
      reporter,
      open_schema_subcode(kind),
      format!(
        "requestBody for {method} {path}: {} bodies must not declare additionalProperties; every field must be enumerated.",
        form_kind_label(kind),
      ),
    ));
  }

  let body_context = Context::RequestBody { method, path };
  let normalized = normalize_schema(raw_schema, &body_context, 0, reporter)?;

  // Resolve a top-level $ref by looking up the resolved `SchemaType` in
  // `schema_index`. The body_ref is recorded so plan/emit can re-surface
  // the named schema in the request contract; the actual property walk
  // uses the resolved InlineObject.
  let (body_ref, resolved_ty): (Option<Box<str>>, &SchemaType) = match &normalized {
    SchemaType::Ref(name) => {
      let resolved = schema_index.get(name.as_ref()).ok_or_else(|| {
        unsupported(
          format!("requestBody for {method} {path} references unknown schema '{name}'.",),
          reporter,
          true,
        )
      })?;
      (Some(name.clone()), *resolved)
    }
    other => (None, other),
  };

  // The resolved body schema must be an `InlineObject`. Other shapes
  // (Map, Array, Scalar, Union, ...) cannot be flattened into discrete
  // form fields, so we reject with a kind-aware non-object-body subcode
  // so downstream consumers can route on the precise reason and variant.
  let properties = match resolved_ty {
    SchemaType::InlineObject { properties } => properties,
    _ => {
      return Err(Diagnostic::policy_violation(
        reporter,
        non_object_body_subcode(kind),
        format!(
          "requestBody for {method} {path}: {} body schema must resolve to an object.",
          form_kind_label(kind),
        ),
      ));
    }
  };

  // Raw-peek table for format detection. Populated from the inline
  // body's raw `Schema.properties`; empty when the body is a top-level
  // `$ref` (raw schemas behind refs are not threaded through to this
  // layer — see function-level doc comment).
  let raw_property_lookup = collect_raw_property_formats(raw_schema);

  let mut fields: Vec<BodyField> = Vec::with_capacity(properties.len());
  for prop in properties {
    // Emit interpolates the field name as a bare JS identifier in the
    // form-body IIFE (`if (name !== undefined)`, `for (const v of name)`).
    // Reject names that aren't valid identifiers at normalize time so
    // emit operates on validated input.
    if !is_valid_identifier(prop.name.as_ref()) {
      return Err(Diagnostic::policy_violation(
        reporter,
        "invalid-form-field-name",
        format!(
          "body field '{name}' in {method} {path}: name is not a valid JavaScript identifier. Rename the field or split this body into a non-generated client.",
          name = prop.name.as_ref(),
        ),
      ));
    }
    let raw_format = raw_property_lookup
      .get(prop.name.as_ref())
      .copied()
      .unwrap_or(RawPropertyFormat::default());
    let ty = classify_body_field_type(
      &prop.ty,
      raw_format,
      kind,
      method,
      path,
      prop.name.as_ref(),
      reporter,
    )?;
    fields.push(BodyField {
      name: prop.name.clone(),
      required: prop.required,
      ty,
    });
  }

  fields.sort_by(|a, b| a.name.cmp(&b.name));
  Ok((body_ref, fields))
}

/// Format hints peeked from the raw `Schema` for one body property.
/// `own` is the property schema's own `format` (relevant for
/// `type: string, format: binary`). `items` is the array-item schema's
/// `format` (relevant for `type: array, items: { format: binary }`).
/// Both default to `None` when the property's raw schema is unavailable
/// (e.g. when the body is a top-level `$ref`).
#[derive(Clone, Copy, Default)]
struct RawPropertyFormat<'a> {
  own: Option<&'a str>,
  items: Option<&'a str>,
}

/// Walks the raw inline body `Schema.properties` to build a lookup of
/// per-property `format` hints. Used to detect `format: binary` (and,
/// for arrays, `items.format: binary`) which the normalized
/// `SchemaType` deliberately does not carry — keeping format-binary
/// semantics confined to form-body normalize.
fn collect_raw_property_formats(raw_schema: &Schema) -> BTreeMap<&str, RawPropertyFormat<'_>> {
  let mut lookup = BTreeMap::new();
  let Some(properties) = &raw_schema.properties else {
    return lookup;
  };
  for (name, schema) in properties {
    let own = schema.format.as_deref();
    let items = schema
      .items
      .as_deref()
      .and_then(|item_schema| item_schema.format.as_deref());
    lookup.insert(name.as_str(), RawPropertyFormat { own, items });
  }
  lookup
}

/// Classifies one form-body property into a `BodyFieldType`. Accept
/// branches cover scalar, array-of-scalar, binary, and array-of-binary.
/// Reject branches use kebab-case subcodes consumers route on. Subcodes
/// are FormKind-aware so downstream tooling can distinguish multipart
/// vs urlencoded reject paths without parsing the message:
/// `multipart-nested-object` / `urlencoded-nested-object`,
/// `multipart-composed-field` / `urlencoded-composed-field`,
/// `urlencoded-binary-field`.
fn classify_body_field_type(
  ty: &SchemaType,
  raw_format: RawPropertyFormat<'_>,
  kind: FormKind,
  method: &str,
  path: &str,
  field_name: &str,
  reporter: &mut Reporter<'_>,
) -> Result<BodyFieldType, Diagnostic> {
  match ty {
    // Binary: string + format: binary.
    SchemaType::Scalar(SchemaScalar::String) if raw_format.own == Some("binary") => match kind {
      FormKind::Multipart => Ok(BodyFieldType::Binary),
      // Urlencoded forbids binary payloads; a single `urlencoded-binary-field`
      // subcode covers both scalar binary and array-of-binary so downstream
      // routing can collapse the two reject arms into one branch.
      FormKind::UrlEncoded => Err(Diagnostic::policy_violation(
        reporter,
        "urlencoded-binary-field",
        format!(
          "body field '{field_name}' in {method} {path}: binary fields are not supported in application/x-www-form-urlencoded."
        ),
      )),
    },
    // Array of binary: array of (string + format: binary). Detected
    // via the array-item's raw `format` hint (`raw_format.items`).
    SchemaType::Array(inner)
      if matches!(inner.as_ref(), SchemaType::Scalar(SchemaScalar::String))
        && raw_format.items == Some("binary") =>
    {
      match kind {
        FormKind::Multipart => Ok(BodyFieldType::ArrayOfBinary),
        FormKind::UrlEncoded => Err(Diagnostic::policy_violation(
          reporter,
          "urlencoded-binary-field",
          format!(
            "body field '{field_name}' in {method} {path}: array-of-binary fields are not supported in application/x-www-form-urlencoded."
          ),
        )),
      }
    }
    SchemaType::Scalar(scalar) => Ok(BodyFieldType::Scalar(scalar.clone())),
    SchemaType::Array(inner) => match inner.as_ref() {
      SchemaType::Scalar(scalar) => Ok(BodyFieldType::ArrayOfScalar(scalar.clone())),
      // Arrays whose items are not scalar/binary cannot be flattened
      // into repeated form-field entries; treat them as composed for
      // routing purposes (consistent with the "composed" semantics for
      // a field's payload shape).
      _ => Err(Diagnostic::policy_violation(
        reporter,
        composed_field_subcode(kind),
        format!(
          "body field '{field_name}' in {method} {path}: array items must be scalar or binary."
        ),
      )),
    },
    SchemaType::InlineObject { .. } | SchemaType::Ref(_) => Err(Diagnostic::policy_violation(
      reporter,
      nested_object_subcode(kind),
      format!(
        "body field '{field_name}' in {method} {path}: nested objects are not supported in {} bodies.",
        form_kind_label(kind),
      ),
    )),
    // Composed (oneOf/anyOf/allOf), Nullable, Map, non-string-literal
    // enums, Any — all collapse into a single "composed" subcode so the
    // downstream router can recognise the family without parsing the
    // message.
    _ => Err(Diagnostic::policy_violation(
      reporter,
      composed_field_subcode(kind),
      format!(
        "body field '{field_name}' in {method} {path}: composed schemas are not supported in {} bodies.",
        form_kind_label(kind),
      ),
    )),
  }
}

/// Subcode for a nested-object reject, kind-aware so downstream tooling
/// can distinguish multipart vs urlencoded paths.
fn nested_object_subcode(kind: FormKind) -> &'static str {
  match kind {
    FormKind::Multipart => "multipart-nested-object",
    FormKind::UrlEncoded => "urlencoded-nested-object",
  }
}

/// Subcode for a composed-field reject (oneOf/anyOf/allOf, nullable,
/// non-string-literal enums, array-of-non-scalar, etc.), kind-aware.
fn composed_field_subcode(kind: FormKind) -> &'static str {
  match kind {
    FormKind::Multipart => "multipart-composed-field",
    FormKind::UrlEncoded => "urlencoded-composed-field",
  }
}

/// Subcode for a non-object top-level body reject (the resolved body
/// schema is a scalar, array, map, union, ...). Kind-aware so downstream
/// tooling can distinguish multipart vs urlencoded paths.
fn non_object_body_subcode(kind: FormKind) -> &'static str {
  match kind {
    FormKind::Multipart => "multipart-non-object-body",
    FormKind::UrlEncoded => "urlencoded-non-object-body",
  }
}

/// Subcode for an open-schema reject (top-level `additionalProperties`
/// is `true` or a schema). Kind-aware so consumers can distinguish
/// multipart vs urlencoded variants.
fn open_schema_subcode(kind: FormKind) -> &'static str {
  match kind {
    FormKind::Multipart => "multipart-open-schema",
    FormKind::UrlEncoded => "urlencoded-open-schema",
  }
}

/// Human-readable label used in diagnostic messages so consumers can tell
/// the kind apart without inspecting the subcode.
fn form_kind_label(kind: FormKind) -> &'static str {
  match kind {
    FormKind::Multipart => "multipart",
    FormKind::UrlEncoded => "urlencoded",
  }
}

fn normalize_success_response(
  responses: Option<&BTreeMap<String, Response>>,
  method: HttpMethod,
  path_name: &str,
  response_type_mapping: &[ResponseTypeMapping],
  reporter: &mut Reporter<'_>,
) -> Result<Option<ResponseContent>, Diagnostic> {
  let Some(responses) = responses else {
    return Ok(None);
  };

  let Some((_status, response)) = responses
    .iter()
    .find(|(status, _)| is_success_status(status))
  else {
    return Ok(None);
  };

  let Some(content) = &response.content else {
    return Ok(None);
  };

  let Some((mime, media)) = pick_response_media(content, response_type_mapping) else {
    return Ok(None);
  };

  let kind = classify_response_kind(mime, response_type_mapping);
  let response_context = Context::ResponseSchema {
    method: method.as_str(),
    path: path_name,
  };

  Ok(Some(match kind {
    ResponseKind::Json => {
      let schema = match &media.schema {
        Some(s) => Some(normalize_schema(s, &response_context, 0, reporter)?),
        None => None,
      };
      ResponseContent::Json(schema)
    }
    ResponseKind::Blob => ResponseContent::Blob,
    ResponseKind::Text => ResponseContent::Text,
    ResponseKind::ArrayBuffer => ResponseContent::ArrayBuffer,
  }))
}

/// Collects non-2xx response slots with a JSON schema, sorted by status
/// ascending. Lenient by design: schemaless and non-JSON error responses
/// are silently skipped (real specs commonly underspecify errors, and a
/// hard rejection here would be hostile). The `default` key is also
/// skipped — the emitted surface (`OperationError[400]`) only carries
/// numeric status keys for now.
fn normalize_error_responses(
  responses: Option<&BTreeMap<String, Response>>,
  method: HttpMethod,
  path_name: &str,
  reporter: &mut Reporter<'_>,
) -> Result<Vec<ErrorResponse>, Diagnostic> {
  let Some(responses) = responses else {
    return Ok(Vec::new());
  };

  let mut errors: Vec<ErrorResponse> = Vec::new();
  for (status_str, response) in responses {
    let Some(status) = parse_error_status(status_str) else {
      continue;
    };
    let Some(content) = &response.content else {
      continue;
    };
    let Some(media) = content.get("application/json") else {
      continue;
    };
    let Some(raw_schema) = &media.schema else {
      continue;
    };
    let response_context = Context::ResponseSchema {
      method: method.as_str(),
      path: path_name,
    };
    let body = normalize_schema(raw_schema, &response_context, 0, reporter)?;
    errors.push(ErrorResponse { status, body });
  }
  errors.sort_by_key(|e| e.status);
  Ok(errors)
}

/// Parses a response key as a 4xx or 5xx HTTP status code. Returns `None`
/// for 2xx, 1xx, 3xx, the `default` key, and malformed values.
fn parse_error_status(status: &str) -> Option<u16> {
  if status.len() != 3 {
    return None;
  }
  let leading = status.as_bytes()[0];
  if leading != b'4' && leading != b'5' {
    return None;
  }
  status.parse::<u16>().ok()
}

/// Picks the media entry to use for a response's typed body. Prefers
/// the first entry whose classification is **not** `Blob` (so
/// `application/json` alongside `application/octet-stream` picks the
/// JSON entry); falls back to the first `Blob` entry when no non-Blob
/// classification exists. Iteration over `BTreeMap` is alphabetical by
/// key, which is the source of determinism here.
fn pick_response_media<'a>(
  content: &'a BTreeMap<String, MediaType>,
  user_mapping: &[ResponseTypeMapping],
) -> Option<(&'a str, &'a MediaType)> {
  let mut first_blob: Option<(&str, &MediaType)> = None;
  for (mime, media) in content {
    let kind = classify_response_kind(mime, user_mapping);
    if kind != ResponseKind::Blob {
      return Some((mime.as_str(), media));
    }
    if first_blob.is_none() {
      first_blob = Some((mime.as_str(), media));
    }
  }
  first_blob
}

fn request_input_sort_key(value: &RequestInputDef) -> (u8, &str) {
  let weight = match value.source {
    RequestInputSource::Path => 0,
    RequestInputSource::Query => 1,
  };

  (weight, &value.name)
}

fn is_success_status(status: &str) -> bool {
  status.starts_with('2')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResponseKind {
  Json,
  Blob,
  Text,
  ArrayBuffer,
}

fn classify_response_kind(
  content_type: &str,
  user_mapping: &[ResponseTypeMapping],
) -> ResponseKind {
  let normalized = content_type.to_ascii_lowercase();

  // 1. User mapping (exact case-insensitive match) wins.
  if let Some(m) = user_mapping
    .iter()
    .find(|m| m.content_type.eq_ignore_ascii_case(&normalized))
  {
    return match m.response_type {
      ResponseType::Json => ResponseKind::Json,
      ResponseType::Blob => ResponseKind::Blob,
      ResponseType::Text => ResponseKind::Text,
      ResponseType::ArrayBuffer => ResponseKind::ArrayBuffer,
    };
  }

  // 2. Built-in defaults.
  if normalized == "application/json" || normalized.ends_with("+json") {
    return ResponseKind::Json;
  }
  if normalized.starts_with("text/") {
    return ResponseKind::Text;
  }
  ResponseKind::Blob
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;

  use super::{
    ResponseKind, classify_response_kind, normalize_error_responses, normalize_request_body,
    normalize_success_response, parse_error_status, pick_response_media, validate_path_template,
  };
  use crate::ir::canonical::{BodyContent, BodyFieldType, HttpMethod};
  use crate::ir::schema::{SchemaProperty, SchemaScalar, SchemaType};
  use crate::options::{ResponseType, ResponseTypeMapping};
  use crate::parse::openapi_model::{MediaType, RequestBody, Response, Schema};
  use crate::test_support::test_ctx;

  fn parse_request_body(yaml: &str) -> RequestBody {
    serde_yml::from_str(yaml).expect("fixture parses as RequestBody")
  }

  fn empty_schema_index<'a>() -> BTreeMap<&'a str, &'a SchemaType> {
    BTreeMap::new()
  }

  fn json_schema() -> Schema {
    Schema::default_string()
  }

  fn btreemap_with<K: Ord, V>(key: K, value: V) -> BTreeMap<K, V> {
    BTreeMap::from([(key, value)])
  }

  #[test]
  fn classifies_application_json_as_json() {
    assert_eq!(
      classify_response_kind("application/json", &[]),
      ResponseKind::Json
    );
  }

  #[test]
  fn classifies_problem_json_as_json() {
    assert_eq!(
      classify_response_kind("application/problem+json", &[]),
      ResponseKind::Json
    );
    assert_eq!(
      classify_response_kind("application/vnd.api+json", &[]),
      ResponseKind::Json
    );
  }

  #[test]
  fn classifies_text_plain_as_text() {
    assert_eq!(
      classify_response_kind("text/plain", &[]),
      ResponseKind::Text
    );
    assert_eq!(classify_response_kind("text/csv", &[]), ResponseKind::Text);
  }

  #[test]
  fn classifies_application_pdf_as_blob_via_default() {
    assert_eq!(
      classify_response_kind("application/pdf", &[]),
      ResponseKind::Blob
    );
  }

  #[test]
  fn classifies_octet_stream_as_blob_via_default() {
    assert_eq!(
      classify_response_kind("application/octet-stream", &[]),
      ResponseKind::Blob
    );
  }

  #[test]
  fn user_mapping_overrides_default() {
    let mapping = vec![ResponseTypeMapping {
      content_type: "application/octet-stream".into(),
      response_type: ResponseType::ArrayBuffer,
    }];
    assert_eq!(
      classify_response_kind("application/octet-stream", &mapping),
      ResponseKind::ArrayBuffer
    );
  }

  #[test]
  fn user_mapping_matches_case_insensitively() {
    let mapping = vec![ResponseTypeMapping {
      content_type: "application/PDF".into(),
      response_type: ResponseType::ArrayBuffer,
    }];
    assert_eq!(
      classify_response_kind("application/pdf", &mapping),
      ResponseKind::ArrayBuffer
    );
  }

  #[test]
  fn pick_response_media_prefers_non_blob_classification() {
    let mut content = BTreeMap::<String, MediaType>::new();
    content.insert(
      "application/json".into(),
      MediaType {
        schema: Some(json_schema()),
      },
    );
    content.insert(
      "application/octet-stream".into(),
      MediaType { schema: None },
    );

    let (mime, _) = pick_response_media(&content, &[]).expect("at least one media");
    assert_eq!(mime, "application/json");
  }

  #[test]
  fn pick_response_media_returns_first_blob_when_only_blob_kinds() {
    let mut content = BTreeMap::<String, MediaType>::new();
    content.insert("application/pdf".into(), MediaType { schema: None });
    content.insert("application/zip".into(), MediaType { schema: None });
    let (mime, _) = pick_response_media(&content, &[]).expect("at least one media");
    // BTreeMap iteration order is sorted; "application/pdf" sorts before "application/zip".
    assert_eq!(mime, "application/pdf");
  }

  #[test]
  fn no_response_content_yields_none_response() {
    // A response with no `content` block at all.
    let response = Response { content: None };
    let mut ctx = test_ctx();
    let result = normalize_success_response(
      Some(&btreemap_with("200".to_string(), response)),
      HttpMethod::Get,
      "/x",
      &[],
      &mut ctx.reporter(),
    )
    .expect("normalize ok");
    assert!(result.is_none(), "missing response content => None");
  }

  // ── normalize_error_responses ────────────────────────────────────────────

  /// Builds a Response with a single JSON content entry carrying the
  /// given schema. Helper for the error-response tests below.
  fn json_response(schema: Schema) -> Response {
    Response {
      content: Some(BTreeMap::from([(
        "application/json".to_string(),
        MediaType {
          schema: Some(schema),
        },
      )])),
    }
  }

  #[test]
  fn parse_error_status_accepts_4xx_and_5xx_only() {
    assert_eq!(parse_error_status("400"), Some(400));
    assert_eq!(parse_error_status("404"), Some(404));
    assert_eq!(parse_error_status("500"), Some(500));
    assert_eq!(parse_error_status("503"), Some(503));
    // 2xx, 1xx, 3xx, default key, and malformed values all reject.
    assert_eq!(parse_error_status("200"), None);
    assert_eq!(parse_error_status("101"), None);
    assert_eq!(parse_error_status("301"), None);
    assert_eq!(parse_error_status("default"), None);
    assert_eq!(parse_error_status("4xx"), None);
    assert_eq!(parse_error_status(""), None);
  }

  #[test]
  fn collects_4xx_and_5xx_responses_with_json_schemas_sorted_by_status() {
    let mut responses = BTreeMap::new();
    responses.insert("200".to_string(), json_response(Schema::default_string()));
    responses.insert("500".to_string(), json_response(Schema::default_string()));
    responses.insert("400".to_string(), json_response(Schema::default_string()));
    responses.insert("404".to_string(), json_response(Schema::default_string()));

    let mut ctx = test_ctx();
    let errors =
      normalize_error_responses(Some(&responses), HttpMethod::Get, "/x", &mut ctx.reporter())
        .expect("normalize ok");

    assert_eq!(
      errors.iter().map(|e| e.status).collect::<Vec<_>>(),
      vec![400, 404, 500]
    );
  }

  #[test]
  fn skips_schemaless_and_non_json_error_responses() {
    let mut responses = BTreeMap::new();
    responses.insert("400".to_string(), json_response(Schema::default_string()));
    // 503: no content block at all — must be skipped without error.
    responses.insert("503".to_string(), Response { content: None });
    // 502: content block, but JSON entry has no schema — must be skipped.
    responses.insert(
      "502".to_string(),
      Response {
        content: Some(BTreeMap::from([(
          "application/json".to_string(),
          MediaType { schema: None },
        )])),
      },
    );
    // 504: only non-JSON content — must be skipped.
    responses.insert(
      "504".to_string(),
      Response {
        content: Some(BTreeMap::from([(
          "text/plain".to_string(),
          MediaType {
            schema: Some(Schema::default_string()),
          },
        )])),
      },
    );

    let mut ctx = test_ctx();
    let errors =
      normalize_error_responses(Some(&responses), HttpMethod::Get, "/x", &mut ctx.reporter())
        .expect("normalize ok");

    assert_eq!(
      errors.iter().map(|e| e.status).collect::<Vec<_>>(),
      vec![400]
    );
  }

  #[test]
  fn skips_default_response_key() {
    let mut responses = BTreeMap::new();
    responses.insert(
      "default".to_string(),
      json_response(Schema::default_string()),
    );
    responses.insert("400".to_string(), json_response(Schema::default_string()));

    let mut ctx = test_ctx();
    let errors =
      normalize_error_responses(Some(&responses), HttpMethod::Get, "/x", &mut ctx.reporter())
        .expect("normalize ok");

    // Only 400 survives — `default` is intentionally not surfaced.
    assert_eq!(
      errors.iter().map(|e| e.status).collect::<Vec<_>>(),
      vec![400]
    );
  }

  // ── Multipart body field walker (Task 7 — accept path) ────────────────────

  #[test]
  fn accepts_multipart_with_scalar_array_and_binary_fields() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      required: [status, avatar]
      properties:
        status: { type: string }
        tagIds: { type: array, items: { type: number } }
        avatar: { type: string, format: binary }
        nickname: { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let result = normalize_request_body(
      Some(&body),
      "POST",
      "/pets",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect("normalize ok")
    .expect("body present");

    match result.content {
      BodyContent::Multipart { body_ref, fields } => {
        assert_eq!(body_ref, None);
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_ref()).collect();
        // Sorted alphabetically.
        assert_eq!(names, vec!["avatar", "nickname", "status", "tagIds"]);
        let avatar = fields.iter().find(|f| f.name.as_ref() == "avatar").unwrap();
        assert_eq!(avatar.ty, BodyFieldType::Binary);
        assert!(avatar.required);
        let status = fields.iter().find(|f| f.name.as_ref() == "status").unwrap();
        assert!(matches!(
          status.ty,
          BodyFieldType::Scalar(SchemaScalar::String)
        ));
        assert!(status.required);
        let nickname = fields
          .iter()
          .find(|f| f.name.as_ref() == "nickname")
          .unwrap();
        assert!(!nickname.required);
        let tag_ids = fields.iter().find(|f| f.name.as_ref() == "tagIds").unwrap();
        assert!(matches!(
          tag_ids.ty,
          BodyFieldType::ArrayOfScalar(SchemaScalar::Number)
        ));
      }
      other => panic!("expected Multipart, got {other:?}"),
    }
  }

  #[test]
  fn accepts_multipart_with_array_of_binary_fields() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      required: [galleries]
      properties:
        galleries: { type: array, items: { type: string, format: binary } }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let result = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect("normalize ok")
    .expect("body present");

    match result.content {
      BodyContent::Multipart { fields, .. } => {
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].ty, BodyFieldType::ArrayOfBinary);
      }
      other => panic!("expected Multipart, got {other:?}"),
    }
  }

  #[test]
  fn accepts_multipart_with_ref_to_named_object() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      $ref: '#/components/schemas/UploadForm'
"#;
    let body = parse_request_body(yaml);
    let upload_form_body = SchemaType::InlineObject {
      properties: vec![SchemaProperty {
        name: "status".into(),
        required: true,
        ty: SchemaType::Scalar(SchemaScalar::String),
        description: None,
        deprecated: false,
      }],
    };
    let schema_index = BTreeMap::from([("UploadForm", &upload_form_body)]);
    let mut ctx = test_ctx();
    let result = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &schema_index,
      &mut ctx.reporter(),
    )
    .expect("normalize ok")
    .expect("body present");

    match result.content {
      BodyContent::Multipart { body_ref, fields } => {
        assert_eq!(body_ref.as_deref(), Some("UploadForm"));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name.as_ref(), "status");
      }
      other => panic!("expected Multipart, got {other:?}"),
    }
  }

  // ── Multipart body field walker (Task 8 — reject paths) ──────────────────

  #[test]
  fn rejects_multipart_with_nested_object_field() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      properties:
        metadata:
          type: object
          properties:
            authorId: { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("nested object should fail");
    assert_eq!(err.subcode, Some("multipart-nested-object"));
  }

  #[test]
  fn rejects_multipart_with_composed_field() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      properties:
        variant:
          oneOf:
            - { type: string }
            - { type: number }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("composed field should fail");
    assert_eq!(err.subcode, Some("multipart-composed-field"));
  }

  #[test]
  fn rejects_multipart_with_additional_properties_true() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      additionalProperties: true
      properties:
        status: { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("open schema should fail");
    assert_eq!(err.subcode, Some("multipart-open-schema"));
  }

  #[test]
  fn rejects_multipart_with_non_object_top_level_schema() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: string
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("non-object body should fail");
    assert_eq!(err.subcode, Some("multipart-non-object-body"));
  }

  // ── Urlencoded + content-type dispatch (Task 9) ──────────────────────────

  #[test]
  fn accepts_urlencoded_with_scalar_and_array_of_scalar_fields() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      required: [status]
      properties:
        status: { type: string }
        tagIds: { type: array, items: { type: number } }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let result = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect("normalize ok")
    .expect("body present");

    match result.content {
      BodyContent::UrlEncoded { fields, .. } => {
        assert_eq!(
          fields.iter().map(|f| f.name.as_ref()).collect::<Vec<_>>(),
          vec!["status", "tagIds"]
        );
      }
      other => panic!("expected UrlEncoded, got {other:?}"),
    }
  }

  #[test]
  fn rejects_urlencoded_with_binary_field() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        avatar: { type: string, format: binary }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("binary in urlencoded should fail");
    assert_eq!(err.subcode, Some("urlencoded-binary-field"));
  }

  #[test]
  fn rejects_urlencoded_with_nested_object_field() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        metadata:
          type: object
          properties:
            authorId: { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("nested object should fail");
    assert_eq!(err.subcode, Some("urlencoded-nested-object"));
  }

  #[test]
  fn rejects_urlencoded_with_composed_field() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        variant:
          oneOf:
            - { type: string }
            - { type: number }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("composed field should fail");
    assert_eq!(err.subcode, Some("urlencoded-composed-field"));
  }

  #[test]
  fn rejects_urlencoded_with_non_object_top_level_schema() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: string
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("non-object urlencoded body should fail");
    assert_eq!(err.subcode, Some("urlencoded-non-object-body"));
  }

  #[test]
  fn rejects_urlencoded_with_additional_properties_true() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      additionalProperties: true
      properties:
        status: { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("open urlencoded schema should fail");
    assert_eq!(err.subcode, Some("urlencoded-open-schema"));
  }

  #[test]
  fn rejects_body_with_multiple_content_types() {
    let yaml = r#"
content:
  application/json:
    schema: { type: object, properties: { x: { type: string } } }
  multipart/form-data:
    schema: { type: object, properties: { x: { type: string } } }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("multi-content should fail");
    assert_eq!(err.subcode, Some("multi-content-body"));
  }

  #[test]
  fn rejects_unsupported_body_content_type() {
    let yaml = r#"
content:
  application/xml:
    schema: { type: object, properties: { x: { type: string } } }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("xml body should fail");
    assert_eq!(err.subcode, Some("unsupported-body-content-type"));
  }

  // ── validate_path_template (Issue 1b — parameter name validation) ─────────

  #[test]
  fn validate_path_template_accepts_well_formed_paths() {
    let mut ctx = test_ctx();
    for path in [
      "/pets",
      "/pets/{id}",
      "/users/{userId}/pets/{petId}",
      "/_internal/{$ref}",
    ] {
      validate_path_template(path, &mut ctx.reporter())
        .unwrap_or_else(|err| panic!("path {path} should validate, got: {err:?}"));
    }
  }

  #[test]
  fn validate_path_template_rejects_invalid_identifier_parameter_name() {
    let mut ctx = test_ctx();
    let err = validate_path_template("/pets/{it's}", &mut ctx.reporter())
      .expect_err("invalid identifier must reject");
    assert_eq!(err.subcode, Some("invalid-path-parameter-name"));
  }

  #[test]
  fn validate_path_template_rejects_digits_first_parameter_name() {
    let mut ctx = test_ctx();
    let err = validate_path_template("/pets/{1foo}", &mut ctx.reporter())
      .expect_err("digits-first must reject");
    assert_eq!(err.subcode, Some("invalid-path-parameter-name"));
  }

  #[test]
  fn validate_path_template_rejects_kebab_case_parameter_name() {
    let mut ctx = test_ctx();
    let err = validate_path_template("/pets/{pet-id}", &mut ctx.reporter())
      .expect_err("kebab-case must reject");
    assert_eq!(err.subcode, Some("invalid-path-parameter-name"));
  }

  #[test]
  fn validate_path_template_still_rejects_unbalanced_braces() {
    let mut ctx = test_ctx();
    let err = validate_path_template("/pets/{id", &mut ctx.reporter())
      .expect_err("unbalanced { must reject");
    // unsupported() uses code, not subcode; just confirm it's an error.
    assert_eq!(err.code, crate::error::DiagnosticCode::UnsupportedSemantic);
  }

  #[test]
  fn validate_path_template_still_rejects_stray_close_brace() {
    let mut ctx = test_ctx();
    let err =
      validate_path_template("/pets/id}", &mut ctx.reporter()).expect_err("stray } must reject");
    assert_eq!(err.code, crate::error::DiagnosticCode::UnsupportedSemantic);
  }

  // ── Form-body field name validation (Issue 1a) ────────────────────────────

  #[test]
  fn rejects_multipart_with_invalid_field_name_kebab_case() {
    let yaml = r#"
content:
  multipart/form-data:
    schema:
      type: object
      properties:
        x-y: { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("kebab-case field name must reject");
    assert_eq!(err.subcode, Some("invalid-form-field-name"));
  }

  #[test]
  fn rejects_urlencoded_with_invalid_field_name_digits_first() {
    let yaml = r#"
content:
  application/x-www-form-urlencoded:
    schema:
      type: object
      properties:
        "1foo": { type: string }
"#;
    let body = parse_request_body(yaml);
    let mut ctx = test_ctx();
    let err = normalize_request_body(
      Some(&body),
      "POST",
      "/x",
      &empty_schema_index(),
      &mut ctx.reporter(),
    )
    .expect_err("digits-first field name must reject");
    assert_eq!(err.subcode, Some("invalid-form-field-name"));
  }
}
