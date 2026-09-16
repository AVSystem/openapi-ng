//! Turns an `ApiModel` into the plan an emitter reads, and validates the caller's mapped types
//! against it.

pub(crate) mod artifact_plan;
pub mod naming;
pub(crate) mod services;

use crate::{
  api_model::canonical::ApiModel,
  bindings::EmitTarget,
  error::{Diagnostic, Reporter},
  options::GenerateConfig,
};

use artifact_plan::{
  ResolvedMappedType, ServicePlan, resolve_service_plans, validate_mapped_types_against_schemas,
};

/// Everything the emitters read: the IR's model symbols, the validated mapped-type list, and
/// the per-group Angular service plans.
pub(crate) struct GenerationPlan<'model> {
  pub(crate) mapped_types: Vec<ResolvedMappedType<'model>>,
  pub(crate) services: Vec<ServicePlan<'model>>,
}

/// Builds the plan for the targets `config` selects.
pub(crate) fn plan_generation<'model>(
  config: &GenerateConfig,
  model: &'model ApiModel,
  reporter: &Reporter,
) -> Result<GenerationPlan<'model>, Diagnostic> {
  let emit_models = config.emit.contains(&EmitTarget::Models);
  let emit_angular = config.emit.contains(&EmitTarget::Angular);

  let mapped_types = if emit_models && !config.mapped_types.is_empty() {
    validate_mapped_types_against_schemas(&model.schemas, &config.mapped_types, reporter)?
  } else {
    Vec::new()
  };

  let services = if emit_angular {
    let resolver = crate::plan::naming::NamingResolver::new(config.naming.clone());
    resolve_service_plans(
      model,
      &resolver,
      reporter,
      config.layout.contains(&crate::bindings::Layout::Operations),
    )?
  } else {
    Vec::new()
  };

  Ok(GenerationPlan {
    mapped_types,
    services,
  })
}
