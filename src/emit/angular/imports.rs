use std::collections::{BTreeMap, BTreeSet};

use crate::emit::ts::{Writer, type_import_block};
use crate::ir::canonical::ResponseContent;
use crate::ir::schema::{SchemaType, collect_type_references};
use crate::plan::artifact_plan::{PlannedOperation, PlannedRequestBody, RequestFieldKind};

/// Path from a generated service file to the model artifact, one
/// directory above it.
const MODEL_IMPORT_PATH: &str = "../model.generated";

pub(super) fn render_service_imports(
  buffer: &mut Writer,
  operations: &[PlannedOperation<'_>],
  helper_import_path: &str,
) {
  buffer.line("import { Injectable } from '@angular/core';");

  let uses_http_params = operations.iter().any(|operation| {
    operation
      .request
      .fields
      .iter()
      .any(|field| field.kind == RequestFieldKind::Query)
  });
  let helper_import = if uses_http_params {
    format!("import {{ httpParams, requestFactory }} from '{helper_import_path}';")
  } else {
    format!("import {{ requestFactory }} from '{helper_import_path}';")
  };
  buffer.line(&helper_import);

  let imports: BTreeSet<&str> =
    operations
      .iter()
      .flat_map(operation_types)
      .fold(BTreeSet::new(), |mut imports, ty| {
        collect_type_references(ty, &mut imports);
        imports
      });

  if !imports.is_empty() {
    type_import_block(buffer, &BTreeMap::from([(MODEL_IMPORT_PATH, imports)]));
  }
}

/// Every model type an operation names. A form body and a non-JSON
/// response name none.
fn operation_types<'a>(
  operation: &'a PlannedOperation<'a>,
) -> impl Iterator<Item = &'a SchemaType> {
  let body: Box<dyn Iterator<Item = &'a SchemaType>> = match &operation.request.body {
    Some(PlannedRequestBody::Nested { ty, .. }) => Box::new(std::iter::once(*ty)),
    Some(PlannedRequestBody::FlatJson { properties, .. }) => {
      Box::new(properties.iter().map(|property| property.ty))
    }
    Some(PlannedRequestBody::Multipart { .. } | PlannedRequestBody::UrlEncoded { .. }) | None => {
      Box::new(std::iter::empty())
    }
  };
  let response = operation
    .response
    .as_ref()
    .and_then(|response| match response {
      ResponseContent::Json(Some(ty)) => Some(ty),
      ResponseContent::Json(None)
      | ResponseContent::Blob
      | ResponseContent::Text
      | ResponseContent::ArrayBuffer => None,
    });

  operation
    .request
    .fields
    .iter()
    .map(|field| field.ty)
    .chain(operation.request.headers.iter().map(|header| header.ty))
    .chain(body)
    .chain(response)
    .chain(operation.errors.iter().map(|error| &error.body))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::canonical::HttpMethod;
  use crate::ir::schema::{SchemaScalar, SchemaType};
  use crate::plan::artifact_plan::{
    PlannedHeader, PlannedRequestContract, PlannedRequestField, RequestFieldKind,
  };
  use crate::test_support::{body_field, empty_request, flat_json_body, nested_body, op_with};

  fn render(operations: &[PlannedOperation<'_>]) -> String {
    let mut buf = Writer::with_capacity(1024);
    render_service_imports(&mut buf, operations, "../rest.util");
    buf.into_string()
  }

  #[test]
  fn always_imports_injectable() {
    let out = render(&[op_with(
      "ping",
      HttpMethod::Get,
      "/x",
      empty_request(),
      None,
    )]);
    assert!(out.contains("import { Injectable } from '@angular/core';"));
    assert!(!out.contains("HttpClient"));
  }

  #[test]
  fn helper_import_omits_http_params_when_no_query_fields_exist() {
    let out = render(&[op_with(
      "ping",
      HttpMethod::Get,
      "/x",
      empty_request(),
      None,
    )]);
    assert!(out.contains("import { requestFactory } from '../rest.util';"));
    assert!(!out.contains("httpParams"));
  }

  #[test]
  fn helper_import_includes_http_params_when_any_operation_has_query_fields() {
    let limit_ty = SchemaType::Scalar(SchemaScalar::Number);
    let request = PlannedRequestContract {
      fields: vec![PlannedRequestField {
        name: "limit".into(),
        optional: true,
        ty: &limit_ty,
        kind: RequestFieldKind::Query,
      }],
      headers: vec![],
      body: None,
    };
    let out = render(&[op_with("listPets", HttpMethod::Get, "/x", request, None)]);
    assert!(out.contains("import { httpParams, requestFactory } from '../rest.util';"));
  }

  #[test]
  fn model_refs_are_deduplicated_across_operations() {
    let pet_ref = SchemaType::Ref("Pet".into());
    let pet_response = ResponseContent::Json(Some(SchemaType::Ref("Pet".into())));
    let op_a = op_with(
      "getPet",
      HttpMethod::Get,
      "/x",
      PlannedRequestContract {
        fields: vec![],
        headers: vec![],
        body: Some(nested_body(&pet_ref, false)),
      },
      Some(&pet_response),
    );
    let op_b = op_with(
      "listPets",
      HttpMethod::Get,
      "/x",
      empty_request(),
      Some(&pet_response),
    );
    let out = render(&[op_a, op_b]);
    // The single import line lists `Pet` exactly once.
    assert!(out.contains("import type { Pet } from '../model.generated';"));
    assert_eq!(out.matches("Pet").count(), 1);
  }

  #[test]
  fn model_refs_from_headers_are_imported() {
    let key_ty = SchemaType::Ref("IdempotencyKey".into());
    let request = PlannedRequestContract {
      fields: vec![],
      headers: vec![PlannedHeader {
        name: "X-Idempotency-Key".into(),
        optional: false,
        ty: &key_ty,
      }],
      body: None,
    };
    let out = render(&[op_with("createPet", HttpMethod::Get, "/x", request, None)]);
    assert!(out.contains("import type { IdempotencyKey } from '../model.generated';"));
  }

  #[test]
  fn nested_body_named_ref_is_imported() {
    let payload_ref = SchemaType::Ref("CreatePetPayload".into());
    let request = PlannedRequestContract {
      fields: vec![],
      headers: vec![],
      body: Some(nested_body(&payload_ref, false)),
    };
    let out = render(&[op_with("createPet", HttpMethod::Post, "/x", request, None)]);
    assert!(out.contains("import type { CreatePetPayload } from '../model.generated';"));
  }

  #[test]
  fn flat_json_body_property_refs_are_imported() {
    // Smart-flatten hoists inline-object body properties to top-level; each
    // property's `SchemaType` contributes imports the same way path/query
    // types do.
    let status_ref = SchemaType::Ref("PetStatus".into());
    let request = PlannedRequestContract {
      fields: vec![],
      headers: vec![],
      body: Some(flat_json_body(
        vec![body_field("status", false, &status_ref)],
        true,
      )),
    };
    let out = render(&[op_with("createPet", HttpMethod::Post, "/x", request, None)]);
    assert!(out.contains("import type { PetStatus } from '../model.generated';"));
  }

  #[test]
  fn nested_body_and_response_sharing_a_ref_yields_a_single_dedupe_import() {
    let pet_ref = SchemaType::Ref("Pet".into());
    let pet_response = ResponseContent::Json(Some(SchemaType::Ref("Pet".into())));
    let request = PlannedRequestContract {
      fields: vec![],
      headers: vec![],
      body: Some(nested_body(&pet_ref, false)),
    };
    let out = render(&[op_with(
      "createPet",
      HttpMethod::Post,
      "/x",
      request,
      Some(&pet_response),
    )]);
    assert!(out.contains("import type { Pet } from '../model.generated';"));
    assert_eq!(out.matches("Pet").count(), 1);
  }

  #[test]
  fn empty_operation_set_emits_only_fixed_imports() {
    let out = render(&[]);
    assert!(out.contains("Injectable"));
    assert!(out.contains("requestFactory"));
    assert!(!out.contains("HttpClient"));
    assert!(!out.contains("httpParams"));
    assert!(!out.contains("../model.generated"));
  }
}
