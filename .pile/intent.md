What makes the crates here one Datalog: built with one pinned toolchain, put
through one check together, and taken by a host from one commit.

The root holds what every pile is held to alike: the flake that pins the
toolchain and says what is checked, the Cargo workspace, CI, and the README
that says how a host takes the crates. Anything that serves only one crate
belongs in that crate's pile.

Read this pile to find out what a change has to pass, and how a host takes
the crates. Change it when the toolchain, the check, or the way the crates
are taken moves.
