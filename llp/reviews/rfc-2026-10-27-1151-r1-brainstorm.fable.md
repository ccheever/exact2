> **Panel provenance.** Family: Claude Fable 5.1 · Provider and runtime: a Claude Code subagent on `claude-fable-5-1`, read-only by instruction · Date: 2026-10-27 · Redacted: no · Method: brainstorm, not a review: blind (shown no other panelist's output), one panel seat, given LLP 1151 r1 (sha256 `9a6314603673f3fd84041ea77d82cd47a1dc757ba61a3c4e63fa50105e335eec`), James Ide's "A verifiable Contract" and read access to the worktree at origin/main `8233879705`; asked for six to ten ideas that make a Contract app easier, cheaper and more natural to verify, each with what the author writes or runs, what it buys, what it costs and where it fits. The text below is the panelist's final message, unedited.

## 1. Properties in the test vocabulary, shaped as `SlotIn`

**What the author writes.** A top-level block in `app.test.contract` whose body is `expect` lines, held at every step instead of one:

```
always "the phone is on one of its five screens"
  expect state screen in "lock" "home" "settings" "display" "messages"

always "the counter is a whole number from 1"
  expect state counter whole
  expect state counter >= 1

always "a probe's result is absent or an answer"
  expect state tsResult in none some
```

`expect state` gains four forms beside `==`: `in` over literals (with `none` and `some` as kinds), `<=`/`>=` against a number literal (IEEE order, as the program's `>=`), and `whole`. Record fields keep the existing `name.field` path; a list index is refused in an `always` (the list's length is not invariant). No expression, no `fn`, no derive: a property names a root state and a literal predicate.

**What it buys.** The same text is checked three ways with one evaluator: the harness's `expectations()` (`semantics/difftest/src/lib.rs`) already compares observation lines, so a property is checked at every step on the runner's observation and on the semantics' observation (identical format, `Contract/Observe.lean`), and the device test runner (`scripts/agent-test.mjs`) reads the same forms through the `state` op (idea 4). And it is exactly the shape the proved rule wants: every `always` is `SlotIn x V` with `V` a decidable literal predicate, which is what `Reachable.slotIn` takes and what seven of the twelve hand theorems are (`screen_always`: `in`; `counter_always`: `whole`; `counter_pos`: `>= 1`; `results_shape`: `in none some`; `paused_bool`: a type; the frame theorems are idea 3). A generated proof (RFC D6) then has `V v` as a goal `simp` or `decide` closes after the `wp` unfolding, instead of a goal of the form "`eval` of the author's expression is `true`", which needs the evaluation rules of every operator the expression used.

This contradicts the RFC's D1 and D2. D2 injects a hidden derive into the program: the property then changes what commits (a property that traps refuses the app's commit, which the RFC admits), it is embedded and typed by both checkers, and its proof is about `eval` of an arbitrary expression. A literal predicate over the observation needs none of that, and reaches the device for free. What it gives up: a relation between two states (`page <= pages`), which is the first extension (a two-slot `expect state page <= state pages`, provable with `Reachable.invariant` over a pair; `slotIn` is the one-slot convenience) and is not in any of the twelve theorems today.

**What it costs.** `contract/syntax/src/ast.rs:466` (`ExpectState` gains a comparison kind), `parser/steps.rs` (the forms, the `always` block in `contract::tests`, `contract/cli/src/lib.rs:981`), refused in a file that declares a component. `difftest/src/lib.rs`: `expectations()` generalized to run the file's properties after every step of every case, on both observation texts; `verify.rs` reads them from `test_files()` and reports with `locate` (step, test line, element, `slot` declaration, runner and Lean values side by side). About 350 lines of Rust plus the device half. Risk: authors want an expression the vocabulary lacks; the report must say the vocabulary is what the proof rule covers, and the extension path (two-slot relations, then `once` for `ready_stays`: a predicate that, once true, stays).

**Where it fits.** Stage 1, in place of D1/D2.

## 2. A refusal both sides agree on is a finding, classified by its reason

**What the author runs.** `contract verify <app>` as today. Nothing new to write.

