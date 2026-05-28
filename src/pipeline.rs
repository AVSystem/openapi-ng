use std::rc::Rc;

use crate::{
  bindings::EmitTarget,
  emit::{
    MODEL_ARTIFACT_PATH,
    angular::{
      REST_MODEL_PATH, REST_MODEL_TEMPLATE, REST_UTIL_PATH, REST_UTIL_TEMPLATE, REST_VALIDATE_PATH,
      REST_VALIDATE_TEMPLATE, emit_service,
    },
    model::emit_ts_models::emit_model,
    render_generated_banner,
  },
  error::{Diagnostic, Reporter},
  ir::canonical::ApiModel,
  options::{GenerateConfig, validate_generate_config},
  plan::plan_generation,
  result::{GenerateSummary, GeneratedArtifact},
};

// ── Result types ────────────────────────────────────────────────────────────

pub struct GenerateResult {
  pub summary: GenerateSummary,
  pub diagnostics: Vec<Diagnostic>,
  pub artifacts: Vec<GeneratedArtifact>,
}

/// Top-level pipeline outcome on failure: the accumulated warnings up to the
/// failure point, plus the fatal diagnostic that ended the pipeline. Warnings
/// "ride on the reporter" inside stages; this struct exists only at the
/// pipeline boundary so the NAPI layer can surface both halves to the
/// consumer.
#[derive(Debug)]
pub struct GenerateFailure {
  pub warnings: Vec<Diagnostic>,
  pub fatal: Diagnostic,
}

// ── Pipeline ────────────────────────────────────────────────────────────────

/// Decode → policy-check → normalize. `normalize_api_model` performs
/// the final semantic step (discriminator narrowing + `$ref`
/// validation) before returning.
pub(crate) fn build_ir(
  config: &GenerateConfig,
  display_path: &Rc<str>,
  reporter: &mut Reporter<'_>,
) -> Result<ApiModel, Diagnostic> {
  let document = match (&config.input_path, &config.input_contents) {
    (Some(path), None) => crate::parse::read_and_decode(path, display_path)?,
    (None, Some(contents)) => {
      crate::parse::decode_input_contents(contents, config.input_format, display_path)?
    }
    // Validator guarantees exactly-one — these branches are unreachable
    // in practice but we keep them defensive rather than panicking.
    _ => {
      return Err(Diagnostic::new(
        crate::error::DiagnosticCode::InvalidOption,
        "internal: pipeline reached build_ir with invalid input config",
        Rc::clone(display_path),
      ));
    }
  };
  crate::parse::validate_openapi_version(&document, reporter)?;
  crate::parse::validate_generation_policy(&document, reporter)?;
  crate::ir::normalize_api_model(&document, &config.response_type_mapping, reporter)
}

pub fn execute_generate(config: GenerateConfig) -> Result<GenerateResult, GenerateFailure> {
  // Self-test hook for `catch_unwind` at the NAPI boundary. The magic
  // input-path string is opaque enough that no real spec path can hit it;
  // kept in release builds so CI exercises the panic-to-E_UNEXPECTED path.
  if config.input_path.as_deref() == Some("__panic_for_test__") {
    panic!("test sentinel: forced panic");
  }

  // Build display_path: honour an explicitly-supplied value (URL inputs,
  // direct inputContents callers); otherwise derive from input_path with
  // backslash-to-slash normalisation.
  let display_path: Rc<str> = config.display_path.as_deref().map_or_else(
    || {
      config.input_path.as_deref().map_or_else(
        || Rc::from(""),
        |path| {
          Rc::from(
            std::path::Path::new(path)
              .to_string_lossy()
              .replace('\\', "/"),
          )
        },
      )
    },
    Rc::from,
  );

  let mut warnings: Vec<Diagnostic> = Vec::new();

  match run_pipeline(config, Rc::clone(&display_path), &mut warnings) {
    Ok((summary, artifacts)) => Ok(GenerateResult {
      summary,
      diagnostics: warnings,
      artifacts,
    }),
    Err(fatal) => Err(GenerateFailure { warnings, fatal }),
  }
}

