//! The cases under cases/ read, and each put through the datalog crate the way
//! a host puts a program through it: `parse`, then `evaluate` against a
//! database holding what the case says a host holds.
//!
//! A case says what should come back, either the answers or the refusal, and
//! running it records what did. Nothing here decides what the language
//! means. The READMEs of datalog-syntax and datalog are that, and a case is a claim about it that
//! somebody wrote down. So a case that fails is a report about the crate,
//! until somebody shows the case misread the README.
//!
//! A case file is strict: a key it does not know is refused, so a misspelled
//! expectation cannot pass by expecting nothing.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use datalog::{Database, Value};
use toml::{Table, Value as Toml};

/// One file of cases: a part of the language, the README's words for it, and
/// the cases that hold the crate to those words.
#[derive(Debug, Clone)]
pub struct Feature {
    pub file: String,
    pub title: String,
    /// The heading of the README the cases bear on: datalog-syntax's or
    /// datalog's.
    pub readme: String,
    pub description: String,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    /// Evaluated one after another into one database, as layers.
    pub programs: Vec<String>,
    /// The relations a host holds before the first program, each with its
    /// rows.
    pub held: BTreeMap<String, Vec<Vec<Value>>>,
    /// What each `@reference` names; any other is refused by the host.
    pub references: BTreeMap<String, Value>,
    pub expect: Expect,
    /// Where the behaviour differs from Soufflé's, or why the case is here.
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// The rows of each query, in the order the queries are written, across
    /// every program.
    Answers(Vec<Vec<Vec<Value>>>),
    Refused {
        stage: Stage,
        /// `line:column`, when the case says where.
        at: Option<String>,
        /// Words the refusal's message holds.
        says: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Evaluate,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Parse => "parse",
            Stage::Evaluate => "evaluate",
        }
    }
}

/// What came back from the crate.
#[derive(Debug, Clone, PartialEq)]
pub enum Got {
    Answers(Vec<Answer>),
    Refused {
        stage: Stage,
        at: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub query: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// A case, what came back, and whether that was what it expected.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub got: Got,
    pub passed: bool,
}

/// Every file of cases in a directory, in the order of their names.
pub fn load(dir: &Path) -> Result<Vec<Feature>, String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let text = fs::read_to_string(p).map_err(|e| format!("{name}: {e}"))?;
            feature(&name, &text).map_err(|e| format!("{name}: {e}"))
        })
        .collect()
}

fn feature(file: &str, text: &str) -> Result<Feature, String> {
    let table: Table = text.parse().map_err(|e| format!("{e}"))?;
    only(
        &table,
        &["title", "readme", "description", "case"],
        "the file",
    )?;
    let cases = match table.get("case") {
        Some(Toml::Array(cases)) => cases
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let Toml::Table(c) = c else {
                    return Err(format!("case {} is not a table", i + 1));
                };
                case(c).map_err(|e| format!("case {}: {e}", i + 1))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("a file holds [[case]] tables".into()),
    };
    Ok(Feature {
        file: file.to_string(),
        title: string(&table, "title")?,
        readme: string(&table, "readme")?,
        description: string(&table, "description")?,
        cases,
    })
}

