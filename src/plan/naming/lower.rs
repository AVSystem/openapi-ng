//! Lowering of the NAPI-boundary `NamingOptions` into the internal
//! [`NamingConfig`].
//!
//! The boundary has no sum type, so a `NamingValue` arrives as three
//! exclusive optional fields. Every failure here is `E_INVALID_OPTION`.

use crate::{
  bindings::{NamingOptions, NamingRuleEntry, NamingValue},
  error::{Diagnostic, DiagnosticCode, Reporter, bail},
  plan::naming::{
    config::{Case, Naming, NamingConfig, Rule, RuleEntry},
    parse_spec::compile as compile_parse_spec,
  },
};

/// Lowers the caller's naming options. Absent options yield the default
/// config, where each key falls back to its hardcoded default.
pub(crate) fn lower(
  options: Option<NamingOptions>,
  reporter: &Reporter,
) -> Result<NamingConfig, Diagnostic> {
  let Some(options) = options else {
    return Ok(NamingConfig::default());
  };
  Ok(NamingConfig {
    method_name: lower_value(options.method_name, "methodName", reporter)?,
    group: lower_value(options.group, "group", reporter)?,
  })
}

/// Lowers one key's value: a bare format string, a single rule, or a
/// fallback chain of either.
fn lower_value(
  value: Option<NamingValue>,
  key: &str,
  reporter: &Reporter,
) -> Result<Option<Naming>, Diagnostic> {
  let Some(value) = value else {
    return Ok(None);
  };

  let declared = u8::from(value.string.is_some())
    + u8::from(value.rule.is_some())
    + u8::from(value.chain.is_some());
  if declared != 1 {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "naming.{key}: must set exactly one of `string`, `rule`, or `chain` (got {declared})."
    );
  }

  if let Some(format) = value.string {
    return Ok(Some(Naming::Single(RuleEntry::Shorthand(format))));
  }
  if let Some(rule) = value.rule {
    return Ok(Some(Naming::Single(lower_rule(rule, key, reporter)?)));
  }

  let items = value.chain.unwrap_or_default();
  let entries = items
    .into_iter()
    .enumerate()
    .map(|(index, item)| lower_entry(item.string, item.rule, &format!("{key}[{index}]"), reporter))
    .collect::<Result<Vec<_>, Diagnostic>>()?;
  Ok(Some(Naming::Chain(entries)))
}

/// Lowers one chain item, which must set exactly one of `string` or `rule`.
fn lower_entry(
  shorthand: Option<String>,
  rule: Option<NamingRuleEntry>,
  path: &str,
  reporter: &Reporter,
) -> Result<RuleEntry, Diagnostic> {
  match (shorthand, rule) {
    (Some(format), None) => Ok(RuleEntry::Shorthand(format)),
    (None, Some(rule)) => lower_rule(rule, path, reporter),
    (Some(_), Some(_)) => bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "naming.{path}: a chain item cannot set both `string` and `rule`."
    ),
    (None, None) => bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "naming.{path}: a chain item must set exactly one of `string` or `rule`."
    ),
  }
}

fn lower_rule(
  rule: NamingRuleEntry,
  path: &str,
  reporter: &Reporter,
) -> Result<RuleEntry, Diagnostic> {
  let case = rule
    .case_
    .as_deref()
    .map(|name| lower_case(name, path, reporter))
    .transpose()?;

  let parse = rule
    .parse
    .map(|spec| {
      compile_parse_spec(&spec.source, &spec.flags).map_err(|error| {
        reporter.error(
          DiagnosticCode::InvalidOption,
          format!(
            "naming.{path}.parse: failed to compile regex `{}` (flags=`{}`): {error:?}",
            spec.source, spec.flags,
          ),
        )
      })
    })
    .transpose()?;

  // Without a `format`, the rule's output is the expanded `from` — which
  // would discard every capture the regex just produced.
  if parse.is_some() && rule.format.is_none() {
    bail!(
      reporter,
      DiagnosticCode::InvalidOption,
      "naming.{path}: when `parse` is present, `format` is required."
    );
  }

  Ok(RuleEntry::Rule(Rule {
    from: rule.from,
    parse,
    format: rule.format,
    case,
  }))
}

fn lower_case(name: &str, path: &str, reporter: &Reporter) -> Result<Case, Diagnostic> {
  Case::parse(name).ok_or_else(|| {
    reporter.error(
      DiagnosticCode::InvalidOption,
      format!(
        "naming.{path}.case: '{name}' is not one of 'camel', 'pascal', 'snake', 'kebab', 'constant'."
      ),
    )
  })
}
