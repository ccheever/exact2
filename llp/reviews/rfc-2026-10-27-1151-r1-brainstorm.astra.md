> **Panel provenance.** Family: GPT-6 Astra (max) · Provider and runtime: `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only` · Date: 2026-10-27 · Redacted: no · Method: brainstorm, not a review: blind (shown no other panelist's output), one panel seat, given LLP 1151 r1 (sha256 `9a6314603673f3fd84041ea77d82cd47a1dc757ba61a3c4e63fa50105e335eec`), James Ide's "A verifiable Contract" and read access to the worktree at origin/main `8233879705`; asked for six to ten ideas that make a Contract app easier, cheaper and more natural to verify, each with what the author writes or runs, what it buys, what it costs and where it fits. The text below is the panelist's final message, unedited.

Eight ideas, in priority order. New syntax and flags below are proposals. Costs are rough engineering estimates from reading this checkout, not measured results.

1. **Let a test remember an earlier observation.**

   **What the author writes or runs:**

   ```contract
   test "Cancel preserves the library"
     remember books = state lib.books
     tap "add-book"
     type "add-title" "Draft"
     tap "add-cancel"
     expect state lib.books == books
   ```

   `remember` captures an immutable value in the test interpreter. The same small addition expresses “reload preserves this,” “opening and closing changes nothing,” and “reset twice has the same result as reset once.” Start with equality between observations; general expressions are unnecessary.

   **What it buys:** Intent expressed across a user journey, without hard-coding the fixture’s current book count. It checks the observed values on whichever implementation executes the test: a device, the Rust harness, or Lean’s observations. It remains a claim about those executions and checkpoints, not every possible journey. Equality of the displayed library does not by itself establish equality of the underlying database.

   **What it costs:** About 2–4 days across the test AST/parser, [tests_json.rs](/Users/ccheever/projects/exact2-wt-verify/contract/cli/src/tests_json.rs), [agent-test.mjs](/Users/ccheever/projects/exact2-wt-verify/scripts/agent-test.mjs), and difftest’s `script.rs`/`lib.rs`. Copy captured values and compare structurally. The main risk is capturing incidental timestamps or unrelated state; require an explicit observation or field path.

   **Where it fits:** An early addition beside stage 1. This materially differs from `after action old(x)`: the boundary is the author’s chosen journey, which can include multiple actions, replies, Back, and reload. It needs neither action instrumentation nor Lean expertise.

2. **Attach evidence to each promise through the existing Verify command.**

   **What the author writes or runs:** Keep ordinary tests and use one entry point, for example:

   ```sh
   bun scripts/verify.mjs web --app shelf --semantics
   ```

   Its report joins results by test file, name, and assertion location:

   ```text
   discover shows an error, then tries again
     web fixture: 2 expectations passed
     model: skipped — fail fetch is outside this harness
   ```

   Include the source/build identity and a compact line suitable for the diary’s Checkpoints.

   **What it buys:** A reader can distinguish an assertion passing on an actual host from implementations merely agreeing. Today, [difftest verify](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/verify.rs) makes failed authored expectations notes because its data is synthetic; [script.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/script.rs) also omits some expectation kinds. Agreement therefore cannot stand in for checking the author’s promise.

   Report assertion results, implementation agreement, and proofs separately. List omitted assertions individually. Preserve the distinction between actual app data and the synthetic oracle.

   **What it costs:** About 1–3 days extending existing JSON results in `scripts/verify.mjs`, `agent-test.mjs`, and difftest verification. Reuse `.exact/verify` output rather than introducing a registry. The main risk is associating evidence from different builds or environments; join only matching identities.

   **Where it fits:** A cheap stage-1 addition. It connects the RFC to the verification command app authors already use. Device testing need not wait for the device’s world to be formalized.

3. **Check whether the assertions notice a deliberate mistake.**

   **What the author writes or runs:**

   ```sh
   contract verify apps/update-lab/app.contract --challenge 6
   ```

   Try at most six temporary, well-typed changes: remove an assignment, flip a branch, replace an assigned literal, or drop a send. Keep the original properties and test expectations unchanged.

   A useful report would say:

   ```text
   Removing increment's write was not detected.
   “counter >= 1” still held on every exercised step.
   ```

   **What it buys:** Evidence that the specification distinguishes selected wrong behaviors from the current implementation. This directly addresses agents writing an app and then writing assertions that merely describe its output. A surviving mutation identifies an unanswered question; it does not establish a bug or yield a meaningful “percent correct.”

   **What it costs:** About 3–5 days. Reuse the AST traversal in [types.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/types.rs), but add behavior-changing, type-preserving mutations. Run them through existing verification with the same seed, store, and oracle policy. Count only an assertion detecting the change; compilation rejection and unrelated execution errors get separate results. Main risks are equivalent mutations and excessive runtime, so bound both candidate count and elapsed work.

   **Where it fits:** Stage 1, initially runner-only and optional. This adds a test of specification quality that the RFC’s observed-property suggestions do not provide. It requires no proof toolchain.

4. **Explore outward from working user journeys.**

   **What the author writes or runs:** Ordinary tests supply the starting points:

   ```sh
   contract verify apps/shelf/app.contract --around-tests
   ```

   Replay supported test prefixes, then explore short continuations from their reached states. Refresh the available controls after every event. Prefer newly encountered handlers, reuse typed inputs from the tests, and try boundary edits such as empty text, a second press, or a clock move immediately around a timer deadline.

   **What it buys:** Better use of a fixed search budget. A test that reaches an edit sheet supplies the difficult setup; the search can then investigate that sheet. The RFC’s walk over controls discovered by its initial explore can miss controls created later.

   Check authored global properties or explicitly declared relations during exploration. An original test’s final expectation is not automatically valid after arbitrary additional input.

   **What it costs:** About 3–5 days in [observe.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/observe.rs), `verify.rs`, and [lib.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/lib.rs). Generalize the existing shrinker to preserve the **same property violation**, not merely any divergence. Freeze the oracle seed and initial store while shrinking. Print a replay command and candidate test; label synthetic-data failures until reproduced with the app’s fixture.

   **Where it fits:** A replacement for stage 1’s boot-derived random walk. Dynamic discovery and preserved setup prefixes materially change where the search spends its effort. State summaries may guide prioritization, but must not support an exhaustive-search claim.

5. **Evaluate properties without changing the app being checked.**

   **What the author writes or runs:** The RFC’s `always` syntax stays unchanged. Internally, compile predicates into a separate probe bundle against the app’s checked slot and derive layout.

   Feed immutable snapshots into the existing Rust VM and Lean evaluator. A probe may return true, false, or an evaluation error; it cannot roll back or poison the app’s transaction.

   **What it buys:** Findings concern the original app. The RFC acknowledges that a hidden derive can refuse a transaction that would otherwise succeed. Separating evaluation removes that interference without introducing a third expression interpreter: [vm::eval](/Users/ccheever/projects/exact2-wt-verify/runner/src/vm.rs:405) and Lean’s `eval` already exist.

   First observe event boundaries. Before claiming “every commit,” add observation inside clock advances and queue drains, including poisoned outcomes. Retain the responsible action and outcome so a preceding timer’s refusal is not attributed to the button pressed afterward.

   **What it costs:** Roughly 1–2 weeks: a compiler entry for probes, snapshot adapters, difftest observation changes, and eventually hooks in `runner/commit.rs`, `runner/advance.rs`, and Lean’s runtime/observation path. The main risks are mismatched scope/index mappings and incomplete observation boundaries. Check that enabling probes leaves the application trace unchanged.

   **Where it fits:** A deliberate replacement for D2, with event-boundary checking as the first increment. It costs more initially, but gives `after` and device-side checking a reusable foundation. It also forces an honest distinction between “after each event” and “after each commit.”

6. **Try a few adverse reply schedules before formalizing all asynchronous behavior.**

   **What the author writes or runs:**

   ```sh
   contract verify app.contract --runner-only --schedules 8
   ```

   Keep answer values fixed while varying delivery: immediate; after another user input; a superseded reply arriving late; or one failed request followed by a successful retry. Use existing `before data`, `fail fetch`, and fixture-control requests when reproducing a finding through the device driver.

   **What it buys:** Concrete tests of the Rust runner executing the app under specified asynchronous conditions. This can expose stale-result overwrites, premature success messages, and retry mistakes while Lean still assumes synchronous successful answers.

   The result must name the schedule and say “runner with scripted data source.” It proves nothing about all schedules or the real service. A bounded unanswered request is not automatically a liveness violation.

   **What it costs:** About 1–2 weeks for a narrow Rust implementation. Extend [oracle.rs](/Users/ccheever/projects/exact2-wt-verify/semantics/difftest/src/oracle.rs) and the harness using the existing request/ticket/fulfillment seam; [queue.rs](/Users/ccheever/projects/exact2-wt-verify/contract/cli/tests/it/queue.rs) already demonstrates immediate and deferred answers. Shrink schedules along with input events. Main risk: generating impossible orderings. Respect ticket lifetimes and serialized queues; reorder only requests the interface permits to overlap.

   **Where it fits:** An empirical addition ahead of stage 4. It does not require the broad D0 inventory and proof migration before becoming useful, and its counterexamples can identify which asynchronous semantics deserve that investment first.

7. **Prove what an action cannot change.**

   **What the author writes or runs:** Offer an effect summary:

   ```sh
   contract verify app.contract --effects
   ```

   An author could retain a small expectation:

   ```contract
   writes "rotate" only turns
   ```

   Report possible root writes, mutation sends, refreshes, and host commands, including those in synchronously called actions.

   **What it buys:** A cheap, inspectable answer to “could this button disturb something unrelated?” Prove an upper bound on an action’s effects in the embedding, then use `runAction_commit` and the existing untouched-slot laws to derive preservation of other root slots.

   This does not prove the action succeeds, that its permitted writes are correct, or that a data source performs no external side effects. Body sends and settlement-triggered queries must remain distinct in the report.

   **What it costs:** About 4–8 days. Extend [Invariant.lean](/Users/ccheever/projects/exact2-wt-verify/semantics/Contract/Invariant.lean:868): its current `noAssign` and `noSend` conservatively reject every action call. Compute summaries through callees and prove them against `ExecR`, using [callPre_of_wp](/Users/ccheever/projects/exact2-wt-verify/semantics/Contract/Axiomatic.lean:243) where useful. Cycles or unresolved callees yield unknown. The main risk is overlooking sends or later callbacks while presenting a direct-write summary as complete.

   **Where it fits:** An early, narrow part of stage 2. It provides useful proofs before numerical automation, and reduces later invariant obligations to the actions that can actually affect the property.

8. **Give finite UI state a small proof certificate.**

   **What the author writes or runs:** An ordinary property and the proposed proof command:

   ```contract
   always "editing belongs to the detail screen"
     !editing || screen == "detail"
   ```

   ```sh
   contract verify app.contract --prove
   ```

   Initially support booleans and strings with statically finite origins. Infer a joint set of possible `(editing, screen)` values from initialization, assignments, and literal handler arguments.

   **What it buys:** An unbounded invariant of the embedding through a small induction certificate: initialization belongs to the set; every modeled transition preserves it; every member satisfies the authored property. Joint states retain correlations that separate per-slot bounds lose.

   Generate the candidate set outside Lean, but have Lean check the closure obligations using `wp` and `Reachable.invariant`. Unknown branches contribute both possibilities. Exclude persisted state and unsupported dependencies initially. A failed certificate means unproved; an abstract bad state becomes a counterexample only after replay establishes reachability.

   **What it costs:** Roughly 2–3 weeks for a deliberately small fragment: a generic certificate checker and soundness theorem beside `Invariant.lean`, plus generation in difftest verification. Account for handlers, tasks, and queue drains—or explicitly refuse programs outside the supported fragment. Cap the tuple set, perhaps at 256 entries. State explosion and accidentally incomplete transition coverage are the main risks.

   **Where it fits:** A smaller alternative within stage 2. I would ship this restricted prover before requiring automation of all twelve existing hand proofs, especially the binary64 counter theorem. It gives authors useful proofs with no handwritten Lean and no new type syntax, while retaining the existing limits between an embedding proof, tested implementation correspondence, and device behavior.