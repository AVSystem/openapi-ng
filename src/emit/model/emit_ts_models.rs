use std::fmt::Write as _;

use crate::{
  emit::typescript::{self as ts, Position, Writer, render_type, write_import_line},
  ir::{
    canonical::ModelSymbol,
    schema::{SchemaProperty, SchemaType},
  },
  plan::artifact_plan::ResolvedMappedType,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn emit_model(
  model_symbols: &[ModelSymbol],
  mapped_types: &[ResolvedMappedType<'_>],
) -> String {
  // Heuristic: each named model symbol expands to ~256 bytes of TS once
  // mapped imports are factored in. Pre-sizing the buffer avoids 4-5
  // reallocs for petstore-rich-sized specs.
  let capacity = (model_symbols.len() * 256).max(1024);
  let mut output = Writer::with_capacity(capacity);

  emit_mapped_imports(mapped_types, &mut output);

  let mapped_by_name: BTreeMap<&str, &ResolvedMappedType<'_>> =
    mapped_types.iter().map(|m| (m.schema, m)).collect();

  // `emit_mapped_imports` writes exactly one line per `mapped_type` (either an
  // import or a re-export) — so non-empty input is sufficient to know we
  // emitted something and need a blank-line separator before the model body.
  if !mapped_types.is_empty() && !model_symbols.is_empty() {
    output.blank_line();
  }

  let mut first = true;

  for symbol in model_symbols {
    let name = symbol.name.as_ref();
    if let Some(mapped_type) = mapped_by_name.get(name) {
      // Re-export self-aliases are emitted as `export type { ... } from
      // '...'` in the imports block above — skip the placeholder alias
      // entirely (`export type X = X;` would collide with the imported
      // binding).
      if is_self_alias(mapped_type) {
        continue;
      }
      if !first {
        output.blank_line();
      }
      first = false;
      emit_mapped_placeholder(name, mapped_type, &mut output);
      continue;
    }

    if !first {
      output.blank_line();
    }
    first = false;

    match &symbol.body {
      SchemaType::InlineObject { properties } => emit_interface(
        name,
        symbol.description.as_deref(),
        symbol.deprecated,
        properties,
        &mut output,
      ),
      SchemaType::StringLiterals { values } => emit_enum(
        name,
        symbol.description.as_deref(),
        symbol.deprecated,
        values,
        &mut output,
      ),
      other => emit_type_alias(
        name,
        symbol.description.as_deref(),
        symbol.deprecated,
        other,
        &mut output,
      ),
    }
  }

  let mut rendered = output.into_string();
  if !rendered.ends_with('\n') {
    rendered.push('\n');
  }
  rendered
}

/// A mapped type is a *self-alias* when the binding it introduces into
/// the file (the alias if set, otherwise the imported type name) already
/// matches the schema name. In that case the regular `import type { Y as
/// X } from '...';` + `export type X = X;` pair would collide on the
/// `X` identifier, so we collapse to a single `export type { Y as X }
/// from '...';` re-export and skip the alias placeholder.
fn is_self_alias(mapped_type: &ResolvedMappedType<'_>) -> bool {
  let binding_name = mapped_type
    .alias
    .as_deref()
    .unwrap_or_else(|| mapped_type.ty.as_ref());
  binding_name == mapped_type.schema
}

fn emit_mapped_imports(mapped_types: &[ResolvedMappedType<'_>], output: &mut Writer) {
  // Group by import path, partitioning each path's entries into
  // re-exports and regular imports so the emitted block has a stable
  // ordering: regular imports first (deterministic per-path), then
  // re-exports.
  let mut imports_by_path = BTreeMap::<&str, BTreeSet<(&str, Option<&str>)>>::new();
  let mut reexports_by_path = BTreeMap::<&str, BTreeSet<(&str, &str)>>::new();

  for mapped_type in mapped_types {
    if is_self_alias(mapped_type) {
      // `export type { ty as schema }` — when `ty == schema`, drop the
      // alias rename so the line stays `export type { X } from '...'`.
      let imported = mapped_type.ty.as_ref();
      let exported_as = mapped_type.schema;
      reexports_by_path
        .entry(mapped_type.import.as_ref())
        .or_default()
        .insert((imported, exported_as));
    } else {
      imports_by_path
        .entry(mapped_type.import.as_ref())
        .or_default()
        .insert((mapped_type.ty.as_ref(), mapped_type.alias.as_deref()));
    }
  }

  for (import_path, type_names) in &imports_by_path {
    write_import_line(output, type_names.iter().copied(), import_path, true);
  }

  for (import_path, entries) in &reexports_by_path {
    write_reexport_line(output, entries, import_path);
  }
}

fn write_reexport_line(output: &mut Writer, entries: &BTreeSet<(&str, &str)>, import_path: &str) {
  output.push("export type { ");
  let mut first = true;
  for (imported, exported_as) in entries {
    if !first {
      output.push(", ");
    }
    first = false;
    output.push(imported);
    if imported != exported_as {
      output.push(" as ");
      output.push(exported_as);
    }
  }
  output.push(" } from '");
  output.push(import_path);
  output.push("';\n");
}

fn emit_mapped_placeholder(name: &str, mapped_type: &ResolvedMappedType<'_>, output: &mut Writer) {
  let native_type = mapped_type
    .alias
    .as_deref()
    .unwrap_or_else(|| mapped_type.ty.as_ref());
  ts::type_alias(output, name, None, false, native_type);
}

fn emit_type_alias(
  name: &str,
  description: Option<&str>,
  deprecated: bool,
  target: &SchemaType,
  output: &mut Writer,
) {
  ts::jsdoc(output, description, deprecated);
  write!(output, "export type {name} = ").unwrap();
  render_type(output, target, Position::Standalone);
  output.push(";\n");
}

fn emit_enum(
  name: &str,
  description: Option<&str>,
  deprecated: bool,
  values: &[String],
  output: &mut Writer,
) {
  ts::string_union(output, name, description, deprecated, values);
}

fn emit_interface(
  name: &str,
  description: Option<&str>,
  deprecated: bool,
  properties: &[SchemaProperty],
  output: &mut Writer,
) {
  if properties.is_empty() {
    ts::type_alias(
      output,
      name,
      description,
      deprecated,
      "Record<string, never>",
    );
    return;
  }
  ts::interface_block(
    output,
    name,
    description,
    deprecated,
    properties.iter().map(|p| {
      (
        p.name.as_ref(),
        !p.required,
        &p.ty,
        p.description.as_deref(),
        p.deprecated,
      )
    }),
    true,
  );
}
