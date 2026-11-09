//! Datalog evaluated: a program, read by `datalog-syntax`, evaluated against
//! relations a host holds, by datafrog. Everything `datalog-syntax` exports is
//! exported here too, so a host takes this crate alone.
//!
//! datafrog evaluates and does not read: its rules are Rust, over tuple types
//! fixed when it is compiled. A program here arrives as text, so this is the
//! compiler between the two — every value interned to a number, every tuple a
//! row of them, every body compiled into a chain of joins keyed on the
//! variables it shares with what came before it. What datafrog contributes is
//! the semi-naive iteration to a fixed point, which is the part worth not
//! writing twice.
//!
//! A program is evaluated in strata, by negation: datafrog anti-joins only
//! against a relation already complete, so a relation is evaluated to its
//! fixed point before anything negates it, and a negation inside a recursion
//! has no such order and is refused. The relations are whatever the host put
//! in the database, and what a reference names is whatever the host says.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use datafrog::{ExtendWith, Iteration, PrefixFilter, Relation, Variable};
pub use datalog_syntax::*;

/// A value a relation holds: what a program's constants become once a
/// reference is resolved. Integers sort before symbols, so an answer is in one
/// order whatever it holds.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Integer(i64),
    Symbol(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(n) => write!(f, "{n}"),
            Value::Symbol(s) => f.write_str(s),
        }
    }
}

type Tuple = Vec<u32>;
type Keyed = (Vec<u32>, Vec<u32>);

/// One relation: what its columns are called, where it came from, what one of
/// its rows says, and its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub columns: Vec<String>,
    pub origin: String,
    /// One row said as a sentence, and more about it — the host's words for a
    /// relation it declares, the doc comment above the first clause for one a
    /// program defines. None where nothing was written.
    pub title: Option<String>,
    pub description: Option<String>,
    rows: Vec<Tuple>,
}

/// Every relation a program can be asked against, and the values they hold.
#[derive(Debug, Clone, Default)]
pub struct Database {
    values: Vec<Value>,
    ids: HashMap<Value, u32>,
    relations: BTreeMap<String, Held>,
}

fn err(span: Span, message: impl Into<String>) -> Error {
    Error {
        span,
        message: message.into(),
    }
}

impl Database {
    fn intern(&mut self, v: Value) -> u32 {
        if let Some(&i) = self.ids.get(&v) {
            return i;
        }
        let i = u32::try_from(self.values.len()).expect("fewer than four billion values");
        self.values.push(v.clone());
        self.ids.insert(v, i);
        i
    }

    /// Declares a relation with no rows yet: its columns name it, origin is
    /// what a program trying to define it again is told it is, and title is
    /// what one of its rows says.
    pub fn declare(&mut self, name: &str, columns: &[&str], origin: &str, title: &str) {
        self.relations.insert(
            name.to_string(),
            Held {
                columns: columns.iter().map(|c| c.to_string()).collect(),
                origin: origin.to_string(),
                title: Some(title.to_string()),
                description: None,
                rows: Vec::new(),
            },
        );
    }

    /// Adds one row of symbols to a declared relation.
    pub fn add(&mut self, name: &str, row: &[&str]) {
        let t: Tuple = row
            .iter()
            .map(|s| self.intern(Value::Symbol(s.to_string())))
            .collect();
        let held = self.relations.get_mut(name).expect("declared first");
        debug_assert_eq!(held.columns.len(), t.len());
        held.rows.push(t);
    }

    /// Adds one row of values to a declared relation: what [`Database::add`]
    /// does, for a host whose rows hold integers as well as symbols.
    pub fn add_values(&mut self, name: &str, row: &[Value]) {
        let t: Tuple = row.iter().map(|v| self.intern(v.clone())).collect();
        let held = self.relations.get_mut(name).expect("declared first");
        debug_assert_eq!(held.columns.len(), t.len());
        held.rows.push(t);
    }

