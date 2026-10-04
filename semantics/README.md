# Contract semantics

A formal semantics of the Contract language in Lean 4. The Rust runner is
tested against it, differentially and at random.

| | |
|---|---|
| `Contract/Syntax.lean` | The abstract syntax: a deep embedding of the expanded root component (every used component's declarations lifted into it), the file's shapes and `fn`s. |
| `Contract/Number.lean` | IEEE-754 binary64 exactly: `%` as fmod, `max`/`min`, and JavaScript's `Number#toString` over exact rationals. |
| `Contract/Value.lean` | Values, structural equality as the runner's `compare::equal`, and the roster's string functions. |
| `Contract/Route.lean` | The router (LLP 1038): canonical locations, the route table's matching, `path`, launch, the six verbs and the reads. |
| `Contract/Eval.lean` | Operational semantics of expressions: the interpreter `eval`. |
| `Contract/Runtime.lean` | Operational semantics of programs: statements, actions as transactions, settlement of derives and resources, rendering with keyed rows, timers, events. |
| `Contract/Big.lean` | The same semantics as inductive big-step relations, with proofs that the interpreter is sound and complete for them and that they are deterministic. |
| `Contract/Axiomatic.lean` | An axiomatic semantics: a Hoare logic for action bodies, proved sound against the operational semantics, plus the transaction laws (a refused action changes nothing, reads see the pre-state, the last write wins). |
| `Contract/Invariant.lean` | Invariants of runs: the configurations a program reaches from boot by any events (`Reachable`), and the rules that prove a property of all of them (`Reachable.invariant`, `Reachable.slotIn`). |
| `Contract/Observe.lean` | The canonical observation a differential run compares. |
| `Contract/Vm.lean` | A model of the expression VM (`runner/src/vm.rs`) for the opcodes expressions and action bodies use, over instructions with symbolic operands, and a decoder from the plan's bytes (`plan/tables/format.json`, `opcodes`). |
| `Contract/Lower.lean` | A compiler from the semantics' expressions and statements to VM code, mirroring `contract/lower` (`expr.rs`, `stmts.rs`) instruction for instruction. |
| `Contract/VmFacts.lean`, `Lower{Types,Sim,Spec,Proof,Lists,Calls,Correct,Stmt}.lean` | Its correctness proof (below). |
| `Contract/LowerCheck.lean` | The Lean half of `difftest lowering`. |
| `Contract/ValTy.lean` | Types as sets of values (`ValTy`, `conforms` without finiteness), the order and join the checker's `?` induces (`Ty.le`, `Ty.unify`), and that `conforms` at a complete type gives `ValTy`. |
| `Contract/Types.lean` | The type system: typing judgments for expressions, statements, views and programs (`HasTy`, `StmtsTy`, `NodesTy`, `WellTyped`), mirroring the Rust checker (contract/types). |
| `Contract/TypeCheck.lean` | The checker as a program (`check`), proved sound for the judgments. |
| `Contract/Soundness.lean` | Type soundness of expressions and statements against `eval` and `exec`. |
| `Contract/TypeInvariant.lean` | The slot invariant over every reachable configuration. |
| `Contract/EnvSound.lean`, `SettleSound.lean`, `RenderSound.lean` | Well-typed environments give `EnvOK`; settlement and rendering preserve typing and fail only legitimately. |
| `Contract/StepSound.lean` | Well-typed programs don't go wrong: every reachable configuration is well typed and every step from it is safe. |
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
cargo run -p contract-difftest -- types --seed 1 --count 200
```

`types` runs the Lean checker (`Contract.check`) against the Rust one: every
corpus program and `count` generated ones, all accepted by the compiler,
must be accepted; each also yields a mutant (a literal of another type, a
field the shape lacks, an argument dropped, added or retyped, `==` across
types, a condition that is not a bool), judged by the Rust checker, which
the Lean checker must judge the same. A mutant the compiler refuses cannot
pass `contract lean`, so its expansion is emitted with the types of the
program it came from (`contract::lean::emit_checked`). Mutants touch only
what the semantics evaluates (a view's presentation attributes are left
alone). A refusal for what the embedding does not carry (a prop's or an
inject's declared type, a source's one signature) is counted as `OUTSIDE`,
not a disagreement. A disagreement is kept under `target/difftest/types/`.

A divergence is kept under `target/difftest/failures/` (the program, the
events, both observations); random failures have their scripts shrunk first.
The async lane (`scripts/async.mjs`, step `semantics`) builds the Lean
project, checks the proofs (the app proofs among them), checks the app
embeddings are current, runs the corpus and a random sweep seeded by the
commit.

## Using it day to day

None of this is a blocking check. Nothing needs Lean except these
commands, and they say how to install it when it is missing (below,
"Lean"). The first run builds the library (minutes, once per worktree);
after that, Lean's observation of a case is kept under
`target/difftest/cache/`, keyed by the case's embedding, oracle and
events and by the semantics' sources, so a rerun only takes Lean time for
what changed. Only `verify` and `quick` use the cache unless
`DIFFTEST_CACHE=1`, and the async lane always runs Lean.

**Writing an app.** `contract verify apps/<app>/app.contract` runs the
app's `test` blocks (its own and those in `app.test.contract` beside it)
plus an explore script, on the runner and on the semantics. A divergence
names the step where the two first differ and the line of the test that
step comes from, where the element it tapped is declared, and the slot,
derive or element whose line differs:

```
DIVERGE apps/counter/app.contract: increments
  at step 2: tap "add" (apps/counter/app.contract:22:3)
  the element "add" is apps/counter/app.contract:17:7
  count is declared at apps/counter/app.contract:3:3
  runner: slot count 7
  lean:   slot count 8
```

`OUTSIDE` means the semantics doesn't model a construct the app uses
("What the semantics leaves out"), not that something is wrong. `NOTE`
means a test's `expect` fails against the seeded oracle's data instead of
the app's own, which is information and not a failure. `--types` also
runs the Lean checker on the program. `--prove <Module>` regenerates
`Apps/<Module>.lean` and builds `Apps/Proofs/<Module>.lean` (the next
section). `contract verify` runs `cargo run -p contract-difftest --
verify` in the checkout it was built from. Warm on a small app it takes
a few seconds.

**Changing the compiler, runner or semantics.** Before landing a change
under `contract/`, `runner/`, `plan/` or `semantics/`, run
`cargo run -p contract-difftest -- quick`. From the files changed since the
merge base with origin/main (`--base <rev>` picks another base), it runs
the corpus directories the changed files' names point at, or the whole
corpus when the names point at no area. It also runs 40 random programs
and the lowering check on 8, seeded by the base, so a branch's reruns are
warm. Warm, it takes about 10 s. Cold, with the semantics changed so Lean
reruns everything, it takes one to three minutes depending on load. It
prints a command that reproduces each divergence. The async lane's
`semantics` step is still the full run.

## The web JS target

The web build's JS target (`host/web-js`, LLP 1071) is a second
implementation of Contract: the plan compiled ahead to one ES module over a
small runtime. `--js` on `corpus`, `explore` and `random` checks every case
a third way, against the runner (`difftest/src/js.rs`); `--js-only` skips
the semantics (no Lean needed).

```
cargo run -p contract-difftest -- corpus --js
cargo run -p contract-difftest -- explore contract/corpus apps/*/app.contract --js-only
cargo run -p contract-difftest -- random --seed 7 --count 2000 --js-only
```

`difftest/js/drive.mjs` is the headless driver. Each case's program is
compiled by `exact-web-js` (the build's compiler), bundled with the runtime
and run under Bun in a fresh VM context over the render DOM
(`host/web-js/dom.js`) given event listeners, the clock the driver's
(`advance`, as under the agent). Its sources answer synchronously from the
runner's oracle transcript, as the runner's do; the host's reserved sources
(viewport, page, time…) answer what the runner answered at boot. A call the
runner never made is noted (`# js: the oracle has no answer…`) and shows as
the refusal it causes. It prints `Contract.Observe`'s format from the
module's own state (names.js) and the DOM: a `testId`'s text is a text's,
an inline run's, an option's or an SVG text's, as `agent.js` reads them.
A tap goes to the element's `press` listener and a `type` to its `change`;
an element without one is refused, as the runner refuses (NoHandler), and so
is a value a browser could not deliver (a select's unknown or disabled
option, a checkbox's text, a date outside HTML's format or its bounds).

A program `exact-web-js` refuses is outside the JS target (`OUTSIDE-JS`),
not a failure. A divergence prints `DIVERGE-JS` with whether the semantics
agreed with the runner, and is kept under `target/difftest/failures/`
(`*-js-*.{contract,events,rust.txt,js.txt}`); random ones are shrunk first.
Known, unfixed: `options/nested.contract` (the runtime holds `some(v)` as
`v`, so `some(none)` is `none`) and `routes/forged.contract` (the runtime
neither refuses a commit that leaves the router slot invalid nor traps on a
verb of an invalid router). The async lane runs the corpus and the explored
programs with `--js`, and a random sweep of 500 with `--js-only`.

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

## The lowering

**The VM model.** `Contract.Vm` steps as the runner's loop does: at a
`Map`/`Filter` body's end it collects what the run left and starts the next
item or pushes the list, else it runs the instruction at `pc`; every trap is
an error. Operands stay symbolic (slot, derive, resource and mutation
indices; a string is the pool's string; a `Record` is its shape's name and
field count; a `Call` is the roster entry's name and arity), and a jump is a
forward instruction offset: the decoder checks what `Plan::check_code`
checks of what the model reads (framing, pool operands, forward and aligned
jumps, a final `Return`) and refuses the rest. A `Call` means
`Contract.stdlib` over the program's route table (the router verbs and reads).

**The compiler.** `Contract.Lower.compile` resolves names through a scope
as `contract/lower` does through `Scope` and emits its instruction choices:
`and`/`or` through `BindLocal`/`LoadLocal`/`JumpIfFalse`/`DropLocal`,
`match` through `JumpIfNone`/`Unwrap`/`BindLocal`, templates through
`toString` and `Concat` (one `Str` when every part is literal), records with
a base bound as a local, a `fn` expanded inline with its arguments as
locals, `map`/`filter` with the callback inline after the opcode, and blocks
whose `let`s are dropped where the block ends. It carries static types of
its own (`STy`), proved sound, and refuses where it cannot know (`+` of an
operand not known to be a number or a string, a member of one not known to
be a record, such as a router read) or where the semantics differs (a tail
call's `@check:`; `path(…)`, which the Rust compiler expands into a template
and the semantics evaluates by name).

**The theorems** (`Contract.LowerStmt`). In a machine state that
corresponds to the semantics' environment (`Ctx`: every name the scope
resolves reads, on the VM, what `eval` reads for it, of its static type;
nothing in flight; the same clock and route table) — `compileBody_correct`: the compiled
code of a derive, slot initializer or resource argument returns `v` exactly
when `eval` answers `v`, and returns at all exactly when `eval` has a value;
`compileAction_correct`: with `writes` admitting the body's writes
(`Writes`), an action's code returns exactly the effects `exec` records,
names resolved as the plan did (`lowerFx`), and returns at all exactly when
`exec` has an outcome. Both rest on `allOk`/`blockOk`, one induction on the
compiler's fuel proving, for every construct, that its code takes the VM
from the state before it to `eval`'s value on top (forward), and that a run
through it that returns had a value (backward); `map`/`filter` by induction
on the items through the VM's callback loop.

**Translation validation.** `difftest lowering` checks the real pipeline:
for each generated program it takes the plan `contract::compile` made, and
in one Lean process per batch decodes every derive, root slot initializer,
resource argument and action body, runs it on the VM model in every
configuration the semantics reaches along the script (actions with sample
arguments, inside a row instance) and compares the result with `eval`'s (or
`exec`'s effects); and compares the body with `Contract.Lower`'s,
instruction for instruction. A divergence or an undecodable body fails the
run and keeps the program under `target/difftest/failures/`; a structural
difference or a body `Contract.Lower` refuses is reported by category.

```
cargo run -p contract-difftest -- lowering --seed 7 --count 500
cargo run -p contract-difftest -- lowering-corpus             # semantics/corpus
```

## Lean

The Lean toolchain is pinned in `lean-toolchain`. To install it:
`curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y`.
Build and check every proof with `lake build`. The library has no
dependencies.

## What is proven

All without `sorry` or axioms beyond Lean's own (`propext`,
`Classical.choice`, `Quot.sound`).

- `eval_sound_ty` (preservation and progress): if `HasTy p G Γ e τ`, the
  program is fit to evaluate (`ProgOK`: the `fn`s are well typed and the
  router shapes are `Contract.Route`'s), the component names `G` reads hold
  values of their types or fail legitimately in `env` (`EnvOK`) and the
  locals match `Γ` (`LocalsOK`), then `eval n env false ls e` is a value
  `v` with `ValTy p v τ`, or an error that is `pending`, `unsupported` or
  `refused` — never a type error, never an unbound name. `ValTy` has no
  finiteness: `1 / 0` is a value inside an evaluation.
- `exec_sound_ty`: a well-typed action body run the same way asks only for
  writes of values of the target slots' types (root and row writes), sends
  to mutations, or fails legitimately.
- `check_sound`: `check p = true → WellTyped p`. Beyond types, both ask
  what the Rust analyzer asks of a task (`analyze-unknown-action`,
  `analyze-handler-arity`): it names an action of no parameters, else a
  timer would fire an unbound name.
- `reachable_slotsOK` and `reachable_valTy` (`TypeInvariant.lean`, over
  `Contract.Reachable`): in every configuration a well-typed program
  reaches, each root slot is a state or mutation holding a value of its
  declared type. Most of this is the runtime's checks (`conforms` at boot
  and at every commit, which also makes numbers finite: that is the
  runtime's refusal, not typing); typing supplies that names are distinct
  and that a `send` targets a mutation, so every check is made at the
  slot's own type. The router slot's boot value is no check's: it is the
  launch of `/`, and `routerValue_ty` gives its type.
- `conforms_valTy`: what the runtime check admits at a complete type is a
  value of that type; `conforms_of_valTy`: a value of a type whose numbers
  are finite passes the check. Finiteness is all that separates them.
- `EnvGood.envOK` (`EnvSound.lean`): an environment whose root slots are
  present and of their types, whose row slots in force are of theirs and
  whose settled derives and resources are declared ones of theirs
  (`EnvGood`) satisfies `EnvOK` for the component scope.
- `settle_good` (`SettleSound.lean`): settlement from well-typed previous
  values returns derives and resources of their declared types
  (`SettledOK`; the runtime checks each, and a kept resource was checked
  when it was asked), and, when the slots are well typed for the scope the
  bodies are typed in, fails only legitimately — by induction over
  `settle`'s passes and its two loops, each derive evaluated by
  `eval_sound_ty` under the derives settled so far.
- `render_good` (`RenderSound.lean`): a well-typed view (`NodesTy`)
  rendered in a well-typed environment and row store fails only
  legitimately (a non-finite row key or row slot initializer, an
  expression's refusal), keeps every live row's slots of their types
  (`StoreOK`), and records only handlers naming an existing action with
  curried arguments typed, in the element's own scope, at most the
  action's leading parameters (`VNodeOK`).
- `reachable_configOK` (`StepSound.lean`): every configuration a
  well-typed program reaches is `ConfigOK` — root slots present and of
  their types, settled values of theirs, live rows' slots of theirs, every
  rendered handler `VNodeOK`, every timer running an action of no
  parameters.
- `runAction_sound`, `dispatch_sound`, `advance_sound`, `step_sound`: from
  a `ConfigOK` configuration every action that exists, every dispatch and
  every advance, whatever the oracle answers, lands in a `ConfigOK`
  configuration and commits, or refuses or poisons for a `Legitimate`
  reason (`refused`: the data seam, a non-finite number at a boundary, the
  host's input, the router, a row slot outside its row, a settlement
  cycle, fuel or the timer fire limit, an earlier poison; `unsupported`;
  `pending`). `handler_args_good` and `curried_conform`: a handler's
  curried arguments evaluate to values of its action's leading parameter
  types, so the action's argument check refuses them only for a non-finite
  number; the rest of that check is about the host's payload.
- `dont_go_wrong`: **for `check p = true`, every reachable configuration
  is well typed, and every event from it lands in a well-typed
  configuration and either commits or fails for a `Legitimate` reason —
  never a type error, never an unbound name.**

Routes are typed: `Router` and `Entry` values, the verbs and reads at the
roster's types (`routerTy`), `path("route", …)` with one string or number
per parameter of a declared route, and the router slot, whose initializer
is never evaluated. A value of the shape `Router` need not be a valid
router (a source can answer one): a verb or read of it traps, which the
semantics gives as a refusal (`Route.verb`, `Route.read`; likewise
`searchParam` of an invalid entry), not a type error.

Left out: that `pending` never happens after settlement (settlement
completes every derive and resource, but that and the absence of `pending`
reads from a complete environment are not proved, so `pending` stays among
the `Legitimate` reasons); that boot fails only legitimately (boot lands in
a `ConfigOK` configuration, `boot_configOK`, but its settlement and timers
read late slots that still hold `()`, so a derive or task reading one can
meet a type error there). The judgments accept a `?` operand
where the Rust checker defers it (no value has type `?`), check only what
the semantics evaluates of a view (a `text`'s text, `testId`, handlers,
regions), type a command's arguments without its host signature, and check
the expanded root, so a component nothing uses is not checked. Rust refuses
more: `let` shadowing, host command signatures, presentation attributes,
placeholders, `t(...)`, a prop's declared type, and a data source's one
signature across its uses.

## What the semantics leaves out

These are refused as unsupported rather than given a meaning:
`t(...)`, the `format*` entries, geometry reads, frame tasks, and a mutation's
`then`. Routes are in (`Contract/Route.lean`, the `exact_route` crate and the
runner's plan boundary transcribed): the router slot starts at the launch of
`/`, as the harness boots the runner, and a commit that leaves it holding an
invalid router is refused; the host's `navigate` event, which the harness
does not deliver, is not modelled, nor a router carried across a reload, nor
the change the runner publishes to a host after a commit. A refused verb
answers its input, and the runner journals the refusal in its log, which no
observation shows. The router slot is observed like any other root slot. Presentation attributes are carried in the embedding but not
evaluated. Only a `text`'s text and an element's `testId` are observed, and nothing
inside a virtualized `list`: the runner builds only the rows its window lays
out, which is layout, so both sides leave a virtualized list's descendants
out of the observation (the list's own line stays). The
runner's resource bounds (string length, list steps, value size) are a
refinement the semantics doesn't model. A program that hits them traps on the
runner and not here.
