//! Tests for the `emit::ts` primitives.

#[cfg(test)]
mod tests {
  use super::super::ts::literal::safe_property_name;
  use super::super::ts::types::render_to_string;
  use super::super::ts::*;
  use crate::api_model::canonical::BodyFieldType;
  use crate::api_model::schema::{SchemaScalar, SchemaType};
  use crate::identifier::is_identifier;
  use crate::test_support::{nullable_property, property};

  #[test]
  fn open_and_close_block_manage_indentation() {
    let mut buffer = Writer::with_capacity(4096);

    buffer.open_block("export interface Pet");
    buffer.line("id: string;");
    buffer.line("name?: string;");
    buffer.close_block("");

    assert_eq!(
      buffer.into_string(),
      "export interface Pet {\n  id: string;\n  name?: string;\n}\n"
    );
  }

  #[test]
  fn blank_line_only_adds_one_empty_line_between_sections() {
    let mut buffer = Writer::with_capacity(4096);

    buffer.line("export type PetId = string;");
    buffer.blank_line();
    buffer.blank_line();
    buffer.line("export type PetName = string;");

    assert_eq!(
      buffer.into_string(),
      "export type PetId = string;\n\nexport type PetName = string;\n"
    );
  }

  #[test]
  fn leaves_valid_identifiers_unquoted() {
    for name in ["id", "Pet", "pet_id", "$ref", "_name", "petId2"] {
      assert_eq!(safe_property_name(name), name);
    }
  }

  #[test]
  fn leaves_reserved_words_unquoted_in_property_position() {
    // class/default/interface/etc. are valid property names in TS.
    for name in ["class", "default", "interface", "new"] {
      assert_eq!(safe_property_name(name), name);
    }
  }

  #[test]
  fn quotes_non_identifier_property_names() {
    assert_eq!(safe_property_name("2legged").as_ref(), "'2legged'");
    assert_eq!(safe_property_name("kebab-case").as_ref(), "'kebab-case'");
    assert_eq!(safe_property_name("dotted.name").as_ref(), "'dotted.name'");
    assert_eq!(safe_property_name("with space").as_ref(), "'with space'");
  }

  #[test]
  fn quotes_empty_name() {
    assert_eq!(safe_property_name("").as_ref(), "''");
  }

  #[test]
  fn escapes_embedded_quotes_and_backslashes() {
    assert_eq!(safe_property_name("it's").as_ref(), "'it\\'s'");
    assert_eq!(safe_property_name("a\\b").as_ref(), "'a\\\\b'");
  }

  #[test]
  fn escapes_embedded_control_chars() {
    assert_eq!(safe_property_name("a\nb").as_ref(), "'a\\nb'");
    assert_eq!(safe_property_name("a\rb").as_ref(), "'a\\rb'");
    assert_eq!(safe_property_name("a\tb").as_ref(), "'a\\tb'");
  }

  #[test]
  fn render_type_reference_covers_every_type_expression_variant() {
    let inline_object = SchemaType::InlineObject {
      properties: vec![
        property("name", true, SchemaType::Scalar(SchemaScalar::String)),
        nullable_property("nickname", false, SchemaType::Scalar(SchemaScalar::String)),
      ],
    };

    let cases = vec![
      (SchemaType::Any, "unknown"),
      (SchemaType::Scalar(SchemaScalar::String), "string"),
      (SchemaType::Scalar(SchemaScalar::Number), "number"),
      (SchemaType::Scalar(SchemaScalar::Boolean), "boolean"),
      (
        SchemaType::Array(Box::new(SchemaType::Ref("Pet".into()))),
        "Pet[]",
      ),
      (
        SchemaType::Map(Box::new(SchemaType::Scalar(SchemaScalar::Boolean))),
        "Record<string, boolean>",
      ),
      (
        SchemaType::StringLiterals {
          values: vec!["available".to_string(), "adopted".to_string()],
        },
        "'available' | 'adopted'",
      ),
      (SchemaType::Ref("Pet".into()), "Pet"),
      (
        SchemaType::Union {
          members: vec![SchemaType::Ref("Cat".into()), SchemaType::Ref("Dog".into())],
          discriminator: None,
        },
        "Cat | Dog",
      ),
      (
        SchemaType::Intersection(vec![
          SchemaType::Ref("AuditFields".into()),
          SchemaType::Ref("ContactFields".into()),
        ]),
        "AuditFields & ContactFields",
      ),
      (
        inline_object,
        "{\n  name: string;\n  nickname?: string | null;\n}",
      ),
      (
        SchemaType::Nullable(Box::new(SchemaType::Ref("Pet".into()))),
        "Pet | null",
      ),
    ];

    for (value, expected) in cases {
      assert_eq!(render_to_string(&value), expected);
    }
  }

