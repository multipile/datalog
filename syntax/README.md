# datalog-syntax

This crate reads Datalog source text into clauses, checks that each could
be evaluated, and writes a program back as text. It depends on nothing. The
crate is `parse`, and the types it returns.

The rest of this file is the language: what a program may say. What a
program derives is `datalog`'s, and its README says it.

```prolog
% a comment runs to the end of the line
edge(a, b).                                  % a fact
path(X, Y) :- edge(X, Y).                    % a rule
path(X, Z) :- path(X, Y), edge(Y, Z).
quiet(X)   :- node(X), !path(X, _), X != a.  % negation and comparison
big(X)     :- size(X, N), N > 1024.          % an ordered comparison
?- path(a, Who).                             % a query
reach(From) ?- path(From, Who).              % a named query
```

## Grammar

```
program     = { clause }
clause      = atom "."                    fact
            | atom ":-" body "."          rule
            | "?-" body "."               query
            | atom "?-" body "."          named query
body        = literal { "," literal }
literal     = atom | "!" atom | term op term
op          = "=" | "!=" | "<" | "<=" | ">" | ">="
atom        = name [ "(" term { "," term } ")" ]
term        = variable | "_" | constant
constant    = name | string | integer | "@" reference

name        = [a-z] [A-Za-z0-9_]*
variable    = [A-Z_] [A-Za-z0-9_]*        but not "_" alone
string      = '"' { char | '\"' | '\\' | '\n' | '\t' } '"'   on one line
integer     = [ "-" ] [0-9]+              fits in 64 bits
reference   = [A-Za-z0-9_-]+
```

- **A name and a string are one kind of value.** `tree` and `"tree"` are the
  same symbol. Quotes are how to write a symbol that is not a name: `"a/b"`,
  `"a b"`. An integer is a value of its own, so `1` is not `"1"`.
- **`@reference` belongs to the host.** The parser keeps what follows the
  `@`, and decides nothing about it. So a host can let its users write a
  value the way they already write it elsewhere, and resolve it before
  evaluation.
- **`_` is any value, and is matched with nothing else.** `_x` is an
  ordinary variable.
- **A relation with no parentheses has no columns.** `p()` is refused, and
  is not read as the same thing.
- A name directly before an `op` is a symbol being compared, not a relation.
- **There are no aggregates.** `count`, `min`, `max` and `sum` are names
  like any other. A host that needs a count derives it from an answer's
  rows, or holds it as a relation of its own.

## Named queries

```prolog
%% a node and every node it reaches
reach(From) ?- path(From, Who).
```

A query may be written with a name and parameters in front of `?-`. Asked
as written, it is the query after `?-`. Each parameter is a variable like
any other, and the answer is written back with the name. The name is for the
host. It gives the host a question it can list by name and title, and ask
with each parameter given a value first.

- **The name is not a relation.** Nothing derives it, and no atom reads it.
  So a relation and a named query may share a word.
- **A parameter is a variable**: not a value, and not `_`. It is written
  once, and bound by a positive atom in the body. So whatever value it is
  given, the query can still be asked.
- **A name is one question** in a program.

`Program::named` is every named query, with the doc comment above it.
`Query::bind` gives a query's parameters values, and `None` leaves a
parameter a variable. It returns the query with no name, which is what a
host asks.

## Doc comments

```prolog
%% Two nodes one edge apart, either way.
%% The edge's direction dropped: what a reader means by "next to".
near(A, B) :- edge(A, B).
near(A, B) :- edge(B, A).
```

`%%` as the first thing on its line begins a doc comment. Every other `%`
begins a plain comment, which is dropped where it is read. `%%` lines on
consecutive lines are one doc comment. It belongs to the clause on the line
directly under its last line. So a relation's doc comment is the one above
its first clause.

- **The first line is the title**: one sentence that says what one row is.
- **The lines after it are the description**, as wrapped prose. A line
  break is a space, and an empty `%%` line is a paragraph break.
- **Refused**:
  - a doc comment with no clause directly under it: a blank line, a plain
    comment, another doc comment or the end of the program comes in between
  - a doc comment whose first line is empty
  - a doc comment inside a clause

`Program::defined` is every relation a program defines, with its first
clause's doc comment. `datalog` gives each relation a program defines
that title and description when it evaluates the program. Whether a
relation must have a doc comment is
for the host to say.

## What is refused

`parse` refuses the first mistake it finds, with the line and column it is
at. Columns are counted in characters, as an editor shows them. A mistake is
any of these:

- anything that is not the grammar above
- a fact with a variable or `_` in it
- `_` in a rule's head
- a variable in a rule's head, a negated atom or a comparison that no
  positive atom in the same body binds
- `_` in a comparison
- a body with no positive atom
- a relation written with two different numbers of terms in one program
- a named query's parameter that is not a variable, is written twice, or
  that no positive atom in its body binds
- two queries with one name in one program

`parse` does not refuse anything that depends on more than the text.
Whoever evaluates a program refuses those mistakes: `datalog`'s README
says what its `evaluate` refuses.

## Writing back

`Display` writes a program as one clause per line. Each doc comment is
above its clause, with every paragraph on one line. Other comments are
dropped. What `Display` writes parses to the same program.
