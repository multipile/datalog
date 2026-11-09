# datalog

This crate evaluates Datalog programs against relations a host holds. A
program is read by `datalog-syntax`, whose README is the language a program
is written in, and this crate exports all of that crate too, so a host takes
this one alone. The crate adds:

- `evaluate`, and the `Database` it evaluates into
- `Value`, what a relation holds, and `Answer`, what a query is answered
  with

The rest of this file is what a program derives.

## Comparisons

`=` and `!=` say whether two values are the same. `<`, `<=`, `>` and `>=`
compare two values in the order they sort in: integers by value, before
every symbol, and symbols by their text, character by character. So
`X > 2` holds for `3` and for every symbol, and is not refused for a symbol.
Both sides are bound, as for `=`.

## Evaluating

A host declares its relations in a `Database`. For each relation it gives a
name, what its columns are called, and who the relation belongs to. Then it
adds the rows: as symbols with `add`, or as values, integers among them,
with `add_values`. Adding a row to a relation that was never declared, or a
row with a number of values other than the relation's columns, panics at
that call. That is a mistake in the host, not in a program, so it is not
left for `evaluate` to find.

`evaluate` then takes three things:

- a parsed program
- the name that the relations the program defines are to belong to
- a function that says what each `@reference` is

It returns an answer per query. The answer holds the query written back, a
column per variable the query binds, in the order the variables are first
written, and the rows, sorted.

- **What a program defines stays in the database**, so the next program can
  use it. What a query derives does not stay. Programs evaluated one after
  another are layers. Each is complete before the next one reads it.
- **A relation the database already holds is not defined again.** The
  refusal names whose relation it is. So nothing a later program does
  changes what an earlier one derived.
- **Negation is stratified.** A relation is evaluated to its fixed point
  before anything negates it. A negation inside its own recursion has no
  such order. It is refused, and the refusal names the atom.
- **Integers and symbols are different values**, and integers sort first.

`evaluate` also refuses these, with the line and column:

- a relation that nothing holds or defines
- a relation written with a number of terms other than the one it is held
  with
- a reference that the host's function names nothing for, with what the
  function said

Evaluation is [datafrog](https://github.com/rust-lang/datafrog)'s semi-naive
iteration. datafrog's rules are Rust over tuple types that are fixed when it
is compiled. So this crate compiles each body into its keyed joins.