    pub fn relations(&self) -> &BTreeMap<String, Held> {
        &self.relations
    }

    /// A relation's rows as values, sorted column by column.
    pub fn rows(&self, name: &str) -> Vec<Vec<Value>> {
        let mut rows: Vec<Vec<Value>> = self.relations[name]
            .rows
            .iter()
            .map(|t| t.iter().map(|&i| self.values[i as usize].clone()).collect())
            .collect();
        rows.sort();
        rows.dedup();
        rows
    }
}

/// What a query was answered with: its text written back, a column per
/// variable it binds, and the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub query: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// Evaluates a program into the database: its facts and rules become
/// relations of the given origin, and each of its queries an answer.
///
/// A relation the database already holds cannot be defined again — origin
/// says by whom — so everything already there is fixed, and only what this
/// program defines is evaluated. `resolve` turns a reference into the value it
/// names, or says why it names none.
pub fn evaluate(
    db: &mut Database,
    program: &Program,
    origin: &str,
    resolve: &dyn Fn(&str) -> Result<Value, String>,
) -> Result<Vec<Answer>, Error> {
    let mut rules: Vec<(Atom, Vec<Literal>)> = Vec::new();
    let mut facts: Vec<&Atom> = Vec::new();
    let mut queries = Vec::new();
    for c in &program.clauses {
        match c {
            Clause::Fact(a) => facts.push(a),
            Clause::Rule(r) => rules.push((r.head.clone(), r.body.clone())),
            Clause::Query(q) => {
                let n = queries.len();
                let columns: Vec<String> = q.variables().iter().map(|v| v.to_string()).collect();
                let head = Atom {
                    relation: format!("?{n}"),
                    terms: columns
                        .iter()
                        .map(|v| Term {
                            kind: TermKind::Variable(v.clone()),
                            span: q.span,
                        })
                        .collect(),
                    span: q.span,
                };
                rules.push((head, q.body.clone()));
                queries.push((q.to_string(), columns));
            }
        }
    }

    // What this program defines, and what each is called: refused where the
    // database already holds it.
    let mut defined: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for head in facts.iter().copied().chain(rules.iter().map(|(h, _)| h)) {
        if let Some(held) = db.relations.get(&head.relation) {
            return Err(err(
                head.span,
                format!(
                    "{} is {}'s, and is not defined again: a new relation is a new name",
                    head.relation, held.origin
                ),
            ));
        }
        let names = defined.entry(head.relation.clone()).or_default();
        for (i, t) in head.terms.iter().enumerate() {
            if names.len() <= i {
                names.push(String::new());
            }
            if names[i].is_empty()
                && let TermKind::Variable(v) = &t.kind
            {
                names[i] = v.clone();
            }
        }
    }

    // Every relation a body uses is held or defined here, with the columns it
    // has. The language already checked that this program writes each with one
    // number of terms; this is that number against the one it is held with.
    for (_, body) in &rules {
        for l in body {
            let (Literal::Positive(a) | Literal::Negative(a)) = l else {
                continue;
            };
            let has = match (db.relations.get(&a.relation), defined.get(&a.relation)) {
                (Some(held), _) => held.columns.len(),
                (None, Some(cols)) => cols.len(),
                (None, None) => {
                    return Err(err(
                        a.span,
                        format!(
                            "{} is no relation: nothing provides or defines it",
                            a.relation
                        ),
                    ));
                }
            };
            if has != a.terms.len() {
                return Err(err(
                    a.span,
                    format!(
                        "{} has {} and is written here with {}",
                        a.relation,
                        count_columns(has),
                        count_columns(a.terms.len())
                    ),
                ));
            }
        }
    }

    // Every reference, resolved once, before anything is evaluated: a program
    // naming a value that is not there is refused whether or not the clause
    // naming it would have mattered.
    let mut constant = |db: &mut Database, t: &Term| -> Result<Option<u32>, Error> {
        match &t.kind {
            TermKind::Constant(Constant::Symbol(s)) => {
                Ok(Some(db.intern(Value::Symbol(s.clone()))))
            }
            TermKind::Constant(Constant::Integer(n)) => Ok(Some(db.intern(Value::Integer(*n)))),
            TermKind::Constant(Constant::Reference(r)) => {
                let v = resolve(r).map_err(|m| err(t.span, m))?;
                Ok(Some(db.intern(v)))
            }
            _ => Ok(None),
        }
    };

    let documented = program.defined();
    let doc = |name: &str| {
        documented
            .iter()
            .find(|d| d.relation == name)
            .and_then(|d| d.doc)
    };
    for (name, cols) in &defined {
        let columns = cols
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if c.is_empty() {
                    format!("_{}", i + 1)
                } else {
                    c.clone()
                }
            })
            .collect();
        db.relations.insert(
            name.clone(),
            Held {
                columns,
                origin: origin.to_string(),
                title: doc(name).map(|d| d.title.clone()),
                description: doc(name)
                    .map(|d| d.description.clone())
                    .filter(|d| !d.is_empty()),
                rows: Vec::new(),
            },
        );
    }
    for f in facts {
        let mut row = Vec::new();
        for t in &f.terms {
            row.push(constant(db, t)?.expect("a fact is values"));
        }
        db.relations.get_mut(&f.relation).unwrap().rows.push(row);
    }

    let mut compiled = Vec::new();
    for (head, body) in &rules {
        compiled.push(compile(db, head, body, &mut constant)?);
    }

    for stratum in strata(&rules, &defined)? {
        run(
            db,
            &stratum,
            compiled.iter().filter(|r| stratum.contains(&r.head)),
        );
    }

    let mut answers = Vec::new();
    for (n, (query, columns)) in queries.into_iter().enumerate() {
        let name = format!("?{n}");
        let rows = db.rows(&name);
        db.relations.remove(&name);
        answers.push(Answer {
            query,
            columns,
            rows,
        });
    }
    Ok(answers)
}

