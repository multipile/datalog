//! The text read into clauses: a lexer, a parser by recursive descent, and the
//! checks in check.rs run over what they produce.
//!
//! A `%%` line is kept, as a line of the doc comment of the clause under it;
//! every other comment is dropped where it is read.
//!
//! The first mistake is the one reported. A program is short and is read by a
//! person, and a second error after the first is as often the first one's echo
//! as a mistake of its own.

use crate::syntax::{
    Atom, Clause, Comparison, Constant, Doc, Literal, Op, Program, Query, Rule, Span, Term,
    TermKind,
};
use crate::{Error, check};

/// Read a program, and refuse one that is not Datalog or that no host could
/// evaluate.
pub fn parse(source: &str) -> Result<Program, Error> {
    let tokens = lex(source)?;
    let program = Parser { tokens, at: 0 }.program()?;
    check::program(&program)?;
    Ok(program)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Name(String),
    Variable(String),
    Wildcard,
    Str(String),
    Int(i64),
    Reference(String),
    LParen,
    RParen,
    Comma,
    Dot,
    If,
    Ask,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// One `%%` line, without the `%%` and the one space after it.
    Doc(String),
    End,
}

impl Tok {
    fn describe(&self) -> String {
        match self {
            Tok::Name(n) => format!("{n:?}"),
            Tok::Variable(v) => format!("variable {v}"),
            Tok::Wildcard => "_".into(),
            Tok::Str(s) => format!("string {s:?}"),
            Tok::Int(n) => format!("{n}"),
            Tok::Reference(r) => format!("@{r}"),
            Tok::LParen => "\"(\"".into(),
            Tok::RParen => "\")\"".into(),
            Tok::Comma => "\",\"".into(),
            Tok::Dot => "\".\"".into(),
            Tok::If => "\":-\"".into(),
            Tok::Ask => "\"?-\"".into(),
            Tok::Not => "\"!\"".into(),
            Tok::Eq => "\"=\"".into(),
            Tok::Ne => "\"!=\"".into(),
            Tok::Lt => "\"<\"".into(),
            Tok::Le => "\"<=\"".into(),
            Tok::Gt => "\">\"".into(),
            Tok::Ge => "\">=\"".into(),
            Tok::Doc(_) => "a doc comment".into(),
            Tok::End => "the end of the program".into(),
        }
    }
}

fn err(span: Span, message: impl Into<String>) -> Error {
    Error {
        span,
        message: message.into(),
    }
}

fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn lex(source: &str) -> Result<Vec<(Tok, Span)>, Error> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = vec![];
    let (mut i, mut line, mut col) = (0, 1, 1);
    // Moves past n characters, keeping the line and column.
    let advance = |i: &mut usize, line: &mut usize, col: &mut usize, n: usize| {
        for _ in 0..n {
            if chars[*i] == '\n' {
                *line += 1;
                *col = 1;
            } else {
                *col += 1;
            }
            *i += 1;
        }
    };
    while i < chars.len() {
        let c = chars[i];
        let span = Span { line, column: col };
        if c.is_whitespace() {
            advance(&mut i, &mut line, &mut col, 1);
            continue;
        }
        if c == '%' {
            // A doc comment is %% first on its line; anywhere else, and with
            // one % at the start, it is a comment like any other.
            let first_on_line =
                col == 1 || chars[i - (col - 1)..i].iter().all(|c| c.is_whitespace());
            let doc = first_on_line && chars.get(i + 1) == Some(&'%');
            let start = i;
            while i < chars.len() && chars[i] != '\n' {
                advance(&mut i, &mut line, &mut col, 1);
            }
            if doc {
                let text: String = chars[start + 2..i].iter().collect();
                let text = text.strip_prefix(' ').unwrap_or(&text).trim_end();
                out.push((Tok::Doc(text.to_string()), span));
            }
            continue;
        }
        let next = chars.get(i + 1).copied();
        let (tok, len) = match c {
            '(' => (Tok::LParen, 1),
            ')' => (Tok::RParen, 1),
            ',' => (Tok::Comma, 1),
            '.' => (Tok::Dot, 1),
            '=' => (Tok::Eq, 1),
            '<' if next == Some('=') => (Tok::Le, 2),
            '<' => (Tok::Lt, 1),
            '>' if next == Some('=') => (Tok::Ge, 2),
            '>' => (Tok::Gt, 1),
            ':' if next == Some('-') => (Tok::If, 2),
            '?' if next == Some('-') => (Tok::Ask, 2),
            '!' if next == Some('=') => (Tok::Ne, 2),
            '!' => (Tok::Not, 1),
            '"' => {
                let mut s = String::new();
                let mut j = i + 1;
                loop {
                    match chars.get(j) {
                        None | Some('\n') => return Err(err(span, "a string is not closed")),
                        Some('"') => break,
                        Some('\\') => {
                            s.push(match chars.get(j + 1) {
                                Some('"') => '"',
                                Some('\\') => '\\',
                                Some('n') => '\n',
                                Some('t') => '\t',
                                _ => {
                                    let at = Span {
                                        line,
                                        column: col + (j - i),
                                    };
                                    return Err(err(
                                        at,
                                        "a string escapes only \\\", \\\\, \\n and \\t",
                                    ));
                                }
                            });
                            j += 2;
                        }
                        Some(&c) => {
                            s.push(c);
                            j += 1;
                        }
                    }
                }
                (Tok::Str(s), j + 1 - i)
            }
            '@' => {
                let n = chars[i + 1..]
                    .iter()
                    .take_while(|&&c| word_char(c) || c == '-')
                    .count();
                if n == 0 {
                    return Err(err(span, "@ is followed by the name it refers to"));
                }
                let r: String = chars[i + 1..i + 1 + n].iter().collect();
                (Tok::Reference(r), n + 1)
            }
            c if c.is_ascii_digit() || c == '-' && next.is_some_and(|d| d.is_ascii_digit()) => {
                let n = 1 + chars[i + 1..]
                    .iter()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                let text: String = chars[i..i + n].iter().collect();
                if chars.get(i + n).is_some_and(|&c| word_char(c)) {
                    return Err(err(span, format!("{text} is followed by a letter")));
                }
                let v = text
                    .parse::<i64>()
                    .map_err(|_| err(span, format!("{text} does not fit in 64 bits")))?;
                (Tok::Int(v), n)
            }
            c if word_char(c) => {
                let n = chars[i..].iter().take_while(|&&c| word_char(c)).count();
                let w: String = chars[i..i + n].iter().collect();
                let tok = if w == "_" {
                    Tok::Wildcard
                } else if c.is_ascii_uppercase() || c == '_' {
                    Tok::Variable(w)
                } else if c.is_ascii_lowercase() {
                    Tok::Name(w)
                } else {
                    return Err(err(span, format!("{w} begins with neither a letter nor _")));
                };
                (tok, n)
            }
            c => return Err(err(span, format!("{c:?} is not part of the language"))),
        };
        out.push((tok, span));
        advance(&mut i, &mut line, &mut col, len);
    }
    out.push((Tok::End, Span { line, column: col }));
    Ok(out)
}

struct Parser {
    tokens: Vec<(Tok, Span)>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> &(Tok, Span) {
        &self.tokens[self.at]
    }

