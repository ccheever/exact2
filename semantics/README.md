# Contract semantics

A formal semantics of the Contract language in Lean 4. The Rust runner is
tested against it, differentially and at random.

| | |
|---|---|
| `Contract/Syntax.lean` | The abstract syntax: a deep embedding of the expanded root component (every used component's declarations lifted into it), the file's shapes and `fn`s. A statement may call an action (`Stmt.call`, LLP 1089 D9): by name, with its whole argument list, never the body the Rust compiler expanded. |
| `Contract/Binary64.lean` | Numbers: IEEE-754 binary64 as its bits (`F64`), every operation the exact result rounded once to nearest, ties to even, over `Nat`/`Int` — computable in the kernel. |
| `Contract/Binary64Facts.lean` | That model proved: correct rounding, overflow, monotonicity, exactness (below). |
| `Contract/Number.lean` | `max`/`min` with the runner's NaN and signed-zero rules, and JavaScript's `Number#toString` over exact rationals. |
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
| `Contract/Components.lean` | The unexpanded file: every component with its props, injects, `provide` section, `slot`, states, derives and actions, and a view whose uses are still uses. |
| `Contract/CompSem.lean` | The component-level semantics: the unexpanded file's meaning, instance by instance, without expanding it. |
| `Contract/Expand.lean` | The component expander, transcribing `contract_syntax::inline` (inline.rs, subst.rs, derives.rs, tail.rs). |
| `Contract/ExpandCheck.lean` | The Lean half of `difftest expansion`. |
| `Contract/CompSemFacts.lean`, `ExpandSubst{,Rev}.lean`, `ExpandFrame.lean`, `ExpandInstance.lean`, `ExpandMap.lean` | The expansion's correctness proofs (below). |
| `Contract/EnvSound.lean`, `SettleSound.lean`, `SettleComplete.lean`, `RenderSound.lean` | Well-typed environments give `EnvOK`; settlement and rendering preserve typing and fail only legitimately; a settlement that succeeds has settled everything. |
| `Contract/StepSound.lean`, `BootSound.lean` | Well-typed programs don't go wrong: boot and every step from a reachable configuration are safe, and every reachable configuration is well typed. |
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
cargo run --release -p contract-difftest -- arith --count 1000000
```

`arith` checks the number model against this machine's `f64`: for each
operand pair (random bits; edge values and their neighbours; pairs that
cancel or sit a few exponents apart; integers around `2^53`; operands
near overflow and underflow) and natural number, `+ - * / %`, `floor`,
`max`, `min`, `trunc`, negation, a natural number as a double, and
`< <= ==`, as the runner computes them, compared by bits (every NaN one).
`numbers` does the same for number printing.

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
commit, and checks component expansion over the corpus and 200 generated
programs (`difftest expansion`, below).

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
The async lane runs the corpus and the explored
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
   (`Stmt.noAssigns`, `BodyKeeps.untouched`); a body that calls an action
   is not, since `Stmt.noAssigns` and `Stmt.noSends` answer `false` for a
   call rather than look into its callee. A fact about one action is
   `runAction_commit` (the body's `ExecR` outcome, the slots after it)
   plus `wp_sound` and `applyWrites_last`. Numbers are `F64`s whose
   arithmetic is defined over `Nat` and `Int`, so a numeric fact is
   proved from `Contract.Binary64Facts` (`F64.add_ofNat`: integer `+` is
   exact up to `2^53`; `F64.ofNat_le_ofNat`; …) or computed by
   `decide +kernel` for given operands. `native_decide` is not used (it
   adds an axiom).

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
| Update Lab | `UpdateLab.counter_always` | `counter` is always exactly one of the doubles `1, 2, …, 2^53`, however many increments: past `2^53` adding one rounds back. |
| Update Lab | `UpdateLab.counter_pos` | `counter ≥ 1`, in IEEE order. |
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
locals, `map`/`filter` with the callback inline after the opcode, blocks
whose `let`s are dropped where the block ends, and a call of an action as
the Rust compiler expands it (LLP 1089 D8): each argument bound as the next
local, the callee's body in a scope of those locals and the component's
names alone, every local dropped at its end, no call opcode. It carries
static types of its own (`STy`), proved sound, and refuses where it cannot
know (`+` of an operand not known to be a number or a string, a member of
one not known to be a record, such as a router read) or where the semantics
differs (`path(…)`, which the Rust compiler expands into a template and the
semantics evaluates by name).

**The theorems** (`Contract.LowerStmt`). In a machine state that
corresponds to the semantics' environment (`Ctx`: every name the scope
resolves reads, on the VM, what `eval` reads for it, of its static type;
nothing in flight; the same clock and route table) — `compileBody_correct`: the compiled
code of a derive, slot initializer or resource argument returns `v` exactly
when `eval` answers `v`, and returns at all exactly when `eval` has a value;
`compileAction_correct`: with `writes` admitting the body's writes
(`Writes`), and the component's names agreeing with no locals bound
(`GlobalsAgree`, which a callee's fresh scope needs), an action's code
returns exactly the effects `exec` records, calls included,
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

## Component expansion

Everything above is about the *expanded* root: `contract lean` embeds what
`contract_syntax::inline::expand` makes of the file. This part stops
trusting that expansion preserves meaning.

**The unexpanded file.** `contract lean --components <file>` emits the file
before expansion as a `Contract.Components.CProgram`: every component with
its props (declared type, whether `action`), injects, `provide` section,
`slot`, states, derives and actions (each with the checker's types: the
root's from the expanded root, a child's from the checker's standalone
pass), and a view whose `Use` and `children` nodes are still there, each
region, use and `children` node numbered within its component.

**Its meaning** (`Contract.CompSem`) is given directly, not by expanding.
A use is an *instance*, identified by its path from the root: every use,
region arm (`when`/`match` arm, `each` row by key and duplicate count) and
`children` node on the way. Its states live in a store under that path,
initialized in the instance's frame the first time it renders, kept while
it renders, dropped when a render leaves it out. Names are read in a
*frame*: the root's reads the root's declarations as `Contract.Eval` does;
an instance's reads its derives (the body, at every read), its states (the
store), its injects and props, each a thunk (the expression the use or the
provider wrote, with the frame and locals where it was written, evaluated
at each read). A fill renders where `children` stands, in the use site's
frame, locals, providers and fill; a `provide` covers the providing
component's view. A handler names an action or an `action` prop, resolved
through the props to the action it names with the curried arguments; its
arguments are evaluated at dispatch. A child's action writes its
instance's states; an `action` prop called last (LLP 1017 §11) runs the
named action's statements in its own frame in the same commit. The root's
own slots, settlement, actions and timers are `Contract.Runtime`'s, of the
root alone (`rootProgram`).

**The expander** (`Contract.Expand`) transcribes inline.rs and its
submodules step for step, numbering and spelling included: capture-avoiding
substitution renaming a binder `x@k` when a replacement mentions it, view
binders `x#n`, derive resolution (freshened binders `x@bk`, a dependency
read twice on every path bound once by a `let`), lifted states and actions
`x#n` with their owners, props and injects captured as hidden parameters
`@capture:n:i`, tail calls resolved (`p@ck`, `x@bk`, `p@tailk`; the
`@check:` statement the flat embedding drops is not emitted).

