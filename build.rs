extern crate napi_build;

use std::fs;
use std::path::Path;

fn main() {
  napi_build::setup();
  propagate_error_marker();
}

/// Read the shared `lib/error-marker.json` file at compile time and
/// expose the marker string to the Rust source via the
/// `OPENAPI_NG_ERROR_MARKER` env var (consumed via `env!()` in
/// `src/bindings.rs`). Keeping the marker in one file consumed by both
/// the Rust binding and `lib/index.js` makes drift impossible — the two
/// sides can never disagree on the sentinel that identifies a thrown
/// GenerateError across realms.
fn propagate_error_marker() {
  let manifest_dir =
    std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
  let marker_path = Path::new(&manifest_dir)
    .join("lib")
    .join("error-marker.json");
  println!("cargo:rerun-if-changed={}", marker_path.display());

  let raw = fs::read_to_string(&marker_path)
    .unwrap_or_else(|err| panic!("failed to read {}: {err}", marker_path.display()));
  let marker = extract_marker(&raw).unwrap_or_else(|| {
    panic!(
      "missing or invalid `marker` field in {}",
      marker_path.display()
    )
  });
  println!("cargo:rustc-env=OPENAPI_NG_ERROR_MARKER={marker}");
}

/// Tiny JSON walker — we only need the value of the top-level
/// `"marker"` string, and pulling in a JSON crate at build time is more
/// machinery than the one-field file warrants.
fn extract_marker(raw: &str) -> Option<String> {
  let after_key = raw.split_once("\"marker\"")?.1;
  let after_colon = after_key.split_once(':')?.1;
  let trimmed = after_colon.trim_start();
  let after_open_quote = trimmed.strip_prefix('"')?;
  let (value, _rest) = after_open_quote.split_once('"')?;
  Some(value.to_string())
}
