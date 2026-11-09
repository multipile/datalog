//! What a program is once it is read, and how it is written back.
//!
//! Every atom and term carries the place it was written, so a host refusing
//! something the parser accepted — a relation it has no such thing as — can
//! say where, the way this crate does. Writing back is Display, and what it
//! writes parses to the same program: that is what the tests hold it to.

use std::fmt;

/// A place in the text: a line and a column, both counted from one, the
/// column in characters rather than bytes so it is the one an editor shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// A program: its clauses, in the order they were written, and beside each
/// the doc comment written above it, if one was.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Program {
    pub clauses: Vec<Clause>,
    pub docs: Vec<Option<Doc>>,
}

/// The `%%` lines above a clause: its first line the title, one sentence, and
/// the rest the description — wrapped prose joined into paragraphs, empty when
/// there was only the title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    pub title: String,
    pub description: String,
    pub span: Span,
}

/// A relation a program defines: where its first clause is, and that clause's
/// doc comment, which is the relation's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defined<'a> {
    pub relation: &'a str,
    pub span: Span,
    pub doc: Option<&'a Doc>,
}

impl Program {
    /// Every relation the program's facts and rules define, in the order they
    /// are first written, each with the doc comment of its first clause.
    pub fn defined(&self) -> Vec<Defined<'_>> {
        let mut out: Vec<Defined<'_>> = Vec::new();
        for (c, doc) in self.clauses.iter().zip(&self.docs) {
            let head = match c {
                Clause::Fact(a) => a,
                Clause::Rule(r) => &r.head,
                Clause::Query(_) => continue,
            };
            if !out.iter().any(|d| d.relation == head.relation) {
                out.push(Defined {
                    relation: &head.relation,
                    span: head.span,
                    doc: doc.as_ref(),
                });
            }
        }
        out
    }

    pub fn facts(&self) -> impl Iterator<Item = &Atom> {
        self.clauses.iter().filter_map(|c| match c {
            Clause::Fact(a) => Some(a),
            _ => None,
        })
    }

    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.clauses.iter().filter_map(|c| match c {
            Clause::Rule(r) => Some(r),
            _ => None,
        })
    }

    /// Every named query, in the order written, with the doc comment above it.
    pub fn named(&self) -> impl Iterator<Item = (&Query, Option<&Doc>)> {
        self.clauses
            .iter()
            .zip(&self.docs)
            .filter_map(|(c, doc)| match c {
                Clause::Query(q) if q.name.is_some() => Some((q, doc.as_ref())),
                _ => None,
            })
    }

    pub fn queries(&self) -> impl Iterator<Item = &Query> {
        self.clauses.iter().filter_map(|c| match c {
            Clause::Query(q) => Some(q),
            _ => None,
        })
    }
}

/// One clause: a fact, `p(a).`; a rule, `p(X) :- q(X).`; or a query,
/// `?- p(X).`, which may be named, `named(X) ?- p(X).`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clause {
    Fact(Atom),
    Rule(Rule),
    Query(Query),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<Literal>,
}

/// A body asked for rather than named: what it binds is the answer.
///
/// A named query, `followers(Of) ?- upstream(P, Of).`, is the same question
/// with a name a host can list it by and parameters a host can fill in before
/// it is asked; asked as it is written, every parameter is a variable like any
/// other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub name: Option<Atom>,
    pub body: Vec<Literal>,
    pub span: Span,
}

impl Query {
    /// A named query's parameters, in the order its name writes them; none
    /// for a query with no name.
    pub fn parameters(&self) -> Vec<&str> {
        self.name
            .as_ref()
            .map(|n| n.variables().collect())
            .unwrap_or_default()
    }

    /// The query with each parameter given a value — None leaves that one a
    /// variable — and no name, since what is left is the question itself.
    /// None when the values are not one per parameter.
    pub fn bind(&self, values: &[Option<Constant>]) -> Option<Query> {
        let parameters = self.parameters();
        if values.len() != parameters.len() {
            return None;
        }
        let value = |v: &str| {
            parameters
                .iter()
                .position(|p| *p == v)
                .and_then(|i| values[i].clone())
        };
        let term = |t: &Term| match &t.kind {
            TermKind::Variable(v) => match value(v) {
                Some(c) => Term {
                    kind: TermKind::Constant(c),
                    span: t.span,
                },
                None => t.clone(),
            },
            _ => t.clone(),
        };
        fn literal(l: &Literal, term: &dyn Fn(&Term) -> Term) -> Literal {
            let atom = |a: &Atom| Atom {
                relation: a.relation.clone(),
                terms: a.terms.iter().map(term).collect(),
                span: a.span,
            };
            match l {
                Literal::Positive(a) => Literal::Positive(atom(a)),
                Literal::Negative(a) => Literal::Negative(atom(a)),
                Literal::Compare(c) => Literal::Compare(Comparison {
                    left: term(&c.left),
                    op: c.op,
                    right: term(&c.right),
                }),
            }
        }
        let body = self.body.iter().map(|l| literal(l, &term)).collect();
        Some(Query {
            name: None,
            body,
            span: self.span,
        })
    }

