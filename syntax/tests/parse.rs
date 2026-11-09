//! The language as README.md writes it: what is read, what each clause becomes,
//! and every refusal with the place it is reported at.

use datalog_syntax::{Clause, Constant, Literal, Op, Span, TermKind, parse};

fn refused(src: &str) -> (usize, usize, String) {
    let e = parse(src).expect_err(src);
    (e.span.line, e.span.column, e.message)
}

fn constants(src: &str) -> Vec<Constant> {
    let p = parse(src).unwrap();
    p.facts()
        .flat_map(|a| a.terms.iter())
        .map(|t| match &t.kind {
            TermKind::Constant(c) => c.clone(),
            k => panic!("not a constant: {k:?}"),
        })
        .collect()
}

#[test]
fn an_empty_program_and_one_of_only_comments_are_programs() {
    assert!(parse("").unwrap().clauses.is_empty());
    assert!(
        parse("  % nothing\n\n% at all\n")
            .unwrap()
            .clauses
            .is_empty()
    );
}

#[test]
fn a_clause_is_a_fact_a_rule_or_a_query() {
    let p = parse(
        "edge(a, b).\n\
         path(X, Y) :- edge(X, Y).\n\
         path(X, Z) :- path(X, Y), edge(Y, Z).\n\
         ?- path(a, X).\n",
    )
    .unwrap();
    assert_eq!(p.facts().count(), 1);
    assert_eq!(p.rules().count(), 2);
    assert_eq!(p.queries().count(), 1);
    assert!(matches!(p.clauses[0], Clause::Fact(_)));
    assert!(matches!(p.clauses[3], Clause::Query(_)));
}

#[test]
fn a_relation_without_parentheses_has_no_columns() {
    let p = parse("ready.\nok :- ready.\n?- ok.").unwrap();
    assert!(p.facts().all(|a| a.terms.is_empty()));
    assert!(p.rules().all(|r| r.head.terms.is_empty()));
}

#[test]
fn empty_parentheses_are_refused_rather_than_read_as_no_columns() {
    let (l, c, m) = refused("p().");
    assert_eq!((l, c), (1, 3));
    assert!(m.contains("expected a term"), "{m}");
}

#[test]
fn a_bare_name_and_the_same_text_quoted_are_one_symbol() {
    assert_eq!(
        constants("p(tree, \"tree\")."),
        vec![
            Constant::Symbol("tree".into()),
            Constant::Symbol("tree".into())
        ]
    );
}

