# Writing a case

This file is for whoever adds a case. What the pile is for is its intent.

A file under `cases/` is one part of the language, and its name orders it
among the others. It holds `title`, `readme` (the heading of the README the
cases bear on, `datalog-syntax`'s or `datalog`'s), `description`, and one
`[[case]]` table per case:

```toml
[[case]]
name = "a node no edge leaves is found by negation"
program = '''
node(a). node(d).
edge(a, b).
?- node(X), !edge(X, _).
'''
answers = [[["d"]]]
```

- **`name`** says what the case shows, as a sentence a reader can doubt.
- **`program`** is the text given to `parse`. Write it in a `'''` string, so
  a backslash is the program's own. **`programs`**, a list, evaluates each
  into the same database in turn, as layers.
- **`held`** is the relations a host holds before the first program, each a
  list of rows: `held = { edge = [["a", "b"]] }`.
- **`references`** is what each `@name` is: `references = { first = "a" }`.
  Any other reference is refused by the host.
- **`answers`** is the rows of each query, in the order the queries are
  written across every program. Order within a query does not matter.
- **`refused`** instead says the program is refused:
  `{ stage = "parse", at = "1:13", says = "..." }`. `stage` is `parse` or
  `evaluate`. `at` is the line and column, and may be left out. `says` is
  words the message holds.
- **`note`** says why the case is there, when its name cannot: where the
  language differs from Soufflé, or why a value comes out as it does.

In `answers`, `held` and `references`, a TOML integer is an integer and a
string is a symbol, so `1` and `"1"` are the two different values the
language says they are.

A key a file does not know is refused, so a misspelled expectation cannot
pass by expecting nothing.

Hold a case to the README, not to what the crate does today. A case written
by copying what came back proves only that the crate is the same as itself.
