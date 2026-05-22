pub(crate) mod emit_ts_models;

#[cfg(test)]
mod tests {
  use super::emit_ts_models;
  use crate::{
    ir::{
      canonical::ModelSymbol,
      schema::{SchemaScalar, SchemaType},
    },
    plan::artifact_plan::ResolvedMappedType,
    test_support::property,
  };

  #[test]
  fn emit_model_renders_aliases_enums_interfaces_and_inline_objects() {
    let model_symbols = vec![
      ModelSymbol {
        name: "AliasId".into(),
        description: None,
        deprecated: false,
        body: SchemaType::Scalar(SchemaScalar::String),
      },
      ModelSymbol {
        name: "PetStatus".into(),
        description: None,
        deprecated: false,
        body: SchemaType::StringLiterals {
          values: vec!["available".to_string(), "adopted".to_string()],
        },
      },
      ModelSymbol {
        name: "Pet".into(),
        description: None,
        deprecated: false,
        body: SchemaType::InlineObject {
          properties: vec![
            property("id", true, SchemaType::Ref("AliasId".into())),
            property(
              "profile",
              false,
              SchemaType::InlineObject {
                properties: vec![
                  property(
                    "displayName",
                    true,
                    SchemaType::Scalar(SchemaScalar::String),
                  ),
                  property(
                    "tags",
                    true,
                    SchemaType::Array(Box::new(SchemaType::Scalar(SchemaScalar::String))),
                  ),
                ],
              },
            ),
          ],
        },
      },
    ];

    let output = emit_ts_models::emit_model(&model_symbols, &[]);

    assert!(output.contains("export type AliasId = string;"));
    // 2 short literals stay inline (joined width well under ENUM_INLINE_WIDTH).
    assert!(output.contains("export type PetStatus = 'available' | 'adopted';"));
    assert!(output.contains("export interface Pet {\n"));
    assert!(output.contains("  id: AliasId;\n"));
    assert!(output.contains("  profile?: {\n"));
    assert!(output.contains("displayName: string;"));
    assert!(output.contains("tags: string[];"));
  }

  #[test]
  fn emit_model_renders_mapped_type_aliases_for_renamed_target() {
    // schema=UserId, type=ExternalUserId, alias=Nickname — the binding
    // brought into scope is `Nickname`, which is distinct from the schema
    // name. The placeholder alias still needs to redirect `UserId` to
    // the in-scope binding.
    let model_symbols = vec![ModelSymbol {
      name: "UserId".into(),
      description: None,
      deprecated: false,
      body: SchemaType::Scalar(SchemaScalar::String),
    }];

    let output = emit_ts_models::emit_model(
      &model_symbols,
      &[ResolvedMappedType {
        schema: "UserId",
        import: "./shared/user-id".into(),
        ty: "ExternalUserId".into(),
        alias: Some("Nickname".into()),
      }],
    );

    assert!(output.contains("import type { ExternalUserId as Nickname }"));
    assert!(output.contains("export type UserId = Nickname;"));
  }

  #[test]
  fn emit_model_uses_reexport_for_self_alias_to_avoid_identifier_collision() {
    // schema=UserId, type=ExternalUserId, alias=UserId — the binding
    // would otherwise be both imported as `UserId` AND aliased to
    // `UserId` in the same file (`export type UserId = UserId;`), a
    // duplicate-identifier error. The emitter sidesteps this by
    // collapsing to a single `export type { ExternalUserId as UserId }
    // from './shared/user-id';` re-export.
    let model_symbols = vec![ModelSymbol {
      name: "UserId".into(),
      description: None,
      deprecated: false,
      body: SchemaType::Scalar(SchemaScalar::String),
    }];

    let output = emit_ts_models::emit_model(
      &model_symbols,
      &[ResolvedMappedType {
        schema: "UserId",
        import: "./shared/user-id".into(),
        ty: "ExternalUserId".into(),
        alias: Some("UserId".into()),
      }],
    );

    assert!(
      output.contains("export type { ExternalUserId as UserId } from './shared/user-id';"),
      "expected re-export shape, got:\n{output}"
    );
    assert!(
      !output.contains("import type"),
      "self-alias case should not emit a separate `import type` line"
    );
    assert!(
      !output.contains("export type UserId = UserId;"),
      "self-alias case should not emit the broken placeholder alias"
    );
  }

  #[test]
  fn emit_model_uses_reexport_when_schema_matches_imported_type_directly() {
    // schema=UserId, type=UserId, no alias — same identifier collision
    // as above, just expressed without the alias field. The emitter
    // drops the `as UserId` because it would be a no-op rename.
    let model_symbols = vec![ModelSymbol {
      name: "UserId".into(),
      description: None,
      deprecated: false,
      body: SchemaType::Scalar(SchemaScalar::String),
    }];

    let output = emit_ts_models::emit_model(
      &model_symbols,
      &[ResolvedMappedType {
        schema: "UserId",
        import: "./shared/user-id".into(),
        ty: "UserId".into(),
        alias: None,
      }],
    );

    assert!(
      output.contains("export type { UserId } from './shared/user-id';"),
      "expected bare re-export shape, got:\n{output}"
    );
    assert!(!output.contains(" as UserId"));
  }
}