    fn next(&mut self) -> (Tok, Span) {
        let t = self.tokens[self.at].clone();
        if t.0 != Tok::End {
            self.at += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if &self.peek().0 == tok {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: Tok, wanted: &str) -> Result<(), Error> {
        let (t, span) = self.next();
        if t == tok {
            Ok(())
        } else {
            Err(err(
                span,
                format!("expected {wanted}, found {}", t.describe()),
            ))
        }
    }

    fn program(mut self) -> Result<Program, Error> {
        let mut clauses = vec![];
        let mut docs = vec![];
        while self.peek().0 != Tok::End {
            docs.push(self.doc()?);
            clauses.push(self.clause()?);
        }
        Ok(Program { clauses, docs })
    }

    /// The doc comment above the next clause, if it has one: `%%` lines on
    /// consecutive lines, the last of them directly above the clause.
    fn doc(&mut self) -> Result<Option<Doc>, Error> {
        let Tok::Doc(_) = self.peek().0 else {
            return Ok(None);
        };
        let span = self.peek().1;
        let mut lines: Vec<String> = Vec::new();
        let mut last = span.line;
        while let (Tok::Doc(text), at) = self.peek().clone() {
            if at.line != last + usize::from(!lines.is_empty()) {
                break;
            }
            lines.push(text);
            last = at.line;
            self.at += 1;
        }
        let (next, at) = self.peek().clone();
        if matches!(next, Tok::End | Tok::Doc(_)) || at.line != last + 1 {
            return Err(err(
                span,
                "a doc comment documents the clause on the line directly under it, and there is none",
            ));
        }
        let title = lines[0].trim().to_string();
        if title.is_empty() {
            return Err(err(
                span,
                "a doc comment begins with its title, on its first line",
            ));
        }
        // The lines after the title are the description: wrapped prose, so a
        // line break is a space, and an empty %% line is a paragraph break.
        let mut paragraphs: Vec<String> = Vec::new();
        let mut current: Vec<&str> = Vec::new();
        for l in &lines[1..] {
            if l.trim().is_empty() {
                if !current.is_empty() {
                    paragraphs.push(current.join(" "));
                    current.clear();
                }
            } else {
                current.push(l.trim());
            }
        }
        if !current.is_empty() {
            paragraphs.push(current.join(" "));
        }
        Ok(Some(Doc {
            title,
            description: paragraphs.join("\n\n"),
            span,
        }))
    }

    fn clause(&mut self) -> Result<Clause, Error> {
        let span = self.peek().1;
        if self.eat(&Tok::Ask) {
            let body = self.body()?;
            return Ok(Clause::Query(Query {
                name: None,
                body,
                span,
            }));
        }
        let head = self.atom()?;
        let (t, span) = self.next();
        match t {
            Tok::Dot => Ok(Clause::Fact(head)),
            Tok::If => Ok(Clause::Rule(Rule {
                head,
                body: self.body()?,
            })),
            Tok::Ask => Ok(Clause::Query(Query {
                span: head.span,
                name: Some(head),
                body: self.body()?,
            })),
            t => Err(err(
                span,
                format!("expected \".\", \":-\" or \"?-\", found {}", t.describe()),
            )),
        }
    }

    /// Literals separated by commas, up to and past the full stop.
    fn body(&mut self) -> Result<Vec<Literal>, Error> {
        let mut out = vec![self.literal()?];
        loop {
            let (t, span) = self.next();
            match t {
                Tok::Comma => out.push(self.literal()?),
                Tok::Dot => return Ok(out),
                t => {
                    return Err(err(
                        span,
                        format!("expected \",\" or \".\", found {}", t.describe()),
                    ));
                }
            }
        }
    }

    fn literal(&mut self) -> Result<Literal, Error> {
        if self.eat(&Tok::Not) {
            return Ok(Literal::Negative(self.atom()?));
        }
        // A name followed by a comparison is a symbol being compared; a name
        // otherwise is an atom.
        let compares =
            |t: &Tok| matches!(t, Tok::Eq | Tok::Ne | Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge);
        let starts_atom = matches!(self.peek().0, Tok::Name(_))
            && !self.tokens.get(self.at + 1).is_some_and(|t| compares(&t.0));
        if starts_atom {
            return Ok(Literal::Positive(self.atom()?));
        }
        let left = self.term()?;
        let (t, span) = self.next();
        let op = match t {
            Tok::Eq => Op::Eq,
            Tok::Ne => Op::Ne,
            Tok::Lt => Op::Lt,
            Tok::Le => Op::Le,
            Tok::Gt => Op::Gt,
            Tok::Ge => Op::Ge,
            t => {
                return Err(err(
                    span,
                    format!(
                        "expected \"=\", \"!=\", \"<\", \"<=\", \">\" or \">=\", found {}",
                        t.describe()
                    ),
                ));
            }
        };
        let right = self.term()?;
        Ok(Literal::Compare(Comparison { left, op, right }))
    }

    fn atom(&mut self) -> Result<Atom, Error> {
        let (t, span) = self.next();
        let Tok::Name(relation) = t else {
            return Err(err(
                span,
                format!("expected a relation, found {}", t.describe()),
            ));
        };
        let mut terms = vec![];
        if self.eat(&Tok::LParen) {
            terms.push(self.term()?);
            while self.eat(&Tok::Comma) {
                terms.push(self.term()?);
            }
            self.expect(Tok::RParen, "\",\" or \")\"")?;
        }
        Ok(Atom {
            relation,
            terms,
            span,
        })
    }

    fn term(&mut self) -> Result<Term, Error> {
        let (t, span) = self.next();
        let kind = match t {
            Tok::Variable(v) => TermKind::Variable(v),
            Tok::Wildcard => TermKind::Wildcard,
            Tok::Name(s) | Tok::Str(s) => TermKind::Constant(Constant::Symbol(s)),
            Tok::Int(n) => TermKind::Constant(Constant::Integer(n)),
            Tok::Reference(r) => TermKind::Constant(Constant::Reference(r)),
            t => {
                return Err(err(
                    span,
                    format!("expected a term, found {}", t.describe()),
                ));
            }
        };
        Ok(Term { kind, span })
    }
}
