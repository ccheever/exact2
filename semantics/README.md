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
| `Contract/Invariant.lean` | Invariants of runs: the configurations a program reaches from boot by any events (`Reachable`), and the rules that prove a property of all of them (`Reachable.invariant`, `Reachable.slotIn`). |
| `Contract/Observe.lean` | The canonical observation a differential run compares. |
| `difftest/` | The differential tester (Rust crate `contract-difftest`). |
| `corpus/` | Scripted programs: `test` blocks whose steps both sides run. |
| `Apps/` | Real apps' embeddings (generated, checked current by `difftest apps`) and, under `Apps/Proofs/`, invariants proved of them. |

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
cargo run -p contract-difftest -- apps                      # app embeddings are current
```

A divergence is kept under `target/difftest/failures/` (the program, the
events, both observations); random failures have their scripts shrunk first.
The async lane (`scripts/async.mjs`, step `semantics`) builds the Lean
project, checks the proofs (the app proofs among them), checks the app
embeddings are current, runs the corpus and a random sweep seeded by the
commit.

## Verifying an app

An app is verified against its embedding, so the proofs are about the
source as it is.

1. **Generate.** Add the app to `APPS` in `difftest/src/main.rs` and run
   `cargo run -p contract-difftest -- apps --write`: it writes
   `Apps/<Module>.lean`, `def <name> : Contract.Program`. Check that the
   semantics agrees with the runner on it
   (`cargo run --release -p contract-difftest -- explore apps/<app>/app.contract`)
   and that it is not refused (routes, `t(...)` and the rest of the list
   below are outside the semantics). From then on `difftest apps`, in the
   async lane, fails when the source moves and the embedding did not.
2. **State.** Write the property in `Apps/Proofs/<Module>.lean` over the
   embedding: of every reachable configuration (`Reachable <name> c`), of
   every event from any configuration (`Event.step`), or of one committed
   action (`runAction … = (c', out)` with `out` not a refusal). Slots are
   read with `lookup`; `SlotIn x V slots` says slot `x` has property `V`.
3. **Prove.** A reachability invariant of one slot is
   `Reachable.slotIn`: the values the slot starts with (`SlotOrigin`:
   unfold the program's `states`), and for every action a handler of the
   view or a task names, the body keeps it (`BodyKeeps`). An action body
   is unfolded by `wp` (`BodyKeeps.of_wp`); `simp` with the `EvalR` rules
   (`EvalR.str_iff`, `EvalR.var_local`, …) turns it into a statement about
   values. A body that never touches the slot is dismissed by `decide`
   (`Stmt.noAssigns`, `BodyKeeps.untouched`). A fact about one action is
   `runAction_commit` (the body's `ExecR` outcome, the slots after it)
   plus `wp_sound` and `applyWrites_last`. Prefer properties that need no
   float arithmetic: `Float` is opaque to the kernel, so `r + 1` can be
   stated but not computed. `native_decide` is not used (it adds an axiom).

Worked example: Type Tour's `screen` is a string and
`go(target: string)` writes any string it is given, yet the phone is only
ever on one of its five screens, because every handler that names `go`
passes a literal screen.

```lean
def screens : List String := ["lock", "home", "settings", "display", "messages"]
def ScreenOK (v : Value) : Prop := ∃ s ∈ screens, v = .str s

theorem screen_always : ∀ c, Reachable typeTour c → SlotIn "screen" ScreenOK c.slots := by
  refine Reachable.slotIn screen_boot ?_ (fun _ a ha => by simp [typeTour] at ha)
  intro c ev a args env ls vs payload rows _ hh hvs
  refine BodyKeeps.of_wp fun ad had hname => ?_
  subst hname
  simp only [typeTour, List.mem_cons, List.mem_nil_iff, or_false] at had
  rcases had with rfl | rfl | … <;> simp only [wp, assignPre] <;>
    simp [Keeps, Effects.write, Effects.rowWrite, Effects.command, ScreenOK, screens]
  -- left: `go`, whose argument `go_handler` (checked over the view by
  -- `decide`) says is a literal screen
  …
```

`screen_boot` unfolds `typeTour.states` to see `screen` starts as
`"lock"`; tasks are none; every action but `go` is dismissed by `simp`
over its `wp`. The whole proof is `Apps/Proofs/TypeTour.lean`. What is
proved, by app:

| App | Theorem | Says |
|---|---|---|
| Type Tour | `TypeTour.screen_always` | `screen` is always one of the five screens. |
| Update Lab | `UpdateLab.advance_keeps_user_state` | Moving the clock (the 250 ms probe) never changes `counter`, `note` or `live`. |
| Update Lab | `UpdateLab.results_shape` | The three result slots always hold `none` or `some` answer. |
| Update Lab | `UpdateLab.increment_adds_one` | A committed `increment` leaves `counter` at its old value plus one. |
| Photo Editor | `PhotoEditor.ready_stays` | Once `ready` is true, no event makes it otherwise. |
| Photo Editor | `PhotoEditor.rotate_only_turns` | `rotate` changes no slot but `turns`. |
| Photo Editor | `PhotoEditor.reset_spec` | A committed `reset` sets `turns` to zero and adds one to `resets`. |
| Video Player | `VideoPlayer.paused_bool` | `paused` is always a bool. |
| Video Player | `VideoPlayer.toggle_flips` | A committed `toggle` negates `paused`. |
| Video Player | `VideoPlayer.done_focuses` | A committed `done` writes nothing and issues `focus("done")`. |

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
