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
pub(crate) fn request_interface_name(method_name: &MethodName) -> TypeName {
  TypeName::new(format!(
    "{}Params",
    apply_case(method_name.as_str(), Case::Pascal)
  ))
}

/// PascalCase name of the interface mapping an operation's 4xx/5xx statuses
/// to their body types, e.g. `updatePet` → `UpdatePetError`.
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


  use proptest::prelude::*;

  /// ASCII-only TS identifier shape, matching `ident::is_ident`.
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

  /// Kebab-case ASCII: no leading, trailing or repeated hyphen. The empty
  /// string passes.
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
