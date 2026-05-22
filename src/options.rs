use std::collections::BTreeSet;

use napi_derive::napi;

use crate::{
  bindings::{EmitTarget, InputFormat, NamingOptions},
  error::{Diagnostic, DiagnosticCode, Reporter},
};

/// Canonical mapped-type record. Used as user input (from CLI/JS options)
/// and as the planning record (after schema-name validation).
///
/// Field names match the CLI YAML config vocabulary (schema/import/type/
/// alias). `ty` is the Rust-side name; the NAPI surface renames it to
/// `type` so the JS API stays idiomatic.
#[napi(object)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappedType {
  pub schema: String,
  pub import: String,
  #[napi(js_name = "type")]
  pub ty: String,
  pub alias: Option<String>,
}

/// User mapping: override the response-kind decoded for a specific
/// response content-type. Pure data — Phase-3 normalize-side reads
/// this when picking the `responseKind` for an operation's response
/// content. Keys are matched case-insensitively against the lowercased
/// media-type from the spec; the `responseType` is one of the JS-facing
/// HttpClient response kinds (`'json' | 'blob' | 'text' | 'arrayBuffer'`).
#[napi(object)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseTypeMapping {
  pub content_type: String,
  pub response_type: ResponseType,
}

/// JS-facing response-kind values. Mirrors the names Angular's
/// `HttpClient.request({ responseType })` and `httpResource.<kind>()`
/// expose, so the config vocabulary stays in JS conventions. The emit
/// boundary translates `ArrayBuffer` to the lowercase `'arraybuffer'`
/// string `HttpClient.request` requires.
#[napi(string_enum = "camelCase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseType {
  Json,
  Blob,
  Text,
  ArrayBuffer,
}

/// Resolved generation config. The `emit` set replaces three booleans:
/// callers (and the validator/pipeline) read membership with
/// `emit.contains(&EmitTarget::Models)`.
#[derive(Clone, Debug)]
pub struct GenerateConfig {
  /// Set when the caller passed `input_path`; mutually exclusive with
  /// `input_contents` (validated in `validate_generate_config`).
  pub input_path: Option<String>,
  pub input_contents: Option<String>,
  pub display_path: Option<String>,
  pub input_format: Option<InputFormat>,
  pub output_path: Option<String>,
  pub emit: BTreeSet<EmitTarget>,
  pub mapped_types: Vec<MappedType>,
  pub response_type_mapping: Vec<ResponseTypeMapping>,
  pub naming_options: Option<NamingOptions>,
  pub naming: crate::plan::naming::NamingConfig,
}

pub(crate) fn validate_generate_config(
  config: &mut GenerateConfig,
  reporter: &mut Reporter<'_>,
) -> Result<(), Diagnostic> {
  // Exactly one of inputPath / inputContents must be set.
  match (config.input_path.is_some(), config.input_contents.is_some()) {
    (true, true) | (false, false) => {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        "Must set exactly one of inputPath or inputContents.",
      ));
    }
    _ => {}
  }
  if config.input_contents.is_some() && config.display_path.is_none() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "displayPath is required when inputContents is set.",
    ));
  }
  if config.input_format.is_some() && config.input_path.is_some() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "inputFormat is only honoured with inputContents; \
       remove it or switch to inputContents.",
    ));
  }

  validate_emit_targets(&mut config.emit, reporter)?;
  validate_mapped_types(&config.mapped_types, reporter)?;
  validate_response_type_mapping(&config.response_type_mapping, reporter)?;
  config.naming = resolve_naming_options(config.naming_options.take(), reporter)?;

  // `output_path` is either omitted (in-memory only) or a real path. An empty
  // string is never a valid path — reject it outright instead of silently
  // coercing to in-memory.
  if matches!(config.output_path.as_deref(), Some("")) {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "outputPath must be a non-empty path. Omit the field (or pass undefined) to generate in-memory.",
    ));
  }
  Ok(())
}

fn validate_emit_targets(
  emit: &mut BTreeSet<EmitTarget>,
  reporter: &mut Reporter<'_>,
) -> Result<(), Diagnostic> {
  if emit.is_empty() {
    return Err(reporter.error(
      DiagnosticCode::InvalidOption,
      "emit must include at least one target ('models' or 'angular').",
    ));
  }
  // Angular services reference the generated model types. Auto-include
  // `models` and warn rather than rejecting the caller's emit set.
  if emit.contains(&EmitTarget::Angular) && !emit.contains(&EmitTarget::Models) {
    emit.insert(EmitTarget::Models);
    reporter.warning(
      DiagnosticCode::InvalidOption,
      None,
      "Auto-included 'models' in emit because 'angular' depends on it. Add 'models' to emit to silence this warning.",
    );
  }
  Ok(())
}

