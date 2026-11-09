Cases that hold the Datalog language to what its specifications say, run from
outside the crates the way a host runs a program.

Each case is a program, what a host holds for it, and the answers or the
refusal that come back. The cases are data, so somebody who has never read
the crates' code can read, doubt or add one. A test that reaches inside a
crate does not belong here. Nothing here decides what the language means: a
part of the language with no case is unproven here, however well a crate's
own tests cover it.

Read this pile to find out what the language supports and refuses, and what
it takes or leaves of Soufflé, case by case. Change it when a specification
gains or loses something a program can say, or when a reader doubts a
behaviour no case shows.