/// The atom a literal reads, if it reads one, with whether the literal's rule
/// must stand above it: it must above one it negates.
fn reads(l: &Literal) -> Option<(bool, &Atom)> {
    match l {
        Literal::Positive(a) => Some((false, a)),
        Literal::Negative(a) => Some((true, a)),
        Literal::Compare(_) => None,
    }
}

fn count_columns(n: usize) -> String {
    match n {
        1 => "1 column".into(),
        n => format!("{n} columns"),
    }
}

/// The relations this program defines, in the order they can be evaluated:
/// each above everything it negates, and level with what it only uses.
fn strata(
    rules: &[(Atom, Vec<Literal>)],
    defined: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<BTreeSet<String>>, Error> {
    let mut level: BTreeMap<&str, usize> = defined.keys().map(|p| (p.as_str(), 0)).collect();
    // A level can only rise as far as there are relations to stand on; one
    // still rising past that is standing on itself through a negation.
    for _ in 0..=defined.len() {
        let mut moved = None;
        for (head, body) in rules {
            for (neg, a) in body.iter().flat_map(reads) {
                let Some(&below) = level.get(a.relation.as_str()) else {
                    continue;
                };
                let need = below + usize::from(neg);
                if level[head.relation.as_str()] < need {
                    level.insert(&head.relation, need);
                    moved = Some((head, a));
                }
            }
        }
        if moved.is_none() {
            let top = level.values().copied().max().unwrap_or(0);
            return Ok((0..=top)
                .map(|n| {
                    level
                        .iter()
                        .filter(|(_, l)| **l == n)
                        .map(|(p, _)| p.to_string())
                        .collect()
                })
                .collect());
        }
    }
    // Name a negation inside the cycle: one whose relation stands, through
    // the rules, on the head negating it.
    for (head, body) in rules {
        for l in body {
            if let Literal::Negative(a) = l
                && defined.contains_key(&a.relation)
                && reaches(rules, &a.relation, &head.relation)
            {
                let why = if a.relation == head.relation {
                    format!("{} is defined by its own negation", a.relation)
                } else {
                    format!(
                        "{} stands on {} through its rules, so neither can be finished first",
                        a.relation, head.relation
                    )
                };
                return Err(err(
                    a.span,
                    format!("{} is negated inside its own recursion: {why}", a.relation),
                ));
            }
        }
    }
    unreachable!("a level that kept rising is a negation in a cycle")
}

/// Whether relation `from` uses `to`, through any number of rules.
fn reaches(rules: &[(Atom, Vec<Literal>)], from: &str, to: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut work = vec![from.to_string()];
    while let Some(r) = work.pop() {
        if r == to {
            return true;
        }
        if !seen.insert(r.clone()) {
            continue;
        }
        for (_, body) in rules.iter().filter(|(h, _)| h.relation == r) {
            for (_, a) in body.iter().flat_map(reads) {
                work.push(a.relation.clone());
            }
        }
    }
    false
}

/// A condition on one tuple: a position holding a value, the same value as
/// another position, or a value before or after another.
#[derive(Debug, Clone)]
enum Check {
    Is(usize, u32),
    Same(usize, usize),
    Differs(usize, usize),
    IsNot(usize, u32),
    Order(Side, Op, Side),
}

/// One side of a comparison: a position in the tuple, or a value.
#[derive(Debug, Clone, Copy)]
enum Side {
    At(usize),
    Value(u32),
}

/// Whether `a op b` holds, in the order values sort in.
fn ordered(values: &[Value], a: u32, op: Op, b: u32) -> bool {
    let o = values[a as usize].cmp(&values[b as usize]);
    match op {
        Op::Eq => o.is_eq(),
        Op::Ne => o.is_ne(),
        Op::Lt => o.is_lt(),
        Op::Le => o.is_le(),
        Op::Gt => o.is_gt(),
        Op::Ge => o.is_ge(),
    }
}

fn pass(t: &Tuple, checks: &[Check], values: &[Value]) -> bool {
    let side = |s: Side| match s {
        Side::At(i) => t[i],
        Side::Value(v) => v,
    };
    checks.iter().all(|c| match *c {
        Check::Is(i, v) => t[i] == v,
        Check::IsNot(i, v) => t[i] != v,
        Check::Same(i, j) => t[i] == t[j],
        Check::Differs(i, j) => t[i] != t[j],
        Check::Order(a, op, b) => ordered(values, side(a), op, side(b)),
    })
}

fn pick(t: &Tuple, at: &[usize]) -> Vec<u32> {
    at.iter().map(|&i| t[i]).collect()
}

/// One atom read for a join: which of its positions must hold what, and where
/// each of its variables is first.
struct Shape {
    checks: Vec<Check>,
    first: BTreeMap<String, usize>,
}

fn shape(
    db: &mut Database,
    a: &Atom,
    constant: &mut impl FnMut(&mut Database, &Term) -> Result<Option<u32>, Error>,
) -> Result<Shape, Error> {
    let mut checks = Vec::new();
    let mut first = BTreeMap::new();
    for (i, t) in a.terms.iter().enumerate() {
        if let Some(v) = constant(db, t)? {
            checks.push(Check::Is(i, v));
        } else if let TermKind::Variable(v) = &t.kind {
            match first.get(v) {
                Some(&j) => checks.push(Check::Same(i, j)),
                None => {
                    first.insert(v.clone(), i);
                }
            }
        }
    }
    Ok(Shape { checks, first })
}

/// A rule as the steps evaluating it, by position: what is joined with what,
/// and which column of the bindings each variable ends up in.
struct Compiled {
    head: String,
    /// A comparison of two values that is false: the rule derives nothing.
    never: bool,
    atoms: Vec<Joined>,
    negations: Vec<Negated>,
    comparisons: Vec<Check>,
    /// Each head term: the column of the bindings it is, or the value it is.
    make: Vec<Result<usize, u32>>,
}

/// One positive atom: filtered by its checks, keyed by the variables bound
/// before it, the rest of its variables carried on.
struct Joined {
    relation: String,
    checks: Vec<Check>,
    /// Positions in the atom, and in the bindings so far, of the shared
    /// variables; positions in the atom of the new ones.
    key: Vec<usize>,
    key_in_bindings: Vec<usize>,
    rest_in_bindings: Vec<usize>,
    new: Vec<usize>,
}

/// One negated atom: the positions it is keyed on in the bindings, and the
/// relation projected onto those, filtered by its constants.
struct Negated {
    relation: String,
    checks: Vec<Check>,
    project: Vec<usize>,
    key_in_bindings: Vec<usize>,
    rest_in_bindings: Vec<usize>,
}

fn compile(
    db: &mut Database,
    head: &Atom,
    body: &[Literal],
    constant: &mut impl FnMut(&mut Database, &Term) -> Result<Option<u32>, Error>,
) -> Result<Compiled, Error> {
    // What each column of the bindings is, as the joins build them up: the
    // shared variables first, then the earlier ones not shared, then the new.
    let mut layout: Vec<String> = Vec::new();
    let mut atoms = Vec::new();
    for l in body {
        let Literal::Positive(a) = l else { continue };
        let s = shape(db, a, constant)?;
        let shared: Vec<&String> = s.first.keys().filter(|v| layout.contains(v)).collect();
        let new: Vec<&String> = s.first.keys().filter(|v| !layout.contains(v)).collect();
        let key_in_bindings: Vec<usize> = shared
            .iter()
            .map(|v| layout.iter().position(|x| x == *v).unwrap())
            .collect();
        let rest_in_bindings: Vec<usize> = (0..layout.len())
            .filter(|i| !key_in_bindings.contains(i))
            .collect();
        let mut next: Vec<String> = shared.iter().map(|v| (*v).clone()).collect();
        next.extend(rest_in_bindings.iter().map(|&i| layout[i].clone()));
        next.extend(new.iter().map(|v| (*v).clone()));
        atoms.push(Joined {
            relation: a.relation.clone(),
            checks: s.checks,
            key: shared.iter().map(|v| s.first[*v]).collect(),
            key_in_bindings,
            rest_in_bindings,
            new: new.iter().map(|v| s.first[*v]).collect(),
        });
        layout = next;
    }

    let mut negations = Vec::new();
    for l in body {
        let Literal::Negative(a) = l else { continue };
        let s = shape(db, a, constant)?;
        let names: Vec<&String> = s.first.keys().collect();
        let key_in_bindings: Vec<usize> = names
            .iter()
            .map(|v| layout.iter().position(|x| x == *v).unwrap())
            .collect();
        let rest_in_bindings: Vec<usize> = (0..layout.len())
            .filter(|i| !key_in_bindings.contains(i))
            .collect();
        let mut next: Vec<String> = key_in_bindings.iter().map(|&i| layout[i].clone()).collect();
        next.extend(rest_in_bindings.iter().map(|&i| layout[i].clone()));
        negations.push(Negated {
            relation: a.relation.clone(),
            checks: s.checks,
            project: names.iter().map(|v| s.first[*v]).collect(),
            key_in_bindings,
            rest_in_bindings,
        });
        layout = next;
    }

    // Comparisons last, against the bindings as they stand at the end. A side
    // that is a value compares a column with it; two values compare now, and
    // one that is false whatever is bound leaves the rule nothing to derive.
    let mut comparisons = Vec::new();
    let mut never = false;
    let col = |t: &Term| match &t.kind {
        TermKind::Variable(v) => layout.iter().position(|x| x == v),
        _ => None,
    };
    for l in body {
        let Literal::Compare(c) = l else { continue };
        let left = (col(&c.left), constant(db, &c.left)?);
        let right = (col(&c.right), constant(db, &c.right)?);
        let eq = c.op == Op::Eq;
        if !matches!(c.op, Op::Eq | Op::Ne) {
            let side = |s: (Option<usize>, Option<u32>)| match s {
                (Some(i), _) => Side::At(i),
                (_, Some(v)) => Side::Value(v),
                _ => unreachable!("the language binds every compared variable"),
            };
            match (side(left), side(right)) {
                (Side::Value(a), Side::Value(b)) => never |= !ordered(&db.values, a, c.op, b),
                (a, b) => comparisons.push(Check::Order(a, c.op, b)),
            }
            continue;
        }
        comparisons.push(match (left, right) {
            ((Some(i), _), (Some(j), _)) => {
                if eq {
                    Check::Same(i, j)
                } else {
                    Check::Differs(i, j)
                }
            }
            ((Some(i), _), (_, Some(v))) | ((_, Some(v)), (Some(i), _)) => {
                if eq {
                    Check::Is(i, v)
                } else {
                    Check::IsNot(i, v)
                }
            }
            ((_, Some(a)), (_, Some(b))) => {
                never |= (a == b) != eq;
                continue;
            }
            _ => unreachable!("the language binds every compared variable"),
        });
    }

    let mut make = Vec::new();
    for t in &head.terms {
        make.push(match constant(db, t)? {
            Some(v) => Err(v),
            None => {
                let TermKind::Variable(v) = &t.kind else {
                    unreachable!("the language refuses _ in a head")
                };
                Ok(layout.iter().position(|x| x == v).unwrap())
            }
        });
    }
    Ok(Compiled {
        head: head.relation.clone(),
        never,
        atoms,
        negations,
        comparisons,
        make,
    })
}

/// One step of a stratum, run every round until nothing changes.
enum Step {
    /// A relation of this stratum's, filtered and keyed for a join.
    Key {
        relation: String,
        checks: Vec<Check>,
        key: Vec<usize>,
        val: Vec<usize>,
        out: Variable<Keyed>,
    },
    Rekey {
        from: Variable<Tuple>,
        key: Vec<usize>,
        val: Vec<usize>,
        out: Variable<Keyed>,
    },
    Join {
        left: Variable<Keyed>,
        right: Variable<Keyed>,
        out: Variable<Tuple>,
    },
    Anti {
        left: Variable<Keyed>,
        right: Relation<Tuple>,
        out: Variable<Tuple>,
    },
    Filter {
        from: Variable<Tuple>,
        checks: Vec<Check>,
        out: Variable<Tuple>,
    },
    Head {
        from: Variable<Tuple>,
        make: Vec<Result<usize, u32>>,
        relation: String,
    },
}

fn run<'a>(
    db: &mut Database,
    stratum: &BTreeSet<String>,
    rules: impl Iterator<Item = &'a Compiled>,
) {
    let mut it = Iteration::new();
    let mut vars: BTreeMap<String, Variable<Tuple>> = BTreeMap::new();
    for name in stratum {
        let v = it.variable::<Tuple>(name);
        // A relation's facts, written in the program, are where it starts.
        v.extend(db.relations[name].rows.iter().cloned());
        vars.insert(name.clone(), v);
    }

    // datafrog's leapjoin wants something that proposes values, and a filter
    // only removes them; a relation of one row proposes the one value there is
    // to every tuple, so filtering one of this stratum's relations is a
    // leapjoin through it.
    let unit: Relation<((), ())> = Relation::from_vec(vec![((), ())]);

    let mut steps = Vec::new();
    for r in rules {
        if r.never {
            continue;
        }
        let mut bindings: Option<Variable<Tuple>> = None;
        for a in &r.atoms {
            let keyed = it.variable::<Keyed>("atom");
            if vars.contains_key(&a.relation) {
                steps.push(Step::Key {
                    relation: a.relation.clone(),
                    checks: a.checks.clone(),
                    key: a.key.clone(),
                    val: a.new.clone(),
                    out: keyed.clone(),
                });
            } else {
                // Held already, so complete: filtered and keyed once, here.
                keyed.extend(
                    db.relations[&a.relation]
                        .rows
                        .iter()
                        .filter(|t| pass(t, &a.checks, &db.values))
                        .map(|t| (pick(t, &a.key), pick(t, &a.new))),
                );
            }
            let left = it.variable::<Keyed>("bindings");
            match bindings.take() {
                // Nothing bound yet: the one empty binding, joined with all of it.
                None => left.extend([(Vec::new(), Vec::new())]),
                Some(from) => steps.push(Step::Rekey {
                    from,
                    key: a.key_in_bindings.clone(),
                    val: a.rest_in_bindings.clone(),
                    out: left.clone(),
                }),
            }
            let out = it.variable::<Tuple>("joined");
            steps.push(Step::Join {
                left,
                right: keyed,
                out: out.clone(),
            });
            bindings = Some(out);
        }
        let mut bindings = bindings.expect("the language gives every body a positive atom");
        for n in &r.negations {
            // Negated relations are below this stratum, so complete.
            let right: Relation<Tuple> = db.relations[&n.relation]
                .rows
                .iter()
                .filter(|t| pass(t, &n.checks, &db.values))
                .map(|t| pick(t, &n.project))
                .collect();
            let left = it.variable::<Keyed>("negating");
            steps.push(Step::Rekey {
                from: bindings,
                key: n.key_in_bindings.clone(),
                val: n.rest_in_bindings.clone(),
                out: left.clone(),
            });
            let out = it.variable::<Tuple>("kept");
            steps.push(Step::Anti {
                left,
                right,
                out: out.clone(),
            });
            bindings = out;
        }
        if !r.comparisons.is_empty() {
            let out = it.variable::<Tuple>("compared");
            steps.push(Step::Filter {
                from: bindings,
                checks: r.comparisons.clone(),
                out: out.clone(),
            });
            bindings = out;
        }
        steps.push(Step::Head {
            from: bindings,
            make: r.make.clone(),
            relation: r.head.clone(),
        });
    }

    let values = &db.values;
    while it.changed() {
        for s in &steps {
            match s {
                Step::Key {
                    relation,
                    checks,
                    key,
                    val,
                    out,
                } => out.from_leapjoin(
                    &vars[relation],
                    (
                        ExtendWith::from(&unit, |_: &Tuple| ()),
                        PrefixFilter::from(|t: &Tuple| pass(t, checks, values)),
                    ),
                    |t, _| (pick(t, key), pick(t, val)),
                ),
                Step::Rekey {
                    from,
                    key,
                    val,
                    out,
                } => out.from_map(from, |t| (pick(t, key), pick(t, val))),
                Step::Join { left, right, out } => out.from_join(left, right, |k, a, b| {
                    let mut t = k.clone();
                    t.extend(a);
                    t.extend(b);
                    t
                }),
                Step::Anti { left, right, out } => out.from_antijoin(left, right, |k, a| {
                    let mut t = k.clone();
                    t.extend(a);
                    t
                }),
                Step::Filter { from, checks, out } => out.from_leapjoin(
                    from,
                    (
                        ExtendWith::from(&unit, |_: &Tuple| ()),
                        PrefixFilter::from(|t: &Tuple| pass(t, checks, values)),
                    ),
                    |t, _| t.clone(),
                ),
                Step::Head {
                    from,
                    make,
                    relation,
                } => vars[relation].from_map(from, |t| {
                    make.iter()
                        .map(|m| match m {
                            Ok(i) => t[*i],
                            Err(v) => *v,
                        })
                        .collect()
                }),
            }
        }
    }
    for (name, v) in vars {
        db.relations.get_mut(&name).unwrap().rows = v.complete().elements;
    }
}
