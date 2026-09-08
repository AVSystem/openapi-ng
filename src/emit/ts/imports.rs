//! `import` and `export … from` statement emit.

use std::collections::{BTreeMap, BTreeSet};

use super::writer::Writer;

/// Width above which a statement wraps to one identifier per line.
///
/// Matches prettier's wrap point, which keeps a consumer's first `format`
/// run a no-op and regeneration an empty diff.
const INLINE_WIDTH: usize = 100;

/// One imported or re-exported name, optionally renamed.
#[derive(Clone, Copy)]
pub(crate) struct Binding<'a> {
  pub(crate) name: &'a str,
  pub(crate) alias: Option<&'a str>,
}

impl<'a> Binding<'a> {
  pub(crate) const fn plain(name: &'a str) -> Self {
    Self { name, alias: None }
  }

  const fn renamed(name: &'a str, alias: &'a str) -> Self {
    Self {
      name,
      alias: Some(alias),
    }
  }

  fn width(self) -> usize {
    self.name.len() + self.alias.map_or(0, |alias| " as ".len() + alias.len())
  }

  fn write(self, out: &mut Writer) {
    out.push(self.name);
    if let Some(alias) = self.alias {
      out.push(" as ");
      out.push(alias);
    }
  }
}

/// Emits one `import type { … } from '…';` per path, names in iteration
/// order.
pub(crate) fn type_import_block(out: &mut Writer, by_path: &BTreeMap<&str, BTreeSet<&str>>) {
  for (path, names) in by_path {
    import_line(
      out,
      names.iter().copied().map(Binding::plain),
      path,
      Statement::TypeImport,
    );
  }
}

/// Which statement keyword the bindings belong to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Statement {
  TypeImport,
  /// `export type { … } from '…'` — re-exports the names instead of
  /// binding them locally.
  TypeReexport,
}

impl Statement {
  const fn open(self) -> &'static str {
    match self {
      Self::TypeImport => "import type { ",
      Self::TypeReexport => "export type { ",
    }
  }

  const fn open_wrapped(self) -> &'static str {
    match self {
      Self::TypeImport => "import type {\n",
      Self::TypeReexport => "export type {\n",
    }
  }
}

/// Emits one statement binding `bindings` from `path`, wrapping when the
/// single-line form would exceed [`INLINE_WIDTH`].
pub(crate) fn import_line<'a>(
  out: &mut Writer,
  bindings: impl IntoIterator<Item = Binding<'a>>,
  path: &str,
  statement: Statement,
) {
  // Buffered to measure the joined width before choosing a layout.
  let bindings: Vec<Binding<'a>> = bindings.into_iter().collect();
  let names: usize = bindings.iter().map(|binding| binding.width()).sum();
  let separators = bindings.len().saturating_sub(1) * ", ".len();
  let tail = " } from '".len() + path.len() + "';".len();

  if statement.open().len() + names + separators + tail <= INLINE_WIDTH || bindings.len() <= 1 {
    out.push(statement.open());
    for (index, binding) in bindings.iter().enumerate() {
      if index > 0 {
        out.push(", ");
      }
      binding.write(out);
    }
    out.push(" } from '");
    out.push(path);
    out.push("';\n");
    return;
  }

  out.push(statement.open_wrapped());
  out.indent();
  for binding in &bindings {
    binding.write(out);
    out.push(",\n");
  }
  out.dedent();
  out.push("} from '");
  out.push(path);
  out.push("';\n");
}

/// Emits `export type { … } from '…';`, dropping the rename where the
/// exported name already matches the imported one.
pub(crate) fn type_reexport_line(out: &mut Writer, entries: &BTreeSet<(&str, &str)>, path: &str) {
  let bindings = entries.iter().map(|&(imported, exported)| {
    if imported == exported {
      Binding::plain(imported)
    } else {
      Binding::renamed(imported, exported)
    }
  });
  import_line(out, bindings, path, Statement::TypeReexport);
}
