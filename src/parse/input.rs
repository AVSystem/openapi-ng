use std::{fs, path::Path, rc::Rc};

use crate::{
  bindings::InputFormat,
  error::{Diagnostic, DiagnosticCode},
  io::host_cwd::resolve_against_host_cwd,
  parse::{
    limits::{MAX_EXPANSION_RATIO, MAX_INPUT_BYTES},
    openapi_model::OpenApiDocument,
    unique_map::DUPLICATE_KEY,
  },
};

/// Reads and decodes the file at `input_path`, failing when it exceeds
/// the input byte cap.
///
/// Every diagnostic raised carries `display_path`.
pub(crate) fn read_and_decode(
  input_path: &str,
  display_path: &Rc<str>,
) -> Result<OpenApiDocument, Diagnostic> {
  let path = resolve_against_host_cwd(Path::new(input_path));

  let metadata = fs::metadata(&path).map_err(|error| {
    Diagnostic::new(
      DiagnosticCode::InputInvalid,
      format!("Failed to read OpenAPI input: {error}"),
      Rc::clone(display_path),
    )
  })?;
  let max_bytes = MAX_INPUT_BYTES.get();
  if metadata.len() > max_bytes {
    return Err(Diagnostic::new(
      DiagnosticCode::InputInvalid,
      format!(
        "Failed to read OpenAPI input: file is {} bytes, exceeds maximum of {} bytes. \
         Set OPENAPI_NG_MAX_INPUT_BYTES to override.",
        metadata.len(),
        max_bytes,
      ),
      Rc::clone(display_path),
    ));
  }

  let source = fs::read_to_string(&path).map_err(|error| {
    Diagnostic::new(
      DiagnosticCode::InputInvalid,
      format!("Failed to read OpenAPI input: {error}"),
      Rc::clone(display_path),
    )
  })?;
  decode_openapi_input(&path, &source, display_path)
}

pub(crate) fn decode_openapi_input(
  path: &Path,
  source: &str,
  display_path: &Rc<str>,
) -> Result<OpenApiDocument, Diagnostic> {
  decode_openapi_input_with_hint(path, source, display_path, None)
}

/// Decodes a spec supplied as source text, failing when it exceeds the
/// input byte cap. Without a `hint` the format is sniffed.
pub(crate) fn decode_input_contents(
  source: &str,
  hint: Option<InputFormat>,
  display_path: &Rc<str>,
) -> Result<OpenApiDocument, Diagnostic> {
  let len_bytes = source.len();
  let max_bytes = MAX_INPUT_BYTES.get();
  if (len_bytes as u64) > max_bytes {
    return Err(Diagnostic::new(
      DiagnosticCode::InputInvalid,
      format!(
        "OpenAPI input is {len_bytes} bytes, exceeds maximum of {max_bytes} bytes. \
         Set OPENAPI_NG_MAX_INPUT_BYTES to override.",
      ),
      Rc::clone(display_path),
    ));
  }
  decode_openapi_input_with_hint(std::path::Path::new(""), source, display_path, hint)
}

pub(crate) fn decode_openapi_input_with_hint(
  path: &Path,
  source: &str,
  display_path: &Rc<str>,
  hint: Option<InputFormat>,
) -> Result<OpenApiDocument, Diagnostic> {
  if let Some(format) = hint {
    return match format {
      InputFormat::Json => serde_json::from_str(source).map_err(|error| {
        Diagnostic::new(
          DiagnosticCode::InputInvalid,
          format!("Failed to decode OpenAPI input as JSON: {error}"),
          Rc::clone(display_path),
        )
      }),
      InputFormat::Yaml => decode_yaml(source, display_path),
    };
  }

  let extension = path
    .extension()
    .and_then(|ext| ext.to_str())
    .map(str::to_ascii_lowercase);

  match extension.as_deref() {
    Some("json") => serde_json::from_str(source).map_err(|error| {
      Diagnostic::new(
        DiagnosticCode::InputInvalid,
        format!("Failed to decode OpenAPI input as JSON: {error}"),
        Rc::clone(display_path),
      )
    }),
    Some("yaml" | "yml") => decode_yaml(source, display_path),
    _ => serde_json::from_str(source)
      .or_else(|_| serde_yml::from_str(source))
      .map_err(|yaml_error| {
        Diagnostic::new(
          DiagnosticCode::InputInvalid,
          format!(
            "Failed to decode OpenAPI input as JSON or YAML: {yaml_error}. \
             Rename the file with a .json, .yaml, or .yml extension so the decoder can pick the right parser.",
          ),
          Rc::clone(display_path),
        )
      }),
  }
}

