//! Evaluation: fixed points, strata, filters and the refusals only a whole
//! program against its relations can be refused for (README.md).

use datalog::{Database, Value, evaluate};

/// A database holding edge(a, b), edge(b, c), edge(c, a), edge(c, d) — a
/// cycle with a tail — and node for each of the four.
fn graph() -> Database {
    let mut db = Database::default();
    db.declare("edge", &["From", "To"], "the test", "an edge");
    db.declare("node", &["Node"], "the test", "a node");
    for (a, b) in [("a", "b"), ("b", "c"), ("c", "a"), ("c", "d")] {
        db.add("edge", &[a, b]);
    }
    for n in ["a", "b", "c", "d"] {
        db.add("node", &[n]);
    }
    db
}

fn sym(s: &str) -> Value {
    Value::Symbol(s.to_string())
}

/// Each query's rows, as the words they hold.
fn ask(db: &mut Database, program: &str) -> Vec<Vec<Vec<String>>> {
    let p = datalog::parse(program).expect("the program is Datalog");
    let none = |r: &str| Err(format!("no @{r} here"));
    evaluate(db, &p, "the program", &none)
        .unwrap_or_else(|e| panic!("{e}"))
        .into_iter()
        .map(|a| {
            a.rows
                .iter()
                .map(|r| r.iter().map(|v| v.to_string()).collect())
                .collect()
        })
        .collect()
}

fn refused(db: &mut Database, program: &str) -> String {
    let p = datalog::parse(program).expect("the program is Datalog");
    let none = |r: &str| Err(format!("no @{r} here"));
    evaluate(db, &p, "the program", &none)
        .expect_err(program)
        .to_string()
}

fn rows(words: &[&[&str]]) -> Vec<Vec<String>> {
    words
        .iter()
        .map(|r| r.iter().map(|w| w.to_string()).collect())
        .collect()
}

#[test]
fn a_recursion_through_a_cycle_reaches_its_fixed_point() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "reach(X, Y) :- edge(X, Y).\n\
         reach(X, Z) :- reach(X, Y), edge(Y, Z).\n\
         ?- reach(d, Y).\n\
         ?- reach(a, Y).",
    );
    assert_eq!(got[0], rows(&[]));
    assert_eq!(got[1], rows(&[&["a"], &["b"], &["c"], &["d"]]));
}

#[test]
fn a_constant_or_a_repeated_variable_in_a_recursive_atom_filters_it() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "reach(X, Y) :- edge(X, Y).\n\
         reach(X, Z) :- reach(X, Y), edge(Y, Z).\n\
         ?- reach(X, X).\n\
         ?- reach(X, d), reach(d, X).\n\
         ?- reach(b, a).",
    );
    assert_eq!(got[0], rows(&[&["a"], &["b"], &["c"]]));
    assert_eq!(got[1], rows(&[]));
    assert_eq!(got[2], rows(&[&[]]));
}

#[test]
fn a_relation_is_finished_before_anything_negates_it() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "reach(X, Y) :- edge(X, Y).\n\
         reach(X, Z) :- reach(X, Y), edge(Y, Z).\n\
         looped(X) :- reach(X, X).\n\
         % negation of a recursion, and of a negation\n\
         straight(X) :- node(X), !looped(X).\n\
         bent(X) :- node(X), !straight(X).\n\
         ?- straight(X).\n\
         ?- bent(X).",
    );
    assert_eq!(got[0], rows(&[&["d"]]));
    assert_eq!(got[1], rows(&[&["a"], &["b"], &["c"]]));
}

#[test]
fn a_negation_with_a_wildcard_asks_whether_any_row_matches() {
    let mut db = graph();
    let got = ask(&mut db, "?- node(X), !edge(X, _).");
    assert_eq!(got[0], rows(&[&["d"]]));
}

#[test]
fn comparisons_hold_between_columns_and_values() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "?- edge(X, Y), X != a, Y != d.\n\
         ?- edge(X, Y), Y = a.\n\
         ?- node(X), a = b.\n\
         ?- node(X), a = a, X = a.",
    );
    assert_eq!(got[0], rows(&[&["b", "c"], &["c", "a"]]));
    assert_eq!(got[1], rows(&[&["c", "a"]]));
    assert_eq!(got[2], rows(&[]));
    assert_eq!(got[3], rows(&[&["a"]]));
}

#[test]
fn facts_in_a_program_are_a_relation_like_any_other() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "weight(a, 3). weight(b, -1). weight(z, 0).\n\
         ?- node(N), weight(N, W).",
    );
    assert_eq!(got[0], rows(&[&["a", "3"], &["b", "-1"]]));
}

#[test]
fn integers_and_symbols_are_different_values_and_integers_sort_first() {
    let mut db = graph();
    db.declare("v", &["V"], "the test", "a value");
    let got = ask(&mut db, "w(1). w(\"1\"). w(x). ?- w(X).");
    assert_eq!(got[0], rows(&[&["1"], &["1"], &["x"]]));
    let mut db = graph();
    let p = datalog::parse("w(2). w(\"1\"). w(10). ?- w(X).").unwrap();
    let none = |_: &str| Err(String::new());
    let a = evaluate(&mut db, &p, "the program", &none).unwrap();
    assert_eq!(
        a[0].rows,
        vec![
            vec![Value::Integer(2)],
            vec![Value::Integer(10)],
            vec![sym("1")]
        ]
    );
}

#[test]
fn a_relation_with_no_columns_holds_or_does_not() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "cyclic :- edge(X, Y), edge(Y, X).\n\
         tailed :- edge(_, d).\n\
         ?- tailed.\n\
         ?- cyclic.",
    );
    assert_eq!(got[0], rows(&[&[]]));
    assert_eq!(got[1], rows(&[]));
}

