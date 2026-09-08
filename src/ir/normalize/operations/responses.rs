//! Response lowering: the typed success body and the 4xx/5xx error map.

use std::collections::BTreeMap;

use crate::error::{Context, Diagnostic};
use crate::ir::canonical::{ErrorResponse, ResponseContent};
use crate::options::{ResponseType, ResponseTypeMapping};
use crate::parse::openapi_model::{MediaType, Response};

use super::super::SchemaWalk;
use super::super::schema::normalize_schema;
use super::OperationCx;

pub(super) fn normalize_success_response(
  responses: Option<&BTreeMap<String, Response>>,
  cx: OperationCx<'_>,
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

  let Some((mime, media)) = pick_response_media(content, cx.response_types()) else {
    return Ok(None);
  };

  let kind = classify_response_kind(mime, cx.response_types());
  let walk = SchemaWalk::root(
    Context::ResponseSchema {
      method: cx.method(),
      path: cx.path(),
    },
    cx.reporter(),
  );

  Ok(Some(match kind {
    ResponseKind::Json => {
      let schema = match &media.schema {
        Some(schema) => Some(normalize_schema(schema, walk)?),
        None => None,
      };
      ResponseContent::Json(schema)
    }
    ResponseKind::Blob => ResponseContent::Blob,
    ResponseKind::Text => ResponseContent::Text,
    ResponseKind::ArrayBuffer => ResponseContent::ArrayBuffer,
  }))
}

/// Collects the 4xx and 5xx responses that declare a JSON schema, sorted
/// by status ascending.
///
/// Skips a schemaless response, a non-JSON one, and the `default` key.
pub(super) fn normalize_error_responses(
  responses: Option<&BTreeMap<String, Response>>,
  cx: OperationCx<'_>,
) -> Result<Vec<ErrorResponse>, Diagnostic> {
  let Some(responses) = responses else {
    return Ok(Vec::new());
  };

  let walk = SchemaWalk::root(
    Context::ResponseSchema {
      method: cx.method(),
      path: cx.path(),
    },
    cx.reporter(),
  );
  let mut errors = responses
    .iter()
    .filter_map(|(status, response)| {
      let status = parse_error_status(status)?;
      let schema = response
        .content
        .as_ref()?
        .get("application/json")?
        .schema
        .as_ref()?;
      Some((status, schema))
    })
    .map(|(status, schema)| {
      Ok(ErrorResponse {
        status,
        body: normalize_schema(schema, walk)?,
      })
    })
    .collect::<Result<Vec<_>, Diagnostic>>()?;
  errors.sort_by_key(|error| error.status);
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

/// Picks the media entry carrying a response's typed body: the first that
/// does not classify as `Blob`, else the first `Blob`.
///
/// Determinism comes from `BTreeMap` iterating alphabetically by key.
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

  if let Some(m) = user_mapping
    .iter()
    .find(|mapping| mapping.content_type.eq_ignore_ascii_case(&normalized))
  {
    return match m.response_type {
      ResponseType::Json => ResponseKind::Json,
      ResponseType::Blob => ResponseKind::Blob,
      ResponseType::Text => ResponseKind::Text,
      ResponseType::ArrayBuffer => ResponseKind::ArrayBuffer,
    };
  }

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
  use super::super::OperationCx;

  fn test_cx<'a>(
    response_types: &'a [ResponseTypeMapping],
    reporter: &'a crate::error::Reporter,
  ) -> OperationCx<'a> {
    static EMPTY: std::sync::LazyLock<BTreeMap<&str, &crate::ir::schema::SchemaType>> =
      std::sync::LazyLock::new(BTreeMap::new);
    OperationCx::new("GET", "/x", &EMPTY, response_types, reporter)
  }
  use std::collections::BTreeMap;

  use super::{
    ResponseKind, classify_response_kind, normalize_error_responses, normalize_success_response,
    parse_error_status, pick_response_media,
  };

  use crate::options::{ResponseType, ResponseTypeMapping};
  use crate::parse::openapi_model::{MediaType, Response, Schema};

  fn json_schema() -> Schema {
    Schema::default_string()
  }

  fn btreemap_with<K: Ord, V>(key: K, value: V) -> BTreeMap<K, V> {
    BTreeMap::from([(key, value)])
  }
  use crate::test_support::test_reporter;

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
    let ctx = test_reporter();
    let result = normalize_success_response(
      Some(&btreemap_with("200".to_string(), response)),
      test_cx(&[], &ctx),
    )
    .expect("normalize ok");
    assert!(result.is_none(), "missing response content => None");
  }

  /// Builds a Response with a single JSON content entry carrying the
  /// given schema. Helper for the error-response tests below.
  fn json_response(schema: Schema) -> Response {
    Response {
      content: Some(
        BTreeMap::from([(
          "application/json".to_string(),
          MediaType {
            schema: Some(schema),
          },
        )])
        .into(),
      ),
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

    let ctx = test_reporter();
    let errors =
      normalize_error_responses(Some(&responses), test_cx(&[], &ctx)).expect("normalize ok");

    assert_eq!(
      errors.iter().map(|error| error.status).collect::<Vec<_>>(),
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
        content: Some(
          BTreeMap::from([("application/json".to_string(), MediaType { schema: None })]).into(),
        ),
      },
    );
    // 504: only non-JSON content — must be skipped.
    responses.insert(
      "504".to_string(),
      Response {
        content: Some(
          BTreeMap::from([(
            "text/plain".to_string(),
            MediaType {
              schema: Some(Schema::default_string()),
            },
          )])
          .into(),
        ),
      },
    );

    let ctx = test_reporter();
    let errors =
      normalize_error_responses(Some(&responses), test_cx(&[], &ctx)).expect("normalize ok");

    assert_eq!(
      errors.iter().map(|error| error.status).collect::<Vec<_>>(),
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

    let ctx = test_reporter();
    let errors =
      normalize_error_responses(Some(&responses), test_cx(&[], &ctx)).expect("normalize ok");

    // Only 400 survives — `default` is intentionally not surfaced.
    assert_eq!(
      errors.iter().map(|error| error.status).collect::<Vec<_>>(),
      vec![400]
    );
  }
}