/// Decodes YAML into an `OpenApiDocument`. Repeated mapping keys are
/// rejected by the model's `UniqueMap` fields. The anchor-expansion
/// guard runs only when the source contains `&`.
fn decode_yaml(source: &str, display_path: &Rc<str>) -> Result<OpenApiDocument, Diagnostic> {
  if source.contains('&') {
    check_anchor_expansion(source, display_path)?;
  }

  serde_yml::from_str(source).map_err(|error| decode_failure(&error.to_string(), display_path))
}

/// Projects a `serde_yml` decode error onto a diagnostic. A duplicate key
/// under `components.schemas` carries the `duplicate-schema-name` subcode so
/// consumers can route on it; anything else is a plain decode failure.
fn decode_failure(message: &str, display_path: &Rc<str>) -> Diagnostic {
  if message.contains(DUPLICATE_KEY) && message.contains(SCHEMAS_FIELD_PATH) {
    return Diagnostic {
      code: DiagnosticCode::PolicyViolation,
      subcode: Some("duplicate-schema-name"),
      message: format!(
        "Failed to decode OpenAPI input: {message}. Each schema name must be declared once."
      ),
      path: Rc::clone(display_path),
    };
  }
  Diagnostic::new(
    DiagnosticCode::InputInvalid,
    format!("Failed to decode OpenAPI input as YAML: {message}"),
    Rc::clone(display_path),
  )
}

/// Field path `serde_yml` prefixes onto an error raised while deserialising
/// `components.schemas`.
const SCHEMAS_FIELD_PATH: &str = "components.schemas";