fn validate_mapped_types(
  mapped_types: &[MappedType],
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  let mut seen = std::collections::BTreeSet::<&str>::new();
  for mapped_type in mapped_types {
    if mapped_type.schema.trim().is_empty()
      || mapped_type.import.trim().is_empty()
      || mapped_type.ty.trim().is_empty()
    {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        "Failed to resolve generation options: mapped type entries require schema, import, and type.",
      ));
    }

    if !is_valid_ts_identifier(&mapped_type.ty) {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!(
          "Failed to resolve generation options: mapped type type '{}' is not a valid TypeScript identifier (expected /^[A-Za-z_$][A-Za-z0-9_$]*$/).",
          mapped_type.ty,
        ),
      ));
    }

    if let Some(alias) = mapped_type.alias.as_deref()
      && !is_valid_ts_identifier(alias)
    {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!(
          "Failed to resolve generation options: mapped type alias '{alias}' is not a valid TypeScript identifier."
        ),
      ));
    }

    if !seen.insert(mapped_type.schema.as_str()) {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!(
          "Failed to resolve generation options: mapped type schema '{}' is duplicated; each schema must appear at most once.",
          mapped_type.schema,
        ),
      ));
    }
  }

  Ok(())
}

fn validate_response_type_mapping(
  mappings: &[ResponseTypeMapping],
  reporter: &Reporter<'_>,
) -> Result<(), Diagnostic> {
  let mut seen = std::collections::BTreeSet::<String>::new();
  for m in mappings {
    let lc = m.content_type.to_ascii_lowercase();
    if lc.is_empty() {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        "responseTypeMapping.contentType must be non-empty.",
      ));
    }
    if !lc.contains('/') {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!("responseTypeMapping.contentType {lc:?} must contain '/'."),
      ));
    }
    if !seen.insert(lc.clone()) {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!("responseTypeMapping has duplicate contentType {lc:?} (case-insensitive)."),
      ));
    }
  }
  Ok(())
}

fn is_valid_ts_identifier(value: &str) -> bool {
  let mut chars = value.chars();
  let Some(first) = chars.next() else {
    return false;
  };
  if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
    return false;
  }
  chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

pub(crate) fn resolve_naming_options(
  options: Option<NamingOptions>,
  reporter: &Reporter<'_>,
) -> Result<crate::plan::naming::NamingConfig, Diagnostic> {
  use crate::plan::naming::{Case, Naming, NamingConfig, Rule, RuleEntry, compile_parse_spec};

  let Some(opts) = options else {
    return Ok(NamingConfig::default());
  };

  fn lower_entry(
    string: Option<String>,
    rule: Option<crate::bindings::NamingRuleEntry>,
    reporter: &Reporter<'_>,
    path: &str,
  ) -> Result<RuleEntry, Diagnostic> {
    match (string, rule) {
      (Some(s), None) => Ok(RuleEntry::Shorthand(s)),
      (None, Some(r)) => {
        let case = match r.case_.as_deref() {
          None => None,
          Some(s) => Some(Case::parse(s).ok_or_else(|| {
            reporter.error(
              DiagnosticCode::InvalidOption,
              format!(
                "naming.{path}.case: '{s}' is not one of 'camel', 'pascal', 'snake', 'kebab', 'constant'.",
              ),
            )
          })?),
        };
        let parse = r
          .parse
          .map(|spec| {
            compile_parse_spec(&spec.source, &spec.flags).map_err(|err| {
              reporter.error(
                DiagnosticCode::InvalidOption,
                format!(
                  "naming.{path}.parse: failed to compile regex `{}` (flags=`{}`): {:?}",
                  spec.source, spec.flags, err,
                ),
              )
            })
          })
          .transpose()?;
        if parse.is_some() && r.format.is_none() {
          return Err(reporter.error(
            DiagnosticCode::InvalidOption,
            format!("naming.{path}: when `parse` is present, `format` is required."),
          ));
        }
        Ok(RuleEntry::Rule(Rule {
          from: r.from,
          parse,
          format: r.format,
          case,
        }))
      }
      (Some(_), Some(_)) => Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!("naming.{path}: a chain item cannot set both `string` and `rule`."),
      )),
      (None, None) => Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!("naming.{path}: a chain item must set exactly one of `string` or `rule`."),
      )),
    }
  }

  fn lower_value(
    value: Option<crate::bindings::NamingValue>,
    reporter: &Reporter<'_>,
    key: &str,
  ) -> Result<Option<Naming>, Diagnostic> {
    let Some(v) = value else {
      return Ok(None);
    };
    let count =
      u8::from(v.string.is_some()) + u8::from(v.rule.is_some()) + u8::from(v.chain.is_some());
    if count != 1 {
      return Err(reporter.error(
        DiagnosticCode::InvalidOption,
        format!(
          "naming.{key}: must set exactly one of `string`, `rule`, or `chain` (got {count})."
        ),
      ));
    }
    if let Some(s) = v.string {
      return Ok(Some(Naming::Single(RuleEntry::Shorthand(s))));
    }
    if let Some(r) = v.rule {
      let entry = lower_entry(None, Some(r), reporter, key)?;
      return Ok(Some(Naming::Single(entry)));
    }
    let Some(items) = v.chain else {
      unreachable!("count==1 guarantees v.chain is Some when string/rule are None")
    };
    let mut entries = Vec::with_capacity(items.len());
    for (i, item) in items.into_iter().enumerate() {
      let path = format!("{key}[{i}]");
      entries.push(lower_entry(item.string, item.rule, reporter, &path)?);
    }
    Ok(Some(Naming::Chain(entries)))
  }

  Ok(NamingConfig {
    method_name: lower_value(opts.method_name, reporter, "methodName")?,
    group: lower_value(opts.group, reporter, "group")?,
  })
}

