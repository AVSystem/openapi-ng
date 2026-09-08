//! The emit-target registry: each [`EmitTarget`] maps to the artifacts it
//! produces, and every artifact path and template lives here.

use crate::bindings::EmitTarget;
use crate::plan::GenerationPlan;
use crate::result::GeneratedArtifact;

use super::MODEL_ARTIFACT_PATH;
use super::angular::{
  REST_MODEL_PATH, REST_MODEL_TEMPLATE, REST_UTIL_PATH, REST_UTIL_TEMPLATE, REST_VALIDATE_PATH,
  REST_VALIDATE_TEMPLATE, emit_service,
};
use super::model::emit_ts_models::emit_model;

/// A family of generated files.
pub(crate) trait Emitter {
  /// Produces this target's artifacts, in the order they should be
  /// emitted. Returns none when the plan gives the target nothing to do.
  fn artifacts(&self, plan: &GenerationPlan<'_>) -> Vec<GeneratedArtifact>;
}

/// The emitters `target` contributes, in emit order.
pub(crate) fn emitters_for(target: EmitTarget) -> Vec<Box<dyn Emitter>> {
  match target {
    EmitTarget::Models => vec![Box::new(TsModels)],
    EmitTarget::Angular => vec![
      Box::new(StaticTemplate::new(REST_MODEL_PATH, REST_MODEL_TEMPLATE)),
      Box::new(StaticTemplate::new(REST_UTIL_PATH, REST_UTIL_TEMPLATE)),
      Box::new(StaticTemplate::new(
        REST_VALIDATE_PATH,
        REST_VALIDATE_TEMPLATE,
      )),
      Box::new(AngularServices),
    ],
  }
}

/// The TypeScript model file. Emits nothing when the spec declares no
/// schemas.
struct TsModels;

impl Emitter for TsModels {
  fn artifacts(&self, plan: &GenerationPlan<'_>) -> Vec<GeneratedArtifact> {
    if plan.schemas.is_empty() {
      return Vec::new();
    }
    vec![GeneratedArtifact::new(
      MODEL_ARTIFACT_PATH.to_string(),
      emit_model(plan.schemas, &plan.mapped_types),
    )]
  }
}

/// A support file copied verbatim from `templates/`.
struct StaticTemplate {
  path: &'static str,
  body: &'static str,
}

impl StaticTemplate {
  const fn new(path: &'static str, body: &'static str) -> Self {
    Self { path, body }
  }
}

impl Emitter for StaticTemplate {
  fn artifacts(&self, _plan: &GenerationPlan<'_>) -> Vec<GeneratedArtifact> {
    vec![GeneratedArtifact::new(
      self.path.to_string(),
      self.body.to_string(),
    )]
  }
}

/// One Angular service per planned group, in `plan.services` order.
struct AngularServices;

impl Emitter for AngularServices {
  fn artifacts(&self, plan: &GenerationPlan<'_>) -> Vec<GeneratedArtifact> {
    plan
      .services
      .iter()
      .map(|service| GeneratedArtifact::new(service.artifact_path.clone(), emit_service(service)))
      .collect()
  }
}
