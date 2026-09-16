//! Every `subcode` a diagnostic can carry; `DiagnosticSubcode` in `index.d.ts.in` publishes the
//! same set.

pub(crate) const DISCRIMINATOR_PROPERTY_MUST_BE_STRING: &str =
  "discriminator-property-must-be-string";
pub(crate) const DUPLICATE_OPERATION_ID: &str = "duplicate-operation-id";
pub(crate) const DUPLICATE_SCHEMA_NAME: &str = "duplicate-schema-name";
pub(crate) const FIELD_COLLISION: &str = "field-collision";
pub(crate) const FORMAT_DROPPED: &str = "format-dropped";
pub(crate) const INVALID_FORM_FIELD_NAME: &str = "invalid-form-field-name";
pub(crate) const INVALID_PATH_PARAMETER_NAME: &str = "invalid-path-parameter-name";
pub(crate) const MAPPING_EXPANSION_EXCEEDED: &str = "mapping-expansion-exceeded";
pub(crate) const MISSING_BODY_SCHEMA: &str = "missing-body-schema";
pub(crate) const MISSING_DISCRIMINATOR_PROPERTY: &str = "missing-discriminator-property";
pub(crate) const MISSING_OPERATION_ID: &str = "missing-operation-id";
/// Published, but no current path emits it.
#[cfg(test)]
pub(crate) const MISSING_TAG: &str = "missing-tag";
pub(crate) const MULTI_CONTENT_BODY: &str = "multi-content-body";
pub(crate) const NAMING_RESOLUTION: &str = "naming-resolution";
pub(crate) const OPERATION_CAP_EXCEEDED: &str = "operation-cap-exceeded";
pub(crate) const RESERVED_IDENTIFIER: &str = "reserved-identifier";
pub(crate) const SCHEMA_CAP_EXCEEDED: &str = "schema-cap-exceeded";
pub(crate) const URLENCODED_BINARY_FIELD: &str = "urlencoded-binary-field";
pub(crate) const UNSUPPORTED_BODY_CONTENT_TYPE: &str = "unsupported-body-content-type";
pub(crate) const UNSUPPORTED_PARAMETER_LOCATION: &str = "unsupported-parameter-location";

/// The form-body subcodes `FormKind::subcode` selects by flavour.
#[cfg(test)]
pub(crate) const FORM: [&str; 8] = [
  "multipart-non-object-body",
  "multipart-open-schema",
  "multipart-nested-object",
  "multipart-composed-field",
  "urlencoded-non-object-body",
  "urlencoded-open-schema",
  "urlencoded-nested-object",
  "urlencoded-composed-field",
];

/// Subcodes the JavaScript wrapper raises before the boundary.
#[cfg(test)]
pub(crate) const WRAPPER: [&str; 1] = ["shape"];

#[cfg(test)]
fn all() -> Vec<&'static str> {
  let mut codes = vec![
    DISCRIMINATOR_PROPERTY_MUST_BE_STRING,
    DUPLICATE_OPERATION_ID,
    DUPLICATE_SCHEMA_NAME,
    FIELD_COLLISION,
    FORMAT_DROPPED,
    INVALID_FORM_FIELD_NAME,
    INVALID_PATH_PARAMETER_NAME,
    MAPPING_EXPANSION_EXCEEDED,
    MISSING_BODY_SCHEMA,
    MISSING_DISCRIMINATOR_PROPERTY,
    MISSING_OPERATION_ID,
    MISSING_TAG,
    MULTI_CONTENT_BODY,
    NAMING_RESOLUTION,
    OPERATION_CAP_EXCEEDED,
    RESERVED_IDENTIFIER,
    SCHEMA_CAP_EXCEEDED,
    UNSUPPORTED_BODY_CONTENT_TYPE,
    URLENCODED_BINARY_FIELD,
    UNSUPPORTED_PARAMETER_LOCATION,
  ];
  codes.extend(FORM);
  codes.extend(WRAPPER);
  codes.sort_unstable();
  codes
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeSet;

  /// A subcode emitted from Rust but absent from `DiagnosticSubcode` is a
  /// value outside the type consumers route on.
  #[test]
  fn every_subcode_is_published_in_the_declared_union() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/index.d.ts.in"))
      .expect("index.d.ts.in is readable");
    let union = source
      .split("DiagnosticSubcode =")
      .nth(1)
      .and_then(|rest| rest.split(';').next())
      .expect("index.d.ts.in declares DiagnosticSubcode");

    let published: BTreeSet<&str> = union
      .split('|')
      .filter_map(|entry| entry.trim().strip_prefix('\'')?.strip_suffix('\''))
      .collect();
    let emitted: BTreeSet<&str> = super::all().into_iter().collect();

    let undeclared: Vec<&&str> = emitted.difference(&published).collect();
    assert!(
      undeclared.is_empty(),
      "emitted but not published in DiagnosticSubcode: {undeclared:?}"
    );
    let unused: Vec<&&str> = published.difference(&emitted).collect();
    assert!(
      unused.is_empty(),
      "published in DiagnosticSubcode but never emitted: {unused:?}"
    );
  }
}