#[cfg(test)]
mod tests {
  use super::{
    GenerateConfig, MappedType, ResponseType, ResponseTypeMapping, validate_generate_config,
  };
  use crate::bindings::EmitTarget;
  use crate::test_support::test_ctx;

  fn config(input_path: &str) -> GenerateConfig {
    GenerateConfig {
      input_path: Some(input_path.to_string()),
      input_contents: None,
      display_path: None,
      input_format: None,
      output_path: Some("out".to_string()),
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      mapped_types: Vec::new(),
      response_type_mapping: Vec::new(),
      naming_options: None,
      naming: crate::plan::naming::NamingConfig::default(),
    }
  }

  fn config_with_mappings(mappings: Vec<ResponseTypeMapping>) -> GenerateConfig {
    GenerateConfig {
      response_type_mapping: mappings,
      ..config("spec.yaml")
    }
  }

  #[test]
  fn validator_accepts_in_memory_default_when_output_path_is_omitted() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      output_path: None,
      emit: [EmitTarget::Models].into_iter().collect(),
      ..config("spec.yaml")
    };
    validate_generate_config(&mut config, &mut ctx.reporter())
      .expect("generate options should validate");

    assert_eq!(config.input_path.as_deref(), Some("spec.yaml"));
    assert_eq!(config.output_path, None);
    assert!(config.emit.contains(&EmitTarget::Models));
    assert!(!config.emit.contains(&EmitTarget::Angular));
    assert!(config.mapped_types.is_empty());
  }

  #[test]
  fn validator_rejects_empty_string_output_path_as_invalid_option() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      output_path: Some(String::new()),
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("empty outputPath should fail during option validation");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("non-empty path"));
  }

  #[test]
  fn validator_rejects_empty_emit_set() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      emit: std::collections::BTreeSet::new(),
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("empty emit set should fail during option validation");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("emit"));
  }

  #[test]
  fn validator_auto_includes_models_when_angular_is_requested_alone() {
    let mut warnings = Vec::new();
    let path: std::rc::Rc<str> = std::rc::Rc::from("spec.yaml");
    let mut reporter = crate::error::Reporter::new(path, &mut warnings);
    let mut config = GenerateConfig {
      emit: std::iter::once(EmitTarget::Angular).collect(),
      ..config("spec.yaml")
    };
    validate_generate_config(&mut config, &mut reporter)
      .expect("auto-include should be a warning, not a fatal");

    assert!(config.emit.contains(&EmitTarget::Models));
    assert_eq!(warnings.len(), 1);
    assert_eq!(
      warnings[0].code,
      crate::error::DiagnosticCode::InvalidOption
    );
    assert!(warnings[0].message.contains("Auto-included 'models'"));
    assert!(warnings[0].message.contains("'angular'"));
  }

  #[test]
  fn validator_emits_no_warning_when_models_already_present() {
    let mut warnings = Vec::new();
    let path: std::rc::Rc<str> = std::rc::Rc::from("spec.yaml");
    let mut reporter = crate::error::Reporter::new(path, &mut warnings);
    let mut config = GenerateConfig {
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      ..config("spec.yaml")
    };
    validate_generate_config(&mut config, &mut reporter).expect("explicit models silences warning");

    assert!(warnings.is_empty());
  }

  #[test]
  fn validator_rejects_blank_mapped_type_entries_as_invalid_option() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      mapped_types: vec![MappedType {
        schema: "UserId".to_string(),
        import: "   ".to_string(),
        ty: "ExternalUserId".to_string(),
        alias: None,
      }],
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("blank mapped type fields should fail during option validation");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("schema, import, and type"));
  }

  #[test]
  fn validator_rejects_naming_chain_item_with_both_string_and_rule() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      naming_options: Some(crate::bindings::NamingOptions {
        method_name: Some(crate::bindings::NamingValue {
          string: Some("x".to_string()),
          rule: Some(crate::bindings::NamingRuleEntry {
            from: None,
            parse: None,
            format: Some("y".to_string()),
            case_: None,
          }),
          chain: None,
        }),
        group: None,
      }),
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("exclusive fields should fail");
    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("exactly one"));
  }

  #[test]
  fn validator_rejects_parse_without_format() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      naming_options: Some(crate::bindings::NamingOptions {
        method_name: Some(crate::bindings::NamingValue {
          string: None,
          rule: Some(crate::bindings::NamingRuleEntry {
            from: Some("{operationId}".to_string()),
            parse: Some(crate::bindings::NamingParseSpec {
              source: "^(?<x>.+)$".to_string(),
              flags: String::new(),
            }),
            format: None,
            case_: None,
          }),
          chain: None,
        }),
        group: None,
      }),
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("parse without format should fail");
    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("`format` is required"));
  }

  #[test]
  fn validator_rejects_both_input_path_and_input_contents_set() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      input_path: Some("spec.yaml".to_string()),
      input_contents: Some("openapi: 3.0.3\n".to_string()),
      display_path: Some("inline".to_string()),
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("input_path + input_contents must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("exactly one"));
    assert!(error.message.contains("inputPath"));
    assert!(error.message.contains("inputContents"));
  }

  #[test]
  fn validator_rejects_neither_input_path_nor_input_contents_set() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      input_path: None,
      input_contents: None,
      ..config("ignored")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("missing both inputs must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("exactly one"));
  }

  #[test]
  fn validator_rejects_input_contents_without_display_path() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      input_path: None,
      input_contents: Some("openapi: 3.0.3\n".to_string()),
      display_path: None,
      ..config("ignored")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("inputContents without displayPath must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("displayPath"));
    assert!(error.message.contains("inputContents"));
  }

  #[test]
  fn validator_rejects_input_format_with_input_path() {
    let mut ctx = test_ctx();
    let mut config = GenerateConfig {
      input_path: Some("spec.yaml".to_string()),
      input_format: Some(crate::bindings::InputFormat::Json),
      ..config("spec.yaml")
    };
    let error = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("inputFormat with inputPath must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("inputFormat"));
    assert!(error.message.contains("inputContents"));
  }

  #[test]
  fn rejects_empty_content_type_string() {
    let mut ctx = test_ctx();
    let mut config = config_with_mappings(vec![ResponseTypeMapping {
      content_type: "".into(),
      response_type: ResponseType::Blob,
    }]);
    let err = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("empty contentType should fail");
    assert_eq!(err.code, crate::error::DiagnosticCode::InvalidOption);
  }

  #[test]
  fn rejects_duplicate_content_type_after_lowercase_normalisation() {
    let mut ctx = test_ctx();
    let mut config = config_with_mappings(vec![
      ResponseTypeMapping {
        content_type: "application/PDF".into(),
        response_type: ResponseType::Blob,
      },
      ResponseTypeMapping {
        content_type: "application/pdf".into(),
        response_type: ResponseType::ArrayBuffer,
      },
    ]);
    let err = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("duplicate contentType should fail");
    assert!(err.message.contains("application/pdf"));
  }

  #[test]
  fn rejects_content_type_without_slash() {
    let mut ctx = test_ctx();
    let mut config = config_with_mappings(vec![ResponseTypeMapping {
      content_type: "notamediatype".into(),
      response_type: ResponseType::Blob,
    }]);
    let err = validate_generate_config(&mut config, &mut ctx.reporter())
      .expect_err("contentType without '/' should fail");
    assert!(err.message.contains("must contain"));
  }

  #[test]
  fn accepts_well_formed_response_type_mapping() {
    let mut ctx = test_ctx();
    let mut config = config_with_mappings(vec![
      ResponseTypeMapping {
        content_type: "application/pdf".into(),
        response_type: ResponseType::Blob,
      },
      ResponseTypeMapping {
        content_type: "text/csv".into(),
        response_type: ResponseType::Text,
      },
    ]);
    validate_generate_config(&mut config, &mut ctx.reporter()).expect("well-formed mapping passes");
  }
}