**`difftest expansion`** checks the real expander on each program:

```
cargo run -p contract-difftest -- expansion                          # semantics/corpus
cargo run -p contract-difftest -- expansion contract/corpus apps/*/app.contract
cargo run -p contract-difftest -- expansion --seed 1 --count 500     # generated
```

(a) the Lean expander's output on the component-level embedding against
Rust's expansion (the flat embedding), declaration by declaration and node
by node, types apart (a lifted type that differs is reported, not failed:
a child's declarations carry the standalone checker's types); (b) the
component-level semantics of the unexpanded file against the flat
semantics of Rust's expansion over the case's script, the oracle the
runner's transcript, every observation line compared but the slots the
expansion lifts out of children (`slot name#n …`), which the
component-level semantics keeps per instance. A disagreement is kept under
`target/difftest/expansion/`, its script shrunk.

**What is proved** (no `sorry`, no axioms beyond Lean's own):

- `ExpandSubst.subst_iff`: the expander's substitution commutes with
  evaluation. For an expression of the fragment (`Plain`: no
  `pending`/`failed`, callbacks of at most two parameters, no binder named
  like a call in its scope), a substitution whose replacements are
  callbacks of at most two parameters and never a callback, and no call
  head replaced: if every free name reads in the source environment what
  its replacement reads in the target (`Agree`), the expression has value
  `v` in the source exactly when its substitution has value `v` in the
  target. The binder cases are `agree_enter`: a binder is renamed exactly
  when a replacement mentions it, to a name the scope, the replacements and
  the enclosing binders do not write, so nothing is captured either way.
  Weakening (`wk_iff`: only free names' locals matter) and environment
  transfer for `fn` bodies (`xferE`) are proved on the way.
- `CompSemFacts.ceval_mono`, `ceval_det`: more fuel never changes the
  component-level evaluator's answer; a name has one value in a frame.
- `ExpandFrame.frame_iff`: an instance's frame is an environment. An
  expression of the fragment whose free names are locals or the frame's
  has value `v` in the frame (at some fuel) exactly when it has value `v`
  in `frameEnv` — the flat semantics over the frame's names as values.
- `ExpandInstance.instance_iff`: an instance's expression means what its
  expansion means. When the frame's names correspond to their replacements
  (`Corr`), a component-level expression has value `v` in the instance
  exactly when the expander's substitution of it has value `v` in the flat
  environment. `prop_reads`, `state_reads` and `derive_reads` discharge
  `Corr` name by name: a prop when its argument corresponds at the use site
  (the same theorem one level up, so it composes through nesting), a state
  when its lifted slot `x#n` holds the store's value, a derive when its
  resolved body means its body.
- `ExpandMap.use_prop`, `use_state`: the same of the expander's own
  substitution for a use (`Expand.baseMap`, `withDerives`, which
  `inlineNode` builds it with), for a component whose props, injects,
  states, actions and derives have distinct names: a prop's replacement is
  its argument substituted at the use site (`map_prop`), a state's its
  lifted name (`map_state`).

**What is left.** The theorem stops at expressions in an instance's
frame. Not proved, and covered only by `difftest expansion`: that derive
resolution (`resolvedDerives`: freshening, `let` placement) keeps a body's
meaning; that the expander's view is the component-level view node for
node (regions and their tags, fills placed at `children`, the instance
store against lifted root and row slots); handler resolution and a child
action's body (captured props as hidden parameters, writes to `x#n`); tail
calls; `provide`/`inject` scoping in a render; and their composition into
a run, where both semantics take a fixed fuel the expansion spends
differently.

**Findings.** Four disagreements, each a program the runner runs as the
flat semantics does (so a bug in inline.rs, if the component-level
reading is the intended one):

1. A stateful child in a slot fill, the slot component showing `children`
   under a `when`: hiding and showing the arm keeps the child's state. The
   fill is inlined before the slot component's view, so its uses are owned
   by the *use site's* region, not the region around `children`.
2. A slot component that shows `children` twice: the fill is inlined once
   and copied, so both copies share one instance's state (and region tags).
3. A slot component that repeats `children` per `each` row: every row's
   copy shares one instance's state, for the same reason.
4. A child whose prop holds a number that is not finite (`1 / d` with `d`
   zero): the child's own actions are refused (`ArgumentType` on
   `@capture:n:i`), though they never read the prop, because the expansion
   passes every prop to a lifted action as a hidden, type-checked argument.

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
  finiteness: `1 / 0` is a value inside an evaluation. `eval_sound_E`,
  `exec_sound_E`: the same for any kind of failure `E` the environment
  keeps to (`EnvOKE`); with `E` excluding `pending`, an evaluation where
  every name it reads has settled never answers `pending` (no roster entry
  or operator does: `stdlib_notPending`, `binop_notPending`).
- `exec_sound_ty`: a well-typed action body run the same way asks only for
  writes of values of the target slots' types (root and row writes), sends
  to mutations, or fails legitimately. With calls (LLP 1089 D9) it takes
  every action's body typed under its parameters, which `WellTyped`
  gives: a callee runs as the action it is.
- `check_sound`: `check p = true → WellTyped p`. Beyond types, both ask
  what the Rust analyzer asks of a task (`analyze-unknown-action`,
  `analyze-handler-arity`): it names an action of no parameters, else a
  timer would fire an unbound name; and what the compiler asks of a
  task's interval (`lower-timer-literal`: a number literal). A derive's
  body and a resource's arguments are typed in `settleScope`, the
  component scope without the states lifted from children (`lifted`: late
  or arm-owned), whose names (`x#N`) the root's source cannot write;
  `pending(x)`/`failed(x)` name a resource in scope (the checker's
  `scope.lookup`), so a root initializer cannot ask after one.
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
  `eval_sound_ty` under the derives settled so far; and it never fails
  `pending` (`settle_strict`: it waits on those).
- `settle_complete` (`SettleComplete.lean`): a settlement that succeeds has
  settled every derive and every resource.
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
  cycle, fuel or the timer fire limit, an earlier poison; `unsupported`) —
  never `pending`: `ConfigOK` carries that every derive and resource has
  settled, so actions, handlers and rendering read settled values. `handler_args_good` and `curried_conform`: a handler's
  curried arguments evaluate to values of its action's leading parameter
  types, so the action's argument check refuses them only for a non-finite
  number; the rest of that check is about the host's payload.