fn run_pipeline(
  mut config: GenerateConfig,
  display_path: Rc<str>,
  warnings: &mut Vec<Diagnostic>,
) -> Result<(GenerateSummary, Vec<GeneratedArtifact>), Diagnostic> {
  let mut reporter = Reporter::new(Rc::clone(&display_path), warnings);
  validate_generate_config(&mut config, &mut reporter)?;
  let ir = build_ir(&config, &display_path, &mut reporter)?;
  let summary = GenerateSummary::from_ir(display_path.as_ref().to_string(), &ir);
  let source_path = summary.normalized_source_path.as_str();

  let plan = plan_generation(&config, &ir, &reporter)?;
  // One banner allocation per pipeline run, threaded into every emitter
  // by reference. Bench-large emits 35+ artifacts; this trims one
  // `format!` per artifact (and on petstore-sized inputs the cost is
  // also paid by every consumer test).
  let banner = render_generated_banner(source_path);

  // Canonical emit order: models → angular-rest support → per-tag
  // services. `plan.services` is already class-name-sorted by
  // `resolve_service_plans`, so artifact ordering is independent of
  // operation insertion order.
  let mut artifacts: Vec<GeneratedArtifact> = Vec::new();
  if config.emit.contains(&EmitTarget::Models) && !ir.schemas.is_empty() {
    let body = emit_model(&ir.schemas, &plan.mapped_types);
    artifacts.push(GeneratedArtifact::new(
      MODEL_ARTIFACT_PATH.to_string(),
      format!("{banner}{body}"),
    ));
  }
  if config.emit.contains(&EmitTarget::Angular) {
    artifacts.push(GeneratedArtifact::new(
      REST_MODEL_PATH.to_string(),
      format!("{banner}{REST_MODEL_TEMPLATE}"),
    ));
    artifacts.push(GeneratedArtifact::new(
      REST_UTIL_PATH.to_string(),
      format!("{banner}{REST_UTIL_TEMPLATE}"),
    ));
    artifacts.push(GeneratedArtifact::new(
      REST_VALIDATE_PATH.to_string(),
      format!("{banner}{REST_VALIDATE_TEMPLATE}"),
    ));
    for service in &plan.services {
      let body = emit_service(service);
      artifacts.push(GeneratedArtifact::new(
        service.artifact_path.clone(),
        format!("{banner}{body}"),
      ));
    }
  }

  crate::io::writer::write_generated_artifacts(
    config.output_path.as_deref(),
    &artifacts,
    &reporter,
  )?;

  Ok((summary, artifacts))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use std::{
    fs,
    path::Path,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
  };

  use crate::{
    bindings::EmitTarget,
    error::{Diagnostic, DiagnosticCode},
    options::GenerateConfig,
    parse::input::decode_openapi_input,
    result::{GenerateSummary, GeneratedArtifact},
    test_support::test_ctx,
  };

  use super::{GenerateResult, build_ir, execute_generate};

  // ── build_ir ─────────────────────────────────────────────────────────────

  fn build_ir_config_for_path(path: &str) -> GenerateConfig {
    GenerateConfig {
      input_path: Some(path.to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: None,
      emit: [EmitTarget::Models].into_iter().collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    }
  }

  #[test]
  fn build_ir_runs_input_validation_policy_and_normalize_in_one_pass() {
    let mut ctx = test_ctx();
    let display: Rc<str> = Rc::from("test/fixtures/petstore-minimal.openapi.yaml");
    let config = build_ir_config_for_path("test/fixtures/petstore-minimal.openapi.yaml");
    let ir = build_ir(&config, &display, &mut ctx.reporter()).expect("compiler stages succeed");

    assert_eq!(ir.info.title, "Petstore Minimal");
    assert_eq!(ir.info.spec_version, "3.0.3");
    assert_eq!(ir.schemas.len(), 1);
    assert_eq!(ir.operations.len(), 1);
    assert_eq!(ir.operations[0].operation_id, "listPets");
  }

  #[test]
  fn build_ir_rejects_malformed_operation_at_decode() {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock works")
      .as_nanos();
    let path =
      std::env::temp_dir().join(format!("openapi-ng-invalid-operation-shape-{nanos}.json"));
    fs::write(
      &path,
      serde_json::json!({
        "openapi": "3.0.3",
        "info": { "title": "Invalid Operation Shape", "version": "1.0.0" },
        "paths": {
          "/pets": {
            "get": []
          }
        }
      })
      .to_string(),
    )
    .expect("fixture should be written");

    let mut ctx = test_ctx();
    let path_str = path.to_str().expect("utf-8 path");
    let display: Rc<str> = Rc::from(path_str);
    let config = build_ir_config_for_path(path_str);
    let Err(failure) = build_ir(&config, &display, &mut ctx.reporter()) else {
      panic!("invalid operation shape should fail")
    };

    assert_eq!(failure.code, DiagnosticCode::InputInvalid);

    let _ = fs::remove_file(path);
  }

  #[test]
  fn decode_rejects_malformed_document_structure() {
    let display: Rc<str> = Rc::from("fixture.json");
    let error = decode_openapi_input(
      Path::new("fixture.json"),
      r#"{"openapi":"3.0.3","info":{"title":"Broken","version":"1.0.0"},"paths":{},"components":{"schemas":[]}}"#,
      &display,
    )
    .expect_err("schemas as array should fail at decode");

    assert_eq!(error.code, DiagnosticCode::InputInvalid);
  }

  fn test_summary() -> GenerateSummary {
    GenerateSummary {
      normalized_source_path: "test/fixtures/petstore-minimal.openapi.yaml".to_string(),
      spec_version: "3.0.3".to_string(),
      title: "Petstore Minimal".to_string(),
      path_count: 1,
      operation_count: 1,
      schema_count: 1,
    }
  }

  // ── GenerateResult ───────────────────────────────────────────────────────

  #[test]
  fn generated_artifact_new_preserves_path_and_contents() {
    let artifact = GeneratedArtifact::new(
      "rest/pet.rest.generated.ts".to_string(),
      "zażółć".to_string(),
    );

    assert_eq!(artifact.path, "rest/pet.rest.generated.ts");
    assert_eq!(artifact.contents, "zażółć");
  }

  #[test]
  fn generate_result_success_builds_the_frozen_success_shape() {
    let diagnostic = Diagnostic::new(
      DiagnosticCode::UnsupportedSemantic,
      "Example warning",
      std::rc::Rc::from("spec.yaml"),
    );
    let artifact = GeneratedArtifact::new(
      "model.generated.ts".to_string(),
      "export interface Pet {}\n".to_string(),
    );

    let result = GenerateResult {
      summary: test_summary(),
      diagnostics: vec![diagnostic.clone()],
      artifacts: vec![artifact.clone()],
    };

    assert_eq!(result.summary, test_summary());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, diagnostic.code);
    assert_eq!(result.diagnostics[0].message, diagnostic.message);
    assert_eq!(result.artifacts, vec![artifact]);
  }

  // ── execute_generate ─────────────────────────────────────────────────────

  #[test]
  fn execute_generate_emits_typescript_and_angular_artifacts_in_canonical_order() {
    let result = execute_generate(GenerateConfig {
      input_path: Some("test/fixtures/petstore-rich.openapi.yaml".to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: None,
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    })
    .expect("generation succeeds");

    assert_eq!(result.summary.title, "Petstore Rich");
    assert_eq!(
      result
        .artifacts
        .iter()
        .map(|artifact| artifact.path.as_str())
        .collect::<Vec<_>>(),
      vec![
        "model.generated.ts",
        "rest.model.ts",
        "rest.util.ts",
        "rest.validate.ts",
        "rest/pet.rest.generated.ts",
      ]
    );
  }

  #[test]
  fn execute_generate_dispatches_support_artifact_template() {
    let result = execute_generate(GenerateConfig {
      input_path: Some("test/fixtures/petstore-rich.openapi.yaml".to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: None,
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    })
    .expect("generation succeeds");

    let util_artifact = result
      .artifacts
      .iter()
      .find(|a| a.path == "rest.util.ts")
      .expect("rest.util.ts present");
    assert_eq!(util_artifact.path, "rest.util.ts");
    assert!(
      util_artifact
        .contents
        .contains("export const requestFactory")
    );
  }

  #[test]
  fn execute_generate_inlines_error_interface_into_service_file_when_operation_has_errors() {
    let result = execute_generate(GenerateConfig {
      input_path: Some("test/fixtures/errors-typed.openapi.yaml".to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: None,
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    })
    .expect("generation succeeds");

    // The artifact list has no `errors.generated.ts` — error interfaces
    // live alongside `*Params` inside the per-tag service file.
    assert!(
      !result
        .artifacts
        .iter()
        .any(|a| a.path == "errors.generated.ts"),
      "errors.generated.ts must not be emitted as a standalone artifact",
    );

    let service = result
      .artifacts
      .iter()
      .find(|a| a.path == "rest/pet.rest.generated.ts")
      .expect("pet service emitted");

    // Per-status pairs render verbatim; numeric keys; refs to model types
    // resolve through the existing model import (no extra import block).
    assert!(service.contents.contains("export interface UpdatePetError"));
    assert!(service.contents.contains("400: ValidationProblem;"));
    assert!(service.contents.contains("404: NotFound;"));
    assert!(service.contents.contains("500: {"));
    assert!(service.contents.contains("traceId: string;"));
    // 503 declared no JSON content — silently skipped.
    assert!(!service.contents.contains("503:"));
    // `default` key intentionally not surfaced.
    assert!(!service.contents.contains("default:"));
    // The same model import that already serves `*Params` also covers
    // the error body refs. The nested `body: UpdatePetRequest` field
    // contributes that ref, so the deduplicated, alphabetised import
    // line carries it alongside the response type (`Pet`) and the
    // error-body refs.
    assert!(
      service
        .contents
        .contains("import type { NotFound, Pet, UpdatePetRequest, ValidationProblem }"),
    );
  }

  #[test]
  fn execute_generate_runs_pipeline_with_input_contents_and_explicit_display_path() {
    let yaml = "openapi: 3.0.3\n\
                info: { title: Inline Test, version: 1.0.0 }\n\
                paths: {}\n";
    let config = GenerateConfig {
      input_path: None,
      input_contents: Some(yaml.to_string()),
      display_path: Some("https://example.com/spec.yaml".to_string()),
      input_format: Some(crate::bindings::InputFormat::Yaml),
      output_path: None,
      emit: [crate::bindings::EmitTarget::Models].into_iter().collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    };
    let result = execute_generate(config).expect("inputContents pipeline must succeed");
    assert_eq!(result.summary.title, "Inline Test");
    // display_path is the supplied URL verbatim — no slash-normalisation,
    // no path resolution.
    assert_eq!(
      result.summary.normalized_source_path,
      "https://example.com/spec.yaml",
    );
  }

  #[test]
  fn execute_generate_executes_generation_through_application_boundary() {
    let result = execute_generate(GenerateConfig {
      input_path: Some("test/fixtures/petstore-minimal.openapi.yaml".to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: None,
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    })
    .expect("generation succeeds");

    assert_eq!(result.summary.title, "Petstore Minimal");
    assert_eq!(
      result
        .artifacts
        .iter()
        .map(|artifact| artifact.path.as_str())
        .collect::<Vec<_>>(),
      vec![
        "model.generated.ts",
        "rest.model.ts",
        "rest.util.ts",
        "rest.validate.ts",
        "rest/pet.rest.generated.ts",
      ]
    );
  }
}
