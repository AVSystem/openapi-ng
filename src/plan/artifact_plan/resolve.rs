//! Lowering the IR into the per-service and per-operation plans.

use crate::subcode;
use std::collections::BTreeMap;

use crate::{
  api_model::canonical::{ApiModel, ModelSymbol},
  error::{Diagnostic, DiagnosticCode, Reporter},
  identifier::MethodName,
  options::MappedType,
};

use crate::plan::naming::{
  error_interface_name, operation_file_stem, request_interface_name, service_class_name,
  service_file_stem,
};
use crate::plan::services::plan_request_contract;

use super::collisions::{reject_artifact_path_collisions, reject_cross_group_path_collisions};
use super::*;

/// Resolves each mapped type against `model_symbols`, failing on the first `schema` the IR does
/// not declare.
pub(crate) fn validate_mapped_types_against_schemas<'model>(
  model_symbols: &'model [ModelSymbol],
  mapped_types: &[MappedType],
  reporter: &Reporter,
) -> Result<Vec<ResolvedMappedType<'model>>, Diagnostic> {
  let by_name = model_symbols
    .iter()
    .map(|symbol| (symbol.name.as_ref(), symbol))
    .collect::<BTreeMap<&str, &ModelSymbol>>();

  mapped_types
    .iter()
    .map(|mapped_type| {
      let symbol = by_name.get(mapped_type.schema.as_str()).ok_or_else(|| {
        reporter.error(
          DiagnosticCode::InvalidOption,
          format!(
            "Failed to resolve generation options: mapped schema {} does not exist in the IR.",
            mapped_type.schema
          ),
        )
      })?;
      Ok(ResolvedMappedType::new(symbol.name.as_ref(), mapped_type))
    })
    .collect()
}

pub(crate) fn resolve_service_plans<'model>(
  model: &'model ApiModel,
  resolver: &crate::plan::naming::NamingResolver,
  reporter: &Reporter,
  standalone: bool,
) -> Result<Vec<ServicePlan<'model>>, Diagnostic> {
  use crate::plan::services::group_operations;

  let mut services = group_operations(&model.operations, resolver, reporter)?
    .into_iter()
    .map(|(group_name, group)| {
      let file_stem = group_file_stem(&group_name, reporter)?;
      let mut operations = group
        .into_iter()
        .map(|(operation, method_name)| {
          plan_operation(operation, method_name, &file_stem, standalone, reporter)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
      operations.sort_by(|left, right| left.method_name.cmp(&right.method_name));

      let operations_barrel_path = standalone.then(|| format!("rest/{file_stem}/index.ts"));
      if let Some(barrel_path) = &operations_barrel_path {
        reject_artifact_path_collisions(&group_name, barrel_path, &operations, reporter)?;
      }

      Ok(ServicePlan {
        class_name: service_class_name(&group_name),
        artifact_path: format!("rest/{file_stem}.rest.ts"),
        operations_barrel_path,
        group_name,
        operations,
      })
    })
    .collect::<Result<Vec<_>, Diagnostic>>()?;
  services.sort_by(|left, right| left.class_name.cmp(&right.class_name));
  reject_cross_group_path_collisions(&services, reporter)?;

  Ok(services)
}

fn plan_operation<'model>(
  operation: &'model crate::api_model::canonical::OperationDef,
  method_name: MethodName,
  file_stem: &str,
  standalone: bool,
  reporter: &Reporter,
) -> Result<PlannedOperation<'model>, Diagnostic> {
  let request = plan_request_contract(operation, reporter)?;
  Ok(PlannedOperation {
    operation_id: operation.operation_id.clone(),
    request_interface: takes_input(&request).then(|| request_interface_name(&method_name)),
    error_interface: (!operation.errors.is_empty()).then(|| error_interface_name(&method_name)),
    artifact_path: standalone
      .then(|| operation_artifact_path(operation, &method_name, file_stem, reporter))
      .transpose()?,
    method_name,
    method: operation.method,
    path: operation.path.clone(),
    request,
    response: operation.response.as_ref(),
    errors: operation.errors.as_slice(),
    description: operation.description.clone(),
    deprecated: operation.deprecated,
  })
}

/// Kebab-case stem for a group's files, rejecting a name that leaves none: the paths built from
/// it would carry an empty segment.
fn group_file_stem(group_name: &str, reporter: &Reporter) -> Result<String, Diagnostic> {
  let stem = service_file_stem(group_name);
  if stem.is_empty() {
    return Err(Diagnostic::policy_violation(
      reporter,
      subcode::NAMING_RESOLUTION,
      format!(
        "group '{group_name}' has no letters or digits, so it cannot name a service file. Adjust naming.group."
      ),
    ));
  }
  Ok(stem)
}

/// `rest/<group>/<method>.ts`, rejecting a method name the barrel cannot re-export or that
/// names no file.
fn operation_artifact_path(
  operation: &crate::api_model::canonical::OperationDef,
  method_name: &MethodName,
  file_stem: &str,
  reporter: &Reporter,
) -> Result<String, Diagnostic> {
  if method_name.as_str() == "default" {
    return Err(Diagnostic::policy_violation(
      reporter,
      subcode::RESERVED_IDENTIFIER,
      format!(
        "methodName 'default' for operation {} {} (operationId={}) cannot be a standalone operation: the barrel would expose it as `ops.default`. Adjust naming.methodName or use layout 'services'.",
        operation.method, operation.path, operation.operation_id,
      ),
    ));
  }
  let stem = operation_file_stem(method_name.as_str());
  if stem.is_empty() {
    return Err(Diagnostic::policy_violation(
      reporter,
      subcode::NAMING_RESOLUTION,
      format!(
        "methodName '{method_name}' for operation {} {} (operationId={}) has no letters or digits, so it cannot name a standalone operation file. Adjust naming.methodName or use layout 'services'.",
        operation.method, operation.path, operation.operation_id,
      ),
    ));
  }
  Ok(format!("rest/{file_stem}/{stem}.ts"))
}

/// True when the operation declares any path, query, header or body input.
#[must_use]
const fn takes_input(request: &PlannedRequestContract<'_>) -> bool {
  !request.fields.is_empty() || request.body.is_some() || !request.headers.is_empty()
}