**What it buys.** Today `compare()` (`difftest/src/lib.rs`) calls a case `Agree` when the runner and the semantics print the same lines, so an app whose every tap is refused for `range(-1)` verifies green: the `#` reason line is never compared and never reported. The property authors most want (the gist's D3: never refuses) is already observable in every step of every test, the explore script and any walk; what is missing is reading the reason. Classify the runner's `RunnerError` (`runner/src/runner.rs:167`) and `Trap` (`runner/src/machine.rs:22`) by the gist's D1 table: the world's (`Data`, `Shape`, `NoHandler`, `InvalidValue`, a host payload's `Arity`) and the bounds (already `OUTSIDE`) are not findings; the app's own (`UnwrapNone`, `SlotType`/`DeriveType` for a non-finite number, `Cycle`, `TaskKey`, `Router`, a refused `range`) are, reported with `locate`:

```
REFUSED apps/photo-editor: explored
  at step 4: tap "reset" (explored)
  the element "reset" is apps/photo-editor/app.contract:88:7
  # Trap(UnwrapNone { pc: 3 }) — the app's own reason; a source's or the host's refusals are not listed
```

A later `never refuses "a"` block (RFC D3) then adds only attribution and "not exercised". The honest limit: a refusal the oracle's data causes (a `Shape` the seeded answer could not meet) is the world's, and a walk that taps an inert element is `NoHandler`, so the classes must be right or the report is noise; the table is in the gist and the variants are finite.

**What it costs.** One function in `difftest/src/lib.rs` beside `BOUNDS` mapping reason text to class (about 60 lines), and `verify.rs` scanning each agreed case's `outcome refused` steps (about 40 lines). Risk: reason strings are `Debug` output and drift; test the mapping as `a_bound_refusal_is_outside_and_another_refusal_diverges` does.

**Where it fits.** Stage 1, before any syntax; the cheapest thing on this list.

## 3. Frame properties, provable today by `decide`

**What the author writes.**

```
only "rotate" writes turns
clock keeps counter note live
```

`only "a" writes x, y` says action `a` writes (assigns or sends into) no root slot but these; `clock keeps …` says no task, `then` or queued send changes them.

**What it buys.** The class of mistake an agent makes most when editing an action is clobbering something else, and the semantics already proves this shape without `wp`: `rotate_only_turns` is `runAction_untouched` with `Stmt.noAssigns`/`noSends` by `decide`; `advance_keeps_user_state` is `Reachable.advance_untouched` with `decide` over `clockActions` (`Contract/Invariant.lean:958, 1093`). So the generated Lean for these two forms is a fixed template with no proof search: `contract verify --prove` can prove them in stage 1, giving an author a proved theorem about the embedding months before D6. The search half is one diff of consecutive `slot` lines at the steps whose dispatch reached a handler naming `a` (`observe::site`) or at `clock` steps, on both sides and on a device.

Stated limit: `Stmt.noAssigns` answers `false` for a call (`Stmt.call`), so `only` of an action that calls another reports "unproved: `a` calls `b`" until a rule looks through callees (small: the Rust checker already refuses call cycles). A command (`done_focuses`) is not a write and is out of this form.

**What it costs.** The two blocks in the test grammar (with idea 1's parser work, about 80 lines); the diff check in `verify.rs` (about 60); the Lean template in `verify.rs::prove` (about 60 lines of string). Risk: `decide` over a long body's `noAssigns` is kernel evaluation; the four apps' bodies are small, Excord's are not, and the kernel time is unknown at 1,500 lines (the gist's scale risk, met early and cheaply here).

**Where it fits.** Stage 1 as syntax; its proofs are the first `--prove` output without a hand, a pilot for D6.

## 4. The same properties checked on the device, after every test step

**What the author runs.** `bun exact.mjs verify` (and `verify ios`), as today; every `always` and `only` of the file is evaluated after each step of each test through the `state` op (`scripts/agent.mjs:1372`, `stateOp`), in the test runner `scripts/agent-test.mjs` where `expect state` is already read.

```
PROPERTY "the phone is on one of its five screens" is false on ios
  after apps/typetour/app.test.contract:12:3: tap "go-settings"
  state screen "setings"
```

**What it buys.** The only claim that reaches G0's world before D11: a device's fetch fails (`fail fetch`), answers land late (`clock data`), state is carried across `reload`, the real backend answers. The claim is weaker and said so: "held at every step of every authored test on this host", not every reachable configuration. It is also the check an agent reading a diary understands first, because it is the test runner it already uses, and it needs no Lean, no difftest build, no new binary. The RFC's §3 refuses checking properties under the driver; this argues the opposite narrowly: idea 1's vocabulary is literal, so the device evaluator is the `expect state` code that exists, and the semantics is not what makes the property true on a phone.

**What it costs.** About 150 lines of JavaScript in `agent-test.mjs`: parse the blocks (the compiler's `tests --json`, `contract/cli/src/tests_json.rs`, can carry them), one `state` read per step (or per step that is a `tap`/`type`/`clock`; `--full` for a state over `STATE_CAP`). Risk: a `state` round trip per step on an iOS simulator; measure on Excord's 1,444-line test file and, if it costs, read only the states the properties name.

**Where it fits.** Stage 1, beside idea 1; a different direction from the RFC's (device first), and the two reports should use the same lines so an author reads them as one.

## 5. Put the semantics on the author's card, and in the diary

**What the author runs.** `bun exact.mjs verify` runs `contract verify` after the device tests when `lake` is installed (`contract/cli/src/verify.rs::lean_installed`), else prints one line saying what it skipped and how to install it. The diary's Checkpoints (`docs/diary.md`) gain a line: `verify (semantics)`.

**What it buys.** Today nothing an app author reads mentions `contract verify`: not `docs/start-here.md`, not the card (`contract/cli/src/vocab/card.md:177`), not `docs/recipes/tests.md`, not any diary or app README. Fifty-nine apps, sixteen test files, and the differential run that holds the runner to the semantics on exactly those tests is reachable only through `semantics/README.md`. Before any new syntax, the cheapest gain is that agents run what exists, and that the diary records whether they did, which is the demand measurement the gist asks for: how many runs, how many `DIVERGE`, how many `OUTSIDE`. The report must say what a divergence is: a finding about exact2 (the runner or the semantics), not about the app, to be filed, as `contract verify apps/native-fixture/app.contract` diverging at boot on main (QUEUE.md:313) shows.

**What it costs.** `scripts/app.mjs` (the `verify` verb, about 40 lines; `contract verify` runs `cargo run -p contract-difftest` in the checkout the compiler was built from, which an `EXACT_APP_DIR` app reaches by path), one sentence on the card, one paragraph in `docs/recipes/tests.md`, one checkpoint line in `docs/diary.md`. Risk: first-run time (difftest compiles, `lake build` takes minutes once per worktree) on an author's machine; keep it after the device tests and never blocking, and say the time.

**Where it fits.** Stage 1, first; it needs no decision beyond the RFC's.

## 6. Walks that start where the tests end, with the tests' values

**What the author runs.** `contract verify --walks N`, as RFC D3, but each walk starts from a prefix of an authored test (every test's end, and `--from-steps` for every step) and appends events drawn from what is on screen then: the `view` lines of the latest observation (the explore script taps only boot's ids, `lib.rs::explore`), `type` values from the file's own `type` literals plus the generator's `STRINGS` and `number_text` (`gen/mod.rs:348`), clock steps from the generator's list.

**What it buys.** A walk from boot cannot pass a sign-in or a search box that wants a term the data has (the RFC's open question 2); the authored tests already pass them. Starting there puts the random search in the states the author cared about, and tapping what is rendered now rather than at boot reaches pushed routes and sheets. Each case is a prefix plus a suffix, so a violation shrinks with `shrink()` as today and is reported by the test line it grew from and its seed.

**What it costs.** `lib.rs`: `walk(seed, start, steps)` beside `explore`, re-reading `observe::run`'s latest `view` lines (about 120 lines); `verify.rs`: the cases and a cap (one walk per test end, N from boot). Candidate generation runs on the runner alone and is cheap; Lean runs the chosen scripts through the cache. Risk: case count (tests × steps × walks); cap it and say what was skipped.

**Where it fits.** Stage 1, a change to D3's walk.

## 7. Mutants measure what the tests and properties would catch

**What the author runs.** `contract verify --mutants` (or `difftest mutants <app>`): the app's source is changed one semantic edit at a time (a comparison flipped, `+` to `-`, a number literal by one, a string literal replaced by another the file has, one statement of an action dropped, two writes swapped since the last wins), the authored tests, the explore script and the file's properties run on the runner alone, and the mutants nothing caught are listed with their edit and the action it touched:

```
mutants: 30 made, 18 caught, 12 survived
  apps/photo-editor/app.contract:91:12 `(turns + 1) % 4` → `(turns - 1) % 4`: survived (rotate)
```

**What it buys.** The gist's first step asks whether properties "would have caught their mistakes"; the honest instrument is a mutation score, which also tells an author which action no test or property reads, which is what to write next. The type check already mutates programs this way (`difftest/src/types.rs::mutate`, `walk_file`, `Kind`), so the walker exists; semantic mutants are new kinds. No Lean is needed, so it costs seconds.

**What it costs.** A `mutate.rs` in difftest with six kinds (about 300 lines). The compiler's entry takes text, so write each mutant as a span edit of the source (the AST carries spans, `e.span()`) rather than printing the AST. Risk: equivalent mutants and `expect`s that fail under the oracle's data on the unmutated program (today's `NOTE` lines); count only expectations that hold on the original.

**Where it fits.** Stage 1's demand measurement, run over the sixteen apps with test files before stage 2 is argued for.

## 8. The recipe as a generated skeleton; the unclosed goals are the report

**What the author runs.** `contract verify --prove Module --skeleton`: when `Apps/Proofs/<Module>.lean` does not exist, write it with one theorem per `always`/`only` of the test file in the recipe's exact form (`Reachable.slotIn`, a `SlotOrigin` case over `states`, `BodyKeeps.of_wp`, `rcases … <;> simp only [wp, assignPre] <;> simp [Keeps, …]`), ending each theorem with `all_goals (trace_state; sorry)`, build it, and report per action what `simp` did not close:

```
prove: 2 properties, 7 actions each
  "screen is one of five": proved for 17 actions; unproved: go (its argument `target`)
  "counter whole from 1": unproved: increment (F64 arithmetic)
```

**What it buys.** D6 is "automate the recipe"; before writing the automation, run the recipe and see what it leaves. The four hand proofs say the leftovers are two kinds (a handler's literal argument, as `go_handler`; integer doubles at 2^53, as `succ_ok`), and the skeleton measures that on the other twelve test-file apps for the price of a string template. The author (or an agent that can read Lean) finishes the file by hand where the report names an action; nothing with `sorry` lands, as `prove()` already refuses.

**What it costs.** About 200 lines of templating in `verify.rs::prove`, reading idea 1's blocks. Risk: the template tracks `Invariant.lean`'s rule signatures; keep one recipe and let `difftest apps` catch drift.

**Where it fits.** Between stage 1 and stage 2: it is the D6 experiment, run before D6 is specified.

## 9. Every property also from a filled store

**What the author runs.** Nothing new: `verify` runs each case again as `<name> (stored)` on a store the oracle fills (`lib.rs::stored_variants`, already used by `corpus`), and checks properties there too, reporting a property false only from the store separately:

```
PROPERTY "counter whole from 1" holds from an empty store; false from a store holding counter 0
  either widen the property or validate the state at boot
```

**What it buys.** The RFC's open question 1 (persisted states) answered by evidence instead of a ruling: whether the properties agents write survive the device's old values, which is the case an update on a phone meets. `SlotOrigin` already has the stored value as a third origin, so a proof's statement needs no change: the report says what the proof assumes.

**What it costs.** About 20 lines in `verify.rs` (`stored_variants` over `located`). Risk: none worth naming; the cases double only for apps that persist.

**Where it fits.** Stage 1, with idea 1.
