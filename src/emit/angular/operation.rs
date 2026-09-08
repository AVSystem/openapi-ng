use crate::wln;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::emit::typescript::{Writer, jsdoc};
use crate::plan::artifact_plan::{PlannedOperation, ServicePlan};
use crate::plan::naming::{error_interface_name, operation_file_stem, request_interface_name};

use super::imports::{
  collect_model_type_imports, uses_http_params, write_helper_import, write_model_imports,
};
use super::request::{
  render_error_interface, render_request_interface, render_requestful_builder,
  render_zero_arg_builder,
};
use super::service::{has_request_interface, write_call_site};

// Operation files live at `rest/<group>/<method>.ts`, one level
// below the service files.
const HELPER_IMPORT_PATH: &str = "../../rest.util";
const MODEL_IMPORT_PATH: &str = "../../model";

// Names that cannot be declared with `export const` in a module: ES
// reserved words, strict-mode reserved words, and the two identifiers
// strict mode refuses as binding names.
const RESERVED_IDENTIFIERS: &[&str] = &[
  "arguments",
  "await",
  "break",
  "case",
  "catch",
  "class",
  "const",
  "continue",
  "debugger",
  "default",
  "delete",
  "do",
  "else",
  "enum",
  "eval",
  "export",
  "extends",
  "false",
  "finally",
  "for",
  "function",
  "if",
  "implements",
  "import",
  "in",
  "instanceof",
  "interface",
  "let",
  "new",
  "null",
  "package",
  "private",
  "protected",
  "public",
  "return",
  "static",
  "super",
  "switch",
  "this",
  "throw",
  "true",
  "try",
  "typeof",
  "var",
  "void",
  "while",
  "with",
  "yield",
];

/// One standalone operation file: the `defineOperation(...)` constant
/// followed by its `{Pascal}Params` / `{Pascal}Error` interfaces.
pub(crate) fn emit_operation(operation: &PlannedOperation<'_>) -> String {
  let operations = std::slice::from_ref(operation);
  let helper_symbols: &[&str] = if uses_http_params(operations) {
    &["defineOperation", "httpParams"]
  } else {
    &["defineOperation"]
  };
  let model_imports = collect_model_type_imports(operations);
  let name = operation.method_name.as_str();
  // Export specifiers accept any IdentifierName, so a name that cannot be
  // a `const` binding is declared under `<name>_` and exported as itself.
  let local_alias = needs_alias(name, helper_symbols, &model_imports).then(|| format!("{name}_"));
  let request_name = has_request_interface(operation).then(|| request_interface_name(name));

  let mut buffer = Writer::with_capacity(1024);
  write_helper_import(&mut buffer, HELPER_IMPORT_PATH, helper_symbols);
  write_model_imports(&mut buffer, &model_imports, MODEL_IMPORT_PATH);
  buffer.blank_line();

  jsdoc(
    &mut buffer,
    operation.description.as_deref(),
    operation.deprecated,
  );
  // `@__PURE__` lets bundlers drop operations a barrel import never touches.
  match &local_alias {
    Some(local) => write!(buffer, "const {local} = /* @__PURE__ */ ").unwrap(),
    None => write!(buffer, "export const {name} = /* @__PURE__ */ ").unwrap(),
  }
  write_call_site(
    &mut buffer,
    "defineOperation",
    operation.response,
    request_name.as_ref(),
  );
  buffer.push("(\n");
  buffer.indent();
  wln!(buffer, "'{name}',");
  match &request_name {
    Some(request) => render_requestful_builder(&mut buffer, operation, request),
    None => render_zero_arg_builder(&mut buffer, operation),
  }
  buffer.dedent();
  buffer.line(");");
  if let Some(local) = &local_alias {
    buffer.blank_line();
    wln!(buffer, "export {{ {local} as {name} }};");
  }

  if let Some(request) = &request_name {
    buffer.blank_line();
    render_request_interface(&mut buffer, operation, request);
  }
  if !operation.errors.is_empty() {
    buffer.blank_line();
    render_error_interface(&mut buffer, operation, &error_interface_name(name));
  }

  buffer.into_string()
}

