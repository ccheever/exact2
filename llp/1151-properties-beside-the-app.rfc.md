# LLP 1151: Properties beside the app, and what checking one means

**Type:** RFC
**Status:** Accepted, 2026-10-28 (Charlie: "accept"; the waiver for the doing-list given the same day, "approve waiver yes"). The text is r5. Charlie asked for this document and its first stage on 2026-10-27 ("Write an RFC for making Contract more verifiable based on the ideas in this Gist and any other good ideas … go ahead and implement it"). It builds on James Ide's assessment "A verifiable Contract" (2026-10-27), whose gaps G0–G7 and proposals D0–D9 are cited here by those names. r1 was reviewed blind by GPT-6 Astra (max), Grok 4.7 (xhigh), Muse Wonder 1.0 (xhigh) and Claude Fable 5.1, all NOT READY on the same points (`llp/reviews/rfc-2026-10-27-1151-r1.{astra,grok,muse,fable}.md`); the same four seats brainstormed separately (`…-r1-brainstorm.*.md`). r2 was reviewed by the same four, each given its own r1 review only: Fable READY with four fixes, Astra, Grok and Muse NOT READY (`…-r2.*.md`), converging on one delivery error and four scope errors. r3 was reviewed in the third and last round the rules allow: Fable and Muse READY, Astra and Grok NOT READY (`…-r3.*.md`), all four on the same three points, which r4 takes. r4 was reviewed in a fourth round Charlie asked for on 2026-10-28, beyond the three the rules allow: Fable and Muse READY, Astra and Grok NOT READY (`…-r4.*.md`), on three points r5 takes. What each round changed is in §R.
**Systems:** the authored-test grammar (`contract/syntax/src/parser.rs`, `parser/steps.rs`, `ast.rs`, `contract/cli/src/tests_json.rs`); `difftest verify` and the harness (`semantics/difftest/src/verify.rs`, `lib.rs`, `script.rs`, `observe.rs`, `oracle.rs`); the observation on both sides (`observe.rs`, `Contract/Observe.lean`, and a trace of `advance` and `dispatch` in `Contract/Runtime.lean`); `contract verify` (`contract/cli/src/verify.rs`); the device test runner (`scripts/agent-test.mjs`); later stages: `Contract/Invariant.lean`, `Axiomatic.lean`, `StepSound.lean`, `TypeCheck.lean`
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Implementer:** stage 1 (§4): this session, 2026-10-27, GPT-6 Astra writing under Claude's direction. Stages 2 to 4 have none; §2's D9 to D12 are directions, not specifications, and say nothing an implementer could be held to.
**Date:** 2026-10-27
**Related:** James Ide, "A verifiable Contract" (gist `89f52c1d7032357fd2fb83522dbb2927`, 2026-10-27); `semantics/README.md`; LLP 1017 P7 (authored tests); LLP 1089 D9 (action calls); LLP 1092 (queues and due work); LLP 1116 D5 (persisted states); LLP 1035.005 (new syntax answers demonstrated friction); LLP 1146 (evidence an agent can trust)

## R. Review, and what r2 changed

