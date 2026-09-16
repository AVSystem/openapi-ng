use std::collections::BTreeSet;

use napi_derive::napi;

use crate::bindings::{EmitTarget, InputFormat, Layout, NamingOptions};

/// Replaces the generated declaration for `schema` with `type_name`, imported from `import`.
#[napi(object)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappedType {
  pub schema: String,
  pub import: String,
  #[napi(js_name = "type")]
  pub type_name: String,
  pub alias: Option<String>,
}

/// Overrides the decoded response kind for one content type, matched case-insensitively against
/// the media type the spec declares.
#[napi(object)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponseTypeMapping {
  pub content_type: String,
  pub response_type: ResponseType,
}

/// How a response body is decoded, named as the JS runtime names it.
#[napi(string_enum = "camelCase")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseType {
  Json,
  Blob,
  Text,
  ArrayBuffer,
}

/// A generation request after validation.
#[derive(Clone, Debug)]
pub struct GenerateConfig {
  /// Set when the caller passed `input_path`; mutually exclusive with `input_contents`
  /// (validated in `resolve_generate_config`).
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
  pub layout: BTreeSet<Layout>,
}

#[must_use]
pub(crate) fn default_layout() -> BTreeSet<Layout> {
  BTreeSet::from([Layout::Services])
}

mod validate;

pub(crate) use validate::resolve_generate_config;

#[cfg(test)]
mod tests {
  use super::{
    GenerateConfig, MappedType, ResponseType, ResponseTypeMapping, resolve_generate_config,
  };
  use crate::bindings::EmitTarget;
  use crate::test_support::test_reporter;

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
      layout: crate::options::default_layout(),
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
    let ctx = test_reporter();
    let config = GenerateConfig {
      output_path: None,
      emit: [EmitTarget::Models].into_iter().collect(),
      ..config("spec.yaml")
    };
    let config = resolve_generate_config(config, &ctx).expect("generate options should validate");

