use std::fs;

use crate::{
  error::{Diagnostic, DiagnosticCode, Reporter, bail},
  io::host_cwd::resolve_against_host_cwd,
  result::GeneratedArtifact,
};

pub(crate) fn write_generated_artifacts(
  output_path: Option<&str>,
  artifacts: &[GeneratedArtifact],
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  let Some(output_path) = output_path else {
    return Ok(());
  };

  for artifact in artifacts {
    write_artifact(output_path, artifact, reporter)?;
  }

  Ok(())
}

fn write_artifact(
  output_path: &str,
  artifact: &GeneratedArtifact,
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  let artifact_rel = std::path::Path::new(&artifact.path);
  if artifact_rel
    .components()
    .any(|component| matches!(component, std::path::Component::ParentDir))
  {
    bail!(
      reporter,
      DiagnosticCode::WriteFailed,
      "Failed to write artifact: artifact path '{}' contains parent traversal ('..').",
      artifact.path
    );
  }

  let output_dir = resolve_against_host_cwd(std::path::Path::new(output_path));
  fs::create_dir_all(&output_dir).map_err(|error| {
    reporter.error(
      DiagnosticCode::WriteFailed,
      format!("Failed to create generator output directory: {error}"),
    )
  })?;

  let artifact_path = output_dir.join(&artifact.path);
  if let Some(parent) = artifact_path.parent() {
    fs::create_dir_all(parent).map_err(|error| {
      reporter.error(
        DiagnosticCode::WriteFailed,
        format!(
          "Failed to create generated artifact parent directory for {}: {error}",
          artifact.path
        ),
      )
    })?;
  }

  fs::write(&artifact_path, &artifact.contents).map_err(|error| {
    reporter.error(
      DiagnosticCode::WriteFailed,
      format!(
        "Failed to write generated artifact {}: {error}",
        artifact.path
      ),
    )
  })?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
  };

  use crate::{result::GeneratedArtifact, test_support::test_reporter};

  fn unique_path(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("clock works")
      .as_nanos();
    std::env::temp_dir().join(format!("openapi-ng-{label}-{nanos}"))
  }

  fn artifact(path: &str, contents: &str) -> GeneratedArtifact {
    GeneratedArtifact {
      path: path.to_string(),
      contents: contents.to_string(),
    }
  }

  #[test]
  fn write_generated_artifacts_writes_nested_artifacts_into_output_directory() {
    let output_path = unique_path("artifact-writer-success");
    let ctx = test_reporter();
    let artifacts = vec![
      artifact("model.generated.ts", "export interface Pet {}\n"),
      artifact("rest/pet.rest.generated.ts", "export class PetService {}\n"),
    ];

    super::write_generated_artifacts(
      Some(output_path.to_str().expect("output path should be utf-8")),
      &artifacts,
      &ctx,
    )
    .expect("writer succeeds");

    assert_eq!(
      fs::read_to_string(output_path.join("model.generated.ts"))
        .expect("model artifact should exist"),
      "export interface Pet {}\n"
    );
    assert_eq!(
      fs::read_to_string(output_path.join("rest/pet.rest.generated.ts"))
        .expect("service artifact should exist"),
      "export class PetService {}\n"
    );

    let _ = fs::remove_dir_all(output_path);
  }

  #[test]
  fn write_generated_artifacts_preserves_write_output_failure_contract() {
    let blocked_output_path = unique_path("artifact-writer-failure");
    fs::create_dir_all(&blocked_output_path).expect("create output directory");
    fs::write(blocked_output_path.join("rest"), "not-a-directory")
      .expect("create blocking parent file");

    let ctx = test_reporter();
    let failure = super::write_generated_artifacts(
      Some(
        blocked_output_path
          .to_str()
          .expect("blocked output path should be utf-8"),
      ),
      &[artifact(
        "rest/pet.rest.generated.ts",
        "export class PetService {}\n",
      )],
      &ctx,
    )
    .expect_err("writer should fail when parent directory cannot be created");

    assert_eq!(failure.code, crate::error::DiagnosticCode::WriteFailed);
    assert!(failure.message.contains("rest/pet.rest.generated.ts"));

    let _ = fs::remove_dir_all(blocked_output_path);
  }

  #[test]
  fn write_generated_artifacts_overwrites_existing_artifact() {
    let output_path = unique_path("artifact-writer-overwrite");
    fs::create_dir_all(&output_path).expect("create output directory");
    fs::write(output_path.join("a.ts"), "stale content").expect("write stale file");

    let ctx = test_reporter();
    let artifacts = vec![artifact("a.ts", "fresh content")];

    super::write_generated_artifacts(
      Some(output_path.to_str().expect("output path should be utf-8")),
      &artifacts,
      &ctx,
    )
    .expect("overwrite should succeed");

    let content =
      fs::read_to_string(output_path.join("a.ts")).expect("artifact should exist after overwrite");
    assert_eq!(content, "fresh content");

    let _ = fs::remove_dir_all(output_path);
  }

  #[test]
  fn write_generated_artifacts_rejects_artifact_path_with_parent_traversal() {
    let output_path = unique_path("artifact-writer-traversal");
    let ctx = test_reporter();
    let artifacts = vec![artifact("../escape.ts", "x")];

    let err = super::write_generated_artifacts(
      Some(output_path.to_str().expect("output path should be utf-8")),
      &artifacts,
      &ctx,
    )
    .expect_err("should reject artifact path containing '..'");

    let _ = fs::remove_dir_all(output_path);

    assert_eq!(err.code, crate::error::DiagnosticCode::WriteFailed);
    let msg = err.message.to_lowercase();
    assert!(
      msg.contains("..") || msg.contains("parent") || msg.contains("traversal"),
      "unexpected diagnostic message: {}",
      err.message,
    );
  }
}
