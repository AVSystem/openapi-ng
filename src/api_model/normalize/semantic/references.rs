//! Fails on a `$ref` that names no declared schema.

use std::collections::BTreeSet;

use crate::api_model::canonical::{ApiModel, BodyContent, OperationDef, ResponseContent};
use crate::api_model::schema::{SchemaType, collect_type_references};
use crate::error::{Diagnostic, DiagnosticCode, Reporter, bail};

/// Every schema-typed position an operation declares.
fn operation_types(operation: &OperationDef) -> impl Iterator<Item = &SchemaType> {
  let body = operation
    .request
    .body
    .as_ref()
    .and_then(|body| match &body.content {
      BodyContent::Json(schema) => Some(schema),
      BodyContent::Multipart { .. } | BodyContent::UrlEncoded { .. } => None,
    });
  let response = operation
    .response
    .as_ref()
    .and_then(|response| match response {
      ResponseContent::Json(Some(schema)) => Some(schema),
      ResponseContent::Json(None)
      | ResponseContent::Blob
      | ResponseContent::Text
      | ResponseContent::ArrayBuffer => None,
    });

  operation
    .request
    .inputs
    .iter()
    .map(|input| &input.schema)
    .chain(
      operation
        .request
        .headers
        .iter()
        .map(|header| &header.schema),
    )
    .chain(body)
    .chain(response)
}

/// Fails on the first `$ref` that names no declared schema.
pub(super) fn validate(document: &ApiModel, reporter: &Reporter) -> Result<(), Diagnostic> {
  let symbol_index: BTreeSet<&str> = document
    .schemas
    .iter()
    .map(|symbol| symbol.name.as_ref())
    .collect();
  let refs: BTreeSet<&str> = document
    .schemas
    .iter()
    .map(|symbol| &symbol.body)
    .chain(document.operations.iter().flat_map(operation_types))
    .fold(BTreeSet::new(), |mut refs, schema| {
      collect_type_references(schema, &mut refs);
      refs
    });

  if let Some(name) = refs.into_iter().find(|name| !symbol_index.contains(name)) {
    bail!(
      reporter,
      DiagnosticCode::InvalidReference,
      "Failed to validate spec: unresolved schema reference {name}. Check for typos in the $ref and confirm that components.schemas defines a top-level entry named '{name}'."
    );
  }

  Ok(())
}
