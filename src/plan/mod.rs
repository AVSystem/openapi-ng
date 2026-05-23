// Planning logic that turns `ApiModel` into emitter-ready service plans
// and validates mapped-type configuration against the IR.

pub(crate) mod artifact_plan;
pub mod naming;
pub(crate) mod services;

use crate::{
  bindings::EmitTarget,
  error::{Diagnostic, Reporter},
  ir::canonical::ApiModel,
  options::GenerateConfig,
};

use artifact_plan::{
  ResolvedMappedType, ServicePlan, resolve_service_plans, validate_mapped_types_against_schemas,
};

/// Pre-emit plan: the validated mapped-type list shared by the model
/// emitter, plus the per-tag Angular service plans. The pipeline
/// decides which artifacts to emit by inspecting `config.emit` directly;
/// `services` is empty when Angular is not selected.
pub(crate) struct GenerationPlan<'ir> {
  pub(crate) mapped_types: Vec<ResolvedMappedType<'ir>>,
  pub(crate) services: Vec<ServicePlan<'ir>>,
}

/// Builds the pre-emit plan from the validated config and IR. All
/// cross-target validation (e.g. `emit_models` gates mapped-type
/// resolution) lives here so the pipeline is a flat sequence of
/// guarded emit calls.
pub(crate) fn plan_generation<'ir>(
  config: &GenerateConfig,
  ir: &'ir ApiModel,
  reporter: &Reporter<'_>,
) -> Result<GenerationPlan<'ir>, Diagnostic> {
  let emit_models = config.emit.contains(&EmitTarget::Models);
  let emit_angular = config.emit.contains(&EmitTarget::Angular);

  let mapped_types = if emit_models && !config.mapped_types.is_empty() {
    validate_mapped_types_against_schemas(&ir.schemas, &config.mapped_types, reporter)?
  } else {
    Vec::new()
  };

  let services = if emit_angular {
    let resolver = crate::plan::naming::NamingResolver::new(config.naming.clone());
    resolve_service_plans(ir, &resolver, reporter)?
  } else {
    Vec::new()
  };

  Ok(GenerationPlan {
    mapped_types,
    services,
  })
}