- `boot_sound` (`BootSound.lean`): boot commits or is refused for a
  `Legitimate` reason. Each root initializer reads only the slots before
  it (`rootScope`), the intervals are literals, settlement reads no late
  slot (`settleScope`; the late slots still hold `()`), and each late
  initializer reads the settled values and the late slots before it
  (`lateScope`, `lateScope_not_late`).
- `dont_go_wrong`, `never_wrong`: **for `check p = true`, boot and every
  event from a reachable configuration, whatever the oracle answers,
  commit or fail for a `Legitimate` reason — never a type error, an
  unbound name or a `pending` read — and every reachable configuration is
  well typed.**
- Numbers (`Binary64Facts.lean`). A finite double `y` is the integer
  `y.scaled` times `2^-1074`, and an operation's exact result `±a / (d *
  2^1074)`, so distances are compared in integers. `round_nearest`: the
  rounding is a finite double nearest the exact value among all finite
  doubles; `round_tie_even`: another as near makes its significand even;
  `roundMag_eq_none_iff`: it overflows to infinity exactly from `2^1024 -
  2^970`; `round_mono`: it is monotone; `roundMag_scale`: it depends on
  the value alone; `round_exact`: a representable value rounds to itself.
  `add_nearest`, `sub_nearest`, `mul_nearest`, `div_nearest`: `+ - * /`
  on finite operands are their exact results so rounded. `ofNat_spec`:
  every natural number up to `2^53` is a double exactly; `add_ofNat`,
  `sub_ofNat`: integer arithmetic there is exact; `add_one_saturates`:
  `2^53 + 1 = 2^53`; `ofNat_le_ofNat`, `ofNat_lt_ofNat`: those integers
  compare as integers. Not proved but tested (`difftest arith`): NaN and
  infinity cases, signed zeros, `%`, `floor`, `trunc`, `max`, `min`.

Routes are typed: `Router` and `Entry` values, the verbs and reads at the
roster's types (`routerTy`), `path("route", …)` with one string or number
per parameter of a declared route, and the router slot, whose initializer
is never evaluated. A value of the shape `Router` need not be a valid
router (a source can answer one): a verb or read of it traps, which the
semantics gives as a refusal (`Route.verb`, `Route.read`; likewise
`searchParam` of an invalid entry), not a type error.

Left out: the judgments accept a `?` operand
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
runner's evaluation bounds (list steps, string length, value size and depth)
are a refinement the semantics doesn't model (LLP 1090 D8): a step the runner
refuses for one (`IterationLimit`, `StringTooLong` as a trap or an argument's
or write's refusal, `ValueTooLarge`, `ValueTooDeep`) is `UNSUPPORTED`, outside
the semantics, not a divergence. The web's JS target refuses the same steps
(`host/web-js/conform.mjs`). The checker's two type refusals that bound a value
(`type-option-option`, `type-too-deep`) are in `Contract.check` too
(`checkBounds`, and where `infer` grows a type: `some`, `map`, the roster).