fn case(t: &Table) -> Result<Case, String> {
    only(
        t,
        &[
            "name",
            "program",
            "programs",
            "held",
            "references",
            "answers",
            "refused",
            "note",
        ],
        "a case",
    )?;
    let name = string(t, "name")?;
    let programs = match (t.get("program"), t.get("programs")) {
        (Some(Toml::String(p)), None) => vec![p.clone()],
        (None, Some(Toml::Array(ps))) => ps
            .iter()
            .map(|p| match p {
                Toml::String(s) => Ok(s.clone()),
                _ => Err("programs is a list of strings".to_string()),
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(format!("{name}: give program, or programs, and not both")),
    };
    let mut held = BTreeMap::new();
    if let Some(h) = t.get("held") {
        let Toml::Table(h) = h else {
            return Err(format!("{name}: held is a table of relations"));
        };
        for (relation, rows) in h {
            held.insert(
                relation.clone(),
                rows_of(rows).map_err(|e| format!("{name}: {e}"))?,
            );
        }
    }
    let mut references = BTreeMap::new();
    if let Some(r) = t.get("references") {
        let Toml::Table(r) = r else {
            return Err(format!("{name}: references is a table"));
        };
        for (k, v) in r {
            references.insert(k.clone(), value(v).map_err(|e| format!("{name}: {e}"))?);
        }
    }
    let expect = match (t.get("answers"), t.get("refused")) {
        (Some(Toml::Array(answers)), None) => Expect::Answers(
            answers
                .iter()
                .map(rows_of)
                .collect::<Result<_, _>>()
                .map_err(|e| format!("{name}: {e}"))?,
        ),
        (None, Some(Toml::Table(r))) => {
            only(r, &["stage", "at", "says"], "refused")?;
            let stage = match string(r, "stage")?.as_str() {
                "parse" => Stage::Parse,
                "evaluate" => Stage::Evaluate,
                s => return Err(format!("{name}: stage is parse or evaluate, not {s}")),
            };
            Expect::Refused {
                stage,
                at: r.get("at").map(|_| string(r, "at")).transpose()?,
                says: string(r, "says")?,
            }
        }
        _ => {
            return Err(format!("{name}: give answers, or refused, and not both"));
        }
    };
    Ok(Case {
        programs,
        held,
        references,
        expect,
        note: t.get("note").map(|_| string(t, "note")).transpose()?,
        name,
    })
}

fn only(t: &Table, keys: &[&str], what: &str) -> Result<(), String> {
    match t.keys().find(|k| !keys.contains(&k.as_str())) {
        Some(k) => Err(format!("{k} is not a key of {what}")),
        None => Ok(()),
    }
}

fn string(t: &Table, key: &str) -> Result<String, String> {
    match t.get(key) {
        Some(Toml::String(s)) => Ok(s.clone()),
        Some(_) => Err(format!("{key} is a string")),
        None => Err(format!("{key} is missing")),
    }
}

/// A TOML integer is an integer, and a string is a symbol: the two kinds of
/// value the language has.
fn value(v: &Toml) -> Result<Value, String> {
    match v {
        Toml::Integer(n) => Ok(Value::Integer(*n)),
        Toml::String(s) => Ok(Value::Symbol(s.clone())),
        v => Err(format!("{v} is neither an integer nor a string")),
    }
}

fn rows_of(v: &Toml) -> Result<Vec<Vec<Value>>, String> {
    let Toml::Array(rows) = v else {
        return Err("rows are a list of rows".into());
    };
    rows.iter()
        .map(|r| match r {
            Toml::Array(r) => r.iter().map(value).collect(),
            _ => Err("a row is a list of values".into()),
        })
        .collect()
}

/// Puts one case through the crate: its held relations into a database, then
/// each program parsed and evaluated into it.
pub fn run(case: &Case) -> Outcome {
    let got = attempt(case);
    let passed = match (&case.expect, &got) {
        (Expect::Answers(want), Got::Answers(got)) => {
            want.len() == got.len()
                && want.iter().zip(got).all(|(w, g)| {
                    let mut w = w.clone();
                    w.sort();
                    w.dedup();
                    w == g.rows
                })
        }
        (
            Expect::Refused { stage, at, says },
            Got::Refused {
                stage: s,
                at: a,
                message,
            },
        ) => stage == s && at.as_ref().is_none_or(|at| at == a) && message.contains(says),
        _ => false,
    };
    Outcome { got, passed }
}

fn attempt(case: &Case) -> Got {
    let mut db = Database::default();
    for (relation, rows) in &case.held {
        let n = rows.first().map_or(0, Vec::len);
        let columns: Vec<String> = (1..=n).map(|i| format!("C{i}")).collect();
        let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
        db.declare(relation, &columns, "the case", "a relation the case holds");
        for row in rows {
            db.add_values(relation, row);
        }
    }
    let resolve = |r: &str| {
        case.references
            .get(r)
            .cloned()
            .ok_or_else(|| format!("the case names nothing @{r}"))
    };
    let mut answers = Vec::new();
    for text in &case.programs {
        let program = match datalog::parse(text) {
            Ok(p) => p,
            Err(e) => {
                return Got::Refused {
                    stage: Stage::Parse,
                    at: e.span.to_string(),
                    message: e.message,
                };
            }
        };
        match datalog::evaluate(&mut db, &program, "the case", &resolve) {
            Ok(got) => answers.extend(got.into_iter().map(|a| Answer {
                query: a.query,
                columns: a.columns,
                rows: a.rows,
            })),
            Err(e) => {
                return Got::Refused {
                    stage: Stage::Evaluate,
                    at: e.span.to_string(),
                    message: e.message,
                };
            }
        }
    }
    Got::Answers(answers)
}

/// Every feature's cases run, in order.
pub fn run_all(features: &[Feature]) -> Vec<Vec<Outcome>> {
    features
        .iter()
        .map(|f| f.cases.iter().map(run).collect())
        .collect()
}

/// A value as a case file writes it: an integer bare, a symbol quoted.
pub fn written(v: &Value) -> String {
    match v {
        Value::Integer(n) => n.to_string(),
        Value::Symbol(s) => format!("{s:?}"),
    }
}

pub fn row(r: &[Value]) -> String {
    let cells: Vec<String> = r.iter().map(written).collect();
    format!("[{}]", cells.join(", "))
}
