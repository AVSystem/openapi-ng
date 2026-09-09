//! Position of a schema walk: breadcrumb, nesting depth and diagnostic
//! sink, carried as one value.

use crate::error::{Context, Diagnostic, Reporter};

use super::{MAX_NORMALIZE_DEPTH, unsupported_rule};

/// One position in a schema tree; every constructor but
/// [`SchemaWalk::root`] descends a level. Call
/// [`SchemaWalk::check_depth`] before recursing.
#[derive(Clone, Copy)]
pub(crate) struct SchemaWalk<'a> {
  context: Context<'a>,
  depth: u16,
  reporter: &'a Reporter,
}

impl<'a> SchemaWalk<'a> {
  /// Starts a walk at a top-level schema, parameter, request body or
  /// response.
  pub(crate) const fn root(context: Context<'a>, reporter: &'a Reporter) -> Self {
    Self {
      context,
      depth: 0,
      reporter,
    }
  }

  /// Descends into the named object property.
  pub(crate) const fn property<'s>(&'s self, name: &'s str) -> SchemaWalk<'s> {
    self.descend(Context::Property {
      parent: &self.context,
      name,
    })
  }

  /// Descends into the 1-based member of a `oneOf` / `anyOf` / `allOf`.
  pub(crate) const fn composition_member<'s>(&'s self, index: usize) -> SchemaWalk<'s> {
    self.descend(Context::CompositionMember {
      parent: &self.context,
      index,
    })
  }

  /// Descends into the `additionalProperties` sub-schema.
  pub(crate) const fn additional_properties<'s>(&'s self) -> SchemaWalk<'s> {
    self.descend(Context::AdditionalProperties {
      parent: &self.context,
    })
  }

  /// Descends into an array's item schema, which shares the array's
  /// breadcrumb.
  pub(crate) const fn item(&self) -> Self {
    Self {
      context: self.context,
      depth: self.depth + 1,
      reporter: self.reporter,
    }
  }

  const fn descend<'s>(&'s self, context: Context<'s>) -> SchemaWalk<'s> {
    SchemaWalk {
      context,
      depth: self.depth + 1,
      reporter: self.reporter,
    }
  }

  /// The breadcrumb for this position, for use in a diagnostic message.
  /// Allocates, so call it only while building one.
  pub(crate) fn here(&self) -> String {
    self.context.render()
  }

  pub(crate) const fn reporter(&self) -> &'a Reporter {
    self.reporter
  }

  /// Fails once the walk has descended past [`MAX_NORMALIZE_DEPTH`].
  pub(crate) fn check_depth(&self) -> Result<(), Diagnostic> {
    if self.depth < MAX_NORMALIZE_DEPTH {
      return Ok(());
    }
    Err(unsupported_rule(
      self.reporter,
      format!(
        "{} nesting exceeds {MAX_NORMALIZE_DEPTH} levels (likely cyclic or pathological spec).",
        self.here()
      ),
    ))
  }
}
