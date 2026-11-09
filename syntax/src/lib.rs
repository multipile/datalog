//! Datalog read: a program's text parsed into clauses, checked, and written
//! back.
//!
//! What is decided here is what a program says: which clauses it has, which
//! terms each atom is written with, and whether every clause is one that could
//! be evaluated at all. What a program derives is not decided here, and nothing
//! here depends on anything. README.md is the grammar.
//!
//! A clause that parses and could not be evaluated whatever the relations held
//! is refused here, with the line and column it is at: a fact with a variable
//! in it, a head variable nothing in the body binds, a relation written with
//! two numbers of terms. Those are mistakes in the text. A relation nothing
//! defines, or a negation inside a recursion, is a question about the whole
//! program and the relations a host brings to it, and is left to whoever
//! evaluates it.

mod check;
mod parse;
mod syntax;

pub use parse::parse;
pub use syntax::{
    Atom, Clause, Comparison, Constant, Defined, Doc, Literal, Op, Program, Query, Rule, Span,
    Term, TermKind,
};

/// Why a program was refused, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

impl std::error::Error for Error {}
