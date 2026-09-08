//! TypeScript emit primitives.
//!
//! Parenthesization is decided by [`types::Position`] and indentation by
//! [`writer::Writer`].

pub(crate) mod decl;
pub(crate) mod imports;
pub(crate) mod literal;
pub(crate) mod types;
pub(crate) mod writer;

pub(crate) use decl::{Doc, Member, interface_block, jsdoc, string_union, type_alias};
pub(crate) use imports::{Binding, Statement, import_line, type_import_block, type_reexport_line};
pub(crate) use types::{Position, Render, member_declaration};
pub(crate) use writer::{Writer, w, wln, write_separated};