  #[test]
  fn render_type_reference_wraps_nested_compositions_when_required() {
    let array_of_union = SchemaType::Array(Box::new(SchemaType::Union {
      members: vec![SchemaType::Ref("Cat".into()), SchemaType::Ref("Dog".into())],
      discriminator: None,
    }));

    let intersection_with_inline = SchemaType::Intersection(vec![
      SchemaType::Ref("AuditFields".into()),
      SchemaType::InlineObject {
        properties: vec![property(
          "nickname",
          false,
          SchemaType::Scalar(SchemaScalar::String),
        )],
      },
    ]);

    assert_eq!(render_to_string(&array_of_union), "(Cat | Dog)[]");
    assert_eq!(
      render_to_string(&intersection_with_inline),
      "AuditFields & {\n  nickname?: string;\n}"
    );
  }

  #[test]
  fn render_type_reference_indents_nested_inline_objects() {
    let nested_inline_object = SchemaType::InlineObject {
      properties: vec![property(
        "profile",
        true,
        SchemaType::InlineObject {
          properties: vec![
            property(
              "displayName",
              true,
              SchemaType::Scalar(SchemaScalar::String),
            ),
            property(
              "metadata",
              true,
              SchemaType::InlineObject {
                properties: vec![property(
                  "active",
                  true,
                  SchemaType::Scalar(SchemaScalar::Boolean),
                )],
              },
            ),
          ],
        },
      )],
    };

    assert_eq!(
      render_to_string(&nested_inline_object),
      "{\n  profile: {\n    displayName: string;\n    metadata: {\n      active: boolean;\n    };\n  };\n}"
    );
  }

  #[test]
  fn render_body_field_type_for_each_variant() {
    assert_eq!(
      render_to_string(&BodyFieldType::Scalar(SchemaScalar::String)),
      "string"
    );
    assert_eq!(
      render_to_string(&BodyFieldType::Scalar(SchemaScalar::Number)),
      "number"
    );
    assert_eq!(
      render_to_string(&BodyFieldType::Scalar(SchemaScalar::Boolean)),
      "boolean"
    );
    assert_eq!(
      render_to_string(&BodyFieldType::ArrayOfScalar(SchemaScalar::String)),
      "string[]"
    );
    assert_eq!(
      render_to_string(&BodyFieldType::ArrayOfScalar(SchemaScalar::Number)),
      "number[]"
    );
    assert_eq!(render_to_string(&BodyFieldType::Binary), "Blob | File");
    assert_eq!(
      render_to_string(&BodyFieldType::ArrayOfBinary),
      "(Blob | File)[]"
    );
  }

  #[test]
  fn write_import_line_emits_single_line_when_under_budget() {
    let mut out = Writer::with_capacity(4096);
    import_line(
      &mut out,
      [Binding::plain("Pet"), Binding::plain("PetId")],
      "./models",
      Statement::TypeImport,
    );
    assert_eq!(
      out.into_string(),
      "import type { Pet, PetId } from './models';\n"
    );
  }

  #[test]
  fn write_import_line_emits_alias_form() {
    let mut out = Writer::with_capacity(4096);
    import_line(
      &mut out,
      [Binding {
        name: "ExternalPetId",
        alias: Some("PetId"),
      }],
      "@demo/types",
      Statement::TypeImport,
    );
    assert_eq!(
      out.into_string(),
      "import type { ExternalPetId as PetId } from '@demo/types';\n"
    );
  }

  #[test]
  fn write_import_line_wraps_to_multi_line_when_over_budget() {
    // The 3 long-named imports exceed the 100-char inline budget; the
    // writer should switch to one-identifier-per-line with trailing
    // commas (prettier-friendly).
    let mut out = Writer::with_capacity(4096);
    let names = vec![
      Binding::plain("ResourceOneInterfaceWithExtraLongName"),
      Binding::plain("ResourceTwoInterfaceWithExtraLongName"),
      Binding::plain("ResourceThreeInterfaceWithExtraLongName"),
    ];
    import_line(&mut out, names, "./models", Statement::TypeImport);
    assert_eq!(
      out.into_string(),
      concat!(
        "import type {\n",
        "  ResourceOneInterfaceWithExtraLongName,\n",
        "  ResourceTwoInterfaceWithExtraLongName,\n",
        "  ResourceThreeInterfaceWithExtraLongName,\n",
        "} from './models';\n",
      ),
    );
  }

  #[test]
  fn write_import_line_keeps_single_entry_inline_even_when_over_budget() {
    // A single identifier always stays on one line — wrapping a single
    // name is just noise.
    let mut out = Writer::with_capacity(4096);
    import_line(
      &mut out,
      [Binding::plain(
        "ExtremelyLongIdentifierNameThatWouldOtherwiseTriggerTheWrapHeuristicYesItWould",
      )],
      "./models",
      Statement::TypeImport,
    );
    let rendered = out.into_string();
    assert!(rendered.starts_with("import type { ExtremelyLongIdentifier"));
    assert!(rendered.ends_with("} from './models';\n"));
    // Single line means no embedded newlines other than the trailing one.
    assert_eq!(rendered.matches('\n').count(), 1);
  }

