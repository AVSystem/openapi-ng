use std::{fs, path::Path, path::PathBuf, rc::Rc, sync::OnceLock};

use crate::{
  bindings::InputFormat,
  error::{Diagnostic, DiagnosticCode},
  parse::openapi_model::OpenApiDocument,
};

const DEFAULT_MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_SCHEMAS: usize = 10_000;
pub(crate) const DEFAULT_MAX_OPERATIONS: usize = 10_000;
/// Maximum acceptable ratio of YAML-re-serialised parsed bytes to source
/// bytes. Anchors that fan out 50× or more from source are rejected before
/// the typed parse runs — see `decode_openapi_input`. The default is sized
/// well above any legitimate spec (Swagger Petstore re-serialises near 1×;
/// hand-written specs that lean on anchors stay well under 10×).
pub(crate) const DEFAULT_MAX_EXPANSION_RATIO: usize = 50;

/// Parse the cap value from an optional env-var string. Returns the default
/// when the argument is `None` or not a valid `u64`.
fn max_input_bytes_from(env: Option<&str>) -> u64 {
  env
    .and_then(|s| s.parse::<u64>().ok())
    .unwrap_or(DEFAULT_MAX_INPUT_BYTES)
}

/// Process-lifetime cached cap. Reads `OPENAPI_NG_MAX_INPUT_BYTES` exactly
/// once and falls back to `DEFAULT_MAX_INPUT_BYTES` on parse failure or
/// absence.
fn max_input_bytes() -> u64 {
  static CACHED: OnceLock<u64> = OnceLock::new();
  *CACHED.get_or_init(|| {
    max_input_bytes_from(std::env::var("OPENAPI_NG_MAX_INPUT_BYTES").ok().as_deref())
  })
}

/// Parse the schemas cap from an optional env-var string. Returns the
/// default when the argument is `None` or not a valid `usize`. Mirrors
/// `max_input_bytes_from` so policy-side cap checks stay testable without
/// touching process env state.
pub(crate) fn max_schemas_from(env: Option<&str>) -> usize {
  env
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(DEFAULT_MAX_SCHEMAS)
}

/// Parse the operations cap from an optional env-var string. Returns the
/// default when the argument is `None` or not a valid `usize`.
pub(crate) fn max_operations_from(env: Option<&str>) -> usize {
  env
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(DEFAULT_MAX_OPERATIONS)
}

/// Process-lifetime cached schemas cap. Reads `OPENAPI_NG_MAX_SCHEMAS` once
/// per process and falls back to `DEFAULT_MAX_SCHEMAS` on parse failure or
/// absence.
pub(crate) fn max_schemas() -> usize {
  static CACHED: OnceLock<usize> = OnceLock::new();
  *CACHED.get_or_init(|| max_schemas_from(std::env::var("OPENAPI_NG_MAX_SCHEMAS").ok().as_deref()))
}

/// Process-lifetime cached operations cap. Reads `OPENAPI_NG_MAX_OPERATIONS`
/// once per process and falls back to `DEFAULT_MAX_OPERATIONS` on parse
/// failure or absence.
pub(crate) fn max_operations() -> usize {
  static CACHED: OnceLock<usize> = OnceLock::new();
  *CACHED
    .get_or_init(|| max_operations_from(std::env::var("OPENAPI_NG_MAX_OPERATIONS").ok().as_deref()))
}

/// Parse the expansion-ratio cap from an optional env-var string. Returns the
/// default when the argument is `None` or not a valid `usize`. Mirrors the
/// other cap helpers so the expansion guard stays testable without touching
/// process env state.
pub(crate) fn max_expansion_ratio_from(env: Option<&str>) -> usize {
  env
    .and_then(|s| s.parse::<usize>().ok())
    .unwrap_or(DEFAULT_MAX_EXPANSION_RATIO)
}

