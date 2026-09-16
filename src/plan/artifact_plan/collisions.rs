//! Two plans that would write the same file.

use std::collections::BTreeMap;

use crate::error::{Diagnostic, Reporter};
use crate::subcode;

use super::{PlannedOperation, ServicePlan};

// Group names differing only in case or separators share a kebab file stem,
// so one group's files would overwrite the other's.
pub(super) fn reject_cross_group_path_collisions(
  services: &[ServicePlan<'_>],
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  services
    .iter()
    .flat_map(|service| service_paths(service).map(move |path| (path, service)))
    .try_fold(BTreeMap::new(), |mut owners, (path, service)| {
      owners.insert(path, service.group_name.as_str()).map_or(Ok(()), |previous| {
        Err(Diagnostic::policy_violation(
          reporter,
          subcode::NAMING_RESOLUTION,
          format!(
            "groups '{previous}' and '{}' both map to the file {path}; adjust naming.group so the file names differ.",
            service.group_name,
          ),
        ))
      })?;
      Ok(owners)
    })
    .map(|_| ())
}

/// Every file a service plan writes.
fn service_paths<'a>(service: &'a ServicePlan<'a>) -> impl Iterator<Item = &'a str> {
  std::iter::once(service.artifact_path.as_str())
    .chain(service.operations_barrel_path.as_deref())
    .chain(
      service
        .operations
        .iter()
        .filter_map(|operation| operation.artifact_path.as_deref()),
    )
}

// Distinct method names can share a kebab stem, and the barrel is a fixed
// file in the same directory.
pub(super) fn reject_artifact_path_collisions(
  group_name: &str,
  barrel_path: &str,
  operations: &[PlannedOperation<'_>],
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  operations
    .iter()
    .map(|operation| {
      let path = operation
        .artifact_path
        .as_deref()
        .expect("standalone operations carry an artifact path");
      (path, operation)
    })
    .try_fold(BTreeMap::new(), |mut by_path, (path, operation)| {
      if path == barrel_path {
        return Err(Diagnostic::policy_violation(
          reporter,
          subcode::RESERVED_IDENTIFIER,
          format!(
            "methodName '{}' for operation {} {} (operationId={}) cannot be a standalone operation: its file {path} is the barrel of group '{group_name}'. Adjust naming.methodName or use layout 'services'.",
            operation.method_name, operation.method, operation.path, operation.operation_id,
          ),
        ));
      }
      by_path.insert(path, operation).map_or(Ok(()), |previous| {
        Err(Diagnostic::policy_violation(
          reporter,
          subcode::NAMING_RESOLUTION,
          format!(
            "methodNames '{}' ({} {}, operationId={}) and '{}' ({} {}, operationId={}) in group '{group_name}' both map to the file {path}; adjust naming.methodName so the file names differ.",
            previous.method_name,
            previous.method,
            previous.path,
            previous.operation_id,
            operation.method_name,
            operation.method,
            operation.path,
            operation.operation_id,
          ),
        ))
      })?;
      Ok(by_path)
    })
    .map(|_| ())
}