  #[test]
  fn string_union_escapes_embedded_quotes_and_control_chars_inline() {
    let mut out = Writer::with_capacity(4096);
    string_union(
      &mut out,
      "Tricky",
      Doc::new(None, false),
      &["it's".to_string(), "a\\b".to_string(), "x\ny".to_string()],
    );
    assert_eq!(
      out.into_string(),
      "export type Tricky = 'it\\'s' | 'a\\\\b' | 'x\\ny';\n"
    );
  }

  #[test]
  fn string_union_escapes_embedded_quotes_when_wrapped_multi_line() {
    // Force the multi-line branch by exceeding ENUM_INLINE_WIDTH; each
    // value renders as its own `| '...'` line and must route through the
    // same escape table.
    let long = "a".repeat(40);
    let values = vec![
      format!("{long}-1'a"),
      format!("{long}-2\\b"),
      format!("{long}-3"),
    ];
    let mut out = Writer::with_capacity(4096);
    string_union(&mut out, "Long", Doc::new(None, false), &values);
    let rendered = out.into_string();
    assert!(rendered.contains("| 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-1\\'a'\n"));
    assert!(rendered.contains("| 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-2\\\\b'\n"));
    assert!(rendered.trim_end().ends_with("-3';"));
  }

  #[test]
  fn render_type_wraps_array_of_array_of_union() {
    // Closes the (Cat | Dog)[][] precedence gap the previous 3-function
    // split could leave uncovered.
    let nested = SchemaType::Array(Box::new(SchemaType::Array(Box::new(SchemaType::Union {
      members: vec![SchemaType::Ref("Cat".into()), SchemaType::Ref("Dog".into())],
      discriminator: None,
    }))));
    let mut buf = Writer::with_capacity(4096);
    nested.render(&mut buf, Position::Standalone);
    assert_eq!(buf.into_string(), "(Cat | Dog)[][]");
  }

  use proptest::prelude::*;

  /// Lexes the output as either a bare identifier or a single-quoted
  /// string literal. Returns true iff the lex succeeds end-to-end —
  /// matches what TypeScript's parser would accept in property position.
  fn is_valid_property_name_lexeme(out: &str) -> bool {
    if out.is_empty() {
      return false;
    }
    if is_identifier(out) {
      return true;
    }
    let bytes = out.as_bytes();
    if bytes[0] != b'\'' || *bytes.last().unwrap() != b'\'' || bytes.len() < 2 {
      return false;
    }
    // Walk the interior, validating the escape table used in
    // `write_string_literal` and ensuring no raw control char survives.
    let interior = &out[1..out.len() - 1];
    let mut chars = interior.chars();
    while let Some(ch) = chars.next() {
      match ch {
        '\\' => match chars.next() {
          Some('\\' | '\'' | 'n' | 'r' | 't') => {}
          _ => return false,
        },
        // Raw control chars (newline/CR/tab) and the closing quote are
        // routed through the escape table; any literal occurrence after
        // `safe_property_name` would indicate an escape miss.
        '\'' | '\n' | '\r' | '\t' => return false,
        _ => {}
      }
    }
    true
  }

  proptest! {
    #[test]
    fn safe_property_name_returns_a_lexable_property_name_for_any_input(name in ".{0,32}") {
      let out = safe_property_name(&name);
      prop_assert!(
        is_valid_property_name_lexeme(out.as_ref()),
        "safe_property_name produced unparseable output {out:?} for input {name:?}",
      );
    }

    #[test]
    fn safe_property_name_is_idempotent_when_input_is_a_valid_bare_ident(
      first in proptest::char::range('A', 'Z'),
      rest in proptest::collection::vec(prop_oneof![
        proptest::char::range('a', 'z'),
        proptest::char::range('A', 'Z'),
        proptest::char::range('0', '9'),
        Just('_'),
      ], 0..16),
    ) {
      let mut ident = String::new();
      ident.push(first);
      ident.extend(rest);
      let escaped = safe_property_name(&ident);
      prop_assert_eq!(escaped.as_ref(), ident.as_str());
    }
  }

  #[test]
  fn jsdoc_escapes_close_comment_sequence() {
    let mut out = Writer::with_capacity(4096);
    jsdoc(&mut out, Doc::new(Some("Crafted */ injection /*"), false));
    let rendered = out.into_string();
    // The only allowed `*/` is the trailing JSDoc closer on its own line.
    // Strip exactly the opener and closer lines, then assert no `*/` remains
    // in the body of the comment — i.e. the description was escaped.
    let body = rendered
      .strip_prefix("/**\n")
      .and_then(|rest| rest.strip_suffix(" */\n"))
      .expect("jsdoc output should be wrapped in /** ... */");
    assert!(
      !body.contains("*/"),
      "raw */ leaked into JSDoc body: {body}"
    );
    assert!(
      rendered.contains("*\\/"),
      "expected escaped *\\/ in output, got: {rendered}"
    );
  }
}
