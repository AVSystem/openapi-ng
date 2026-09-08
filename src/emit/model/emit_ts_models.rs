use std::collections::{BTreeMap, BTreeSet};

use crate::{
  emit::ts::{
    Binding, Doc, Member, Position, Render, Statement, Writer, import_line, interface_block, jsdoc,
    string_union, type_alias, type_reexport_line, w,
  },
  ir::{
    canonical::ModelSymbol,
    schema::{SchemaProperty, SchemaType},
  },
  plan::artifact_plan::ResolvedMappedType,
};

pub(crate) fn emit_model(
  symbols: &[ModelSymbol],
  mapped_types: &[ResolvedMappedType<'_>],
) -> String {
  // Roughly 256 bytes of TypeScript per named symbol.
  let mut out = Writer::with_capacity((symbols.len() * 256).max(1024));

  emit_mapped_imports(mapped_types, &mut out);

  let mapped_by_name: BTreeMap<&str, &ResolvedMappedType<'_>> = mapped_types
    .iter()
    .map(|mapped| (mapped.schema, mapped))
    .collect();

  // `emit_mapped_imports` writes one line per mapped type.
  if !mapped_types.is_empty() && !symbols.is_empty() {
    out.blank_line();
  }

  let mut first = true;
  for symbol in symbols {
    let name = symbol.name.as_ref();
    let mapped = mapped_by_name.get(name).copied();

    // A self-aliasing mapped type was already written as a re-export.
    if mapped.is_some_and(|mapped| is_self_alias(mapped)) {
      continue;
    }
    if !first {
      out.blank_line();
    }
    first = false;

    match mapped {
      Some(mapped) => type_alias(&mut out, name, Doc::default(), native_binding(mapped)),
      None => emit_symbol(symbol, &mut out),
    }
  }

  let mut rendered = out.into_string();
  if !rendered.ends_with('\n') {
    rendered.push('\n');
  }
  rendered
}

fn emit_symbol(symbol: &ModelSymbol, out: &mut Writer) {
  let doc = Doc::new(symbol.description.as_deref(), symbol.deprecated);
  let name = symbol.name.as_ref();
  match &symbol.body {
    SchemaType::InlineObject { properties } if properties.is_empty() => {
      type_alias(out, name, doc, "Record<string, never>");
    }
    SchemaType::InlineObject { properties } => {
      interface_block(out, name, doc, properties.iter().map(member), true);
    }
    SchemaType::StringLiterals { values } => string_union(out, name, doc, values),
    other => {
      jsdoc(out, doc);
      w!(out, "export type {name} = ");
      other.render(out, Position::Standalone);
      out.push(";\n");
    }
  }
}

fn member(property: &SchemaProperty) -> Member<'_> {
  Member {
    name: property.name.as_ref(),
    optional: !property.required,
    ty: &property.ty,
    doc: Doc::new(property.description.as_deref(), property.deprecated),
  }
}

/// The name a mapped type introduces into the file.
fn native_binding<'a>(mapped: &'a ResolvedMappedType<'_>) -> &'a str {
  mapped
    .alias
    .as_deref()
    .unwrap_or_else(|| mapped.ty.as_ref())
}

/// True when the binding a mapped type introduces already equals the schema
/// name it replaces. The usual `import type { Y as X }` plus
/// `export type X = X;` pair would then collide on `X`, so the pair
/// collapses to a single re-export.
fn is_self_alias(mapped: &ResolvedMappedType<'_>) -> bool {
  native_binding(mapped) == mapped.schema
}

/// Emits the mapped types' import block: regular imports first, grouped by
/// path, then the re-exports.
fn emit_mapped_imports(mapped_types: &[ResolvedMappedType<'_>], out: &mut Writer) {
  let mut imports = BTreeMap::<&str, BTreeSet<(&str, Option<&str>)>>::new();
  let mut reexports = BTreeMap::<&str, BTreeSet<(&str, &str)>>::new();

  for mapped in mapped_types {
    let path = mapped.import.as_ref();
    if is_self_alias(mapped) {
      reexports
        .entry(path)
        .or_default()
        .insert((mapped.ty.as_ref(), mapped.schema));
    } else {
      imports
        .entry(path)
        .or_default()
        .insert((mapped.ty.as_ref(), mapped.alias.as_deref()));
    }
  }

  for (path, bindings) in &imports {
    let bindings = bindings
      .iter()
      .map(|&(name, alias)| Binding { name, alias });
    import_line(out, bindings, path, Statement::TypeImport);
  }
  for (path, entries) in &reexports {
    type_reexport_line(out, entries, path);
  }
}