/// Process-lifetime cached expansion-ratio cap. Reads
/// `OPENAPI_NG_MAX_EXPANSION_RATIO` once per process and falls back to
/// `DEFAULT_MAX_EXPANSION_RATIO` on parse failure or absence.
pub(crate) fn max_expansion_ratio() -> usize {
  static CACHED: OnceLock<usize> = OnceLock::new();
  *CACHED.get_or_init(|| {
    max_expansion_ratio_from(
      std::env::var("OPENAPI_NG_MAX_EXPANSION_RATIO")
        .ok()
        .as_deref(),
    )
  })
}

/// Read the input file and decode it into a typed `OpenApiDocument`. The
/// display path is owned by the pipeline boundary (`execute_generate`) and
/// passed in so every diagnostic — read, decode, normalize, plan, write —
/// carries the exact same `Rc<str>` without re-deriving it at each layer.
pub(crate) fn read_and_decode(
  input_path: &str,
  display_path: &Rc<str>,
) -> Result<OpenApiDocument, Diagnostic> {
  let path = PathBuf::from(input_path);

  let metadata = fs::metadata(&path).map_err(|error| {
    Diagnostic::new(
      DiagnosticCode::InputInvalid,
      format!("Failed to read OpenAPI input: {error}"),
      Rc::clone(display_path),
    )
  })?;
  let max_bytes = max_input_bytes();
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

/// Entry point for the `inputContents` branch. Enforces the byte cap on
/// the supplied source (the 16 MiB default that `read_and_decode` enforces
/// for file inputs via `fs::metadata().len()` — without this check a
/// caller who bypasses the JS-side fetch cap could pass an arbitrarily
/// large string), then delegates to the hint-aware decoder.
///
/// The synthetic `Path::new("")` is fine: when `hint` is `Some`, the
/// decoder skips extension lookup entirely; when `hint` is `None`, the
/// extension is `None` and the decoder falls through to the
/// sniff-both-parsers branch (which is the desired behaviour for
/// hint-less inputContents anyway).
pub(crate) fn decode_input_contents(
  source: &str,
  hint: Option<InputFormat>,
  display_path: &Rc<str>,
) -> Result<OpenApiDocument, Diagnostic> {
  let len_bytes = source.as_bytes().len();
  let max_bytes = max_input_bytes();
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
  // Explicit hint wins over extension/sniff.
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

  // No hint: extension-based dispatch (unchanged behaviour).
  let extension = path
    .extension()
    .and_then(|ext| ext.to_str())
    .map(str::to_ascii_lowercase);

  // Both `serde_json::Error` and `serde_yml::Error` already include the
  // source position ("at line X column Y") in their `Display` impls, so we
  // forward the raw error text verbatim — adding our own `(line X, column Y)`
  // prefix would just duplicate what serde already prints. If we ever switch
  // to a parser that omits position info, lift `err.line()/err.column()`
  // (serde_json) or `err.location()` (serde_yml) into the message here.
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

/// Decode a YAML source into an `OpenApiDocument`, applying the duplicate-key
/// and anchor-fanout guards. Sequencing rationale, post-T4.1:
///
/// 1. **Value parse always runs.** It is required by both behavioural
///    guarantees: the typed `BTreeMap` deserialiser silently last-wins on
///    duplicate keys, so we need the Value-side "duplicate entry" error to
///    surface the `duplicate-schema-name` diagnostic; and the expansion
///    guard from T3.4 needs the parsed Value to measure post-decode size.
///    The duplicate-key fixture itself has no `&`, so we cannot gate the
///    Value parse on anchor presence without regressing that diagnostic.
///
/// 2. **`to_string` re-serialisation is gated on `source.contains('&')`.**
///    That is the genuinely expensive part of T3.4's expansion guard — for
///    a document with no anchors the re-serialised output is bytewise
///    close to the source and the guard is structurally unreachable.
///    Skipping the re-serialisation eliminates the bulk of the T3.4 cost
///    on every anchor-free spec (the common case) without weakening
///    defense on the anchor path. `&` may appear inside string literals;
///    the false-positive is harmless (we just pay the re-serialisation
///    once on a spec that has no real anchors).
///
/// 3. **Typed parse runs last**, on the original source (serde decodes from
///    `&str`, not from a `Value`), and its error carries the field-path
///    context users expect.
///
/// The T4.1 plan called for "typed-first, Value-fallback only on error",
/// but that ordering pre-dated T3.4 and breaks both the duplicate-key
/// detection (typed never fails on duplicates) and the expansion guard
/// (needs the Value). The `&`-gated re-serialisation is the cleanest
/// reconciliation: the Value parse stays cheap, the re-serialisation is
/// elided on the no-anchor common case.
fn decode_yaml(source: &str, display_path: &Rc<str>) -> Result<OpenApiDocument, Diagnostic> {
  // Step 1: Value parse — catches duplicate mapping keys. `serde_yml` rejects
  // duplicate keys when deserialising to `Value` (which preserves key
  // ordering) but silently last-wins into a `BTreeMap`. We exploit this
  // difference here.
  match serde_yml::from_str::<serde_yml::Value>(source) {
    Err(value_err) => {
      let msg = value_err.to_string();
      if msg.contains("duplicate entry") {
        // The serde_yml error message format for a duplicate key in a
        // mapping deserialised as `Value` is:
        //   "<path>: duplicate entry with key \"<name>\" at line N column M"
        // Extract the key name from between the quotes.
        let key_name = extract_duplicate_key_name(&msg).unwrap_or("<unknown>");
        return Err(Diagnostic {
          code: DiagnosticCode::PolicyViolation,
          subcode: Some("duplicate-schema-name"),
          message: format!(
            "Failed to decode OpenAPI input: schema name '{key_name}' is defined more than once in components.schemas.",
          ),
          path: Rc::clone(display_path),
        });
      }
      // Non-duplicate Value error: fall through to the typed decode below so
      // the message carries field-path context.
    }
    Ok(value) => {
      // Step 2: anchor-fanout guard. The re-serialisation is the expensive
      // operation; skip it entirely when the source has no anchor markers,
      // since the guard is structurally unreachable on anchor-free input.
      // This is the T4.1 perf win: anchor-free specs pay only the Value
      // parse, not the re-serialisation.
      if source.contains('&') {
        if let Ok(expanded) = serde_yml::to_string(&value) {
          let source_len = source.len().max(1);
          let cap = max_expansion_ratio();
          // Saturating arithmetic on the cap multiplication: source.len() is
          // already bounded by the input-byte cap upstream, but the product
          // could overflow on a pathologically small source × huge cap.
          let threshold = source_len.saturating_mul(cap);
          if expanded.len() > threshold {
            let ratio = expanded.len() / source_len;
            return Err(Diagnostic {
              code: DiagnosticCode::PolicyViolation,
              subcode: Some("mapping-expansion-exceeded"),
              message: format!(
                "Failed to decode OpenAPI input: YAML anchor expansion produced {expanded_len} bytes from {source_len} bytes of source — {ratio}× ratio exceeds the cap of {cap}×. The spec likely uses anchors with deep fan-out; inline the aliases or set OPENAPI_NG_MAX_EXPANSION_RATIO to override.",
                expanded_len = expanded.len(),
              ),
              path: Rc::clone(display_path),
            });
          }
        }
      }
    }
  }

  // Step 3: typed decode on the original source. serde_yml deserialises from
  // `&str`, not from a `Value`, so this is a second parse of the same bytes.
  // Field-path context lives in the typed decoder's error path.
  serde_yml::from_str(source).map_err(|error| {
    Diagnostic::new(
      DiagnosticCode::InputInvalid,
      format!("Failed to decode OpenAPI input as YAML: {error}"),
      Rc::clone(display_path),
    )
  })
}

/// Extract the duplicate key name from a `serde_yml` "duplicate entry" error
/// message. The message format is:
///   "<path>: duplicate entry with key \"<name>\" at line N column M"
/// Returns the text between the first pair of double-quotes, or `None` if the
/// pattern is not found (defensive fallback).
fn extract_duplicate_key_name(msg: &str) -> Option<&str> {
  let start = msg.find('"')?;
  let end = msg[start + 1..].find('"')?;
  Some(&msg[start + 1..start + 1 + end])
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
  };

  use super::{
    DEFAULT_MAX_INPUT_BYTES, decode_openapi_input, max_input_bytes_from, read_and_decode,
  };
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

  // Regression guard for Phase 4.4: the user-facing decode message must
  // surface the source position so authors can jump to the offending byte
  // without re-parsing the file by hand. `serde_json::Error::Display` already
  // appends "at line X column Y"; if a future upgrade drops that, this test
  // fails and forces us to construct the position ourselves.
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

  // Inline-source variant of the duplicate-key regression: pins behaviour
  // independently of the fixture file. Together with
  // `duplicate_schema_name_is_rejected_in_yaml`, this guards against silent
  // BTreeMap last-wins regressions if T4.1's typed-first reorder ever drops
  // the Value-parse probe on the no-anchor success path.
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

  // --- size-cap tests ---

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

    // Write 17 MiB of content so the cap fires before any parse attempt.
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

  // Pure-function tests for the cap helper — not affected by OnceLock state.

  #[test]
  fn cap_helper_default() {
    assert_eq!(max_input_bytes_from(None), DEFAULT_MAX_INPUT_BYTES);
    assert_eq!(max_input_bytes_from(None), 16 * 1024 * 1024);
  }

  #[test]
  fn cap_helper_respects_valid_env() {
    assert_eq!(max_input_bytes_from(Some("1024")), 1024);
    assert_eq!(max_input_bytes_from(Some("0")), 0);
  }

  #[test]
  fn cap_helper_rejects_invalid_env_uses_default() {
    assert_eq!(
      max_input_bytes_from(Some("not-a-number")),
      DEFAULT_MAX_INPUT_BYTES
    );
    assert_eq!(max_input_bytes_from(Some("")), DEFAULT_MAX_INPUT_BYTES);
    assert_eq!(max_input_bytes_from(Some("-1")), DEFAULT_MAX_INPUT_BYTES);
  }

  // --- expansion-ratio cap tests ---

  #[test]
  fn max_expansion_ratio_from_default_value() {
    use super::{DEFAULT_MAX_EXPANSION_RATIO, max_expansion_ratio_from};
    assert_eq!(max_expansion_ratio_from(None), DEFAULT_MAX_EXPANSION_RATIO);
    assert_eq!(max_expansion_ratio_from(None), 50);
  }

  #[test]
  fn max_expansion_ratio_from_env_override() {
    use super::{DEFAULT_MAX_EXPANSION_RATIO, max_expansion_ratio_from};
    assert_eq!(max_expansion_ratio_from(Some("100")), 100);
    assert_eq!(max_expansion_ratio_from(Some("1")), 1);
    assert_eq!(max_expansion_ratio_from(Some("0")), 0);
    // Invalid forms fall back to default.
    assert_eq!(
      max_expansion_ratio_from(Some("not-a-number")),
      DEFAULT_MAX_EXPANSION_RATIO,
    );
    assert_eq!(
      max_expansion_ratio_from(Some("")),
      DEFAULT_MAX_EXPANSION_RATIO,
    );
    assert_eq!(
      max_expansion_ratio_from(Some("-1")),
      DEFAULT_MAX_EXPANSION_RATIO,
    );
  }

  #[test]
  fn anchor_expansion_within_ratio_accepts() {
    // A handful of aliases on a small anchor stays well under the default
    // 50× expansion cap. Pins that legitimate anchor use is not regressed
    // by the guard.
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
    // Construct a YAML where the anchor body × alias count blows past the
    // 50× ratio cap on re-serialisation. 500 A-rows × 16 aliases each ×
    // a ~250-byte body re-serialises into ~2 MB from a ~30 KB source
    // (~70× ratio). The check is independent of the OnceLock-cached cap
    // because the cap setter is `max_expansion_ratio()`; this test
    // exercises the same path the cached value would.
    let mut yaml = String::from(
      "openapi: 3.0.3\ninfo:\n  title: Fanout\n  version: 1.0.0\npaths: {}\ncomponents:\n  schemas:\n    Base: &b\n      type: object\n      properties:\n",
    );
    for i in 0..20 {
      yaml.push_str(&format!(
        "        prop_{i:02}: {{ type: string, description: \"property {i:02} padding text here\" }}\n",
      ));
    }
    for r in 0..500 {
      let aliases: String = std::iter::repeat("*b")
        .take(16)
        .collect::<Vec<_>>()
        .join(", ");
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

  // Sanity check on the YAML success path: a spec with no `&` anchors
  // decodes cleanly and lands every schema. This test is intentionally
  // structural — it does NOT directly verify that T4.1's
  // `source.contains('&')` gate skips the `to_string` re-serialisation;
  // observing that skip would require `cfg(test)`-gated instrumentation on
  // the decode hot path, which is out of proportion for a single assertion.
  // The perf-relevant skip is verified by `pnpm bench` medians (see
  // `decode_yaml`'s docstring and commit `ab550fb`); this test would still
  // pass even if the gate were deleted. It guards the surrounding shape:
  // that anchor-free YAML still decodes successfully through the helper.
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
    // Sanity-check the precondition: the source contains no anchor markers.
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

    // File named .yaml but contents are valid JSON. With the hint we
    // skip extension lookup and decode as JSON directly.
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
    // Use a tab character inside a flow mapping — syntactically invalid in both JSON and YAML.
    fs::write(&path, "{\t\"key\": [}").unwrap();

    let path_str = path.to_str().expect("utf-8 path");
    let display: Rc<str> = Rc::from(path_str);
    let err = read_and_decode(path_str, &display).expect_err("should fail");
    let _ = fs::remove_file(&path);

    let msg = &err.message;
    // The "Rename" hint must still be present.
    assert!(msg.contains("Rename"), "missing Rename hint: {msg}");
    // The underlying parser error info should be there too — serde_yml includes
    // "line" and "column" in its Display output so authors can jump to the
    // offending byte without re-parsing by hand.
    assert!(
      msg.contains("line ") && msg.contains("column "),
      "expected line/column from parser in message: {msg}",
    );
  }

  #[test]
  fn decode_openapi_input_yaml_hint_on_json_content_fails_decode_as_yaml() {
    use super::decode_openapi_input_with_hint;
    use crate::bindings::InputFormat;

    // A YAML hint must force YAML decoding even when the source is
    // wire-compatible JSON — this proves the hint suppresses the
    // sniff fallback rather than just steering it.
    //
    // JSON-shaped maps happen to parse as YAML (flow-style), so we
    // pick content that is unambiguously NOT yaml: a leading tab inside
    // a flow mapping, which serde_yml rejects.
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

    // No path extension and no Content-Type — but with an explicit
    // Json hint the decoder should still succeed. This is the URL-input
    // shape where the JS wrapper hands us inputContents + an empty path.
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

    // Build a string larger than the default 16 MiB cap: 17 MiB of 'a'
    // padding inside an otherwise-valid YAML header.
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
      // Keep iteration count reasonable for CI — boundary fuzzing doesn't need millions.
      cases: 256,
      ..ProptestConfig::default()
    })]

    #[test]
    fn read_and_decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..16384)) {
      // Write to a unique temp file per case so concurrent property invocations don't collide.
      let dir = std::env::temp_dir().join(format!(
        "oapi-ng-prop-decode-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_nanos(),
      ));
      std::fs::create_dir_all(&dir).unwrap();
      // Pick an extension at random-ish to exercise both code paths.
      let ext = if bytes.len() % 2 == 0 { "yaml" } else { "json" };
      let path = dir.join(format!("input.{ext}"));
      std::fs::write(&path, &bytes).unwrap();

      let path_str = path.to_str().expect("utf-8 path");
      let display: Rc<str> = Rc::from(path_str);
      let result = read_and_decode(path_str, &display);
      let _ = std::fs::remove_dir_all(&dir);

      // Property: never panic. Either Ok, or Err with a typed code.
      if let Err(diag) = result {
        prop_assert!(
          matches!(diag.code, DiagnosticCode::InputInvalid | DiagnosticCode::PolicyViolation),
          "unexpected diagnostic code: {:?}", diag.code,
        );
      }
    }
  }
}
