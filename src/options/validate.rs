//! Every caller-supplied option, checked before the pipeline reads it.

use std::collections::BTreeSet;

use crate::{
  bindings::{EmitTarget, Layout},
  error::{Diagnostic, DiagnosticCode, Reporter, bail},
  identifier::is_identifier,
};

use super::{GenerateConfig, MappedType, ResponseTypeMapping};

/// The caller's options, validated and resolved: `emit` gains the targets it
/// implies, `naming` is lowered.
pub(crate) fn resolve_generate_config(
  mut config: GenerateConfig,
  reporter: &Reporter,
) -> Result<GenerateConfig, Diagnostic> {
  CHECKS
    .iter()
    .try_for_each(|check| check(&config, reporter))?;

  config.emit = resolve_emit_targets(config.emit, reporter)?;
  config.naming = crate::plan::naming::lower(config.naming_options.take(), reporter)?;
  Ok(config)
}

/// Every check that reads the config without changing it.
type Check = fn(&GenerateConfig, &Reporter) -> Result<(), Diagnostic>;
const CHECKS: [Check; 6] = [
  check_input_source,
  check_display_path,
  check_input_format,
  check_output_path,
  validate_layout,
  check_mapped_types,
];

fn check_input_source(config: &GenerateConfig, reporter: &Reporter) -> Result<(), Diagnostic> {
  if config.input_path.is_some() == config.input_contents.is_some() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "Must set exactly one of inputPath or inputContents.",
    ));
  }
  Ok(())
}

fn check_display_path(config: &GenerateConfig, reporter: &Reporter) -> Result<(), Diagnostic> {
  if config.input_contents.is_some() && config.display_path.is_none() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "displayPath is required when inputContents is set.",
    ));
  }
  Ok(())
}

fn check_input_format(config: &GenerateConfig, reporter: &Reporter) -> Result<(), Diagnostic> {
  if config.input_format.is_some() && config.input_path.is_some() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "inputFormat is only honoured with inputContents; \
       remove it or switch to inputContents.",
    ));
  }
  Ok(())
}

/// Omitted means in-memory; an empty string is neither.
fn check_output_path(config: &GenerateConfig, reporter: &Reporter) -> Result<(), Diagnostic> {
  if matches!(config.output_path.as_deref(), Some("")) {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "outputPath must be a non-empty path. Omit the field (or pass undefined) to generate in-memory.",
    ));
  }
  Ok(())
}

fn check_mapped_types(config: &GenerateConfig, reporter: &Reporter) -> Result<(), Diagnostic> {
  validate_mapped_types(&config.mapped_types, reporter)?;
  validate_response_type_mapping(&config.response_type_mapping, reporter)
}

/// Adds the targets `emit` implies, warning once when it does.
fn resolve_emit_targets(
  mut emit: BTreeSet<EmitTarget>,
  reporter: &Reporter,
) -> Result<BTreeSet<EmitTarget>, Diagnostic> {
  if emit.is_empty() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "emit must include at least one target ('models' or 'angular').",
    ));
  }
  // Angular services import the generated model types.
  if emit.contains(&EmitTarget::Angular) && !emit.contains(&EmitTarget::Models) {
    emit.insert(EmitTarget::Models);
    reporter.warning(
      DiagnosticCode::InvalidOption,
      None,
      "Auto-included 'models' in emit because 'angular' depends on it. Add 'models' to emit to silence this warning.",
    );
  }
  Ok(emit)
}

/// The first key that repeats, in iteration order.
#[must_use]
fn first_duplicate<K: Ord + Clone>(keys: impl IntoIterator<Item = K>) -> Option<K> {
  let mut seen = std::collections::BTreeSet::new();
  keys.into_iter().find(|key| !seen.insert(key.clone()))
}

/// Only the angular emitter reads `layout`.
fn validate_layout(config: &GenerateConfig, reporter: &Reporter) -> Result<(), Diagnostic> {
  if config.layout.is_empty() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "layout must include at least one entry ('services' or 'operations').",
    ));
  }
  if config.layout.contains(&Layout::Operations) && !config.emit.contains(&EmitTarget::Angular) {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "layout 'operations' requires the 'angular' emit target.",
    ));
  }
  Ok(())
}

fn validate_mapped_types(
  mapped_types: &[MappedType],
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  mapped_types
    .iter()
    .try_for_each(|mapped_type| validate_mapped_type(mapped_type, reporter))?;

  if let Some(schema) = first_duplicate(mapped_types.iter().map(|entry| entry.schema.as_str())) {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "Failed to resolve generation options: mapped type schema '{schema}' is duplicated; each schema must appear at most once.",
    );
  }

  Ok(())
}

fn validate_mapped_type(mapped_type: &MappedType, reporter: &Reporter) -> Result<(), Diagnostic> {
  if mapped_type.schema.trim().is_empty()
    || mapped_type.import.trim().is_empty()
    || mapped_type.type_name.trim().is_empty()
  {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "Failed to resolve generation options: mapped type entries require schema, import, and type.",
    ));
  }

  if !is_identifier(&mapped_type.type_name) {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "Failed to resolve generation options: mapped type type '{}' is not a valid TypeScript identifier (expected /^[A-Za-z_$][A-Za-z0-9_$]*$/).",
      mapped_type.type_name,
    );
  }

  if let Some(alias) = mapped_type.alias.as_deref()
    && !is_identifier(alias)
  {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "Failed to resolve generation options: mapped type alias '{alias}' is not a valid TypeScript identifier."
    );
  }

  Ok(())
}

fn validate_response_type_mapping(
  mappings: &[ResponseTypeMapping],
  reporter: &Reporter,
) -> Result<(), Diagnostic> {
  let content_types: Vec<String> = mappings
    .iter()
    .map(|mapping| mapping.content_type.to_ascii_lowercase())
    .collect();

  content_types
    .iter()
    .try_for_each(|content_type| validate_content_type(content_type, reporter))?;

  if let Some(duplicate) = first_duplicate(content_types) {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "responseTypeMapping has duplicate contentType {duplicate:?} (case-insensitive)."
    );
  }
  Ok(())
}

fn validate_content_type(content_type: &str, reporter: &Reporter) -> Result<(), Diagnostic> {
  if content_type.is_empty() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "responseTypeMapping.contentType must be non-empty.",
    ));
  }
  if !content_type.contains('/') {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "responseTypeMapping.contentType {content_type:?} must contain '/'."
    );
  }
  Ok(())
}
