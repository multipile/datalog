# datalog

A small Datalog, in Rust: a parser that refuses a mistake with the line and
column it is at, and an evaluator on
[datafrog](https://github.com/rust-lang/datafrog). It reads facts, rules
and queries, named queries, doc comments, negation, comparisons, integers,
strings and `@reference`s that the host resolves. It has no aggregates.

```prolog
path(X, Y) :- edge(X, Y).
path(X, Z) :- path(X, Y), edge(Y, Z).
quiet(X)   :- node(X), !path(X, _), X != a.
?- path(a, Who).
```

## The crates

- [`datalog-syntax`](syntax/README.md) reads a program, checks it, and
  writes it back. It depends on nothing. Its README is the language.
- [`datalog`](evaluate/README.md) evaluates a program against relations a
  host holds, and exports all of `datalog-syntax` too. Its README is what a
  program derives.
- [`datalog-conformance`](conformance/README.md) holds the language to the
  two READMEs, case by case, through the public API alone.

## Using it

The crates are not on crates.io. Take one as a `git` dependency, pinned to a
commit:

```toml
[dependencies]
datalog = { git = "https://github.com/multipile/datalog", rev = "<commit>" }
```

## Working on it

The repository is a Nix flake, which pins nixpkgs and the Rust toolchain.

```sh
nix develop          # a shell with the pinned Rust, clippy and rustfmt
cargo test --workspace
nix flake check      # what CI runs: the build, the tests, clippy and rustfmt
nix build            # packages.default: the crates built and tested
```

## License

[MIT](LICENSE).