fn needs_alias(name: &str, helper_symbols: &[&str], model_imports: &BTreeSet<&str>) -> bool {
  RESERVED_IDENTIFIERS.contains(&name)
    || helper_symbols.contains(&name)
    || model_imports.contains(name)
}

/// `rest/<group>/index.ts`: re-exports every operation file in its
/// directory so a namespace import of the barrel keeps only the members
/// it touches.
pub(crate) fn emit_operations_barrel(service_plan: &ServicePlan<'_>) -> String {
  let mut buffer = Writer::with_capacity(service_plan.operations.len() * 48 + 16);
  for operation in &service_plan.operations {
    let file_stem = operation_file_stem(&operation.method_name);
    wln!(buffer, "export * from './{file_stem}';");
  }
  buffer.into_string()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ir::canonical::{ErrorResponse, HttpMethod, ResponseContent};
  use crate::ir::schema::{SchemaScalar, SchemaType};
  use crate::plan::artifact_plan::{PlannedRequestContract, PlannedRequestField, RequestFieldKind};
  use crate::test_support::{empty_request, op_with, path_field, string_ty};

  fn op<'a>(
    method_name: &str,
    request: PlannedRequestContract<'a>,
    response: Option<&'a ResponseContent>,
  ) -> PlannedOperation<'a> {
    let mut operation = op_with(method_name, HttpMethod::Get, "/x/{id}", request, response);
    operation.artifact_path = Some(format!(
      "rest/pet/{}.ts",
      crate::plan::naming::operation_file_stem(method_name)
    ));
    operation
  }

  fn requestful<'a>(
    method_name: &str,
    path_ty: &'a SchemaType,
    response: &'a ResponseContent,
  ) -> PlannedOperation<'a> {
    op(
      method_name,
      PlannedRequestContract {
        fields: vec![path_field("id", path_ty)],
        headers: vec![],
        body: None,
      },
      Some(response),
    )
  }

  fn zero_arg<'a>(method_name: &str, response: &'a ResponseContent) -> PlannedOperation<'a> {
    op(method_name, empty_request(), Some(response))
  }

  #[test]
  fn define_operation_call_shapes_mirror_request_factory() {
    let str_ty = string_ty();
    let json = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let cases: [(PlannedOperation<'_>, &str); 8] = [
      (
        requestful("listPets", &str_ty, &json),
        "export const listPets = /* @__PURE__ */ defineOperation<ListPetsParams, string>(\n  'listPets',",
      ),
      (
        requestful("download", &str_ty, &ResponseContent::Blob),
        "export const download = /* @__PURE__ */ defineOperation.blob<DownloadParams>(",
      ),
      (
        requestful("rawConfig", &str_ty, &ResponseContent::Text),
        "export const rawConfig = /* @__PURE__ */ defineOperation.text<RawConfigParams>(",
      ),
      (
        requestful("fetch", &str_ty, &ResponseContent::ArrayBuffer),
        "export const fetch = /* @__PURE__ */ defineOperation.arrayBuffer<FetchParams>(",
      ),
      (
        zero_arg("listPets", &json),
        "export const listPets = /* @__PURE__ */ defineOperation.zeroArg<string>(\n  'listPets',",
      ),
      (
        zero_arg("download", &ResponseContent::Blob),
        "export const download = /* @__PURE__ */ defineOperation.zeroArg.blob(",
      ),
      (
        zero_arg("rawConfig", &ResponseContent::Text),
        "export const rawConfig = /* @__PURE__ */ defineOperation.zeroArg.text(",
      ),
      (
        zero_arg("fetch", &ResponseContent::ArrayBuffer),
        "export const fetch = /* @__PURE__ */ defineOperation.zeroArg.arrayBuffer(",
      ),
    ];
    for (operation, expected) in &cases {
      let out = emit_operation(operation);
      assert!(out.contains(expected), "expected {expected:?} in:\n{out}");
      assert!(
        !out.contains("requestFactory"),
        "no requestFactory in:\n{out}"
      );
    }
  }

  #[test]
  fn operation_file_imports_two_levels_up_and_ends_with_interfaces() {
    let str_ty = string_ty();
    let json = ResponseContent::Json(Some(SchemaType::Scalar(SchemaScalar::String)));
    let out = emit_operation(&requestful("listPets", &str_ty, &json));

    assert!(out.starts_with("import { defineOperation } from '../../rest.util';\n\n"));
    assert!(!out.contains("Injectable"));
    assert!(out.contains("(request: ListPetsParams) => {"));
    assert!(out.contains("\nexport interface ListPetsParams {"));
  }

  #[test]
  fn query_field_imports_http_params_and_model_types_two_levels_up() {
    let pet = SchemaType::Ref("Pet".into());
    let json = ResponseContent::Json(Some(pet.clone()));
    let mut operation = op(
      "listPets",
      PlannedRequestContract {
        fields: vec![PlannedRequestField {
          name: "status".into(),
          optional: true,
          ty: &pet,
          kind: RequestFieldKind::Query,
        }],
        headers: vec![],
        body: None,
      },
      Some(&json),
    );
    operation.path = "/pets".to_string();
    let out = emit_operation(&operation);

    assert!(out.contains("import { defineOperation, httpParams } from '../../rest.util';"));
    assert!(out.contains("import type { Pet } from '../../model';"));
  }

  #[test]
  fn reserved_word_is_declared_under_alias_and_exported_as_itself() {
    let str_ty = string_ty();
    let void = ResponseContent::Json(None);
    let out = emit_operation(&requestful("delete", &str_ty, &void));

    assert!(out.contains(
      "const delete_ = /* @__PURE__ */ defineOperation<DeleteParams, void>(\n  'delete',"
    ));
    assert!(out.contains("\nexport { delete_ as delete };\n"));
    assert!(!out.contains("export const delete"));
  }

  #[test]
  fn runtime_import_collision_is_aliased() {
    let str_ty = string_ty();
    let void = ResponseContent::Json(None);
    let mut operation = op(
      "httpParams",
      PlannedRequestContract {
        fields: vec![PlannedRequestField {
          name: "limit".into(),
          optional: true,
          ty: &str_ty,
          kind: RequestFieldKind::Query,
        }],
        headers: vec![],
        body: None,
      },
      Some(&void),
    );
    operation.path = "/x".to_string();
    let out = emit_operation(&operation);

    assert!(out.contains("import { defineOperation, httpParams } from '../../rest.util';"));
    assert!(out.contains("const httpParams_ = "));
    assert!(out.contains("export { httpParams_ as httpParams };"));
  }

  #[test]
  fn model_type_import_collision_is_aliased() {
    let pet = SchemaType::Ref("Pet".into());
    let json = ResponseContent::Json(Some(pet));
    let out = emit_operation(&zero_arg("Pet", &json));

    assert!(out.contains("import type { Pet } from '../../model';"));
    assert!(out.contains("const Pet_ = /* @__PURE__ */ defineOperation.zeroArg<Pet>("));
    assert!(out.contains("export { Pet_ as Pet };"));
  }

  #[test]
  fn error_interface_follows_params_interface() {
    let body = SchemaType::Ref("Problem".into());
    let errors = [ErrorResponse { status: 404, body }];
    let mut operation = crate::test_support::op_with_errors("getPet", &errors);
    operation.artifact_path = Some("rest/pet/get-pet.ts".to_string());
    let out = emit_operation(&operation);

    assert!(out.contains("export interface GetPetError {"));
    assert!(out.contains("import type { Problem } from '../../model';"));
  }

  #[test]
  fn barrel_reexports_each_operation_file_by_sibling_specifier() {
    let str_ty = string_ty();
    let void = ResponseContent::Json(None);
    let plan = ServicePlan {
      group_name: "pet".into(),
      class_name: "PetRest".into(),
      artifact_path: "rest/pet.rest.ts".to_string(),
      operations_barrel_path: Some("rest/pet/index.ts".to_string()),
      operations: vec![
        requestful("delete", &str_ty, &void),
        zero_arg("listPets", &void),
      ],
    };
    assert_eq!(
      emit_operations_barrel(&plan),
      "export * from './delete';\nexport * from './list-pets';\n"
    );
  }
}
