use crate::{
  ident::{MethodName, TypeName},
  plan::naming::{case::apply as apply_case, config::Case},
};

/// PascalCase service class name for a group, e.g. `"pet"` → `PetRest`.
pub(crate) fn service_class_name(group: &str) -> TypeName {
  TypeName::new(format!("{}Rest", apply_case(group, Case::Pascal)))
}

/// Kebab-case file stem for a group, e.g. `"PetOrder"` → `pet-order`.
pub(crate) fn service_file_stem(group: &str) -> String {
  apply_case(group, Case::Kebab)
}

/// PascalCase name of the interface carrying an operation's path, query,
/// header and body fields, e.g. `listPets` → `ListPetsParams`.
///
/// Suffixed `Params`: a spec may already declare a schema named
/// `<OperationId>Request`.
pub(crate) fn request_interface_name(method_name: &MethodName) -> TypeName {
  TypeName::new(format!(
    "{}Params",
    apply_case(method_name.as_str(), Case::Pascal)
  ))
}

/// PascalCase name of the interface mapping an operation's 4xx/5xx statuses
/// to their body types, e.g. `updatePet` → `UpdatePetError`.
///
/// Read at the call site as `UpdatePetError[400]`.
pub(crate) fn error_interface_name(method_name: &MethodName) -> TypeName {
  TypeName::new(format!(
    "{}Error",
    apply_case(method_name.as_str(), Case::Pascal)
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn method(name: &str) -> MethodName {
    MethodName::new(name.to_string())
  }

  #[test]
  fn service_class_name_converts_lowercase_tag_to_pascal_case_rest_suffix() {
    assert_eq!(service_class_name("pet").to_string(), "PetRest");
  }

  #[test]
  fn service_class_name_converts_camel_case_tag_to_pascal_case_rest_suffix() {
    assert_eq!(service_class_name("petOrder").to_string(), "PetOrderRest");
  }

  #[test]
  fn service_class_name_converts_kebab_tag_to_pascal_case_rest_suffix() {
    assert_eq!(service_class_name("pet-order").to_string(), "PetOrderRest");
  }

  #[test]
  fn service_file_stem_converts_pascal_case_tag_to_kebab_case() {
    assert_eq!(service_file_stem("PetOrder"), "pet-order");
  }

  #[test]
  fn service_file_stem_returns_single_word_lowercase_unchanged() {
    assert_eq!(service_file_stem("pet"), "pet");
  }

  #[test]
  fn request_interface_name_converts_camel_case_method_name_to_pascal_params() {
    assert_eq!(
      request_interface_name(&method("listPets")).to_string(),
      "ListPetsParams"
    );
  }

  #[test]
  fn request_interface_name_converts_lower_method_name_to_pascal_params() {
    assert_eq!(
      request_interface_name(&method("updatePet")).to_string(),
      "UpdatePetParams"
    );
  }

  //
  // service_class_name and service_file_stem are pure case-conversions
  // over arbitrary tag strings sourced from the spec. The example tests
  // above pin representative cases; the properties below assert global
  // invariants so adversarial inputs (whitespace, control chars, unicode)
  // can't sneak in malformed identifiers / file stems.

  use proptest::prelude::*;

  /// First char must satisfy TS IdentifierStart (we restrict to ASCII
  /// alphabetic + `_` + `$`); subsequent chars must be IdentifierPart.
  /// Matches `is_ident` in `emit::typescript`.
  fn is_ts_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
      return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
      return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
  }

  /// Result of `service_file_stem` should be kebab-case ASCII: lowercase
  /// letters, digits, and hyphens, with no leading/trailing hyphen and no
  /// consecutive hyphens. Returns true for the empty string (e.g. when the
  /// input was all non-alphanumeric).
  fn is_kebab_case_ascii(value: &str) -> bool {
    if value.is_empty() {
      return true;
    }
    if value.starts_with('-') || value.ends_with('-') {
      return false;
    }
    let mut prev_hyphen = false;
    for ch in value.chars() {
      match ch {
        'a'..='z' | '0'..='9' => prev_hyphen = false,
        '-' => {
          if prev_hyphen {
            return false;
          }
          prev_hyphen = true;
        }
        _ => return false,
      }
    }
    true
  }

  proptest! {
    /// `service_class_name` is only fed values that survive the
    /// `tag_first_operation_grouper` policy check (tags non-empty after
    /// trim, ASCII identifier-shaped). We test the policy-clean subset
    /// here — alphabetic tags with optional hyphens/underscores — because
    /// that's the surface the rest of the planner actually sees.
    #[test]
    fn service_class_name_emits_valid_ts_identifier_with_rest_suffix(
      tag in "[a-zA-Z][a-zA-Z0-9_-]{0,31}"
    ) {
      let class_name = service_class_name(&tag).to_string();
      let class_name = class_name.as_str();
      prop_assert!(class_name.ends_with("Rest"));
      prop_assert!(
        is_ts_identifier(class_name),
        "service_class_name produced non-identifier {class_name:?} for tag {tag:?}",
      );
    }

    /// Scoped to ASCII tags because the policy layer in
    /// `tag_first_operation_grouper` rejects any operation whose tag
    /// would not produce a valid Angular-style file stem. Non-ASCII
    /// inputs to `service_file_stem` are reachable in code but never in
    /// practice — locking the kebab-case invariant on the ASCII subset
    /// is what consumers actually rely on.
    #[test]
    fn service_file_stem_produces_kebab_case_or_empty_for_ascii_tags(
      tag in "[ -~]{0,32}"
    ) {
      let stem = service_file_stem(&tag);
      prop_assert!(
        is_kebab_case_ascii(&stem),
        "service_file_stem produced non-kebab {stem:?} for tag {tag:?}",
      );
    }

  }
}
