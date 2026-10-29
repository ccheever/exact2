> **Panel provenance.** Family: Grok 4.7 (xhigh) · Provider and runtime: `grok --prompt-file … -m grok-4.7 --reasoning-effort xhigh --output-format plain --disable-web-search` · Date: 2026-10-27 · Redacted: no · Method: brainstorm, not a review: blind (shown no other panelist's output), one panel seat, given LLP 1151 r1 (sha256 `9a6314603673f3fd84041ea77d82cd47a1dc757ba61a3c4e63fa50105e335eec`), James Ide's "A verifiable Contract" and read access to the worktree at origin/main `8233879705`; asked for six to ten ideas that make a Contract app easier, cheaper and more natural to verify, each with what the author writes or runs, what it buys, what it costs and where it fits. The first lines are its progress messages. The text below is the panelist's final message, unedited.

I'll read the semantics guide, James's assessment, and the draft RFC first, then the proof and test surfaces they point at.The three source documents are in. Next I’ll look at the invariant rules, the test language, and what authors actually write.The cheapest honest claim is about the tests and the expanded program an agent already has. `contract verify` today skips a test wholesale at the first step it cannot deliver, turns `expect` failures into notes, and explores only the controls visible at boot. The twelve hand proofs are four shapes of `Reachable.slotIn` and `wp`. None of the ideas below puts a property in an app's source, adds a blocking check, or requires Lean before the first verdict.

## 1. The authored test is the verdict

**What the author writes or runs.** Nothing new. `contract verify apps/shelf/app.contract` changes what it counts as a result.

Today `script::test_case` returns an error at the first step that is not a plain tap, a `type` of text, or `clock +ms`, and `verify.rs` prints `SKIP` for the whole test. Shelf's "the add sheet validates, saves and persists" never has its earlier expects judged, because `clock data` on the following line aborts the test. A `type … key` does the same to variable-type.

Split each test instead of skipping it.

- The deliverable prefix runs on the runner and in the semantics, as now.
- `expect state`, `expect text`, and `expect tree` on that prefix are failures when they do not hold. A field path (`visible.0.id`) is projected from the slot line the observation already prints. An expect whose slot is written by a send, or whose text reads a resource, stays a `NOTE` and says `oracle data`.
- `clock data` and `clock settle` are empty events under the synchronous oracle, and the report says that on the line.
- `reload` runs on the runner only. The runner already carries the store across a reload. The semantics stops at that line: `semantics stopped at apps/shelf/app.test.contract:38: reload`. Expects after it are runner-only.
- `fail fetch`, `back`, `key`, and `request` stop both lanes, naming the step. They are not reported as skipped tests.

When an expect fails, shrink the event prefix with the loop in `difftest/src/lib.rs` (`shrink`), on the runner alone, keeping the expect as the predicate. Confirm the shortest prefix once on both sides if it is still in the prefix.

**What it buys.** For each expect: it holds of the runner under the seeded synchronous oracle, on this prefix, or it failed at a shrunk script. For the prefix: the runner and the semantics agree, or they diverge at a named step. The summary says which expects were not run. This is one script, not every reachable state, and not a phone.

**What it costs.** A few hundred lines in `semantics/difftest/src/script.rs` and `verify.rs`, plus one corpus case whose `reload` sits between two expects. The main risk is reading `clock data` as a no-op and missing a bug that depends on a late answer. The report line is the mitigation. A second risk is failing data apps for oracle noise. The resource cone keeps those as notes.

**Where it fits.** Stage 1, ahead of new syntax. The RFC treats an `expect` as one point of one script and therefore not worth searching. The point is real for a later proof. It is the wrong reason to ignore the only specification agents already write.

## 2. Discharge `slotIn` by a fragment walk

**What the author writes or runs.** Nothing. `contract verify` prints one block, computed from the expanded program:

```
fragment (expanded program, trusted walk; not a kernel proof):
  screen is one of lock, home, settings, display, messages
    from the initializer; every handler of go passes one of those literals
    a stored value is outside this claim (see STORE)
    a host payload appended past the curried argument is outside this claim
  counter stays an integer while |counter| ≤ 2^53, from 1
    the only write is counter = counter + 1; endpoints 1 and 2^53 ran on the runner
  ready: the only write is true; once it is true, a press or a clock keeps it
  tsResult is none or some(answer); no action assigns it
  advance leaves counter, note, live unchanged
  draft#4 is Note.draft; not in a fragment; writers: set, clear
  turns is a queue mutation's slot; Reachable.slotIn does not cover it
STORE screen "other" conforms and is restored
  the screen line above does not hold from that store
```

The walk follows the hypotheses of `Reachable.slotIn`, `BodyKeeps.untouched`, `BodyKeeps.of_noAssign`, and `Reachable.advance_untouched`:

- Skip a slot with `isQueue`, and say so.
- Boot value from the initializer. A persisted slot also gets one adversarial boot: a value of its type outside the claimed set, through the existing `Oracle::with_store` path. `verify` does not run that path today. The corpus does, and it drops expects.
- Writers are assignments and sends in the expanded body. Inline calls. `Stmt.noAssigns` refuses to enter a call. The walk enters calls and says that it is stronger than that predicate.
- Literal image, which is Type Tour. Every write of the slot is a literal, or a parameter, and every entry of the handler list passes a literal for it, and no clock action writes it. The set is the initializer plus those literals. A press carries an empty host payload. The claim is about that press. It does not quantify over a forged payload the way `hhandler`'s `vs ++ payload` does.
- Affine counter, which is Update Lab's `counter`. The only write is `x = x + k` or `x = <literal>`, `k` a numeric literal, no send into `x`. State the `2^53` bound in the line. Run the body at the endpoints on the runner.
- Flag, which is Photo Editor's `ready`. The only write is `true`.
- Shape, which is Update Lab's result slots. No action assigns it, so a mutation stays `none` or `some`.
- Clock. Every clock action has no assign and no send into the slot, so `advance` leaves it.

Lifted slots `name#n` are printed with the component state they came from, through the source map `compile_path_mapped` already returns.

**What it buys.** For each slot the walk classifies: the same conclusion as the twelve theorems, about the expanded program, from boot's initializer, for presses and clock actions, with queue mutations excluded. The walk is trusted. Lean is not in the claim. A stored value is a separate sentence, so a diary reader can see that an update which tightens the set does not hold on a device that kept the old one.

Run once over `apps/*/app.contract` and count slots per fragment. That is the demand measurement, with no author and no new syntax.

**What it costs.** About 400 to 700 lines in `semantics/difftest/src/fragment.rs`, called from `verify.rs`, and a test that the four apps' printed lines match the conclusions of `Apps/Proofs/*.lean`. The main risk is drifting from the Lean predicates. Pin the four apps. The second risk is inlining a call the body reaches only on a dead branch. The line says `may write`.

**Where it fits.** Stage 1, beside the RFC's D5. D5 suggests a property because a sample happened to satisfy it. This decides the slots whose shape is exhaustive, and it should not then suggest them as observations. It disagrees with waiting for stage 2's kernel recipe before an agent can see these claims. It does not replace that recipe. It names the trusted walk the recipe would remove.

## 3. Search the view after each event, and aim at actions

**What the author writes or runs.** Nothing. The same `contract verify` grows a search under the report above.

`explore` in `difftest/src/lib.rs` reads the boot observation and taps those test ids twice. A control that appears after a tap is never pressed. The RFC's walk draws from that same boot set, so Type Tour never reaches a screen behind the lock.

Replace the event source.

- After every event, read the observation again. The next tap is a test id on screen now.
- A `type` draws its string from a `type` step for that test id in the app's test files, then from one value of the input's kind. That answers the RFC's open question on typed values: the tests already name the strings that open a screen.
- A clock advance is the smallest task interval, and also 60 seconds, so a timer and a `then` both get a chance.
- Keep the seeded 20 walks of 40 steps. Record which action ran from `observe::site` and the plan's handler on that node. The outcome line does not name the action today.
- Then one directed search per action: breadth-first, up to 40 events, stop when a delivered event's handler names it. An action with no rendered handler, or none reached, is `not reached`, with its test ids or `no handler rendered`. It is not a pass.

A violation of a fragment line, an expect, or a later property shrinks with the existing shrinker on the runner, then one confirmation on both sides.

**What it buys.** Every reached commit of the walks and the directed scripts satisfies the fragment lines and the expects, on the runner and in the semantics, in the synchronous world. Actions not reached are listed. A violation names the seed and the shrunk events. This is still a search: an unreached action can break the fragment for a reason the endpoint run did not hit, and the fragment section is what covers that action when it classified the slot.

**What it costs.** About 250 lines in `verify.rs` and `lib.rs`. The main risk is a walk that loops on one screen. The directed search is the backstop, and `not reached` stays a visible count. The second risk is treating a handler the site resolver missed as unexercised. Use the same `observe::site` the divergence report already uses.

**Where it fits.** A change to the RFC's stage-1 search. The explore observation is the wrong pool. Re-observing is what makes that search able to falsify a property of a screen the app actually has.

## 4. Predicates on the observation, including `after`

**What the author writes or runs.** Lines in the test file, parsed by the harness, not by the app compiler:

```
always slot screen in "lock" "home" "settings" "display" "messages"
always slot counter >= 1
always slot counter whole
always derive title == "Messages"
after "rotate" slot turns == old + 1
after "rotate" only slot turns
after "done" command focus "done"
after "done" writes none
never refuses "reset"
```

Checked on every chunk of every script, both sides. `old` is the previous chunk's slot line. `chunks` in `lib.rs` already splits the observation that way. `never refuses` is an `outcome refused` on a step whose handler names the action. A name that is not a slot, a derive, or a command is an error at the line, with that repair. A line that reads a resource is checked and stamped `follows the oracle`.

**What it buys.** On the scripts of idea 3: the line holds of both observations, or a shrunk script shows it false. `after "rotate" only slot turns` is Photo Editor's `rotate_only_turns`. `after "done" command focus "done"` is `done_focuses`, because command lines are already in the observation. The twelve theorems' relational half (`increment_adds_one`, `reset_spec`, `toggle_flips`) is this shape.

The limit, printed once per run: delivery runs due work before the event, inside one chunk, so `old` is the state before that delivery, and a timer that wrote the slot is part of the step. A line checked only on steps whose clock did not move is stamped `no due timer`.

**What it costs.** About 200 lines of matching in `verify.rs`, and a small parser beside `script.rs`. No change to `contract/syntax`, settlement, or fuel. The main risk is a diary reader treating `old` as the action's pre-state when a timer ran first. The stamp is the mitigation.

**Where it fits.** A replacement for the RFC's stage-1 mechanism. The RFC compiles each `always` into a hidden derive so both evaluators run Contract. That derive can refuse a commit the app would have committed, and it has no pre-state, which is why the RFC delays `after`. The observation is already the differential spec. A matcher on its lines is one implementation. It cannot drift between runner and semantics except when the observations differ, which is already a divergence. Stage 2's kernel proof then translates this vocabulary into `SlotIn` and `wp` postconditions. It does not have to accept arbitrary Contract in a property.

## 5. Replay the host run, and label each step

**What the author writes or runs.**

```
bun scripts/agent.mjs web --test apps/shelf/app.test.contract --trace target/verify/shelf.json
contract verify apps/shelf/app.contract --trace target/verify/shelf.json
```

The trace is one record per step the driver already performs: the event, the test id, each source call with its answer or its failure, the store, the refusal reason, and the slot values. `contract verify --trace` replays the prefix the semantics can take. The Lean side already consumes the runner's transcript.

A step whose answers are synchronous, successful, and of the declared shape is replayed and compared. A failed fetch, a late answer, a `reload`, a `back`, or a `key` is `OUTSIDE` at that step. The expect on it is `held on web` or `failed on web`, and `not checked in the semantics`.

**What it buys.** Per step, one of three sentences. The semantics agrees with the runner on the app's own answers. The step is outside the model, and the host expect held or failed. The two sides diverged, at this step of this test. A person reading the diary sees which of the agent's bugs the semantics could have seen. A failed fetch that the model cannot represent stays outside, which is the honest form of the gap where `failed` is constantly false.

**What it costs.** A trace writer in `scripts/agent.mjs` and a transcript player in `semantics/difftest/src/oracle.rs`. Failed answers are not added to the Lean oracle. Those steps stay `OUTSIDE`. The main risk is a host fact (viewport, locale) that the semantics only has at boot, reported as a divergence. Classify those as `OUTSIDE` unless the values match the boot facts. This needs the same human approval the RFC already records for stage 1, because it extends `agent.mjs`.

**Where it fits.** The step after idea 1, and a different direction from the RFC. The RFC refuses to check anything under the agent driver, and it refuses a claim about a device until the semantics models the device. This does not extend the model. It separates the steps of a test the agent already runs into the ones the model can replay and the ones it cannot.

## 6. A refusal census that fails closed on class T

**What the author writes or runs.** Nothing. Every `#` reason the observation already prints is classified. `compare` in `lib.rs` strips `#` lines before it diffs, so both sides can refuse for a type error and the case still agrees.

Put a table beside `BOUNDS`:

| Class | Rust | Verdict |
|---|---|---|
| T, a type error | `SlotType`, `DeriveType`, `Trap::Arity`, and `ArgumentType` when the value is finite | fail the case |
| V, a boundary value | `ArgumentType` of a non-finite number, `TaskKey`, `NonFiniteClock` | print |
| W, the world | `Data`, `Shape`, `NoHandler`, `UnknownView`, `InvalidValue`, host `Arity` | print |
| L, a limit | `TimerFireLimit`, `QueueFull`, fuel | print |
| P | `Poisoned` | print |
| C | `Cycle` | print |
| app | a refusal whose text comes from the body (`range` of a negative count) | print |

Lean's reason strings get the same prefixes, tested by one program each. A class the table does not know is printed as `unclassified` and does not fail.

```
refused: 1 app, 0 of class T
  reset at step 2: # range(-1): a count is a whole number of at least 0
```

A `never refuses "reset"` from idea 4 turns an `app` row for that action into a failure. The census reports it either way.

**What it buys.** On these scripts, no step refused for class T on either side, including a step where both sides agree. That is the checker-bug shape the headline theorem allows, caught here only when one of these scripts reaches it. Other classes are listed so a diary reader can see a fetch failure without mistaking it for a proof that the action succeeds. The theorem is unchanged.

**What it costs.** Under a hundred lines in `lib.rs`, plus a fixture per class. The main risk is marking a host payload's arity as class T and failing a test that sent the wrong event. Host `Arity` stays class W. Internal `Trap::Arity` is class T.

**Where it fits.** A cheap addition to stage 1, and a different order from the gist. The gist keeps the harness comparing what it compares and repairs the theorem later. The harness can tell these classes apart now. The theorem's closed list can follow the table once the table has sat on the corpus.

## 7. What a send may read

**What the author writes or runs.** Nothing for the report. One optional line in the test file when the diary depends on it:

```
sends "save" unread otherDraft
```

The report, from the expanded body:

```
sends (may read; a dead branch counts; the oracle's answer does not):
  save reads draft, channelId
    action save, apps/excord/app.contract:120
```

The walk is `free_names` on each send's arguments and on the conditions of the `if` and `match` that guard it, inlining calls the way idea 2 does. A name read only to choose between two constants still counts. A slot that influences the send through the oracle, or through which row is live, does not appear, and the header says so.

The optional line fails verify when the cone contains the name.

**What it buys.** A syntactic may-read of the expanded action, which is the check for "the save took the other channel's draft." It is not non-interference. Two runs that differ only in a slot outside the cone can still differ in the send if the walk missed a path. The header is the limit.

**What it costs.** About 200 lines, reusing `contract/syntax/src/inline/subst.rs` (`free_names`) or the reads walk in `contract/lower/src/anatomy.rs`, over the expanded statements the compiler already built. The optional line is the same parser as idea 4. The main risk is under-approximation from a call the inliner did not enter. An unentered call makes the cone `unknown` and fails an `unread` line, rather than passing it.

**Where it fits.** Stage 1, and a different direction from the gist. The gist holds "a state never influences a send" until there is a relational proof rule. The bug an agent writes is a name that should not have been in the argument. The cone answers that without a proof, and it says it is a cone.

## 8. A `wp` in Rust for the obligations the fragment leaves

**What the author writes or runs.** Nothing new, and no Lean. For a slot idea 2 printed as `not in a fragment`, and for an `always` or `after` line of idea 4 that mentions more than one slot, `verify` runs a weakest precondition in the shape of `Axiomatic.lean`'s `wp`:

- The domain is finite sets of strings, bools, `none`/`some` of those, and integer intervals inside `[-2^53, 2^53]`. Anything else is top.
- `assign` evaluates the expression in that domain. `if` and `match` join. A call is the callee's `wp`, as `callPre_of_wp` allows, not `Stmt.noAssigns`.
- Multiplication and division are not evaluated. The obligation stays open and names them.
- An obligation that stays inside the claimed set is `discharged`. Each integer bound is also run as a concrete pre-state on the runner. A concrete failure demotes the line to a counterexample.
- An obligation that joins to top is `open`, naming the action and the expression, which is the place a true property that is not inductive shows up.

```
counter, note: open at setNote
  note = draft may be any string; the interval does not apply
screen, phase: discharged
  both stay in their literal sets; endpoints ran on the runner
  trusted abstract walk; not a kernel proof
```

**What it buys.** A two-slot invariant, which `slotIn` does not state, discharged or left open by a syntax-directed walk of the expanded bodies. The runner has executed the integer endpoints. The walk is trusted. A discharge is not a theorem of the kernel, and the line says so. It holds of the expanded program in the synchronous world, from the initializer, with the same queue and payload limits as idea 2.

**What it costs.** On the order of a thousand lines in `semantics/difftest/src/wp.rs`, mirroring `wp`, `assignPre`, and `callPre`. Pin it by re-discharging the fragment lines of the four apps. The main risk is an unsound join that the endpoints miss, especially around rounding. Refusing `×` and `÷`, and refusing any interval that is not a subset of the exact-integer range, is the mitigation. The second risk is a long body blowing the verify budget. Cap the walk and print `open (not evaluated)` past the cap.

**Where it fits.** After ideas 2 and 4, and before the RFC's kernel stage. The RFC's first universal claim is a generated Lean proof. An agent gets a usable answer from a walk that runs in the same command, with Lean absent. The kernel proof is how that `trusted` word comes off a line the walk closed. It is not how the agent finds out the property is false.

## 9. The JS target in the same report

**What the author writes or runs.** `contract verify --js apps/shelf/app.contract`. With the flag absent, the last line is still `js: not run`.

The driver in `semantics/difftest/src/js.rs` already runs a case against the web JS module and compares observations with the runner. `verify.rs` does not call it. The async lane runs it over the corpus and explored programs, and a failure there is reported after the push.

Wire the same cases idea 1 delivered, and the walks of idea 3, through that driver. Expects and observation predicates are checked on the JS observation too. A program the JS compiler refuses is `OUTSIDE-JS`, as the driver already prints.

**What it buys.** On these scripts, the JS module's observation matches the runner, and the expects hold there. The limits are the ones the driver already has: `sin` and `cos` are answered with the runner's bits, a virtualized list's descendants are omitted, and a fetch the runner never made is a refusal. This is the artifact `host/web/dev.mjs` serves. It is not a phone, and it is not the semantics.

**What it costs.** A call from `verify.rs` into `js.rs`, and the summary line. The main risk is doubling a warm verify. Keep it behind `--js`, and keep the `js: not run` line so a green report cannot be read as covering the web module.

**Where it fits.** A cheap addition to stage 1. The RFC's stage-1 claim is the runner and the semantics. The web app an agent is editing is the JS module. The comparison for it already exists.

---

What I would not build first: a hidden derive, a Lean proof, or a model of late and failed answers. The derive changes settlement to evaluate a predicate the observation already contains. The proof removes the words `trusted walk` from lines ideas 2 and 8 have already classified, and it is worth doing for the lines those walks close on apps chosen without regard to difficulty. The device model is what makes a failed fetch a counterexample inside the semantics. Until it exists, idea 5's `OUTSIDE` line is the true sentence, and a green semantics run must keep saying `synchronous oracle`.