    assert_eq!(config.input_path.as_deref(), Some("spec.yaml"));
    assert_eq!(config.output_path, None);
    assert!(config.emit.contains(&EmitTarget::Models));
    assert!(!config.emit.contains(&EmitTarget::Angular));
    assert!(config.mapped_types.is_empty());
  }

  #[test]
  fn validator_rejects_empty_string_output_path_as_invalid_option() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      output_path: Some(String::new()),
      ..config("spec.yaml")
    };
    let error = resolve_generate_config(config, &ctx)
      .expect_err("empty outputPath should fail during option validation");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("non-empty path"));
  }

  #[test]
  fn validator_rejects_empty_emit_set() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      emit: std::collections::BTreeSet::new(),
      ..config("spec.yaml")
    };
    let error = resolve_generate_config(config, &ctx)
      .expect_err("empty emit set should fail during option validation");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("emit"));
  }

  #[test]
  fn validator_auto_includes_models_when_angular_is_requested_alone() {
    let path: std::rc::Rc<str> = std::rc::Rc::from("spec.yaml");
    let reporter = crate::error::Reporter::new(path);
    let config = GenerateConfig {
      emit: std::iter::once(EmitTarget::Angular).collect(),
      ..config("spec.yaml")
    };
    let config = resolve_generate_config(config, &reporter)
      .expect("auto-include should be a warning, not a fatal");

    assert!(config.emit.contains(&EmitTarget::Models));
    let warnings = reporter.into_warnings();
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
    let path: std::rc::Rc<str> = std::rc::Rc::from("spec.yaml");
    let reporter = crate::error::Reporter::new(path);
    let config = GenerateConfig {
      emit: [EmitTarget::Models, EmitTarget::Angular]
        .into_iter()
        .collect(),
      ..config("spec.yaml")
    };
    resolve_generate_config(config, &reporter).expect("explicit models silences warning");

    assert!(reporter.into_warnings().is_empty());
  }

  #[test]
  fn validator_rejects_blank_mapped_type_entries_as_invalid_option() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      mapped_types: vec![MappedType {
        schema: "UserId".to_string(),
        import: "   ".to_string(),
        type_name: "ExternalUserId".to_string(),
        alias: None,
      }],
      ..config("spec.yaml")
    };
    let error = resolve_generate_config(config, &ctx)
      .expect_err("blank mapped type fields should fail during option validation");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("schema, import, and type"));
  }

  #[test]
  fn validator_rejects_naming_chain_item_with_both_string_and_rule() {
    let ctx = test_reporter();
    let config = GenerateConfig {
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
    let error = resolve_generate_config(config, &ctx).expect_err("exclusive fields should fail");
    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("exactly one"));
  }

  #[test]
  fn validator_rejects_parse_without_format() {
    let ctx = test_reporter();
    let config = GenerateConfig {
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
    let error =
      resolve_generate_config(config, &ctx).expect_err("parse without format should fail");
    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("`format` is required"));
  }

  #[test]
  fn validator_rejects_both_input_path_and_input_contents_set() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      input_path: Some("spec.yaml".to_string()),
      input_contents: Some("openapi: 3.0.3\n".to_string()),
      display_path: Some("inline".to_string()),
      ..config("spec.yaml")
    };
    let error = resolve_generate_config(config, &ctx)
      .expect_err("input_path + input_contents must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("exactly one"));
    assert!(error.message.contains("inputPath"));
    assert!(error.message.contains("inputContents"));
  }

  #[test]
  fn validator_rejects_neither_input_path_nor_input_contents_set() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      input_path: None,
      input_contents: None,
      ..config("ignored")
    };
    let error =
      resolve_generate_config(config, &ctx).expect_err("missing both inputs must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("exactly one"));
  }

  #[test]
  fn validator_rejects_input_contents_without_display_path() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      input_path: None,
      input_contents: Some("openapi: 3.0.3\n".to_string()),
      display_path: None,
      ..config("ignored")
    };
    let error = resolve_generate_config(config, &ctx)
      .expect_err("inputContents without displayPath must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("displayPath"));
    assert!(error.message.contains("inputContents"));
  }

  #[test]
  fn validator_rejects_input_format_with_input_path() {
    let ctx = test_reporter();
    let config = GenerateConfig {
      input_path: Some("spec.yaml".to_string()),
      input_format: Some(crate::bindings::InputFormat::Json),
      ..config("spec.yaml")
    };
    let error = resolve_generate_config(config, &ctx)
      .expect_err("inputFormat with inputPath must be rejected");

    assert_eq!(error.code, crate::error::DiagnosticCode::InvalidOption);
    assert!(error.message.contains("inputFormat"));
    assert!(error.message.contains("inputContents"));
  }

  #[test]
  fn rejects_empty_content_type_string() {
    let ctx = test_reporter();
    let config = config_with_mappings(vec![ResponseTypeMapping {
      content_type: "".into(),
      response_type: ResponseType::Blob,
    }]);
    let err = resolve_generate_config(config, &ctx).expect_err("empty contentType should fail");
    assert_eq!(err.code, crate::error::DiagnosticCode::InvalidOption);
  }

  #[test]
  fn rejects_duplicate_content_type_after_lowercase_normalisation() {
    let ctx = test_reporter();
    let config = config_with_mappings(vec![
      ResponseTypeMapping {
        content_type: "application/PDF".into(),
        response_type: ResponseType::Blob,
      },
      ResponseTypeMapping {
        content_type: "application/pdf".into(),
        response_type: ResponseType::ArrayBuffer,
      },
    ]);
    let err = resolve_generate_config(config, &ctx).expect_err("duplicate contentType should fail");
    assert!(err.message.contains("application/pdf"));
  }

  #[test]
  fn rejects_content_type_without_slash() {
    let ctx = test_reporter();
    let config = config_with_mappings(vec![ResponseTypeMapping {
      content_type: "notamediatype".into(),
      response_type: ResponseType::Blob,
    }]);
    let err =
      resolve_generate_config(config, &ctx).expect_err("contentType without '/' should fail");
    assert!(err.message.contains("must contain"));
  }

  #[test]
  fn accepts_well_formed_response_type_mapping() {
    let ctx = test_reporter();
    let config = config_with_mappings(vec![
      ResponseTypeMapping {
        content_type: "application/pdf".into(),
        response_type: ResponseType::Blob,
      },
      ResponseTypeMapping {
        content_type: "text/csv".into(),
        response_type: ResponseType::Text,
      },
    ]);
    resolve_generate_config(config, &ctx).expect("well-formed mapping passes");
  }
}
