# LLP 1106: The prompt behind Contract's semantics, recorded for Tudor

**Type:** Research
**Status:** Draft, 2026-10-07. A record: it decides nothing and is not in `llp/current/`.
**Systems:** `semantics/` (the Lean semantics, its proofs, `difftest/`, `vm-extract/`, `corpus/`); `contract/cli/src/lean.rs` (the Lean backend); `scripts/async.mjs` (the `semantics` step)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07
**Related:** `semantics/README.md` (the authority on what is built and proved); LLP 1017 (Contract); LLP 1089, LLP 1092 (later language work the semantics tracks)

## 1. Why this exists

Charlie asked for this record for Tudor. Contract's formal semantics came from one prompt given
to Opus 5.5. Its first result landed as `ea02f2024` on 2026-10-03. This
document keeps the prompt word for word and points to what it produced. What the semantics
covers and proves is described in `semantics/README.md`, which is kept current, so this
record does not repeat it.

## 2. The prompt

Charlie gave it verbatim as follows, introduced with "Here's the prompt for Opus 5.5":

> Write an axiomatic and operational semantics for the Contract language. Then write a code
> generator backend for the compiler that emits an embedding in those semantics of any user
> code. Then write a differential random testing library that can do large-scale testing of
> the generated code (using the operational semantics) against the Rust interpreter
> version. Then add a large number of test scripts to the repo and update CI to do
> differential random testing between the lean and rust versions

## 3. What the first commit delivered

Each clause of the prompt matches a part of `ea02f2024`:

| The prompt asks for | What landed |
|---|---|
| an axiomatic and operational semantics | `semantics/`, a Lake project that uses only Lean core. `Eval.lean` and `Runtime.lean` are the operational semantics. Expressions, statements, actions as transactions, settlement of derives and resources, keyed rendering, timers and events are all executable. `Big.lean` restates them as big-step relations and proves the interpreter sound, complete, fuel-monotone and deterministic for those relations. `Axiomatic.lean` is a Hoare logic for action bodies, sound and relatively complete (weakest preconditions), together with the transaction laws: a refusal changes nothing, reads see the pre-state, the last write wins. There is no `sorry`. |
| a backend that emits an embedding of any user code | `contract lean <file>` (`contract/cli/src/lean.rs`). It turns any program the compiler accepts into a `Contract.Program` term, a deep embedding of the expanded root. |
| a differential random testing library | `semantics/difftest` (crate `contract-difftest`). It runs a program on the Rust runner and on the Lean semantics and compares canonical observations after every step. Its modes: `corpus` (scripted tests), `explore` (any program, with every `testId` tapped), `random` (a seeded generator of well-typed programs; a failing script is shrunk) and `numbers`. |
| a large number of test scripts | `semantics/corpus`, which held 135 files when it landed. |
| CI running the comparison | The async lane's `semantics` step (`scripts/async.mjs`) runs `lake build` and fails on any `sorry`, then runs difftest over the corpus and a fixed random sweep. |

The comparison found two runner bugs on its first run, and both were fixed in the same
commit. Non-finite numbers printed as Rust's `inf`. When two shortest decimals tied, the
runner printed the one with the odd digit, where JavaScript takes the even one. Since then
JavaScript number text has one implementation, `exact_num::push_js`.

## 4. What grew from it afterward

None of the following was in the prompt. Each came later, from smaller asks or as fixes,
and each is in `git log -- semantics` (80 commits as of this date):

- **Numbers as IEEE-754 bits.** `Binary64.lean` and `Binary64Facts.lean` give correctly
  rounded binary64 over `Nat`/`Int`, with proofs.
- **A type system and type soundness.** `Types.lean`, `TypeCheck.lean` (`check_sound`),
  `Soundness.lean`, `StepSound.lean`, `BootSound.lean`: well-typed programs don't go wrong
  from boot through every reachable configuration. `difftest types` compares the Lean
  checker with the Rust one.
- **A verified lowering.** `Vm.lean` models the expression VM. `Lower*.lean` compiles to it
  instruction for instruction, the way `contract/lower` does, and proves the compilation
  correct. `difftest lowering` checks the Rust lowering against it.
- **The shipped VM, extracted.** `vm-extract/`: Charon and Aeneas translate
  `runner/src/machine.rs`, exactly as shipped, into Lean, and a proof shows it refines
  `Contract/Vm.lean` step by step and run by run. Its trusted base is listed in
  `vm-extract/README.md`.
- **Component expansion.** `Components.lean`, `CompSem.lean`, `Expand*.lean`: the
  component-level semantics of the unexpanded file, a transcription of the Rust expander,
  and proofs that expansion preserves meaning. `difftest expansion` checks this.
- **Tools for authors.** `contract verify <app>` runs an app's `test` blocks on both sides
  and names the line where they first diverge. `difftest quick` does the same for whatever
  changed in a checkout, in about 10 s warm. `difftest apps` keeps the shipped apps'
  embeddings current. The JS target is checked against the same observations.
- **Corpus.** It has grown to 191 `.contract` files.

## 5. Where to start reading

1. `semantics/README.md`: the parts, "What is proven", and "What the semantics leaves out".
2. `semantics/Contract/Syntax.lean`, then `Eval.lean` and `Runtime.lean`.
3. `semantics/Contract/Axiomatic.lean`, which answers the "axiomatic" half of the prompt.
4. `semantics/difftest/src/main.rs`, which lists the modes; `cargo run -p
   contract-difftest -- quick` runs it.
