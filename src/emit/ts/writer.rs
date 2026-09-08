//! The output buffer every emitter writes through.

/// Indent-aware string writer.
///
/// Tracks line-start state, so consecutive [`Writer::push`] calls share one
/// indent prefix without the caller threading it. Every method is
/// infallible: the sink is an in-memory `String`.
#[derive(Debug, Default)]
pub(crate) struct Writer {
  buf: String,
  indent_cache: String,
  indent_level: usize,
  line_start: bool,
  last_was_blank: bool,
}

impl Writer {
  pub(crate) fn with_capacity(capacity: usize) -> Self {
    Self {
      buf: String::with_capacity(capacity),
      indent_cache: String::new(),
      indent_level: 0,
      line_start: true,
      last_was_blank: false,
    }
  }

  pub(crate) fn push(&mut self, value: &str) {
    // Fast path for the mid-line token case, which needs no indent
    // bookkeeping and no newline scan.
    if !self.line_start && !value.contains('\n') {
      if !value.is_empty() {
        self.buf.push_str(value);
        self.last_was_blank = false;
      }
      return;
    }

    let mut rest = value;
    while !rest.is_empty() {
      if self.line_start {
        if let Some(stripped) = rest.strip_prefix('\n') {
          self.buf.push('\n');
          self.last_was_blank = true;
          rest = stripped;
          continue;
        }
        self.write_indent();
      }

      if let Some(newline) = rest.find('\n') {
        self.buf.push_str(&rest[..=newline]);
        let line_had_content = newline > 0;
        rest = &rest[newline + 1..];
        self.line_start = true;
        if line_had_content {
          self.last_was_blank = false;
        }
      } else {
        self.buf.push_str(rest);
        self.line_start = false;
        self.last_was_blank = false;
        break;
      }
    }
  }

  /// Appends formatted text. Prefer the `w!` / `wln!` macros at call
  /// sites.
  pub(crate) fn put(&mut self, args: std::fmt::Arguments<'_>) {
    // A format string with no arguments is already a `&str`; only an
    // interpolated one needs the intermediate allocation.
    if let Some(literal) = args.as_str() {
      self.push(literal);
    } else {
      self.push(&std::fmt::format(args));
    }
  }

  pub(crate) fn line(&mut self, value: &str) {
    let was_empty = value.is_empty() && self.line_start;
    self.push(value);
    self.buf.push('\n');
    self.line_start = true;
    if was_empty {
      self.last_was_blank = true;
    }
  }

  /// Ends the current line and leaves exactly one blank line behind.
  /// Collapses repeats, and does nothing at the start of the buffer.
  pub(crate) fn blank_line(&mut self) {
    if self.buf.is_empty() || self.last_was_blank {
      return;
    }
    if !self.buf.ends_with('\n') {
      self.buf.push('\n');
    }
    self.buf.push('\n');
    self.line_start = true;
    self.last_was_blank = true;
  }

  /// Writes `header` followed by ` {`, then indents. Pass an empty header
  /// for a bare `{`.
  pub(crate) fn open_block(&mut self, header: &str) {
    if header.is_empty() {
      self.line("{");
    } else {
      self.push(header);
      self.push(" {");
      self.end_line();
    }
    self.indent();
  }

  /// Writes an inline `{ … }`, indenting whatever `members` writes. Leaves
  /// the closing brace unterminated, for a type position.
  pub(crate) fn inline_block(&mut self, members: impl FnOnce(&mut Self)) {
    self.push("{\n");
    self.indent();
    members(self);
    self.dedent();
    self.push("}");
  }

  /// Dedents, then writes `}` followed by `suffix`.
  pub(crate) fn close_block(&mut self, suffix: &str) {
    self.dedent();
    if suffix.is_empty() {
      self.line("}");
    } else {
      self.push("}");
      self.push(suffix);
      self.end_line();
    }
  }

  pub(crate) fn indent(&mut self) {
    self.indent_level += 1;
    self.indent_cache.push_str("  ");
  }

  /// Panics when called more times than [`Writer::indent`].
  pub(crate) fn dedent(&mut self) {
    self.indent_level = self
      .indent_level
      .checked_sub(1)
      .expect("over-dedent in emitter");
    self.indent_cache.truncate(self.indent_level * 2);
  }

  pub(crate) fn into_string(self) -> String {
    self.buf
  }

  fn end_line(&mut self) {
    self.buf.push('\n');
    self.line_start = true;
    self.last_was_blank = false;
  }

  fn write_indent(&mut self) {
    self.buf.push_str(&self.indent_cache);
    self.line_start = false;
  }
}

/// Writes each of `items` through `write`, separated by `separator`.
pub(crate) fn write_separated<T>(
  out: &mut Writer,
  items: impl IntoIterator<Item = T>,
  separator: &str,
  write: impl Fn(&mut Writer, T),
) {
  items.into_iter().enumerate().for_each(|(index, item)| {
    if index > 0 {
      out.push(separator);
    }
    write(out, item);
  });
}

/// Appends formatted text to a [`Writer`].
macro_rules! w {
  ($writer:expr, $($arg:tt)*) => {
    $writer.put(::std::format_args!($($arg)*))
  };
}

/// Appends formatted text to a [`Writer`], then ends the line.
macro_rules! wln {
  ($writer:expr, $($arg:tt)*) => {{
    $writer.put(::std::format_args!($($arg)*));
    $writer.push("\n");
  }};
}

pub(crate) use {w, wln};
