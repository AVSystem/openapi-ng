use crate::ident::Ident;
use crate::ir::schema::{SchemaScalar, SchemaType};

/// A named, top-level schema declaration.
///
/// Interface, enum and alias shapes share one `body` carrier:
///
/// * `SchemaType::InlineObject { properties }` → `export interface X { … }`
/// * `SchemaType::StringLiterals { values }`   → `export type X = 'a' | 'b'`
/// * any other variant                         → `export type X = …`
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModelSymbol {
  pub(crate) name: Box<str>,
  pub(crate) description: Option<String>,
  /// The source schema declared `deprecated: true`.
  pub(crate) deprecated: bool,
  pub(crate) body: SchemaType,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RequestDef {
  pub(crate) inputs: Vec<RequestInputDef>,
  /// `in: header` parameters. Travel in a different request slot to `inputs`.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// An operation's request-body content. A form variant's `body_ref` names
/// the source schema when the body was declared as a top-level `$ref`.
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

/// One field of a `multipart/form-data` or urlencoded body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BodyField {
  pub(crate) name: Ident,
  pub(crate) required: bool,
  pub(crate) ty: BodyFieldType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BodyFieldType {
  Scalar(SchemaScalar),
  ArrayOfScalar(SchemaScalar),
  Binary,
  ArrayOfBinary,
}

/// The HTTP methods the generator supports. A spec declaring TRACE is
/// rejected with its own diagnostic.
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

  /// The lower-case name, as an OpenAPI path item spells it.
  pub(crate) const fn as_lowercase(self) -> &'static str {
    match self {
      Self::Get => "get",
      Self::Post => "post",
      Self::Put => "put",
      Self::Delete => "delete",
      Self::Patch => "patch",
      Self::Options => "options",
      Self::Head => "head",
    }
  }

  pub(crate) fn from_lowercase(value: &str) -> Option<Self> {
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
  /// The 4xx and 5xx responses that declare a JSON schema, by ascending
  /// status. A schemaless or non-JSON error response is skipped.
  pub(crate) errors: Vec<ErrorResponse>,
  /// The OpenAPI Operation's `summary` and `description`, joined by a
  /// blank line.
  pub(crate) description: Option<String>,
  /// The source operation declared `deprecated: true`.
  pub(crate) deprecated: bool,
}

/// One response slot keyed by an explicit 4xx or 5xx status. `default`,
/// 1xx and 3xx are excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ErrorResponse {
  pub(crate) status: u16,
  pub(crate) body: SchemaType,
}

/// An operation's success-response content. `Json(None)` is a JSON
/// response that declares no schema.
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
    // TRACE returns None; `normalize_operation` raises the diagnostic.
    assert_eq!(HttpMethod::from_lowercase("trace"), None);
  }

  #[test]
  fn http_method_rejects_uppercase_and_unknown_keywords() {
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
        name: Ident::parse("avatar").expect("identifier"),
        required: true,
        ty: BodyFieldType::Binary,
      }],
    };
    let url_encoded = BodyContent::UrlEncoded {
      body_ref: Some("LoginForm".into()),
      fields: vec![BodyField {
        name: Ident::parse("username").expect("identifier"),
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

    assert!(matches!(json_with_schema, ResponseContent::Json(Some(_))));
    assert!(matches!(json_without, ResponseContent::Json(None)));
    assert!(matches!(blob, ResponseContent::Blob));
    assert!(matches!(text, ResponseContent::Text));
    assert!(matches!(array_buffer, ResponseContent::ArrayBuffer));
  }
}