#[test]
fn quoting_writes_what_a_bare_name_cannot() {
    assert_eq!(
        constants(r#"p("a/b", "a b", "x.y", "Upper")."#),
        vec![
            Constant::Symbol("a/b".into()),
            Constant::Symbol("a b".into()),
            Constant::Symbol("x.y".into()),
            Constant::Symbol("Upper".into()),
        ]
    );
}

#[test]
fn a_string_escapes_a_quote_a_backslash_a_newline_and_a_tab() {
    assert_eq!(
        constants(r#"p("a\"b\\c\nd\te")."#),
        vec![Constant::Symbol("a\"b\\c\nd\te".into())]
    );
}

#[test]
fn any_other_escape_is_refused_where_it_is() {
    let (l, c, m) = refused(r#"p("ab\q")."#);
    assert_eq!((l, c), (1, 6));
    assert!(m.contains("escapes only"), "{m}");
}

#[test]
fn a_string_left_open_is_refused_where_it_opens() {
    let (l, c, m) = refused("p(\"abc).");
    assert_eq!((l, c), (1, 3));
    assert!(m.contains("not closed"), "{m}");
    // and a newline does not continue it
    assert_eq!(refused("p(\"ab\nc\").").0, 1);
}

#[test]
fn integers_are_values_of_their_own() {
    assert_eq!(
        constants("p(0, 42, -7)."),
        vec![
            Constant::Integer(0),
            Constant::Integer(42),
            Constant::Integer(-7)
        ]
    );
    assert_ne!(constants("p(1).")[0], constants("p(\"1\").")[0]);
}

#[test]
fn an_integer_that_does_not_fit_is_refused() {
    let (_, c, m) = refused("p(99999999999999999999).");
    assert_eq!(c, 3);
    assert!(m.contains("64 bits"), "{m}");
}

#[test]
fn a_number_running_into_a_letter_is_refused() {
    assert!(refused("p(12ab).").2.contains("followed by a letter"));
}

#[test]
fn a_reference_is_kept_for_the_host_to_resolve() {
    assert_eq!(
        constants("p(@overview, @release-notes, @a_1)."),
        vec![
            Constant::Reference("overview".into()),
            Constant::Reference("release-notes".into()),
            Constant::Reference("a_1".into()),
        ]
    );
}

#[test]
fn an_at_sign_with_nothing_after_it_is_refused() {
    assert_eq!(refused("p(@).").1, 3);
}

#[test]
fn variables_begin_with_a_capital_or_an_underscore_and_underscore_alone_is_a_wildcard() {
    let p = parse("p(X) :- q(X, _y, _, Long_Name).").unwrap();
    let r = p.rules().next().unwrap();
    let Literal::Positive(a) = &r.body[0] else {
        panic!()
    };
    let kinds: Vec<_> = a.terms.iter().map(|t| t.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            TermKind::Variable("X".into()),
            TermKind::Variable("_y".into()),
            TermKind::Wildcard,
            TermKind::Variable("Long_Name".into()),
        ]
    );
}

#[test]
fn a_body_negates_with_a_bang_and_compares_with_equals_and_not_equals() {
    let p = parse("p(X) :- q(X, Y), !r(Y), X != Y, Y = done, \"a\" != X.").unwrap();
    let body = &p.rules().next().unwrap().body;
    assert!(matches!(body[0], Literal::Positive(_)));
    assert!(matches!(&body[1], Literal::Negative(a) if a.relation == "r"));
    assert!(matches!(&body[2], Literal::Compare(c) if c.op == Op::Ne));
    let Literal::Compare(c) = &body[3] else {
        panic!()
    };
    // a name before = is a symbol compared, not a relation
    assert_eq!(c.op, Op::Eq);
    assert_eq!(
        c.right.kind,
        TermKind::Constant(Constant::Symbol("done".into()))
    );
    assert!(matches!(&body[4], Literal::Compare(_)));
}

#[test]
fn a_name_first_in_a_comparison_is_a_symbol() {
    let p = parse("p(X) :- q(X), done = X.").unwrap();
    let Literal::Compare(c) = &p.rules().next().unwrap().body[1] else {
        panic!()
    };
    assert_eq!(
        c.left.kind,
        TermKind::Constant(Constant::Symbol("done".into()))
    );
}

#[test]
fn a_query_answers_with_the_variables_its_atoms_bind_in_the_order_written() {
    let p = parse("?- edge(B, A, _, K), label(A, N), !q(B), B != A.").unwrap();
    assert_eq!(
        p.queries().next().unwrap().variables(),
        vec!["B", "A", "K", "N"]
    );
}

#[test]
fn spans_are_lines_and_columns_counted_in_characters() {
    let p = parse("% first\n  é(x).\n").err().unwrap();
    // é is not a name: the column is the character's, not the byte's
    assert_eq!(p.span, Span { line: 2, column: 3 });
    let p = parse("p(\"é\").\n  q(Y).").unwrap_err();
    assert_eq!(p.span, Span { line: 2, column: 5 });
    let p = parse("p(\"é\") :-\n   q(\"ü\"),\n   r(Z).\n?- p(X).").unwrap();
    let q = p.queries().next().unwrap();
    assert_eq!(q.span, Span { line: 4, column: 1 });
    let r = p.rules().next().unwrap();
    let Literal::Positive(a) = &r.body[1] else {
        panic!()
    };
    assert_eq!(a.span, Span { line: 3, column: 4 });
    assert_eq!(a.terms[0].span, Span { line: 3, column: 6 });
}

#[test]
fn an_error_prints_where_it_is_and_what_it_is() {
    assert_eq!(
        parse("p(a)\nq(b).").unwrap_err().to_string(),
        "2:1: expected \".\", \":-\" or \"?-\", found \"q\""
    );
}

#[test]
fn what_is_not_the_language_is_refused_where_it_is() {
    for (src, at) in [
        ("p(a) & q(b).", (1, 6)),
        ("p(a)", (1, 5)),
        ("p(a :- q.", (1, 5)),
        ("p(X) :- q(X)", (1, 13)),
        ("p(X) :- q(X) r(X).", (1, 14)),
        ("?- .", (1, 4)),
        ("X(a).", (1, 1)),
        ("p(X) :- X.", (1, 10)),
        ("p(X) :- q(X), X.", (1, 16)),
        ("p(a) :- !.", (1, 10)),
        ("p(,).", (1, 3)),
    ] {
        let (l, c, _) = refused(src);
        assert_eq!((l, c), at, "{src}");
    }
}

#[test]
fn a_fact_is_values_only() {
    let (_, c, m) = refused("p(a, X).");
    assert_eq!(c, 6);
    assert!(m.contains("X in a fact"), "{m}");
    assert_eq!(refused("p(a, _).").1, 6);
}

#[test]
fn a_head_variable_comes_from_the_body() {
    let (_, c, m) = refused("p(X, Y) :- q(X).");
    assert_eq!(c, 6);
    assert!(m.contains("Y is in the head of p"), "{m}");
    // a negation does not bind
    assert!(refused("p(X) :- q(Y), !r(X).").2.contains("negated"));
    assert_eq!(refused("p(X, _) :- q(X).").1, 6);
}

#[test]
fn a_negated_or_compared_variable_comes_from_an_atom_that_is_not_negated() {
    assert!(refused("?- q(X), !r(Y).").2.contains("Y is negated"));
    assert!(refused("?- q(X), X != Y.").2.contains("Y is compared"));
    assert!(refused("?- q(X), X != _.").2.contains("_ in a comparison"));
    // a wildcard in a negation is allowed: no tuple of r has X first
    assert!(parse("?- q(X), !r(X, _).").is_ok());
}

#[test]
fn a_body_ranges_over_something() {
    assert!(refused("?- !q(a).").2.contains("not negated"));
    assert!(refused("p :- !q.").2.contains("not negated"));
    assert!(refused("?- a = a.").2.contains("not negated"));
}

#[test]
fn a_relation_has_one_number_of_columns_wherever_it_is_written() {
    let (l, c, m) = refused("p(a).\nq(X) :- p(X, Y).");
    assert_eq!((l, c), (2, 9));
    assert_eq!(m, "p is written with 2 terms here and 1 term at 1:1");
    assert!(refused("p(a). ?- !p(a, b), q.").2.contains("p is written"));
    assert!(refused("p :- q. ?- p(X).").2.contains("p is written"));
}

#[test]
fn what_display_writes_parses_to_the_same_program() {
    let src = r#"
        % a comment is not kept
        edge(a, "b c").  edge(@x, -3).  ready.
        path(X, Y) :- edge(X, Y).
        path(X, Z) :- path(X, Y), edge(Y, Z), !blocked(Y, _), X != Z, Z = "q\"\\\n\t".
        ?- path(a, Who), Who != a.
        ?- ready.
        who(X) ?- path(X, _).
        anyone ?- ready.
    "#;
    let once = parse(src).unwrap().to_string();
    let twice = parse(&once).unwrap().to_string();
    assert_eq!(once, twice);
    assert_eq!(
        once,
        "edge(a, \"b c\").\n\
         edge(@x, -3).\n\
         ready.\n\
         path(X, Y) :- edge(X, Y).\n\
         path(X, Z) :- path(X, Y), edge(Y, Z), !blocked(Y, _), X != Z, Z = \"q\\\"\\\\\\n\\t\".\n\
         ?- path(a, Who), Who != a.\n\
         ?- ready.\n\
         who(X) ?- path(X, _).\n\
         anyone ?- ready.\n"
    );
}

#[test]
fn a_doc_comment_is_the_percent_percent_lines_directly_above_a_clause() {
    let p = parse(
        "% an ordinary comment, dropped\n\
         %% Two nodes one edge apart.\n\
         %% Wrapped prose, so a line break\n\
         %% is a space.\n\
         %%\n\
         %% And a second paragraph.\n\
         near(A, B) :- edge(A, B).\n\
         near(A, B) :- edge(B, A).\n\
         %% Just a title.\n\
         far(A, B) :- near(A, C), near(C, B).\n\
         plain(a).\n",
    )
    .unwrap();
    assert_eq!(p.docs.len(), p.clauses.len());
    let defined = p.defined();
    let names: Vec<&str> = defined.iter().map(|d| d.relation).collect();
    assert_eq!(names, vec!["near", "far", "plain"]);

    let near = defined[0].doc.expect("near is documented");
    assert_eq!(near.title, "Two nodes one edge apart.");
    assert_eq!(
        near.description,
        "Wrapped prose, so a line break is a space.\n\nAnd a second paragraph."
    );
    assert_eq!(near.span, Span { line: 2, column: 1 });
    // The relation's doc is its first clause's; the second has none.
    assert!(p.docs[1].is_none());

    let far = defined[1].doc.expect("far is documented");
    assert_eq!(far.title, "Just a title.");
    assert_eq!(far.description, "");

    assert!(defined[2].doc.is_none());
    assert_eq!(
        defined[2].span,
        Span {
            line: 11,
            column: 1
        }
    );
}

#[test]
fn a_double_percent_after_something_on_its_line_is_an_ordinary_comment() {
    let p = parse("p(a). %% not a doc\nq(b).").unwrap();
    assert!(p.docs.iter().all(Option::is_none));
    // and indented, it is still first on its line
    let p = parse("  %% Indented.\n  q(b).").unwrap();
    assert_eq!(p.docs[0].as_ref().unwrap().title, "Indented.");
}

#[test]
fn a_doc_comment_with_nothing_directly_under_it_is_refused() {
    for (src, at) in [
        ("%% Nothing follows.\n", (1, 1)),
        ("%% A blank line after it.\n\np(a).", (1, 1)),
        ("%% A comment between.\n% here\np(a).", (1, 1)),
        (
            "p(a).\n%% Two blocks,\n\n%% one documents nothing.\nq(b).",
            (2, 1),
        ),
    ] {
        let (l, c, m) = refused(src);
        assert_eq!((l, c), at, "{src}");
        assert!(m.contains("documents the clause"), "{m}");
    }
}

#[test]
fn a_doc_comment_begins_with_its_title() {
    let (_, _, m) = refused("%%\n%% The title came second.\np(a).");
    assert!(m.contains("begins with its title"), "{m}");
}

#[test]
fn a_doc_comment_inside_a_clause_is_refused() {
    let (l, _, m) = refused("p(X) :-\n%% No.\n  q(X).");
    assert_eq!(l, 2);
    assert!(m.contains("found a doc comment"), "{m}");
}

#[test]
fn display_writes_doc_comments_back_above_their_clauses() {
    let src =
        "%% Near.\n%% One line,\n%% wrapped.\n%%\n%% Two.\nnear(A) :- p(A).\nfar(A) :- near(A).\n";
    let once = parse(src).unwrap().to_string();
    assert_eq!(
        once,
        "%% Near.\n%% One line, wrapped.\n%%\n%% Two.\nnear(A) :- p(A).\nfar(A) :- near(A).\n"
    );
    assert_eq!(parse(&once).unwrap().to_string(), once);
}

#[test]
fn a_query_may_be_named_with_parameters_its_body_binds() {
    let p = parse(
        "%% every node that reaches Of, however far\n\
         reaching(Of) ?- path(P, Of).\n\
         ?- edge(a, X).\n",
    )
    .unwrap();
    let named: Vec<_> = p.named().collect();
    assert_eq!(named.len(), 1);
    let (q, doc) = named[0];
    assert_eq!(q.name.as_ref().unwrap().relation, "reaching");
    assert_eq!(q.parameters(), vec!["Of"]);
    assert_eq!(q.span, Span { line: 2, column: 1 });
    assert_eq!(
        doc.unwrap().title,
        "every node that reaches Of, however far"
    );
    assert_eq!(p.queries().count(), 2);
    assert!(p.defined().is_empty(), "a query's name is not a relation");
}

#[test]
fn a_named_query_with_no_parameters_is_written_without_parentheses() {
    let p = parse("lonely ?- node(P), !edge(P, _).").unwrap();
    let (q, _) = p.named().next().unwrap();
    assert!(q.parameters().is_empty());
}

#[test]
fn a_named_query_and_a_relation_of_the_same_word_are_not_one_thing() {
    assert!(parse("near(A) ?- edge(A, B).\nnear(A, B, C) :- edge(A, B), edge(B, C).").is_ok());
}

#[test]
fn a_parameter_is_a_variable_written_once_and_bound_by_the_body() {
    let (l, c, m) = refused("f(a) ?- edge(a, X).");
    assert_eq!((l, c), (1, 3));
    assert!(m.contains("a parameter is a variable"), "{m}");
    let (l, c, m) = refused("f(_) ?- edge(X, Y).");
    assert_eq!((l, c), (1, 3));
    assert!(m.contains("a parameter is a variable"), "{m}");
    let (l, c, m) = refused("f(X, X) ?- edge(X, Y).");
    assert_eq!((l, c), (1, 6));
    assert!(m.contains("X is a parameter of f twice"), "{m}");
    let (l, c, m) = refused("f(Y) ?- edge(X, _).");
    assert_eq!((l, c), (1, 3));
    assert!(m.contains("Y is a parameter of f and no atom"), "{m}");
}

#[test]
fn a_name_is_one_question_in_a_program() {
    let (l, c, m) = refused("f(X) ?- edge(X, _).\nf(X) ?- edge(_, X).");
    assert_eq!((l, c), (2, 1));
    assert!(m.contains("a query is named f here and at 1:1"), "{m}");
}

#[test]
fn binding_gives_parameters_values_and_leaves_the_question() {
    let p = parse("f(Of, By) ?- edge(P, Of, By), P != Of.").unwrap();
    let q = p.queries().next().unwrap();
    let bound = q
        .bind(&[Some(Constant::Reference("start".into())), None])
        .unwrap();
    assert_eq!(bound.name, None);
    assert_eq!(bound.to_string(), "?- edge(P, @start, By), P != @start.");
    let all = q.bind(&[
        Some(Constant::Symbol("/a b".into())),
        Some(Constant::Integer(3)),
    ]);
    assert_eq!(
        all.unwrap().to_string(),
        "?- edge(P, \"/a b\", 3), P != \"/a b\"."
    );
    assert!(q.bind(&[None]).is_none(), "one value per parameter");
    assert!(parse(&bound.to_string()).is_ok());
}

#[test]
fn a_body_compares_in_order_with_less_and_greater() {
    let p = parse("?- q(X, Y), X < Y, X <= 3, Y > a, 2 >= Y.").unwrap();
    let ops: Vec<Op> = p
        .queries()
        .flat_map(|q| q.body.iter())
        .filter_map(|l| match l {
            Literal::Compare(c) => Some(c.op),
            _ => None,
        })
        .collect();
    assert_eq!(ops, vec![Op::Lt, Op::Le, Op::Gt, Op::Ge]);
    assert!(refused("?- q(X), X < _.").2.contains("_ in a comparison"));
    assert!(refused("?- q(X), X > Y.").2.contains("Y is compared"));
}
