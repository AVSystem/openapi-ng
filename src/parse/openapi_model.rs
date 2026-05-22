use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(crate) struct OpenApiDocument {
  pub(crate) openapi: String,
  pub(crate) info: OpenApiInfo,
  pub(crate) paths: BTreeMap<String, PathItem>,
  #[serde(default)]
  pub(crate) components: Components,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenApiInfo {
  pub(crate) title: String,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct Components {
  #[serde(default)]
  pub(crate) schemas: BTreeMap<String, Schema>,
}

/// A path item in OpenAPI 3.x. Fields are in alphabetical method order to match
/// the BTreeMap ordering that the previous untyped implementation produced.
#[derive(Debug, Deserialize, Default)]
pub(crate) struct PathItem {
  pub(crate) delete: Option<Operation>,
  pub(crate) get: Option<Operation>,
  pub(crate) head: Option<Operation>,
  pub(crate) options: Option<Operation>,
  pub(crate) patch: Option<Operation>,
  pub(crate) post: Option<Operation>,
  pub(crate) put: Option<Operation>,
  pub(crate) trace: Option<Operation>,
}

impl PathItem {
  /// Iterate over all operations in this path item, yielding (method, operation) pairs.
  /// Methods are yielded in alphabetical order (delete, get, head, ...) matching the
  /// BTreeMap ordering of the previous untyped implementation.
  pub(crate) fn operations(&self) -> impl Iterator<Item = (&'static str, &Operation)> {
    [
      ("delete", self.delete.as_ref()),
      ("get", self.get.as_ref()),
      ("head", self.head.as_ref()),
      ("options", self.options.as_ref()),
      ("patch", self.patch.as_ref()),
      ("post", self.post.as_ref()),
      ("put", self.put.as_ref()),
      ("trace", self.trace.as_ref()),
    ]
    .into_iter()
    .filter_map(|(method, op)| op.map(|op| (method, op)))
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Operation {
  pub(crate) operation_id: Option<String>,
  #[serde(default)]
  pub(crate) tags: Vec<String>,
  #[serde(default)]
  pub(crate) parameters: Vec<Parameter>,
  pub(crate) request_body: Option<RequestBody>,
  pub(crate) responses: Option<BTreeMap<String, Response>>,
  pub(crate) summary: Option<String>,
  pub(crate) description: Option<String>,
  /// OpenAPI `deprecated: true` on the operation. Emitted as `@deprecated`
  /// in the JSDoc above the service method so call sites surface the IDE
  /// deprecation marker.
  #[serde(default)]
  pub(crate) deprecated: bool,
}

impl Operation {
  /// Returns summary and description joined with a blank line, or whichever
  /// is present alone. Whitespace-only values are treated as absent.
  pub(crate) fn merged_description(&self) -> Option<String> {
    match (
      self.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()),
      self.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
    ) {
      (None, None) => None,
      (Some(s), None) => Some(s.to_string()),
      (None, Some(d)) => Some(d.to_string()),
      (Some(s), Some(d)) => Some(format!("{s}\n\n{d}")),
    }
  }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Parameter {
  pub(crate) name: String,
  #[serde(rename = "in")]
  pub(crate) location: String,
  #[serde(default)]
  pub(crate) required: bool,
  pub(crate) schema: Option<Schema>,
  pub(crate) content: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequestBody {
  pub(crate) content: BTreeMap<String, MediaType>,
  #[serde(default)]
  pub(crate) required: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MediaType {
  pub(crate) schema: Option<Schema>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Response {
  pub(crate) content: Option<BTreeMap<String, MediaType>>,
}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename_all = "camelCase")]
pub(crate) struct Schema {
  #[serde(rename = "$ref")]
  pub(crate) ref_: Option<String>,
  #[serde(rename = "type")]
  pub(crate) type_: Option<String>,
  #[serde(rename = "enum")]
  pub(crate) enum_: Option<Vec<Value>>,
  pub(crate) one_of: Option<Vec<Schema>>,
  pub(crate) any_of: Option<Vec<Schema>>,
  pub(crate) all_of: Option<Vec<Schema>>,
  pub(crate) not: Option<Box<Schema>>,
  /// Preserves spec-author insertion order so generated TypeScript matches the source document.
  /// `BTreeMap` would silently re-sort properties alphabetically, destroying meaningful
  /// ordering (e.g. id/name/status/tags/nickname becoming id/name/nickname/status/tags).
  pub(crate) properties: Option<IndexMap<String, Schema>>,
  #[serde(default)]
  pub(crate) required: Vec<String>,
  pub(crate) additional_properties: Option<AdditionalProperties>,
  pub(crate) items: Option<Box<Schema>>,
  pub(crate) nullable: Option<bool>,
  pub(crate) discriminator: Option<Discriminator>,
  pub(crate) description: Option<String>,
  /// OpenAPI `deprecated: true` on the schema. Emitted as `@deprecated` in
  /// the JSDoc above the corresponding TypeScript declaration (top-level
  /// model or property) so consumers see the IDE deprecation marker at the
  /// reference site.
  #[serde(default)]
  pub(crate) deprecated: bool,
  /// OpenAPI `format` hint (e.g. `uuid`, `date-time`, `int32`). Currently
  /// not carried into the IR — the schema walker surfaces every occurrence
  /// as an `E_UNSUPPORTED_SEMANTIC` warning (subcode `format-dropped`) so
  /// spec authors see what's being dropped instead of the field being
  /// silently ignored.
  pub(crate) format: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[cfg_attr(test, derive(Clone))]
#[serde(rename_all = "camelCase")]
pub(crate) struct Discriminator {
  pub(crate) property_name: String,
  /// OpenAPI `discriminator.mapping`: maps a wire-value string to either
  /// a full `$ref` (`#/components/schemas/Cat`) or a bare schema name.
  /// Resolved at IR-build time to bare schema names so the emit-time
  /// narrowing pass can compare against `SchemaType::Ref` payloads
  /// directly. Defaults to empty when the spec omits the field.
  #[serde(default)]
  pub(crate) mapping: BTreeMap<String, String>,
}

#[cfg(test)]
impl Schema {
  pub(crate) fn default_string() -> Self {
    Self {
      type_: Some("string".to_string()),
      ..Default::default()
    }
  }

  pub(crate) fn wrap_array(items: Self) -> Self {
    Self {
      type_: Some("array".to_string()),
      items: Some(Box::new(items)),
      ..Default::default()
    }
  }

  pub(crate) fn wrap_one_of(members: Vec<Self>) -> Self {
    Self {
      one_of: Some(members),
      ..Default::default()
    }
  }

  pub(crate) fn wrap_nullable(inner: Self) -> Self {
    Self {
      nullable: Some(true),
      ..inner
    }
  }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(Clone))]
#[serde(untagged)]
pub(crate) enum AdditionalProperties {
  Schema(Box<Schema>),
  // The bool value (true vs false) is intentionally discarded — both
  // forms map to the same "unsupported subset" rejection in
  // normalize/schema.rs. Deserializing as a typed variant (rather than
  // a generic catch-all) keeps the rejection message accurate.
  Boolean(#[allow(dead_code)] bool),
}
