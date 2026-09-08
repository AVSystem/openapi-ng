//! Property tests for the schema walk.

use std::rc::Rc;

use proptest::prelude::*;

use super::normalize_named_schema;
use crate::error::{DiagnosticCode, Reporter};
use crate::parse::openapi_model::Schema;

fn arb_schema(max_depth: u32) -> impl Strategy<Value = Schema> {
  let leaf = Just(Schema::default_string());
  leaf.prop_recursive(max_depth, 32, 4, |inner| {
    prop_oneof![
      inner.clone().prop_map(Schema::wrap_array),
      proptest::collection::vec(inner.clone(), 0..3).prop_map(Schema::wrap_one_of),
      inner.prop_map(Schema::wrap_nullable),
    ]
  })
}

proptest! {
  #![proptest_config(ProptestConfig {
    cases: 128,
    ..ProptestConfig::default()
  })]

  #[test]
  fn normalize_named_schema_never_panics(schema in arb_schema(40)) {
    let path: Rc<str> = Rc::from("test");
    let reporter = Reporter::new(path);
    let result = normalize_named_schema("Root", &schema, &reporter);

    if let Err(diag) = result {
      prop_assert!(
        matches!(
          diag.code,
          DiagnosticCode::UnsupportedSemantic | DiagnosticCode::PolicyViolation,
        ),
        "unexpected diagnostic code: {:?}", diag.code,
      );
    }
  }
}

#[test]
fn depth_exceeded_diagnostic_includes_breadcrumb_chain() {
  // Build a 40-level-deep schema by wrapping in array; MAX_NORMALIZE_DEPTH is 32.
  let mut schema = Schema::default_string();
  for _ in 0..40 {
    schema = Schema::wrap_array(schema);
  }

  let path: Rc<str> = Rc::from("test");
  let reporter = Reporter::new(path);
  let err = normalize_named_schema("Root", &schema, &reporter)
    .expect_err("should fail with depth exceeded");

  assert!(
    err.message.contains("32"),
    "expected depth limit in message: {}",
    err.message,
  );
  assert!(
    err.message.contains("Root"),
    "expected root breadcrumb in message: {}",
    err.message,
  );
}
