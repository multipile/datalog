//! What makes a clause one no host could evaluate, whatever its relations held.
//!
//! Each of these is a property of the text alone. A fact has to be a tuple,
//! so it holds no variable. Every variable a rule produces, negates or
//! compares has to have come from a positive atom in its body — the rule
//! Datalog calls range restriction — or the answer would be every value there
//! is. And a relation is one number of columns wherever it is written.
//!
//! A named query's name is not a relation: nothing derives it and no atom can
//! read it. Its parameters are distinct variables its body binds, so filling
//! any of them in leaves a query that could still be asked, and it is one
//! question per name in a program, so a name says which.

use std::collections::{BTreeMap, BTreeSet};

use crate::Error;
use crate::syntax::{Atom, Clause, Literal, Program, Span, Term, TermKind};

pub fn program(p: &Program) -> Result<(), Error> {
    let mut arity: BTreeMap<&str, (usize, Span)> = BTreeMap::new();
    let mut names: BTreeMap<&str, Span> = BTreeMap::new();
    for c in &p.clauses {
        match c {
            Clause::Fact(a) => fact(a)?,
            Clause::Rule(r) => {
                if let Some(t) = r.head.terms.iter().find(|t| t.kind == TermKind::Wildcard) {
                    return Err(err(t.span, format!("_ in the head of {}", r.head.relation)));
                }
                let bound = body(&r.body, r.head.span)?;
                head_bound(&r.head, &bound)?;
            }
            Clause::Query(q) => {
                let bound = body(&q.body, q.span)?;
                if let Some(name) = &q.name {
                    if let Some(first) = names.get(name.relation.as_str()) {
                        return Err(err(
                            name.span,
                            format!(
                                "a query is named {} here and at {first}: a name is one question",
                                name.relation
                            ),
                        ));
                    }
                    names.insert(&name.relation, name.span);
                    parameters(name, &bound)?;
                }
            }
        }
        for a in atoms(c) {
            let n = a.terms.len();
            match arity.get(a.relation.as_str()) {
                Some(&(m, first)) if m != n => {
                    return Err(err(
                        a.span,
                        format!(
                            "{} is written with {} here and {} at {}",
                            a.relation,
                            columns(n),
                            columns(m),
                            first
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    arity.insert(&a.relation, (n, a.span));
                }
            }
        }
    }
    Ok(())
}

fn columns(n: usize) -> String {
    match n {
        1 => "1 term".into(),
        n => format!("{n} terms"),
    }
}

fn atoms(c: &Clause) -> Vec<&Atom> {
    fn lits(b: &[Literal]) -> Vec<&Atom> {
        b.iter()
            .flat_map(|l| match l {
                Literal::Positive(a) | Literal::Negative(a) => vec![a],
                Literal::Compare(_) => vec![],
            })
            .collect()
    }
    match c {
        Clause::Fact(a) => vec![a],
        Clause::Rule(r) => {
            let mut v = vec![&r.head];
            v.extend(lits(&r.body));
            v
        }
        Clause::Query(q) => lits(&q.body),
    }
}

fn err(span: Span, message: impl Into<String>) -> Error {
    Error {
        span,
        message: message.into(),
    }
}

fn fact(a: &Atom) -> Result<(), Error> {
    for t in &a.terms {
        match &t.kind {
            TermKind::Variable(v) => {
                return Err(err(
                    t.span,
                    format!(
                        "{v} in a fact: a fact is values, so {} needs :- and a body to bind it",
                        a.relation
                    ),
                ));
            }
            TermKind::Wildcard => {
                return Err(err(t.span, format!("_ in a fact of {}", a.relation)));
            }
            TermKind::Constant(_) => {}
        }
    }
    Ok(())
}

/// The variables the body's positive atoms bind, having refused a body that
/// has none or that negates or compares a variable none of them binds.
fn body(body: &[Literal], at: Span) -> Result<BTreeSet<&str>, Error> {
    if !body.iter().any(|l| matches!(l, Literal::Positive(_))) {
        return Err(err(
            at,
            "a body needs an atom that is not negated, to say what it ranges over",
        ));
    }
    let bound: BTreeSet<&str> = body.iter().flat_map(|l| l.binds()).collect();
    filters(body, &bound)?;
    Ok(bound)
}

/// Refuses a negation or a comparison in a body that uses a variable nothing
/// in the body binds, or a comparison with `_`.
fn filters(body: &[Literal], bound: &BTreeSet<&str>) -> Result<(), Error> {
    let unbound = |t: &Term, what: &str| -> Result<(), Error> {
        if let TermKind::Variable(v) = &t.kind
            && !bound.contains(v.as_str())
        {
            return Err(err(
                t.span,
                format!("{v} is {what} and no atom that is not negated binds it"),
            ));
        }
        Ok(())
    };
    for l in body {
        match l {
            Literal::Positive(_) => {}
            Literal::Negative(a) => {
                for t in &a.terms {
                    unbound(t, "negated")?;
                }
            }
            Literal::Compare(c) => {
                for t in [&c.left, &c.right] {
                    if t.kind == TermKind::Wildcard {
                        return Err(err(t.span, "_ in a comparison is equal to anything"));
                    }
                    unbound(t, "compared")?;
                }
            }
        }
    }
    Ok(())
}

/// A named query's parameters: each a variable, none twice, and each bound by
/// its body.
fn parameters(name: &Atom, bound: &BTreeSet<&str>) -> Result<(), Error> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for t in &name.terms {
        let TermKind::Variable(v) = &t.kind else {
            return Err(err(
                t.span,
                format!(
                    "{t} is a parameter of {}, and a parameter is a variable: a value is given when it is asked",
                    name.relation
                ),
            ));
        };
        if !seen.insert(v) {
            return Err(err(
                t.span,
                format!("{v} is a parameter of {} twice", name.relation),
            ));
        }
        if !bound.contains(v.as_str()) {
            return Err(err(
                t.span,
                format!(
                    "{v} is a parameter of {} and no atom that is not negated in its body binds it",
                    name.relation
                ),
            ));
        }
    }
    Ok(())
}

fn head_bound(head: &Atom, bound: &BTreeSet<&str>) -> Result<(), Error> {
    for t in &head.terms {
        if let TermKind::Variable(v) = &t.kind
            && !bound.contains(v.as_str())
        {
            return Err(err(
                t.span,
                format!(
                    "{v} is in the head of {} and no atom that is not negated in its body binds it",
                    head.relation
                ),
            ));
        }
    }
    Ok(())
}
