use crate::ir::schema::{SchemaScalar, SchemaType};

/// Named, top-level schema declaration. Produced by normalize and consumed
/// by the IR validator and downstream emitters. The body is a `SchemaType`
/// — interface, enum, and alias shapes all use the same carrier:
///
/// * `SchemaType::InlineObject { properties }` → `export interface X { ... }`
/// * `SchemaType::StringLiterals { values }`   → `export type X = 'a' | 'b'`
/// * any other variant                          → `export type X = ...`
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelSymbol {
  pub(crate) name: Box<str>,
  pub(crate) description: Option<String>,
  /// Source schema's OpenAPI `deprecated: true`. Surfaces as
  /// `@deprecated` in the JSDoc above the emitted TS declaration.
  pub(crate) deprecated: bool,
  pub(crate) body: SchemaType,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RequestDef {
  pub(crate) inputs: Vec<RequestInputDef>,
  /// `in: header` parameters. Kept separate from `inputs` so plan and
  /// emit treat them as a structurally distinct group — the request
  /// interface renders them as a nested `headers: { ... }` field that is
  /// threaded through to `CommonRequest.headers`.
  pub(crate) headers: Vec<HeaderDef>,
  pub(crate) body: Option<RequestBodyDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestInputDef {
  pub(crate) name: Box<str>,
  pub(crate) source: RequestInputSource,
  pub(crate) required: bool,
  pub(crate) ty: SchemaType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RequestInputSource {
  Path,
  Query,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HeaderDef {
  pub(crate) name: Box<str>,
  pub(crate) required: bool,
  pub(crate) ty: SchemaType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequestBodyDef {
  pub(crate) required: bool,
  pub(crate) content: BodyContent,
}

/// Typed carrier for an operation's request-body content. `Json`
/// keeps the existing schema-carrying behaviour; `Multipart` and
/// `UrlEncoded` carry a flat field list (with an optional
/// `body_ref` recording the source named schema when the body was
/// declared as a top-level `$ref`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BodyContent {
  Json(SchemaType),
  Multipart {
    body_ref: Option<Box<str>>,
    fields: Vec<BodyField>,
  },
  UrlEncoded {
    body_ref: Option<Box<str>>,
    fields: Vec<BodyField>,
  },
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyField {
  pub(crate) name: Box<str>,
  pub(crate) required: bool,
  pub(crate) ty: BodyFieldType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BodyFieldType {
  #[allow(dead_code)]
  Scalar(SchemaScalar),
  #[allow(dead_code)]
  ArrayOfScalar(SchemaScalar),
  #[allow(dead_code)]
  Binary,
  #[allow(dead_code)]
  ArrayOfBinary,
}

// TRACE is intentionally absent: Angular's HttpClient has no `.trace()`
// method, and TRACE is disabled at most production gateways for security
// reasons (XST). Specs that include it are rejected explicitly at
// normalize-time (`normalize_operation`) so the failure is visible
// rather than a silent drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HttpMethod {
  Get,
  Post,
  Put,
  Delete,
  Patch,
  Options,
  Head,
}

impl HttpMethod {
  pub(crate) const fn as_str(self) -> &'static str {
    match self {
      Self::Get => "GET",
      Self::Post => "POST",
      Self::Put => "PUT",
      Self::Delete => "DELETE",
      Self::Patch => "PATCH",
      Self::Options => "OPTIONS",
      Self::Head => "HEAD",
    }
  }

  pub(crate) fn from_lowercase(value: &str) -> Option<Self> {
    // NOTE: TRACE intentionally returns None here; the strict rejection
    // (with its TRACE-specific remediation message) happens in
    // `ir::normalize::operations::normalize_operation`, which inspects the
    // raw `method` string after this returns None.
    match value {
      "get" => Some(Self::Get),
      "post" => Some(Self::Post),
      "put" => Some(Self::Put),
      "delete" => Some(Self::Delete),
      "patch" => Some(Self::Patch),
      "options" => Some(Self::Options),
      "head" => Some(Self::Head),
      _ => None,
    }
  }
}

impl std::fmt::Display for HttpMethod {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OperationDef {
  pub(crate) operation_id: String,
  pub(crate) tags: Vec<String>,
  pub(crate) method: HttpMethod,
  pub(crate) path: String,
  pub(crate) request: RequestDef,
  pub(crate) response: Option<ResponseContent>,
  /// Non-2xx responses with a JSON schema, sorted by status ascending.
  /// Populated by normalize regardless of emit config; the `errors` emit
  /// target reads from here. Schemaless or non-JSON error responses are
  /// silently skipped — error responses in real specs are commonly
  /// underspecified, so the strict-rejection model used for success
  /// responses would be hostile here.
  pub(crate) errors: Vec<ErrorResponse>,
  /// Combined `summary` (first line) and `description` (subsequent
  /// paragraph) from the OpenAPI Operation. Rendered as a JSDoc block
  /// above the service operation member.
  pub(crate) description: Option<String>,
  /// Source operation's OpenAPI `deprecated: true`. Surfaces as
  /// `@deprecated` in the JSDoc above the service method so call sites
  /// see the IDE deprecation marker.
  pub(crate) deprecated: bool,
}

/// One non-2xx response slot keyed by HTTP status. `default` and 1xx/3xx
/// are intentionally excluded — the strict typing surface the errors emit
/// builds (`OpError[400]`) only makes sense for explicit 4xx/5xx codes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ErrorResponse {
  pub(crate) status: u16,
  pub(crate) body: SchemaType,
}

/// Typed carrier for an operation's success-response content. `Json`
/// keeps the existing schema-carrying behaviour (with `None` covering
/// JSON responses that declare no schema); `Blob`, `Text`, and
/// `ArrayBuffer` will be produced by the response-kind classifier in a
/// later phase so non-JSON responses can be rendered with the right
/// `HttpClient` responseType. They carry no payload because their TS
/// surface is fixed (`Blob` / `string` / `ArrayBuffer`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResponseContent {
  Json(Option<SchemaType>),
  Blob,
  Text,
  ArrayBuffer,
}

#[derive(Clone, Debug)]
pub(crate) struct ApiInfo {
  pub(crate) spec_version: String,
  pub(crate) title: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ApiModel {
  pub(crate) info: ApiInfo,
  pub(crate) schemas: Vec<ModelSymbol>,
  pub(crate) operations: Vec<OperationDef>,
}

#[cfg(test)]
mod tests {
  use super::*;

  // ── HttpMethod ─────────────────────────────────────────────────────────────
  //
  // The rest of this module is plain data carriers — `ModelSymbol`,
  // `OperationDef`, `RequestDef`, `ApiInfo`, `ApiModel`. They hold no
  // logic worth testing in isolation; their behaviour is exercised by
  // every normalize/plan/emit test that builds an `ApiModel`. Only
  // `HttpMethod` carries a parse path (`from_lowercase`) and a render
  // path (`as_str` / `Display`) that benefit from direct coverage.

  #[test]
  fn http_method_round_trips_lowercase_keyword_to_uppercase_string() {
    let cases = [
      ("get", HttpMethod::Get, "GET"),
      ("post", HttpMethod::Post, "POST"),
      ("put", HttpMethod::Put, "PUT"),
      ("delete", HttpMethod::Delete, "DELETE"),
      ("patch", HttpMethod::Patch, "PATCH"),
      ("options", HttpMethod::Options, "OPTIONS"),
      ("head", HttpMethod::Head, "HEAD"),
    ];
    for (keyword, variant, rendered) in cases {
      assert_eq!(HttpMethod::from_lowercase(keyword), Some(variant));
      assert_eq!(variant.as_str(), rendered);
      assert_eq!(format!("{variant}"), rendered);
    }
  }

  #[test]
  fn http_method_rejects_trace_so_normalize_can_emit_a_targeted_diagnostic() {
    // TRACE returns None here — the strict rejection with its
    // remediation message lives in `normalize_operation` (the comment
    // on `from_lowercase` explains why this is split).
    assert_eq!(HttpMethod::from_lowercase("trace"), None);
  }

  #[test]
  fn http_method_rejects_uppercase_and_unknown_keywords() {
    // `from_lowercase` is strict about casing — the caller normalises
    // the method string before invoking this. Asserting the strictness
    // pins the contract so a refactor doesn't silently start accepting
    // mixed-case input.
    assert_eq!(HttpMethod::from_lowercase("GET"), None);
    assert_eq!(HttpMethod::from_lowercase("Get"), None);
    assert_eq!(HttpMethod::from_lowercase("connect"), None);
    assert_eq!(HttpMethod::from_lowercase(""), None);
  }

  #[test]
  fn body_content_variants_have_distinct_payload_shapes() {
    use crate::ir::schema::{SchemaScalar, SchemaType};

    let json = BodyContent::Json(SchemaType::Scalar(SchemaScalar::String));
    let multipart = BodyContent::Multipart {
      body_ref: None,
      fields: vec![BodyField {
        name: "avatar".into(),
        required: true,
        ty: BodyFieldType::Binary,
      }],
    };
    let url_encoded = BodyContent::UrlEncoded {
      body_ref: Some("LoginForm".into()),
      fields: vec![BodyField {
        name: "username".into(),
        required: true,
        ty: BodyFieldType::Scalar(SchemaScalar::String),
      }],
    };

    assert!(matches!(json, BodyContent::Json(_)));
    assert!(matches!(multipart, BodyContent::Multipart { .. }));
    assert!(matches!(url_encoded, BodyContent::UrlEncoded { .. }));
  }

  #[test]
  fn body_field_type_variants_cover_value_space() {
    use crate::ir::schema::SchemaScalar;

    let scalar = BodyFieldType::Scalar(SchemaScalar::String);
    let array_of_scalar = BodyFieldType::ArrayOfScalar(SchemaScalar::Number);
    let binary = BodyFieldType::Binary;
    let array_of_binary = BodyFieldType::ArrayOfBinary;

    for variant in [&scalar, &array_of_scalar, &binary, &array_of_binary] {
      let _ = format!("{variant:?}"); // ensures Debug is derived
    }
  }

  #[test]
  fn response_content_variants_carry_expected_payloads() {
    use crate::ir::schema::{SchemaScalar, SchemaType};

    let json_with_schema = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let json_without = ResponseContent::Json(None);
    let blob = ResponseContent::Blob;
    let text = ResponseContent::Text;
    let array_buffer = ResponseContent::ArrayBuffer;

    // Json variant carries an Option<SchemaType>; others carry no payload.
    assert!(matches!(json_with_schema, ResponseContent::Json(Some(_))));
    assert!(matches!(json_without, ResponseContent::Json(None)));
    assert!(matches!(blob, ResponseContent::Blob));
    assert!(matches!(text, ResponseContent::Text));
    assert!(matches!(array_buffer, ResponseContent::ArrayBuffer));
  }
}