    /// The variables an answer has a column for: every one a positive atom
    /// binds, in the order they are first written. A variable only a negation
    /// or a comparison mentions is bound by one of these or refused, so these
    /// are all of them.
    pub fn variables(&self) -> Vec<&str> {
        let mut out: Vec<&str> = vec![];
        for l in &self.body {
            for v in l.binds() {
                if !out.contains(&v) {
                    out.push(v);
                }
            }
        }
        out
    }
}

/// One condition in a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// `p(X)`: a tuple of p binds it.
    Positive(Atom),
    /// `!p(X)`: no tuple of p matches it.
    Negative(Atom),
    /// `X = Y`, `X != "a"`, `N > 1`.
    Compare(Comparison),
}

impl Literal {
    /// The variables this literal binds for the rest of its body: a positive
    /// atom's. A negation or a comparison binds nothing.
    pub fn binds(&self) -> Vec<&str> {
        match self {
            Literal::Positive(a) => a.variables().collect(),
            Literal::Negative(_) | Literal::Compare(_) => vec![],
        }
    }
}

/// `X = Y`, `X != "a"`, `N > 1`: two terms and how they compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub left: Term,
    pub op: Op,
    pub right: Term,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A relation and the terms it is written with. `p` with no parentheses is a
/// relation of no columns, which holds or does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub relation: String,
    pub terms: Vec<Term>,
    pub span: Span,
}

impl Atom {
    /// The named variables in it, in order, with repeats.
    pub fn variables(&self) -> impl Iterator<Item = &str> {
        self.terms.iter().filter_map(|t| match &t.kind {
            TermKind::Variable(v) => Some(v.as_str()),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub kind: TermKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermKind {
    /// `X`, `Node`, `_x`: begins with a capital or an underscore.
    Variable(String),
    /// `_`: any value, never compared with anything else.
    Wildcard,
    Constant(Constant),
}

/// A value written in the program.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Constant {
    /// `tree` and `"tree"` are the same symbol: quoting is how a symbol that is
    /// not a bare name — `"a/b"`, `"a b"` — is written, not a second
    /// kind of value.
    Symbol(String),
    Integer(i64),
    /// `@name`: a value the host resolves, written as the host's users write
    /// it. The parser keeps the name and decides nothing about it.
    Reference(String),
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, c) in self.clauses.iter().enumerate() {
            if let Some(Some(doc)) = self.docs.get(i) {
                write!(f, "{doc}")?;
            }
            writeln!(f, "{c}")?;
        }
        Ok(())
    }
}

/// A doc comment as `%%` lines: the title, then each paragraph of the
/// description on a line of its own, an empty `%%` line between them.
impl fmt::Display for Doc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "%% {}", self.title)?;
        for (i, p) in self
            .description
            .split("\n\n")
            .filter(|p| !p.is_empty())
            .enumerate()
        {
            if i > 0 {
                writeln!(f, "%%")?;
            }
            writeln!(f, "%% {p}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Clause::Fact(a) => write!(f, "{a}."),
            Clause::Rule(r) => write!(f, "{r}"),
            Clause::Query(q) => write!(f, "{q}"),
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} :- ", self.head)?;
        body(f, &self.body)
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "{name} ")?;
        }
        write!(f, "?- ")?;
        body(f, &self.body)
    }
}

fn body(f: &mut fmt::Formatter<'_>, body: &[Literal]) -> fmt::Result {
    for (i, l) in body.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{l}")?;
    }
    write!(f, ".")
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Positive(a) => write!(f, "{a}"),
            Literal::Negative(a) => write!(f, "!{a}"),
            Literal::Compare(c) => write!(f, "{c}"),
        }
    }
}

impl fmt::Display for Comparison {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.left, self.op, self.right)
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Op::Eq => "=",
            Op::Ne => "!=",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
        })
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.relation)?;
        if self.terms.is_empty() {
            return Ok(());
        }
        write!(f, "(")?;
        for (i, t) in self.terms.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{t}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TermKind::Variable(v) => f.write_str(v),
            TermKind::Wildcard => f.write_str("_"),
            TermKind::Constant(c) => write!(f, "{c}"),
        }
    }
}

impl fmt::Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Symbol(s) if is_name(s) => f.write_str(s),
            Constant::Symbol(s) => {
                f.write_str("\"")?;
                for c in s.chars() {
                    match c {
                        '"' => f.write_str("\\\"")?,
                        '\\' => f.write_str("\\\\")?,
                        '\n' => f.write_str("\\n")?,
                        '\t' => f.write_str("\\t")?,
                        c => write!(f, "{c}")?,
                    }
                }
                f.write_str("\"")
            }
            Constant::Integer(n) => write!(f, "{n}"),
            Constant::Reference(r) => write!(f, "@{r}"),
        }
    }
}

/// A symbol that can be written without quotes: a lowercase letter, then
/// letters, digits and underscores.
pub(crate) fn is_name(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_lowercase())
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
