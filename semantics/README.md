# Contract semantics

A formal semantics of the Contract language in Lean 4. The Rust runner is
tested against it, differentially and at random.

| | |
|---|---|
| `Contract/Syntax.lean` | The abstract syntax: a deep embedding of the expanded root component (every used component's declarations lifted into it), the file's shapes and `fn`s. |
| `Contract/Number.lean` | IEEE-754 binary64 exactly: `%` as fmod, `max`/`min`, and JavaScript's `Number#toString` over exact rationals. |
| `Contract/Value.lean` | Values, structural equality as the runner's `compare::equal`, and the roster's string functions. |
| `Contract/Eval.lean` | Operational semantics of expressions: the interpreter `eval`. |
| `Contract/Runtime.lean` | Operational semantics of programs: statements, actions as transactions, settlement of derives and resources, rendering with keyed rows, timers, events. |
| `Contract/Big.lean` | The same semantics as inductive big-step relations, with proofs that the interpreter is sound and complete for them and that they are deterministic. |
| `Contract/Axiomatic.lean` | An axiomatic semantics: a Hoare logic for action bodies, proved sound against the operational semantics, plus the transaction laws (a refused action changes nothing, reads see the pre-state, the last write wins). |
| `Contract/Observe.lean` | The canonical observation a differential run compares. |
| `difftest/` | The differential tester (Rust crate `contract-difftest`). |
| `corpus/` | Scripted programs: `test` blocks whose steps both sides run. |

## The pieces

**Codegen.** `contract lean <file.contract> [--name <ident>] [-o <file.lean>]`
(contract/cli/src/lean.rs) emits any program the compiler accepts as a
`Contract.Program` term. It runs after the whole compiler (a program the
plan backend refuses is refused here too) and embeds the expanded root with
the checker's types. Names stay names: the semantics does its own scoping.

**Differential testing.** For each case, `difftest` compiles the program to a
plan and boots it on the runner, delivers the script's events, and prints a
canonical observation after every step: the outcome, every root slot, derive
and resource, the commands issued, and the text of every element with a
`testId`. Then it emits the program with the Lean backend, and runs many cases
in one generated Lean module whose `main` prints `Contract.Observe.run` for
each. The two texts must match line for line. Data sources are a seeded oracle
that answers any call with a value of the declared shape. The runner's
transcript of that oracle becomes the Lean side's oracle, so if the semantics
makes a call the runner didn't, that is a divergence.

```
cargo run -p contract-difftest -- corpus                    # every test block in corpus/
cargo run -p contract-difftest -- random --seed 7 --count 500
```

A divergence is kept under `target/difftest/failures/` (the program, the
events, both observations); random failures have their scripts shrunk first.
The async lane (`scripts/async.mjs`, step `semantics`) builds the Lean
project, checks the proofs, runs the corpus and a random sweep seeded by the
commit.

## Lean

The Lean toolchain is pinned in `lean-toolchain`. To install it:
`curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y`.
Build and check every proof with `lake build`. The library has no
dependencies, so the build takes seconds.

## What the semantics leaves out

These are refused as unsupported rather than given a meaning: routes,
`t(...)`, the `format*` entries, geometry reads, frame tasks, and a mutation's
`then`. Presentation attributes are carried in the embedding but not
evaluated. Only a `text`'s text and an element's `testId` are observed. The
runner's resource bounds (string length, list steps, value size) are a
refinement the semantics doesn't model. A program that hits them traps on the
runner and not here.