| Change | Raised by | Where |
|---|---|---|
| A property is no longer a hidden derive settled inside the program. It is a line of the test vocabulary, matched by the harness against the observation both sides already print. A property can then neither refuse the app's commit nor change its fuel, `after` with `old` comes for free, and the device test runner can check the same line. Expression properties, evaluated by the runner's VM and Lean's `eval` on snapshots, are stage 2. | all four; Fable and Grok brainstorms | D1, D2 |
| The observation names what each step ran. Both sides print one `ran` line per commit inside a step (the handler's action, a task's, a `then`, a queue's send) with that commit's own outcome. `never refuses`, `after` and the action coverage read those lines; the aggregate outcome line, which is the due work's when the event succeeded, attributes nothing. | all four | D3, D5 |
| The unit of checking is the observed step, and the document says so: a commit inside a clock advance that a later commit in the same advance undoes is not seen. Poisoned configurations are not observed and are out of stage 1. | Astra, Grok, Fable | D2 |
| The search is the runner's. Walks run on the runner, choose each event from the controls on screen after the previous one, draw typed values from the app's own tests, and start from the end of every deliverable authored test as well as from boot. A violation is shrunk on the runner against the same predicate; Lean confirms the shrunk case and a bounded sample. | all four; Astra, Grok, Fable brainstorms | D4 |
| Authored tests are delivered as far as they go: `clock data` and `clock settle` are empty events under the synchronous oracle, and a test is split at its first step the run cannot deliver rather than skipped whole. The report counts what stopped where. The population is corrected: ten apps have test files, 222 blocks, about twelve deliverable today. | Fable, Astra, Grok; Grok brainstorm | D4 |
| Every refusal is classified and the app's own are findings, with or without a `never refuses`; a class-T refusal (a type error reported as a refusal) fails the run. | Fable, Grok, Muse brainstorms | D5 |
| Suggestions keep finite sets, whole numbers, never-`none` and frame lines (`only writes`, what the clock keeps), and drop the observed extrema; a set is suggested only when two values or two writers were seen. | all four | D6 |
| The summary is a receipt: digests, the command that reproduces it, denominators (actions committed of declared, tests delivered of authored, stores drawn), and the world it ran in. | Muse brainstorm | D7 |
| Persisted states: the search boots from the empty store and from seeded filled stores, and a property that a filled store falsifies is reported apart. A proof (stage 2) is from a boot whose store satisfied the property, and says so. | Grok, Muse; Fable brainstorm | D1, D4 |
| A property's reads are closed under derives and `fn`s: a resource, a mutation's status, the clock or geometry anywhere in that cone is refused at the line. | all four | D1 |
| D6–D12 are cut to directions. The gist's demand measurement runs over the ten apps as part of stage 1 and is recorded in §5 before stage 2 is argued for. | Muse, Fable | §2, §5 |
| The device runner checks the same properties after each test step; §3's refusal of that was wrong for a literal vocabulary. | Fable brainstorm | D8 |

Second round (r2 → r3):

| Change | Raised by | Where |
|---|---|---|
| `clock data` is not an empty event: an answer lands inside the sending commit, but its `then` is armed there and runs as its own commit at the next advance. It is delivered as a `land` event on both sides (the runner's `land_then`; a Lean variant of `advance` that fires no timer), and `clock settle` is expanded on the runner into the `land` and `clock +N` events the driver's settle performs. The delivered counts are recomputed in §5 rather than claimed. | all four | D4.1 |
| The traced `advance` does not replace the proved one as the corpus's target: `Contract.Observe` runs both and prints a line no runner can match when their configurations differ. The runner's half is a traced-commit API, not a print of what exists, and the commit kinds it leaves out are listed. | Grok, Muse, Fable, Astra | D3 |
| Derives are admitted, under the cone rule: a derive whose dependencies reach a resource, the clock or geometry is refused at the line. r2's reason for refusing them was false: both matchers already read `derive` lines. Resources stay refused. | Fable, Grok | D1 |
| Record fields, list indices and view lines (`expect text`, `expect tree`) are out of stage-1 properties: both observations print a record positionally and a node's own text only, so one matcher cannot be the device's. Stage 2 specifies the adapters. | Muse, Grok, Astra, Fable | D1 |
| `expect state x stays v` is added, so `ready_stays` has a form and the twelve theorems have twelve. | Muse, Grok, Fable | D1 |
| Class T cannot be read from today's reasons: `SlotType` and `DeriveType` cover a non-finite number too. The refusal sites tell the two apart before discarding the value. | Astra | D5 |
| Frame lines exclude a `queue` mutation's slot only, as `Reachable.slotIn` does, not every mutation's. | Grok | D1, D6 |
| On a device only `always` is checked, and a false line fails `bun exact.mjs verify` as a failed `expect` does; `after`, frames, `command` and `never refuses` need the commits, which the driver does not see. `bun exact.mjs verify` always runs `contract verify`, which falls back to the runner alone without Lean. | Astra, Grok, Muse, Fable | D8 |
| A divergence is shrunk with both engines; a property violation or a classified refusal on the runner alone. | Astra | D4 |
| The blocks and the matcher move ahead of the walk, so the walk has properties to falsify; the walk lands after every tap so a step is one commit. | Muse, Fable | §4 |
| A proof of a property over a persisted state is from a boot whose store satisfied it, stated in D9. | Muse | D9 |

Third round (r3 → r4):

| Change | Raised by | Where |
|---|---|---|
| `clock settle` is expanded as the driver performs it: a `clock +0`, then, while any task is due within a millisecond, an advance to the latest such and again, sixteen rounds at most, from the runner's live task state; the host's waits on motion, scrolling and hatches have no image here and the world line says so. | Astra, Grok, Muse, Fable | D4.1 |
| An authored `tap` or `type` is followed by a `land` on the harness, as the driver's input is; the unit of `after "a"` is that pair, and so is a suggested frame. | Fable | D1, D4.1 |
| The harness's `land` is the `then` and `next` slice. The runner's `land_then` can also commit what the semantics has no branch for (a transport's or MIDI port's answer, a re-ask, a wake, the tail's presence and polls); such a commit is counted, disqualifies the step for `after`, and is a divergence named as such if the configuration moves. | Grok, Muse, Astra | D3, D4.1 |
| The derive cone is computed by a walk of derive and `fn` bodies, as `contract/analyze/src/gates.rs`'s clock walk does for `performanceNow()`, extended to resources, `pending`/`failed`/`failure` and geometry; r3 cited a file that records derive-to-derive reads only. | Grok, Muse, Fable, Astra | D1 |
| Frame lines compare every root slot, a queue's included, in authored and suggested lines alike; the stage-2 templates' exclusion of queue slots is a limit of the proof, stated in D9, not of the check. | Astra, Muse | D1, D6, D9 |
| `Contract.Observe` prints the proved functions' configuration and outcome and the twin's `ran` lines, and `trace disagrees` covers the outcome too. | Astra, Muse, Fable | D3 |
| The line beside the deferred list's entry is written in the change that adds the grammar, pending Charlie's confirmation. | Fable | §4, Decisions |

Fourth round (r4 → r5):

| Change | Raised by | Where |
|---|---|---|
| An input's pair is two observed steps: `after "a"` reads its lines against the `land`'s configuration with `old` the configuration before the input, and is checked when the input's `ran` lines are exactly `handler a` and the `land`'s only the `then`s and `next`s it armed; the pair's `land` is not a clock step, so `after clock` keeps `advance_keeps_user_state`'s shape. | Grok | D1, D2 |
| The device runner advances its test clock after every step, one millisecond per mutation request, sixteen rounds at most; delivery appends the same `clock +N` steps from the runner's counter, both sides replay them, and the census is recounted. | Astra | D4.1, D1, §5 |
| D9's templates prove less than the frame check: a line is proved only when every other root slot is shown untouched, a queue slot is the unproved case whether or not the line names it, and the input pair needs a lemma of its own. | Grok, Astra | D9 |
| The settle bound does not stop a block; the host's seek to a re-ask's due time is in the carve-out; the undeliverable steps are the parser's kinds minus the delivered set, printed by the census. | Grok, Fable | D4.1 |
| D3's left-out kinds are §5's list, the runner's work before its advance loop is judged by the configuration's movement, and a tap whose arguments refused prints a `handler` line that names the action. | Muse, Grok, Fable | D3 |
| A device checks `always` after boot and after every state-changing step, the test clock's advances and a `reload` included. | Astra | D8 |
| §1's counts are §5's; the capture of the boot's host facts is named in §4; §3's `enum<…>` line says what exists. | Fable, Astra | §1, §3, §4 |
| What the four seats left for the implementer is listed in §4. | all four | §4 |

## Summary

Contract has a formal semantics in Lean: some 25,000 lines and 836 theorems under
`semantics/Contract/`, no `sorry`, no axioms beyond Lean's own. It proves that a checked
program never goes wrong, that the compiler to VM bytecode is correct, and that the
shipped VM machine refines its model. Of the 59 apps under `apps/`, four carry a proved
property, twelve theorems in all, each written by hand by someone who knows the
semantics: 67 to 159 lines of Lean per app for 26 to 51 lines of embedding. Ten apps
have test files, 222 `test` blocks, and the differential run delivers about a dozen of
them whole: it skips a block at its first `clock data`. The unit of verification an
author actually writes is an `expect` at one point of one script.

This RFC makes a property as cheap to write as an `expect`, checked at every step of
every script the search runs, on the runner, in the semantics and on a device, before
anyone tries to prove it; and it makes the search reach the app.

- **A property lives in the test file** as a block of the test vocabulary: `always`,
  `after "action"`, `after clock`, `never refuses "action"` (D1), over the root's states
  and derives. It is matched against the observation the differential run already
  prints, so it needs no evaluator, cannot disturb the app, and holds the runner and the
  semantics to the same line (D2).
- **The observation says what ran.** Each step lists the commits inside it and their
  outcomes, so a refusal is the action's or the due work's, never guessed (D3).
- **The search reaches the app.** Authored tests are delivered as far as they go, with
  `clock data` landing what a device lands; walks re-observe after every event, take
  values from the tests, start where the tests end, and run on the runner, with Lean
  confirming what they find (D4).
- **Silence is a result.** Every refusal is classified and the app's own are findings;
  the report says which actions it never ran, which tests it stopped in and where,
  which stores it drew, and ends in a receipt that reproduces it (D5, D7).
- **The search proposes properties** it saw hold, for the author to keep (D6); the
  `always` lines are checked on a device by the test runner the author already uses
  (D8).
- **Proof is stage 2**, with the direction set by what the twelve theorems and the
  panel's brainstorms say is cheap: frame properties by `decide`, then generated
  `slotIn` proofs (D9); the gist's D0–D5 are stages 3 and 4 (D10–D12).

Every stage-1 claim is about the runner under the harness and the semantics' world,
after each observed step, and the receipt prints that boundary beside every count.

## 1. Where verification stands

**Proved** (`semantics/README.md`, "What is proven"): `dont_go_wrong` and `never_wrong`
(`Contract/BootSound.lean`): if `check p = true`, boot and every step from a reachable
configuration, whatever the data sources answer, commit or fail with `refused` or
`unsupported`; never a type error, an unbound name or `pending`. A sound and complete
Hoare logic for action bodies (`Axiomatic.lean`: `wp`, `wp_sound`, `runAction_commit`).
Invariant rules over reachable configurations (`Invariant.lean`: `Reachable.invariant`,
`Reachable.slotIn`, `BodyKeeps.of_wp`, `BodyKeeps.untouched`, `Event.step_slotIn`,
`Reachable.advance_untouched`). The compiler's correctness (`LowerStmt.lean`) and the
extracted VM's refinement (`vm-extract/`). Twelve app theorems under `Apps/Proofs/`.

**Tested, not proved** (`difftest`): the Rust runner and the web JS target against the
semantics; the Rust checker against the Lean one; the Rust lowering and expander
against Lean's; the number model's edge cases.

**The two gaps that limit what a proof says about a phone** (the gist's G0 and G1):

- *The model's world is narrower than a device's* (G0). A send is answered inside the
  commit that makes it (`Contract/Runtime.lean`, `runAction`: `o.ask src vs` in the
  commit); a source never fails (`Contract/Eval.lean`, `failed` is `false` once a
  resource settles); the model has two events, `dispatch` and `advance`
  (`Contract/Invariant.lean`, `Event`). A property provable in the semantics can be
  false on a device whose fetch fails.
- *The headline theorem admits any refusal* (G1). `Legitimate` (`StepSound.lean`) is any
  `refused` or `unsupported`, whatever the reason, so a checker bug that the semantics
  reports as a refusal does not falsify it.

**What a property costs today.** `Apps/Proofs/UpdateLab.lean` proves `counter` is always
one of the doubles `1 … 2^53` in about seventy lines, after its author has read
`Invariant.lean` to know that `Reachable.slotIn` wants a `SlotOrigin` case, a
`BodyKeeps` per handler and per clock action, and `F64.add_ofNat` for the arithmetic.
The twelve theorems are four shapes: a slot stays in a set (`screen_always`,
`counter_always`, `counter_pos`, `results_shape`, `paused_bool`); an action changes a
slot by a fixed amount or to a literal (`increment_adds_one`, `reset_spec`,
`toggle_flips`); an action or the clock writes only named slots (`rotate_only_turns`,
`advance_keeps_user_state`, `done_focuses`, which also names a command); once true,
stays (`ready_stays`). Nothing checks a property before a proof is attempted: the first
signal that one is false is a failed `simp`.

**What the search sees today.** `contract verify` runs each test block the differential
run can deliver, plus an explore script: every `testId` shown at boot, tapped twice,
then two clock advances (`difftest/src/lib.rs`, `explore`). A block with a step the run
does not deliver (`clock data`, `clock settle`, `request`, `clock until`, `fail fetch`,
`reload`, `back`, a `key`) is skipped whole with `SKIP` (`script.rs`, `test_case`). Of
the 222 blocks in the ten test files (228 counting a nested app's), about a dozen are
delivered whole; Excord's and the design tool's none; 83 blocks stop at a `clock until`
and 72 at a `request`. A control that is not on the boot screen is never tapped. The
observation is one block per event: a tap runs the work already due and then the
event, a clock advance runs every due task in turn, and the printed outcome is "the
event's when it failed, else the due work's" (`observe.rs`, `Contract/Observe.lean`),
naming no action. A refusal's reason is a `#` line the comparison skips, so an app
every tap of which refuses verifies green.

## 2. Decisions

### D1. Properties are blocks of the test vocabulary

Four new blocks in a `*.test.contract` file, beside its `test` blocks, in the authored
test grammar (`docs/contract-grammar.md`, "Authored tests"): a keyword, a name, an
indented body of lines. The body's lines are the vocabulary of `expect state` with a
few forms added, never an expression.

```
always "the phone is on one of its five screens"
  expect state screen in "lock" "home" "settings" "display" "messages"

always "the counter is a whole number of at least one"
  expect state counter whole
  expect state counter >= 1

always "a probe's result is absent or an answer"
  expect state tsResult in none some

always "once the photo has loaded it stays loaded"
  expect state ready stays true

after "increment" "increment adds one"
  expect state counter == old + 1

after "reset" "reset puts the turns back and counts itself"
  expect state turns == 0
  expect state resets == old + 1

after "rotate" "rotating touches nothing but the turns"
  only writes turns

after "done" "done writes nothing and focuses the button"
  writes none
  command focus "done"

after clock "the probe never touches what the user set"
  only writes tsResult rustResult rustExecutorResult

never refuses "reset"
```

```ebnf
property      = "always" STRING block( { hold } )
              | "after" ( STRING | "clock" ) STRING block( { hold | frame } )
              | "never" "refuses" STRING NL ;
hold          = "expect" "state" IDENT ( "==" ( test-value | "old" [ ( "+" | "-" ) NUMBER ] | "not" "old" )
                                      | "in" { test-value | "none" | "some" }
                                      | ( ">=" | "<=" ) [ "-" ] NUMBER
                                      | "whole"
                                      | "stays" test-value ) NL ;
frame         = "only" "writes" { IDENT } NL          (* no other root slot changes *)
              | "writes" "none" NL
              | "command" IDENT { test-value } NL ;    (* the commit issued this host command *)
```

- **`always "name"`** holds after every observed step of every script the search runs
  (D4), from a boot that committed. `in` is membership in a list of literals (`none` and
  `some` name an option's kinds); `>=` and `<=` compare in IEEE order as the program's
  operators do; `whole` says the number is an integer; `stays v` says that when the
  previous step's value was `v`, this step's is `v` too, which is `ready_stays`'s shape
  and the one form that reads the previous step inside `always`.
- **`after "a" "name"`** holds over an input's pair (D4.1): the input step, then the
  `land` step that follows it, as the driver's input ends by landing what it settled.
  The two are two observed steps on both sides. The pair is checked when the input's
  `ran` lines are exactly `handler a` and the `land`'s are only the `then`s and queue
  `next`s that input armed; its `hold` and `frame` lines are read against the
  `land`'s configuration, and `old` is the configuration before the input, the step
  before the pair. A pair in which `a` committed beside another handler's or a task's
  commit, whose input refused, or whose `land` ran a commit the trace leaves out (D3),
  is counted, not checked, and the report says how many per line. The pair's `land` is
  not a clock step. `only writes x y` says no root slot but those changed across the
  pair, a `queue` mutation's slot included; `writes none` that none did; `command f
  args` that the pair's commands include that call.
- **`after clock "name"`** holds after every clock step: a `clock +N`, the test
  clock's advances (D4.1) and a `land` that delivers `clock data` or opens a settle,
  never the `land` of an input's pair, whose `then`s are the input's. Every commit
  inside such a step is a task's, a `then`'s or a queue's send, so it has the shape of
  `advance_keeps_user_state`; what D9 can prove of it is narrower.
- **`never refuses "a"`** says no commit of `a` was refused, counted over the `ran`
  lines of D3; an action never run is `not exercised`, never passed.

A line names a root state or a root derive, by its bare name, and its whole value.
Both observations print every root slot and derive after every step and both matchers
already read them (`lib.rs`, `expectations`; `scripts/agent-test.mjs`, `expect state`).
A derive is admitted under the cone rule: `verify` walks the derive's body, the bodies
of the derives it names and the `fn`s it calls, as `contract/analyze/src/gates.rs`'s
clock walk does for `performanceNow()`, and refuses the property at its line when the
walk reaches a resource, a mutation's status (`pending`, `failed`, `failure`), the
clock (`performanceNow()` and the time reads) or a geometry read (`frame`, `measure`),
with the reason: its value would be the oracle's or would change on a step that
commits nothing. No existing analysis computes this cone; the inliner's derive
resolution records derive-to-derive reads only. A resource is refused for the same
reason. A mutation's slot is a
state here, and a property over one is a property about the oracle's answers, which
the receipt says.

Out of stage 1, each for a stated reason: a record's field or a list's index (both
observations print a record positionally with no field names, and the device's
`state` is named JSON, so one matcher cannot be both until the plan's shapes resolve
fields on the observation text; stage 2 specifies that adapter); the view (`expect
text`, `expect tree`: the observation prints a node's own text and omits a
virtualized list's descendants, where the device joins descendants' text, so the same
line would mean two things); and expressions (stage 2, evaluated by the runner's VM and
Lean's `eval` on snapshots, where `(old + 1) % 4` goes). A property over a persisted
state is admitted and searched from the stores D4 draws; what a proof of it assumes is
D9's.

A property in a file that declares a component is a syntax error ("a property lives in
the test file beside the app"). A build never reads a test file.

*Why this vocabulary and not an expression.* The four shapes of §1 are literal
predicates over one or two observation lines, and so is every `expect` an author has
written. A literal line is matched the same way on the runner's observation, the
semantics' and a device's `state` reply, by one matcher; an expression needs the
runner's VM and Lean's `eval` called on a snapshot of each configuration, with an
adapter each. A property as a hidden derive, r1's mechanism, settles inside the
program: a non-total one refuses the app's commit, its failure carries no name on
either side, and it adds a pass of settlement headroom. The panel refused it
unanimously.

*Why new blocks.* LLP 1035.005 asks that new Contract syntax answer friction
demonstrated in source. These blocks are the authored-test grammar's, which `test`,
`clock until`, `fail fetch` and `request` have grown without a take, because a test
file compiles to nothing an app runs. The friction they answer is §1's: a property
today is a theorem, and the only thing an author can write is an `expect` that holds
at one point.

### D2. A property is matched against the observation, after each step

Both sides print, after every step, every root slot, derive, resource, host command and
`testId` text (`observe.rs`, `Contract/Observe.lean`). The harness already compares
them line for line and already reads `expect` lines against them (`lib.rs`,
`expectations`). A property is an `expect` applied to every step instead of one: the
harness splits each observation into steps, evaluates each `always` line against the
step's lines (and the previous step's for `stays`), each `after "a"` line against an
input's pair (D1: the `land`'s lines, with `old` from the step before the input) where
D3's `ran` lines say the pair is that action's alone, each `after clock` line against
a clock step and the step before it, and reports
the first step on which a line is false, on either side. The runner's and the
semantics' observations are matched separately, so a property true on one and false on
the other is both a violation and a divergence, and the report says which.

What this does not see, stated:

- **Commits inside a step.** A clock advance runs every due task in turn and prints the
  configuration after the last (`Runtime.lean`, `advance`; `Observe.lean`); a tap runs
  the due work first. A property false after one commit and true after the next in the
  same step is not seen. D4's scripts land after every tap and advance the clock by the
  smallest task interval, so a step mostly holds one commit; an authored `clock +60000`
  holds many. The receipt counts steps with more than one commit and, per `after`
  line, steps checked against steps counted only; open question 1 says what the counts
  decide.
- **Poisoned configurations.** A step whose render failed prints its outcome and no
  configuration on either side. Properties are checked on unpoisoned steps; the
  receipt counts the poisoned ones.
- **The world.** The oracle answers every call at once with a value of the declared
  shape and never fails; no answer lands late; a reload carries nothing. A violation
  found is real in the semantics' world and may not occur on a device; a device may
  have violations the search cannot reach. The receipt's world line says this.

### D3. The observation names what each step ran

After a step's outcome line, both sides print one line per commit the step ran, in
order, each with that commit's own outcome and, for a refusal, its reason on a `#`
line:

```
== tap "reset"
outcome ok
ran then save ok
ran handler reset ok
== clock +60000
outcome refused
ran task sample ok
ran task sample ok
ran queue upload refused
# a queue's send answered with the wrong shape
```

`handler` is the event's own action; `task`, `then` and `queue` are the due work's
kinds (LLP 1092). The lines are compared, so the two sides must agree on what ran. A
tap whose curried arguments refused before the action prints `ran handler a refused`
and a `#` line that says the arguments refused, the same on both sides, so a D5
finding names the action; a tap that reached no handler prints no `handler` line, and
the outcome line says why on a `#` line as today.

**The runner's half is new.** The runner's advance returns kernel commit receipts
(`kernel/src/txn.rs`, `CommitReceipt`) that carry no action, kind or outcome, and a
refused commit leaves none. Stage 1 adds a traced-commit record inside the runner's
advance and dispatch (`runner/src/runner/advance.rs`: kind, action or mutation, outcome,
refusal reason, in the runner's order), read by `observe.rs`. The runner has commit
kinds the semantics does not (a transport's or MIDI port's answer landing, a re-ask, a
wake's delivery and expiry, a drop deadline, a transport-end action, and the advance
tail's presence, polls, live times and text-axis frames; §5's list is this one); they
print no `ran` line, the list of them is in `semantics/README.md` beside the format, a
step that had one is counted and disqualified for `after`, and if the configuration
moved the comparison fails as a divergence the receipt names as such. The work the
runner's advance does before its loop and without a receipt (`advance_steps`: module
deadlines, listed due work, presence) is judged the same way, by whether the
configuration moved.

**The semantics' half keeps the proved function.** `advance` and `dispatch` compute
these commits and discard their names (`Runtime.lean`, `advance`'s `go`). Stage 1 adds
variants that return the trace with the configuration, with the same structure.
`Contract.Observe` runs both on every step, prints the configuration and outcome of
the original and the `ran` lines of the variant, and, when the two differ in
configuration or outcome, prints a line no runner prints (`trace disagrees`), so the
corpus fails rather than silently testing the twin; the theorems keep quantifying
over `advance` and `dispatch`, and the corpus keeps testing them. The one-page proof
that the variant's configuration is `advance`'s (the same induction as
`advance_preserves`) is open question 2; it is wanted before a theorem is stated over
`ran` lines, which stage 2's first templates do not do.

This is what `never refuses` reads, what `after` needs to know a step was one action's,
what the coverage counts (D5), and what attributes a refusal (D5). r1 resolved a tap to
its plan node by replaying the script (`observe::site`), which yields a node, not an
action, and cannot see the due work; it is kept for naming where an element is
declared.

### D4. The search reaches the app, on the runner

`contract verify <app>` runs, on the runner, every script below, matches every property
on every step, classifies every refusal (D5), and then runs on the semantics the
authored tests, the explore script, every shrunk finding and `--confirm N` of the
walks (default 2), comparing observations as today.

1. **Authored tests, as far as they go.** A new script event, `land`, delivers what a
   device's `clock data` delivers: every armed `then` and every queue's next send, each
   its own commit, the clock unmoved and no timer fired. On the runner it is
   `Runner::land_then` (`runner/src/runner/advance.rs`), which exists for the driver; in
   the semantics it is a variant of `advance` that takes the `then` and `next` branches
   of its loop and never the timer's (a timer due exactly now is not fired), traced as
   D3 says. `land_then` can also commit what the semantics has no branch for; D3 says
   what happens then. An authored `tap` or `type` is delivered as the event followed by
   a `land`, as the driver's input ends by landing what it settled (LLP 1012 §2), so an
   `expect` after a `tap` reads what the harness's `tap` alone would not; the input and
   its `land` are two observed steps, and D1 says which lines read which. The device
   runner then advances its test clock after every step (`scripts/agent-test.mjs`,
   `testMutationClock`): one millisecond per mutation request the step issued, as the
   runner counts them (`Runner::mutation_requests`, `runner/src/runner/queue.rs`),
   again while an advance issues more, sixteen rounds at most, so an input that sends
   once and arms `after(1, …)` runs that task before the next authored `expect` on a
   phone. Delivery appends the same: after each delivered step, the `clock +N` steps
   that rule yields from the runner's counter at that point, each a clock step both
   sides replay (and an `after clock` step, D1). `clock data` is delivered as one
   `land` (one `land_then` runs a whole `then` chain). `clock settle` is expanded as
   the driver performs it (`scripts/agent.mjs`, `clock`; `agent-inspect.mjs`,
   `nextCommitTasks`): a `clock +0`, then, while any task is due within a millisecond
   of the clock, a `clock +N` to the latest such (N the difference; the driver moves to
   an absolute time) and a `clock +0` again, sixteen rounds at most, after which the
   driver goes on to the next step and so does delivery, the receipt counting the
   bound; the expansion is computed from the runner's live task state at that point of
   the script and replaces the step in the case, so both sides replay the same events.
   The host's part of a settle, its wait on motion, scrolling and hatches, its seek to
   a re-ask's due time (the oracle schedules no re-ask) and its advance to an
   animation's end, has no image here, and the world line says so. A block is
   delivered up to its first step the run cannot take, which is any step kind the
   parser knows other than `tap`, `type`, `clock +N`, `clock data`, `clock settle` and
   `expect`; the census prints that list rather than this document keeping it (today
   `request`, `clock until`, `fail fetch`, `reload`, `back`, `close`, `size`, `pick`,
   `resize`, `fresh`, `drag`, a `type … key`, a tap form that is not a press, a
   mid-test `prefer`): the prefix runs, its `expect`s are judged, and the verdict names
   the step the block stopped at and why. A block is `SKIP` only when its first step
   cannot be taken. How many blocks this delivers is measured in §5, not claimed; the
   blocks that
   stop at `clock until` and `request` are the next steps worth delivering, with
   `request` the larger and the harder (it writes through the app's server).
2. **The explore script**, as today, each tap followed by a `land`.
3. **Walks.** `--walks N` (default 20) scripts of `--steps M` (default 40) events, half
   from boot and half from the end of a delivered test prefix chosen in turn, seeded
   from the app's path (`--walk-seed` for another). Each event is chosen after the
   previous one is delivered, from what the runner then shows: a tap on a `testId` with
   a press handler, followed by `land`; a `type` into an input whose value is drawn
   first from a `type` step for that `testId` in the app's test files, then from a
   string literal of the app's source, then from the generator's strings and numbers,
   followed by `land`; or a clock advance of the app's smallest task interval or of
   60 s. A control that appears after a tap is a candidate from then on; r1's walk drew
   from the boot screen only and could not leave Type Tour's lock screen. Actions with
   a rendered handler the walks never reached are named in the receipt.
4. **Stores.** Each script also runs from a seeded filled store when the app persists a
   state (`lib.rs`, `stored_variants`, corpus-only today; a third of its seeds leave a
   slot out, a third store a value of the type, a third a mismatch), and a property
   false only from a filled store is reported apart, since the device's store is what
   an update meets.

A property violation or a classified refusal found on the runner is shrunk on the
runner against the same predicate (the property's first false step; the same refusal
class and action), which generalises `lib.rs`'s `shrink` from `Diverge` alone; the
shrunk script is then run on the semantics, so every reported finding has been seen by
both evaluators unless the receipt says the semantics did not run. A divergence is a
relation between the two executions and is shrunk as today, with both engines on
every candidate; the receipt prints what that cost. The report names the step, the
test line that wrote it (or the walk, its seed and its starting test), the element it
tapped and where it is declared, the slot's declaration, and both sides' values:

```
PROPERTY "the phone is on one of its five screens" is false
  at step 3: tap "go-settings" (walk 7 from "the lock opens", seed 0x3a1f…)
  the element "go-settings" is apps/typetour/app.contract:41:7
  screen is declared at apps/typetour/app.contract:4:3
  runner: slot screen "setings"
  lean:   slot screen "setings"
  kept: target/difftest/failures/4121-verify-2.{contract,events,rust.txt,lean.txt}
```

Without Lean (`lake` not installed, or `--runner-only`), everything above the semantics
line runs and the receipt says the semantics did not run and how to install it; today
`contract verify` exits at once. `--js` runs the same scripts through the web JS target
(`js.rs`, as `difftest corpus --js` does) and matches `always` lines on its observation
(it prints no `ran` lines, so `after` and `never refuses` are not checked there, and the
receipt says so); without the flag the receipt says `js: not run`.

Timing is measured in §5 before the defaults above are final.

### D5. Silence is a result: refusals, attribution and coverage

Every `outcome refused` and `ran … refused` on the runner is classified by its reason,
after the gist's D1 table: the world's (no answer, an answer of the wrong shape, no
element or handler, a host payload of the wrong arity or type), a limit's (fuel, timer
fires, a full queue; `OUTSIDE` today), poisoned, a cycle, a boundary value (a non-finite
number written or passed, a bad `range` count), and the app's own (`UnwrapNone`, a
refused `range`, a router verb refused). Each class but the last is counted; the app's
own are findings, printed with the step and the `ran` line that refused:

```
REFUSED reset at step 2: tap "reset" (apps/photo-editor/app.test.contract:8:3)
  # range(-1): a count is a whole number of at least 0
```

A reason that is a type error is class T and fails the run on sight: it is the shape of
checker bug the headline theorem admits (G1), caught here only when a script reaches it.
Today's reasons do not say it: the runner's `SlotType` and `DeriveType` are raised for a
value of the wrong type and for a non-finite number alike, and the sites discard the
value (`runner/src/runner/commit.rs`, `settlement.rs`; `plan/src/value.rs`,
`conforms`). Stage 1 splits them where the value is in hand, so a non-finite number is
a boundary value and only a wrong type is T, and the VM's `Arity` (not a host
payload's) is T. A class the table does not know is printed `unclassified` and does not
fail. The Lean side's reasons are not matched by text; the comparison of `ran` lines
holds the two sides to the same outcomes.

Coverage is counted from the `ran` lines against the plan: actions committed at least
once of actions declared, by name; actions with a rendered handler never reached;
clock actions fired; tests delivered whole, delivered in part (with the stopping step),
and skipped; stores drawn; steps with more than one commit; poisoned steps; per
`after` line, steps checked and steps counted only. A `never refuses "a"` with no `ran`
line naming `a` is `not exercised`.

### D6. Properties the search saw hold

After the search, `verify` prints what held on every observed step, as blocks of D1 the
author can paste, under one heading that says what they are:

```
Observed over 31 scripts and 1,204 steps, not proved; keep the ones the app means:

always "screen is one of 5 values"
  expect state screen in "lock" "home" "settings" "display" "messages"
always "counter is a whole number"
  expect state counter whole
always "ready stays true"
  expect state ready stays true
after "rotate" "rotate writes only turns"
  only writes turns
after clock "the clock writes only tsResult rustResult rustExecutorResult"
  only writes tsResult rustResult rustExecutorResult
```

The forms: a string or number state or derive that took at most six distinct values,
when at least two values were seen or two actions wrote it; a number always whole; an
option never `none` after boot; a value that, once it was `v`, was `v` on every later
step, for a bool or a small set; for each action and for the clock, the set of root
slots it ever changed on a pair that was its alone (D1), a `queue` mutation's slot
included when it changed. Not suggested: a bound at the smallest or largest value
seen, which every finite sample has; anything the type already says; a property
already in the file. Suggestions are printed, never written to a file.

This is the gist's first step, measured: how many suggestions an author keeps, over the
ten apps with tests, and whether any kept one would have caught a mistake the apps'
diaries record, decides whether stage 2 is worth doing (§5). The population is the ten
apps that have tests, which is the population that will write properties first; the
gist's random sample is the next measurement, not this one.

### D7. The receipt

The summary is a block an author pastes into the diary unchanged, and a reader can
falsify: the command that reproduces it, the digests of the app, its tests and its
embedding, the counts of D5 with their denominators, and the world line:

```
verify receipt — reproduce: contract verify apps/typetour/app.contract --walks 20 --walk-seed 0x3a1f…
  app sha256:9f2c…  tests sha256:41ab…  embedding sha256:77e0…  exact2 8233879705
  scripts: 4 tests delivered (2 in part: "search finds a book" stopped at clock until; 3 skipped at request), 1 explored, 20 walks
  steps: 1,204 (37 with more than one commit; 0 poisoned; 0 with a commit the trace leaves out); stores: empty, 4 seeded
  actions: committed 14 of 17 (never reached: delete, share, archive); clock actions 3 of 3
  properties: 3 held, 0 false, 1 counted only (after "save": 12 steps with other commits, 0 checked)
  refusals: 2 of the app's own (above); 0 of class T; 41 of the world's
  world: seeded oracle answering at once and never failing; tap, type, land and clock events only; no reload, no late answer, no host input
  semantics: agreed on 29 of 31 scripts, 2 outside (geometry); configurations matched advance on every step; js: not run
```

A receipt whose digests no longer match the tree is stale on its face.

### D8. The same `always` lines on a device

`bun exact.mjs verify` (and `verify ios`, `verify macos`) runs the app's tests under the
driver (`scripts/agent-test.mjs`). It reads the `tests --json` output, which carries the
properties as it carries the tests, and reads `state` and matches every `always` line
the same way after boot and after every step that can change the state: `tap`, `type`,
`clock`, `reload`, and the test clock's advances that follow each step (D4.1); the
claim covers every configuration the test observed, the first included. A false line is
printed in D4's form with `on <host>` and the step, and fails the run as a failed
`expect` does: the author wrote the line for the app's world, and the device run is the
one stage-1 surface that reaches a failed fetch, a late answer, a reload and the real
backend; it is the weaker claim and says so: "held after every step of every authored
test on this host". `after`, its frames, `command` and `never refuses` are not checked
on a device: the driver knows a step's target, never the action a press committed, the
commands a commit issued, or the `then` that ran before the next tap, so it cannot
establish D1's conditions; they are the harness's until the driver's tap reply names
them, which is a later decision.

`bun exact.mjs verify` then runs `contract verify`, which runs on the runner alone when
`lake` is missing (D4) and says so. The author's card (`contract/cli/src/vocab/card.md`),
`docs/recipes/tests.md` and `docs/diary.md`'s checkpoints name the command and the
receipt; today none of what an app author reads mentions `contract verify`.

### D9. Stage 2: proofs, from the cheap tier up

The direction, not a specification. The twelve theorems are four shapes, and two of them
are `decide` today: `rotate_only_turns` is `runAction_untouched` with
`Stmt.noAssigns`, and `advance_keeps_user_state` is `Reachable.advance_untouched`
(`Invariant.lean`). A kept `only writes` or `after clock` line is the first `--prove`
candidate, with the gap between check and template stated: the check says no other
root slot changed, a `queue` mutation's included; `Reachable.advance_untouched` and
`Reachable.slotIn` require `isQueue p x = false` and so show a non-queue slot
untouched and nothing of a queue slot, and `runAction_untouched` has no queue
exclusion but speaks of one action's body, not of the `then` an input's pair lands.
A line is proved only when every other root slot is shown untouched; an app with a
queue slot gets `unproved` for that slot's sake whether or not the line names it, and
the input pair's frame needs a lemma over the pair before any is proved. That lemma
and `Stmt.noAssigns` looking through action calls (it answers `false` at every
`.call`; a write set over the call graph, with cycles as "may write", is 150 to 250
lines of Lean) are stage 2's first Lean work. Then `always` lines
over states by the `slotIn` recipe (`Reachable.slotIn`, a `SlotOrigin` case,
`BodyKeeps.of_wp`, `simp` with the `EvalR` rules and the number facts), with three
verdicts and never a Lean error: proved; false in the semantics, with D4's
counterexample; unproved, naming the action and the obligation. A property over a
persisted state is proved from a boot whose store satisfied it: `SlotOrigin`'s third
case admits any stored value of the type, so the theorem takes the property at the
store as a hypothesis and the report prints it. The exit criterion comes first and
needs no syntax: the twelve theorems restated as D1 blocks and proved with no
hand-written proof, then the properties kept in D6's measurement. A finite certificate
for joint boolean and string states, generated outside Lean and checked by
`Reachable.invariant`, is the smaller alternative the Astra brainstorm names; expression
properties evaluated by the runner's VM and Lean's `eval` on snapshots, record fields
through the plan's shapes, and `after` with arithmetic (`(old + 1) % 4`) are the other
items.

### D10. Stage 3: a closed list of refusals and termination (the gist's D1, D2)

Direction only. D5's table is the gist's, sitting on the corpus first; when it has, the
theorem's `Legitimate` becomes `Allowed` over the same classes and the three
type-error refusals become `.type`. The Lean checker gains the three acyclicity checks,
the third a language change with a doing-list entry; fuel becomes an inequality on a
structural measure; the `partial def`s on the step path become total.

### D11. Stage 3: `never refuses` proved (the gist's D3)

Direction only. D5 checks it; the proof needs a must-succeed `wp` and its soundness,
and signed interval rules for the numbers.

### D12. Stage 4: a device's world and certificates (the gist's D0, D4, D5)

Direction only. The inventory of the runner's inputs first; then the model; then per-app
certificates for the bytecode and the expansion. Before it, D8 is how a property meets
a failed fetch, and the Muse brainstorm's fault and reload steps under `verify`,
labelled unmodelled, are the search that tells D12 which failures are worth modelling.

## 3. Not proposed

- A property in a component's source, or checked in a running app outside the test
  runner. The deferred list's take stands: `contract` blocks as executable assertions
  stay off the doing-list.
- Properties that are types (`enum<…>` for a closed vocabulary, which the Muse
  brainstorm argues would make Type Tour's proof the checker's). `enum<…>` exists for
  parameters and attributes (`docs/contract-grammar.md`; `Ty::Enum` lowers to the
  plan's string enum, `contract/lower/src/lib.rs`); whether a root state may carry it
  is a type-system question with its own RFC, and D6 may print the vocabulary it saw,
  which is evidence for that RFC.
- An SMT solver or `native_decide` in a proof.
- Making any of this a blocking check. `verify` is the author's command and the async
  lane's; it is advice.
- Liveness, timing, layout and geometry.
- A model of late and failed answers before stage 4; D8 and fault steps are the search
  that precedes it.

## 4. Plan

**Stage 1 (this session), in this order:**

1. **The trace** (D3): the runner's traced-commit record and `ran` lines in
   `observe.rs`; the traced `advance`, `dispatch` and `land` in Lean, run by
   `Contract/Observe.lean` beside the originals with the `trace disagrees` line; the
   kinds left out, listed; corpus cases per kind; `difftest corpus` green.
2. **Delivery and the census** (D4.1, D5): the `land` event on both sides, an input
   followed by `land`, `clock data` as `land`, `clock settle` expanded on the runner
   from its live task state, blocks delivered to their first undeliverable step, the
   refusal classes with the non-finite split at the runner's sites (`Value::typed`
   against `conforms`; the VM's `Arity` apart from the action's), the coverage counts;
   the test clock's advances after each delivered step; the capture of the boot's host
   facts, so an app whose boot calls a host operation outside the oracle diverges at
   that call and not at boot (§5); `--runner-only` and the no-Lean path, with
   `contract verify`'s `lake` check moved after the runner's half.
3. **The blocks and the matcher** (D1, D2): grammar in `contract_syntax` (`always`,
   `after`, `never refuses`; the `hold` and `frame` forms, `stays` included), refused
   beside a component; `contract tests --json` carries them; the matcher over
   authored tests and the explore script; the cone walk; `apps/typetour/`,
   `apps/update-lab/`, `apps/photo-editor/` and `apps/video-player/` get test files
   stating the twelve theorems, so `contract verify` on each checks them; and the one
   line beside the deferred list's node-map entry (Decisions, below).
4. **The walk and the shrinker** (D4.3, D4.4): the stepwise runner driver, the
   adaptive walk with `land` after every input and values from the tests, walks from
   test ends, stored variants in `verify`, the generalised shrinker, Lean confirmation
   of shrunk findings and a sample.
5. **The receipt and the suggestions** (D6, D7), and the demand run over the ten apps
   recorded in §5.
6. **The device runner** (D8): `agent-test.mjs` matches `always` lines after boot and
   after each state-changing step; `bun exact.mjs verify` runs `contract verify` after; the card, the recipe and
   the diary checkpoints.
7. `--js` (D4), last.

Tests: `contract/cli/tests/it/` for the grammar and its refusals; corpus cases under
`semantics/corpus/properties/` (a property that holds; one the explore script
falsifies; a `never refuses` the trace attributes to due work, not the tap; a
`land` whose `then` writes what an `expect` after `clock data` reads; a class-T fixture
per class); a harness unit test per refusal class on canned observation text;
`difftest corpus`, `random` and `quick` green with the new lines. Docs:
`docs/contract-grammar.md` "Authored tests"; `semantics/README.md` "Using it day to
day" and the observation format; `docs/recipes/` one file on stating a property and
reading the receipt.

Stage 1 touches `contract/`, `runner/`'s advance and `semantics/`, so it runs the six
checks and `contract-difftest quick`, and `contract verify` on the four apps and on
Excord for the timing in §5. If the session runs short, items 6 and 7 are cut first,
then 5's suggestions; the trace, the delivery, the census, the blocks and the walk are
the slice.

**From the fourth round, for the implementer**, one line each, beside the decisions
above:

- The data oracle's seed is the case name (`oracle.rs`, `for_case`) and
  `stored_variants` changes the name: hold the seed fixed across a script's store
  variants, or "false only from a filled store" conflates two changed inputs.
- Generate an enum-valued string from its members: the oracle draws unrestricted
  strings where `conforms` enforces membership.
- The shrinker removes single events; keep an input and its `land` together, and
  drop the `land` when the input did not deliver, as the driver does.
- The cone walk runs on the expanded root and the merged file's `fn`s as
  `gates::check` does, respects binders (`Arrow`, `Match`, `Let`), resolves a resource
  through a `Member` base and a bare ident, takes the status names from `scope.rs`'s
  list, classifies `pending`/`failed`/`failure` by the argument's target, and runs on
  the `tests --json` path too, so a device never checks a line `contract verify`
  refuses; corpus cases: a resource reached through a derive chain, `performanceNow()`
  through an `fn`.
- Class T is split by value at every `SlotType` site, slot initializers included
  (`router.rs`, `init_slot`); the VM's `Trap::Arity` stays apart from the action's
  `RunnerError::Arity`.
- `land` is a new `Event` constructor and label in `Observe.lean` and a new arm in
  both of `script.rs`'s printers; the label is byte-identical on both sides.
- A timer due exactly at the clock stands after a `land` and fires at the next
  dispatch, so that step is counted, not checked, for `after`; the receipt's
  multi-commit count shows it, and the format's note says so.
- §5's Lean timing predates the twin, which runs on every step; re-measure before
  `--confirm`'s default is final.
- §5's blanks (multi-commit and per-`after` counts, suggestions kept, the four proof
  apps' receipts, twin and walk timing) are filled in the stage-1 change, and the lane
  logs it cites land with it.
- Charlie's line beside the deferred list's entry is a placeholder until it is in his
  words; items 1, 2, 4 and 5 land first so nothing waits on it.

**Stage 2:** D9, the cheap tier first, deferred by the D6 measurement (§5) until a
kept line is one a reader cannot settle from the code. **Stages 3 and 4:** D10–D12,
each a direction until it has an implementer and a date.

## 5. Measured

Stage 1's lanes measured these on this Mac (an Apple Silicon machine, Lean's
observation cache warm, Cargo's startup included); the lanes' reports under
`target/review-1151/` hold the raw logs.

**Delivery** (lane D, under r5's rule: `land`, inputs followed by `land`, the test
clock's advances after each delivered step, the settle expansion, a settle bound that
does not stop a block), over the 232 blocks of the ten apps' test files, the nested
Harmony app's included:

| App | Whole | Partial | Skipped | Partial blocks stop at | Skipped blocks stop at |
|---|---:|---:|---:|---|---|
| Agents | 0 | 0 | 3 | | `size` 3 |
| Caltrain | 3 | 0 | 0 | | |
| Design Tool | 11 | 41 | 47 | `request` 35; `type … key` 5; `reload` 1 | `request` 44; `fail fetch` 3 |
| Excord | 0 | 85 | 3 | `clock until` 85 | `fail fetch` 2; `size` 1 |
| Harmony | 0 | 0 | 6 | | `fresh` 6 |
| Messages | 0 | 2 | 0 | a tap that is not a press 2 | |
| Metronome | 5 | 0 | 4 | | `midi` 4 |
| Reflow | 3 | 0 | 0 | | |
| Shelf | 9 | 3 | 2 | `reload` 2; `back` 1 | a tap that is not a press 1; `fail fetch` 1 |
| Variable Type | 2 | 0 | 3 | | `drag` 2; `type … key` 1 |
| all | 33 | 131 | 68 | | |

Before stage 1 about a dozen blocks ran at all. Against lane A1b's count under r4's
rule (37, 129, 62): the two Excord blocks the settle bound stopped now run to their
`clock until`; Metronome's four MIDI blocks are skipped, since their first step is a
`prefer-midi` the census now classifies as undeliverable rather than consuming; the
Design Tool's `size` skips became `request` stops once a leading `size` counted as a
launch fact. The next steps worth delivering are `request` (79 blocks) and `clock
until` (85). Lane F (2026-10-30) delivered `clock until` as one `land` and a read, and
a leading `size` as a launch fact: 36 whole, 139 partial, 65 skipped of 240. All 93
Excord blocks that reach a `clock until` wait on `gateway.state`, a field path the
matcher cannot read, so the field adapter D1 defers to stage 2 is what unblocks them
(`QUEUE.md`).

**What delivery exposed.** Two divergences that no run reached before: Shelf's Back
test, at its one-millisecond task, has the semantics emit `setContentIndex` and
`setAppShortcuts` commands on every commit where the runner emits each once; and
Agents' `remove` poisons the runner on a path parameter the semantics accepts. Both
are findings about exact2, not about the apps, and are in `QUEUE.md`. The explore
divergences on eight apps that call host operations outside the oracle
(`registerLoginItem`, `getUserMedia`, `getCurrentPosition`, `selectContacts`,
`addGeofence`, `cancelSpeech`, `exactTime`, a live time) are pre-existing: on
origin/main `8233879705` all eight diverge at boot; after stage 1's capture of the
boot's host facts they diverge at the first such call.

**Wall-clock** (lane D, the twin beside `advance`; `contract verify`, median of three
warm runs, `DIFFTEST_CACHE=1`): with the default twenty walks, Type Tour 2.6 s
runner-only and 3.3 s with Lean, Excord 44 s and 88 s; with `--walks 0`, Type Tour
1.5 s and 2.0 s, Excord 23 s and 47 s. The walks cost Excord 21 s on the runner and
40 s with Lean; Lean's share of a default Excord run is 43 s. The device runner's cost
on the web host (2026-10-30, three warm runs of `verify web --app typetour`: the
authored test with its `always` line checked after every step, the screens at two
widths light and dark, and the relaunch): 10.9 s median inside the run, 20.6 s wall
with the build check, against 1.5 s for the harness's runner-only verify of the same
app.

**The trace.** The projection proof (open question 2) did not close in a page; the
twin runs beside the original on every step and `trace disagrees` has never printed
on the corpus. Commit kinds the trace leaves out: a transport's or MIDI port's answer,
a re-ask, a wake's delivery and expiry, a drop deadline, a transport-end action, and
the advance tail's presence, polls, live times and text-axis frames.

**Steps and `after` lines** (lane D, empty and seeded stores included). Over the ten
apps' default runs, 13,224 observed steps, 785 with more than one commit. The ten
apps have no authored `after` line, so `after` checked and counted only are both
zero and open question 1's trigger cannot fire on them; on the four proof apps every
`after` step was checked (Photo Editor 94, Update Lab 324, Video Player 168; none
counted only). Under the question's stated rule, proofs come before per-commit
observation.

**The proof apps.** All twelve theorems, restated as blocks (Type Tour one, Update Lab
five, Photo Editor three, Video Player three), held on every observed step. Of the
fourteen verifications eight exited clean and six reported runner/Lean
disagreements (Agents, the Design Tool, Excord, Harmony, Photo Editor, Shelf), none
falsifying an authored line; the Design Tool's revert walk, shrunk from 55 to 29
events, is in `QUEUE.md`, and Photo Editor's are host operations outside the oracle.

**Suggestions, and the D6 measurement.** The search printed 394 property blocks over
thirteen apps (Excord 107, the Design Tool 99, Messages 45, Shelf 36, Photo Editor
23, Agents 19, Caltrain 16, Reflow 15, Video Player 11, Type Tour 9, Update Lab 8,
Metronome 5, Variable Type 1; lane D's report has them verbatim). Charlie had no
strong view on which to keep and asked the panel to decide, so on 2026-10-28 four
judges, one per family and blind to each other, read each app as its author and
kept or dropped every block (`llp/reviews/d6-2026-10-28-1151.{astra,fable,grok,muse}.md`).
They kept 30, 12, 133 and 82. Twenty-one blocks had three or more votes, over ten
apps, and those are now in the apps' test files: fifteen frames (`only writes`,
`writes none`, two of them `after clock`), five closed sets (`screen`, `material`,
`ageGate`, `draftStatus`, `filter`, `subject`, `printSize`) and one `whole`. The rest
were what a finite sample shows or what the type says. No judge found a mistake a
diary records that a kept block would have caught: the nearest (a dead `failure`
branch, a read mark sent behind the lock, a timer counted from the wrong moment)
need a mutation's status, a condition or the clock, which D1 refuses. The Fable
judge found at least eleven suggested blocks that are false in the app and only
unreached by the search (a `lock` that also clears `thread`, a `canvasView` with a
third value); none had three votes, and `QUEUE.md` asks for a static pass over an
action's writes and literal writes before a frame or a set is suggested.

**Stage 2, decided.** Three of the four judges said the search earns its keep and
the cheap proof tier should wait; one said build it now, since 74 of its 82 keeps
are frames the templates already prove. The decision, taken for Charlie at his
request: stage 2 is deferred. Every kept line is settled by reading one action's
body, so a Lean verdict adds little to the check that runs on every `verify`; the
templates return `unproved` for every frame on an app with a queue slot (six of
the ten); and the input pair needs its lemma first. Stage 2 opens when a kept line
is one a reader cannot settle from the code (a joint or conditional invariant, or
arithmetic over `old`), which this corpus does not yet hold. The precision pass
above and the coverage the §5 census names (`request`, `clock until`) come first.

**The precision pass, measured** (lane E, 2026-10-30). Before a frame, a set, a
`stays` or a `whole` is printed, `contract/analyze` walks the expanded program for
what the code can do to the slot: an action's frame through its branches, the
actions it calls, the sends it issues and their continuations (a cycle means no
frame); a set from the initializer and every write, callers' arguments included; a
latch refused when any write resets it; a whole number only from whole writes. Over
the thirteen apps the search's candidates fell from 394 printed to 186, with 75
frames widened to the set the code writes and the rest dropped by rule; all thirteen
blocks the Fable judge found false are now widened or dropped, and all 21 kept blocks
are still suggested. The pass costs about 10 ms on Type Tour and 300 ms on Excord. A
suggestion is now observed and code-consistent, not proved; the walk reads no
branch guard, so it can still drop a true property, which the receipt counts.

## Open questions

1. **Steps with more than one commit.** D2 counts them per `after` line. The trigger,
   stated now: if over the ten apps more `after` steps are counted only than checked,
   observing every commit (a format change both sides and every cached case pay) is
   stage 2's first item, ahead of proofs; otherwise proofs first, since the invariant
   rules already reason through every commit of an advance.
2. **The trace's proof.** Stage 1 runs the traced `advance` beside the original and
   fails the corpus when they differ; the one-page projection proof is wanted before
   any theorem is stated over `ran` lines. Stage 1 if it is a page; stage 2's first
   Lean item otherwise.
3. **Record fields and the view.** Out of stage 1 for the reasons D1 gives. Which comes
   first in stage 2: field resolution through the plan's shapes on both observations,
   or expression properties, which subsume it?
4. **The device claim's growth.** D8 checks `always` only. Should the driver's tap reply
   name the action it committed and the commands it issued, so `after` can be the same
   predicate on a device, or is that the per-commit observation's job?

## Decisions needed under the repository's rules

- **The doing-list.** `rules/DEFERRED.md`'s node-map entry (2026-09-10) takes "no new
  Contract syntax until formatting and navigation have landed, and `contract` blocks
  as executable assertions stay off the doing-list". Formatting and navigation have
  landed. D1's blocks are the authored-test grammar's, compile to nothing an app runs,
  and keep executable assertions out of a component's source; this document reads
  them as outside that take, as `clock until`, `fail fetch` and `request` were, and
  that reading is written beside the entry, with the waiver Charlie gave on
  2026-10-28 ("approve waiver yes"), since the list wants a take or a waiver in his
  words.
- **Apparatus.** Stage 1 adds no check. It adds work to `verify`, a traced observation,
  a script event, and a recipe file; Charlie's instruction of 2026-10-27 is the
  approval `rules/RULES.md` "Agents" asks for this document and that work. Stages 2 to
  4 add nothing until they have an implementer.
- **No refine loop.** This document was reviewed blind by the panel in the three
  rounds `rules/DEFERRED.md` "Process" allows and, at Charlie's request on
  2026-10-28, in a fourth; r5 takes what that round left and is the last revision.
  What a reviewer still wants is the implementer's list in §4, not a fifth round. It
  lands when Charlie says so, not when a reviewer does.