/// Rejects a source whose YAML aliases expand far beyond its own size,
/// measured by re-serialising the node tree. A source this cannot parse
/// passes, leaving the typed parse to report the error.
fn check_anchor_expansion(source: &str, display_path: &Rc<str>) -> Result<(), Diagnostic> {
  let Ok(value) = serde_yml::from_str::<serde_yml::Value>(source) else {
    return Ok(());
  };
  let Ok(expanded) = serde_yml::to_string(&value) else {
    return Ok(());
  };

  let source_len = source.len().max(1);
  let cap = MAX_EXPANSION_RATIO.get();
  if expanded.len() <= source_len.saturating_mul(cap) {
    return Ok(());
  }

  Err(Diagnostic {
    code: DiagnosticCode::PolicyViolation,
    subcode: Some("mapping-expansion-exceeded"),
    message: format!(
      "Failed to decode OpenAPI input: YAML anchor expansion produced {expanded_len} bytes from {source_len} bytes of source — {ratio}× ratio exceeds the cap of {cap}×. The spec likely uses anchors with deep fan-out; inline the aliases or set OPENAPI_NG_MAX_EXPANSION_RATIO to override.",
      expanded_len = expanded.len(),
      ratio = expanded.len() / source_len,
    ),
    path: Rc::clone(display_path),
  })
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
  };

  use super::{decode_openapi_input, read_and_decode};
  use crate::error::DiagnosticCode;
  use std::path::PathBuf;

  #[test]
  fn read_and_decode_returns_typed_document_for_supported_input() {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock works")
      .as_nanos();
    let path = std::env::temp_dir().join(format!("openapi-ng-read-and-decode-{nanos}.json"));
    fs::write(
      &path,
      r#"{"openapi":"3.0.3","info":{"title":"Decode","version":"1.0.0"},"paths":{}}"#,
    )
    .expect("fixture should be written");

    let path_str = path.to_str().expect("utf-8 path");
    let display: Rc<str> = Rc::from(path_str);
    let document = read_and_decode(path_str, &display).expect("decode should succeed");

    assert_eq!(document.info.title, "Decode");
    assert_eq!(document.openapi, "3.0.3");

    let _ = fs::remove_file(path);
  }

  #[test]
  fn decode_error_for_malformed_json_includes_line_and_column() {
    let path = PathBuf::from("spec.json");
    let display: Rc<str> = Rc::from("spec.json");
    let source = "{\"openapi\": \"3.0.3\", \"info\":}";
    let err =
      decode_openapi_input(&path, source, &display).expect_err("malformed JSON must fail decode");

    assert!(
      err.message.contains("line ") && err.message.contains("column "),
      "expected line/column in JSON decode error, got: {message}",
      message = err.message,
    );
  }

  #[test]
  fn decode_error_for_malformed_yaml_includes_line_and_column() {
    let path = PathBuf::from("spec.yaml");
    let display: Rc<str> = Rc::from("spec.yaml");
    let source = "openapi: 3.0.3\ninfo:\n  title: M\n  version: 1.0.0\npaths:\n  broken: [\n";
    let err =
      decode_openapi_input(&path, source, &display).expect_err("malformed YAML must fail decode");

    assert!(
      err.message.contains("line ") && err.message.contains("column "),
      "expected line/column in YAML decode error, got: {message}",
      message = err.message,
    );
  }

  #[test]
  fn duplicate_schema_name_in_yaml_is_diagnosed() {
    let yaml = r#"
openapi: 3.0.3
info: { title: t, version: '1.0.0' }
paths: {}
components:
  schemas:
    Pet: { type: object }
    Pet: { type: string }
"#;
    let path = PathBuf::from("inline.yaml");
    let display: Rc<str> = Rc::from("inline.yaml");
    let err = decode_openapi_input(&path, yaml, &display)
      .expect_err("inline duplicate-key YAML should be diagnosed");
    assert_eq!(err.code, DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("duplicate-schema-name"));
    assert!(
      err.message.contains("Pet"),
      "expected duplicate key 'Pet' in message: {}",
      err.message,
    );
  }

  #[test]
  fn duplicate_schema_name_is_rejected_in_yaml() {
    let yaml = include_str!("../../test/fixtures/duplicate-schema-name.openapi.yaml");
    let path = PathBuf::from("dup.yaml");
    let display: Rc<str> = Rc::from("dup.yaml");
    let err =
      decode_openapi_input(&path, yaml, &display).expect_err("should reject duplicate schema name");
    assert_eq!(err.code, crate::error::DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("duplicate-schema-name"));
  }


  #[test]
  fn rejects_input_larger_than_cap() {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock works")
      .as_nanos();
    let dir = std::env::temp_dir().join(format!(
      "oapi-ng-oversized-{}-{}",
      std::process::id(),
      nanos
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("huge.yaml");

    let header = "openapi: 3.0.3\ninfo: { title: x, version: 1.0.0 }\npaths: {}\n# ";
    let pad_bytes = (17 * 1024 * 1024) - header.len();
    let mut content = String::with_capacity(17 * 1024 * 1024);
    content.push_str(header);
    content.push_str(&"a".repeat(pad_bytes));
    fs::write(&path, &content).unwrap();

    let path_str = path.to_str().expect("utf-8 path");
    let display: Rc<str> = Rc::from(path_str);
    let result = read_and_decode(path_str, &display);
    let _ = fs::remove_dir_all(&dir);

    let err = result.expect_err("should reject oversized input");
    assert_eq!(err.code, DiagnosticCode::InputInvalid);
    assert!(
      err.message.contains("exceeds maximum"),
      "unexpected message: {}",
      err.message
    );
  }

  #[test]
  fn anchor_expansion_within_ratio_accepts() {
    let yaml = r#"openapi: 3.0.3
info: { title: modest-anchor, version: '1.0.0' }
paths: {}
components:
  schemas:
    Base: &b
      type: object
      properties:
        id: { type: string }
        name: { type: string }
    A1: { allOf: [*b] }
    A2: { allOf: [*b] }
    A3: { allOf: [*b] }
"#;
    let path = PathBuf::from("modest.yaml");
    let display: Rc<str> = Rc::from("modest.yaml");
    decode_openapi_input(&path, yaml, &display).expect("modest anchor use should decode");
  }

  #[test]
  fn anchor_expansion_exceeding_ratio_rejects() {
    // 500 rows × 16 aliases × ~250-byte body: ~30 KB source, ~2 MB
    // re-serialised, past the 50× cap.
    let mut yaml = String::from(
      "openapi: 3.0.3\ninfo:\n  title: Fanout\n  version: 1.0.0\npaths: {}\ncomponents:\n  schemas:\n    Base: &b\n      type: object\n      properties:\n",
    );
    for i in 0..20 {
      yaml.push_str(&format!(
        "        prop_{i:02}: {{ type: string, description: \"property {i:02} padding text here\" }}\n",
      ));
    }
    for r in 0..500 {
      let aliases: String = std::iter::repeat_n("*b", 16).collect::<Vec<_>>().join(", ");
      yaml.push_str(&format!("    A{r:04}: {{ allOf: [{aliases}] }}\n"));
    }

    let path = PathBuf::from("fanout.yaml");
    let display: Rc<str> = Rc::from("fanout.yaml");
    let err = decode_openapi_input(&path, &yaml, &display)
      .expect_err("fanned-out anchors should be rejected");

    assert_eq!(err.code, DiagnosticCode::PolicyViolation);
    assert_eq!(err.subcode, Some("mapping-expansion-exceeded"));
    assert!(
      err.message.contains("OPENAPI_NG_MAX_EXPANSION_RATIO"),
      "expected env-var hint in message: {}",
      err.message,
    );
    assert!(
      err.message.contains("anchor expansion"),
      "expected anchor-expansion phrasing: {}",
      err.message,
    );
  }

  #[test]
  fn anchor_free_yaml_decodes_successfully() {
    let mut yaml = String::from(
      "openapi: 3.0.3\ninfo:\n  title: NoAnchors\n  version: 1.0.0\npaths: {}\ncomponents:\n  schemas:\n",
    );
    for i in 0..50 {
      yaml.push_str(&format!(
        "    S{i:03}:\n      type: object\n      properties:\n        id: {{ type: string }}\n        name: {{ type: string }}\n",
      ));
    }
    assert!(
      !yaml.contains('&'),
      "fixture must be anchor-free to exercise the fast path",
    );

    let path = PathBuf::from("noanchor.yaml");
    let display: Rc<str> = Rc::from("noanchor.yaml");
    let document =
      decode_openapi_input(&path, &yaml, &display).expect("anchor-free spec should decode");
    assert_eq!(
      document.components.schemas.len(),
      50,
      "all 50 schemas should land in the typed doc",
    );
  }

  #[test]
  fn decode_openapi_input_honours_explicit_format_hint_over_extension() {
    use super::decode_openapi_input_with_hint;
    use crate::bindings::InputFormat;

    let path = PathBuf::from("misnamed.yaml");
    let display: Rc<str> = Rc::from("misnamed.yaml");
    let json_source =
      r#"{"openapi":"3.0.3","info":{"title":"Hinted","version":"1.0.0"},"paths":{}}"#;
    let doc = decode_openapi_input_with_hint(&path, json_source, &display, Some(InputFormat::Json))
      .expect("explicit Json hint must decode as JSON regardless of extension");
    assert_eq!(doc.info.title, "Hinted");
  }

  #[test]
  fn decode_openapi_input_with_no_hint_falls_back_to_extension() {
    use super::decode_openapi_input_with_hint;
    let path = PathBuf::from("spec.json");
    let display: Rc<str> = Rc::from("spec.json");
    let source = r#"{"openapi":"3.0.3","info":{"title":"X","version":"1.0.0"},"paths":{}}"#;
    let doc = decode_openapi_input_with_hint(&path, source, &display, None)
      .expect("None hint should still decode JSON via extension");
    assert_eq!(doc.info.title, "X");
  }

  #[test]
  fn no_extension_decode_error_includes_parser_message() {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock works")
      .as_nanos();
    let path = std::env::temp_dir().join(format!("oapi-ng-noext-{nanos}")); // no extension
    // A tab inside a flow mapping: invalid in both JSON and YAML.
    fs::write(&path, "{\t\"key\": [}").unwrap();

    let path_str = path.to_str().expect("utf-8 path");
    let display: Rc<str> = Rc::from(path_str);
    let err = read_and_decode(path_str, &display).expect_err("should fail");
    let _ = fs::remove_file(&path);

    let msg = &err.message;
    assert!(msg.contains("Rename"), "missing Rename hint: {msg}");
    assert!(
      msg.contains("line ") && msg.contains("column "),
      "expected line/column from parser in message: {msg}",
    );
  }

  #[test]
  fn decode_openapi_input_yaml_hint_on_json_content_fails_decode_as_yaml() {
    use super::decode_openapi_input_with_hint;
    use crate::bindings::InputFormat;

    let path = PathBuf::from("ambiguous");
    let display: Rc<str> = Rc::from("ambiguous");
    let source = "{\t\"openapi\": \"3.0.3\"}";
    let err = decode_openapi_input_with_hint(&path, source, &display, Some(InputFormat::Yaml))
      .expect_err("Yaml hint must route through the YAML decoder");
    assert_eq!(err.code, DiagnosticCode::InputInvalid);
    assert!(
      err.message.contains("YAML"),
      "expected YAML decoder error, got: {}",
      err.message,
    );
  }

  #[test]
  fn decode_openapi_input_json_hint_with_no_extension_decodes_successfully() {
    use super::decode_openapi_input_with_hint;
    use crate::bindings::InputFormat;

    // The URL-input shape: no extension, no Content-Type, explicit hint.
    let path = PathBuf::from("");
    let display: Rc<str> = Rc::from("https://example.com/openapi");
    let source = r#"{"openapi":"3.0.3","info":{"title":"NoExt","version":"1.0.0"},"paths":{}}"#;
    let doc = decode_openapi_input_with_hint(&path, source, &display, Some(InputFormat::Json))
      .expect("Json hint must succeed even without a path extension");
    assert_eq!(doc.info.title, "NoExt");
  }

  #[test]
  fn decode_input_contents_enforces_byte_cap() {
    use super::decode_input_contents;

    let header = "openapi: 3.0.3\ninfo: { title: Big, version: 1.0.0 }\npaths: {}\n# ";
    let pad_bytes = (17 * 1024 * 1024) - header.len();
    let mut content = String::with_capacity(17 * 1024 * 1024);
    content.push_str(header);
    content.push_str(&"a".repeat(pad_bytes));

    let display: Rc<str> = Rc::from("inline://big");
    let err = decode_input_contents(&content, None, &display)
      .expect_err("oversize inputContents must be rejected");

    assert_eq!(err.code, DiagnosticCode::InputInvalid);
    assert!(
      err.message.contains("exceeds maximum"),
      "message: {}",
      err.message,
    );
    assert!(
      err.message.contains("OPENAPI_NG_MAX_INPUT_BYTES"),
      "expected env-var hint, got: {}",
      err.message,
    );
  }

  #[test]
  fn decode_input_contents_under_cap_decodes_successfully() {
    use super::decode_input_contents;
    use crate::bindings::InputFormat;

    let source = r#"{"openapi":"3.0.3","info":{"title":"Small","version":"1.0.0"},"paths":{}}"#;
    let display: Rc<str> = Rc::from("inline://small");
    let doc = decode_input_contents(source, Some(InputFormat::Json), &display)
      .expect("small JSON inputContents must decode");
    assert_eq!(doc.info.title, "Small");
  }
}

#[cfg(test)]
mod proptests {
  use std::rc::Rc;

  use proptest::prelude::*;

  use super::read_and_decode;
  use crate::error::DiagnosticCode;

  proptest! {
    #![proptest_config(ProptestConfig {
      cases: 256,
      ..ProptestConfig::default()
    })]

    #[test]
    fn read_and_decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..16384)) {
      let dir = std::env::temp_dir().join(format!(
        "oapi-ng-prop-decode-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_nanos(),
      ));
      std::fs::create_dir_all(&dir).unwrap();
      let ext = if bytes.len() % 2 == 0 { "yaml" } else { "json" };
      let path = dir.join(format!("input.{ext}"));
      std::fs::write(&path, &bytes).unwrap();

      let path_str = path.to_str().expect("utf-8 path");
      let display: Rc<str> = Rc::from(path_str);
      let result = read_and_decode(path_str, &display);
      let _ = std::fs::remove_dir_all(&dir);

      if let Err(diag) = result {
        prop_assert!(
          matches!(diag.code, DiagnosticCode::InputInvalid | DiagnosticCode::PolicyViolation),
          "unexpected diagnostic code: {:?}", diag.code,
        );
      }
    }
  }
}
