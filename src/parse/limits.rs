//! Per-document input caps, each overridable by its environment
//! variable.

use std::str::FromStr;
use std::sync::OnceLock;

/// A cap read once per process from the environment, falling back to a
/// compile-time default when the variable is absent or unparsable.
pub(crate) struct EnvCap<T> {
  variable: &'static str,
  default: T,
  cached: OnceLock<T>,
}

impl<T: FromStr + Copy> EnvCap<T> {
  pub(crate) const fn new(variable: &'static str, default: T) -> Self {
    Self {
      variable,
      default,
      cached: OnceLock::new(),
    }
  }

  pub(crate) fn get(&self) -> T {
    *self
      .cached
      .get_or_init(|| self.parse(std::env::var(self.variable).ok().as_deref()))
  }

  /// Resolves the cap from `raw` rather than from the environment, and
  /// caches nothing.
  pub(crate) fn parse(&self, raw: Option<&str>) -> T {
    raw
      .and_then(|value| value.parse().ok())
      .unwrap_or(self.default)
  }
}

/// Largest accepted input, in bytes.
pub(crate) static MAX_INPUT_BYTES: EnvCap<u64> =
  EnvCap::new("OPENAPI_NG_MAX_INPUT_BYTES", 16 * 1024 * 1024);

/// Largest accepted `components.schemas` count.
pub(crate) static MAX_SCHEMAS: EnvCap<usize> = EnvCap::new("OPENAPI_NG_MAX_SCHEMAS", 10_000);

/// Largest accepted operation count across all paths.
pub(crate) static MAX_OPERATIONS: EnvCap<usize> = EnvCap::new("OPENAPI_NG_MAX_OPERATIONS", 10_000);

/// Largest accepted ratio of re-serialised parsed bytes to source bytes.
///
/// The default sits well above real specs: the Swagger Petstore
/// re-serialises near 1×, and anchor-heavy hand-written specs under 10×.
pub(crate) static MAX_EXPANSION_RATIO: EnvCap<usize> =
  EnvCap::new("OPENAPI_NG_MAX_EXPANSION_RATIO", 50);

#[cfg(test)]
mod tests {
  use super::{EnvCap, MAX_EXPANSION_RATIO, MAX_INPUT_BYTES, MAX_OPERATIONS, MAX_SCHEMAS};

  #[test]
  fn absent_or_unparsable_values_fall_back_to_the_default() {
    let cap: EnvCap<usize> = EnvCap::new("UNUSED", 42);
    assert_eq!(cap.parse(None), 42);
    assert_eq!(cap.parse(Some("")), 42);
    assert_eq!(cap.parse(Some("not-a-number")), 42);
    assert_eq!(cap.parse(Some("-1")), 42);
  }

  #[test]
  fn a_parsable_value_overrides_the_default() {
    let cap: EnvCap<usize> = EnvCap::new("UNUSED", 42);
    assert_eq!(cap.parse(Some("7")), 7);
    assert_eq!(cap.parse(Some("0")), 0);
  }

  #[test]
  fn declared_defaults_match_the_documented_values() {
    assert_eq!(MAX_INPUT_BYTES.parse(None), 16 * 1024 * 1024);
    assert_eq!(MAX_SCHEMAS.parse(None), 10_000);
    assert_eq!(MAX_OPERATIONS.parse(None), 10_000);
    assert_eq!(MAX_EXPANSION_RATIO.parse(None), 50);
  }
}