#[test]
fn a_head_constant_is_written_into_every_row() {
    let mut db = graph();
    let got = ask(
        &mut db,
        "kind(X, start) :- edge(X, _).\nkind(X, end) :- node(X), !edge(X, _).\n?- kind(X, K).",
    );
    assert_eq!(
        got[0],
        rows(&[
            &["a", "start"],
            &["b", "start"],
            &["c", "start"],
            &["d", "end"]
        ])
    );
}

#[test]
fn what_the_database_holds_is_not_defined_again() {
    let mut db = graph();
    assert!(
        refused(&mut db, "edge(x, y). ?- edge(X, Y).")
            .contains("edge is the test's, and is not defined again")
    );
}

#[test]
fn a_negation_inside_its_own_recursion_is_refused_where_it_is() {
    let mut db = graph();
    let says = refused(&mut db, "win(X) :- edge(X, Y), !win(Y).\n?- win(X).");
    assert!(
        says == "1:24: win is negated inside its own recursion: win is defined by its own negation",
        "{says}"
    );
}

#[test]
fn a_reference_is_what_the_resolver_says() {
    let mut db = graph();
    let p = datalog::parse("?- edge(@first, Y).").unwrap();
    let first = |r: &str| match r {
        "first" => Ok(sym("a")),
        _ => Err("no".into()),
    };
    let a = evaluate(&mut db, &p, "the program", &first).unwrap();
    assert_eq!(a[0].rows, vec![vec![sym("b")]]);

    let says = refused(&mut db, "?- edge(@nobody, Y).");
    assert_eq!(says, "1:9: no @nobody here");
}

#[test]
fn what_a_program_defines_stays_for_the_next_and_its_queries_do_not() {
    let mut db = graph();
    ask(
        &mut db,
        "two(X, Z) :- edge(X, Y), edge(Y, Z). ?- two(X, Z).",
    );
    assert!(db.relations().contains_key("two"));
    assert!(!db.relations().keys().any(|k| k.starts_with('?')));
    assert_eq!(db.relations()["two"].columns, vec!["X", "Z"]);
    let got = ask(&mut db, "?- two(a, Z).");
    assert_eq!(got[0], rows(&[&["c"]]));
}

#[test]
fn a_relation_carries_the_title_its_host_or_its_doc_comment_gave_it() {
    let mut db = graph();
    let p = datalog::parse(
        "%% Two nodes one edge apart, either way.\n\
         %% The edge's direction dropped.\n\
         near(A, B) :- edge(A, B).\n\
         near(A, B) :- edge(B, A).\n\
         bare(A) :- node(A).\n",
    )
    .unwrap();
    let none = |_: &str| Err(String::new());
    evaluate(&mut db, &p, "the program", &none).unwrap();
    let held = db.relations();
    assert_eq!(held["edge"].title.as_deref(), Some("an edge"));
    assert_eq!(held["edge"].description, None);
    assert_eq!(
        held["near"].title.as_deref(),
        Some("Two nodes one edge apart, either way.")
    );
    assert_eq!(
        held["near"].description.as_deref(),
        Some("The edge's direction dropped.")
    );
    assert_eq!(held["bare"].title, None);
}

#[test]
fn a_named_query_asked_as_written_is_answered_like_any_other() {
    let mut db = graph();
    let p = datalog::parse("into(To) ?- edge(From, To), From != b.").unwrap();
    let none = |r: &str| Err(format!("no @{r} here"));
    let answers = evaluate(&mut db, &p, "the program", &none).unwrap();
    assert_eq!(answers[0].query, "into(To) ?- edge(From, To), From != b.");
    assert_eq!(answers[0].columns, vec!["From", "To"]);
    assert_eq!(answers[0].rows.len(), 3);
    assert!(db.relations().keys().all(|k| k != "into"));
}

#[test]
fn a_named_query_bound_asks_about_the_values_it_was_given() {
    let mut db = graph();
    let p = datalog::parse("into(To) ?- edge(From, To).").unwrap();
    let q = p.queries().next().unwrap();
    let bound = q
        .bind(&[Some(datalog::Constant::Symbol("a".into()))])
        .unwrap()
        .to_string();
    assert_eq!(ask(&mut db, &bound), vec![rows(&[&["c"]])]);
}

#[test]
fn a_host_adds_integers_as_integers() {
    let mut db = Database::default();
    db.declare(
        "kept",
        &["Thing", "Days"],
        "the test",
        "how long a thing is kept",
    );
    db.add_values("kept", &[sym("logs"), Value::Integer(30)]);
    db.add("kept", &["traces", "30"]);
    // The integer matches 30 written in a program; the symbol does not.
    assert_eq!(ask(&mut db, "?- kept(T, 30)."), vec![rows(&[&["logs"]])]);
}

#[test]
fn values_compare_in_the_order_an_answer_sorts_in() {
    let mut db = Database::default();
    db.declare("w", &["W"], "the test", "a value");
    for v in [Value::Integer(2), Value::Integer(10), sym("b"), sym("a")] {
        db.add_values("w", &[v]);
    }
    let got = ask(
        &mut db,
        "?- w(X), X > 2.\n\
         ?- w(X), X <= 2.\n\
         ?- w(X), X >= a, X < b.\n\
         ?- w(X), w(Y), X < Y, Y <= 10.\n\
         ?- w(X), 1 > 2.",
    );
    // integers sort before symbols, so a symbol is greater than any integer
    assert_eq!(got[0], rows(&[&["10"], &["a"], &["b"]]));
    assert_eq!(got[1], rows(&[&["2"]]));
    assert_eq!(got[2], rows(&[&["a"]]));
    assert_eq!(got[3], rows(&[&["2", "10"]]));
    assert_eq!(got[4], rows(&[]));
}
