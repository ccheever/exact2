# Review: LLP 1107: Settled, and the two clocks (r1, astra)

> **Renumbered 2026-10-08.** These RFCs were drafted as LLP 1105, 1106 and 1107 in a checkout 542 commits behind origin, where those numbers had since been allocated to other documents. They were committed as LLP 1111 (apps as tools), 1112 (a URL under a state) and 1113 (settled and the two clocks); LLP 1101.003 kept its number. The reviews below are transcribed as returned and keep the numbers they were written against: read 1105 as 1111, 1106 as 1112, 1107 as 1113.

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the four drafts (1101.003 r4, 1105 and 1106 parked, 1107) and only Astra's own earlier reviews; 2026-10-08T01:49:32Z to 2026-10-08T02:05:06Z.
- **Method:** one brief (sha256 `9a0edf89ff0d5d76983699f7dcf76be52cf8011440bfc45ce03d8e2eda74d6f9`), shared verbatim with the other reviewer; round 1, blind to the other review. Requested by Charlie ("sounds good, do the review with astra and grok"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r2 (2026-10-08) takes all nine. C1 is removed at the root: a capture is a sample of a cloned engine, no clock moves, and LLP 1012 §2 is unamended (D1). C2: `dispatch_settling` with timers off at five callers (D2). C3: each pass runs host commands and drains scroll events and announcements. C4: no seek, so no inner loop, and `exhausted` is described as a budget. C5: four helpers return errors, with an error shape (D3). C6 and C7: fulfilment captures the id before `remove`; a kept ticket gets LLP 1105's outcome; an overwritten `then_due` is superseded; streams and background work are refused; timers and announcements are outside the set; one lock per `ExactApp` (D4). C8: two pass kinds, and `clock settle` keeps its world and device results (D5). C9: citations corrected.

---

**1. Overall assessment.** The extraction is worthwhile, but 1107 is not ready to implement. The clock separation is conceptually sound; the proposed changes do not yet enforce it. D3 can declare completion with queued work remaining, and D4 needs ownership rules for reused or overwritten work.

This review covers 1107 at `2ce193976`, with the siblings used as context. All verification was by source inspection. I changed no files and ran no builds; this worktree has no `target/` directory.

**2. Strengths.**

- **§0:** One authority for settling should prevent the documented sibling drift. Separating settlement from publication policy, invocation outcomes, and process shutdown is appropriate.
- **D2:** Keeping application time unchanged during a presentation-only seek is a defensible policy. The existing monotonic motion clamp supports starting subsequent transitions from the displayed state.
- **D3:** Repeating data, motion, painting, and image processing is the right overall shape. In particular, painting before checking newly visible images addresses a real dependency.
- **D4:** Including queued sends, armed continuations, and resource refreshes fixes the premature-completion problem that waiting only for result dependencies would leave.
- **D5:** The command-line column substantially agrees with **1101.003 r4 D2**: no deferred activation, live timers, future armed `after` work holding the run, no `every`/`frame` hold, synchronous images, whole-tree busy checks, and no round cap.

The principal current-behavior claims I verified are:

| Claim | Source verification |
|---|---|
| Linux presentation time can advance without runner time; dispatch currently advances the runner to host time | [host.rs:851](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/host.rs:851), [host.rs:1175](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/host.rs:1175). |
| Reply receipts currently use host time; motion clamps receipt time against engine time | [host.rs:794](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/host.rs:794), [host.rs:1266](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/host.rs:1266). |
| `land_then` disables timers while retaining due `then` and queue `next` processing | [commit.rs:285](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:285), including selection at lines 323–345. |
| First-pixel activation requires a clean successful frame; pumping processes announcements without replies | [delivery.rs:104](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter/delivery.rs:104), [presenter.rs:1317](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter.rs:1317). |
| Finite motion determines settle time; infinite and paused animations are excluded | [engine.rs:672](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/motion/src/engine.rs:672), [animate.rs:362](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/motion/src/engine/animate.rs:362). |
| Frames refine collections, record boxes, then synchronize images; failed painting can return retained/white pixels | [content_region/presenter.rs:49](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/content_region/presenter.rs:49), particularly lines 63, 147–180, 195 and 226–230. |
| Image pending includes unfinished decode/admission; `Prepared::Failed` ends that wait | [image.rs:412](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/image.rs:412). |
| Requests are removed before `then` is armed; queued sends exist before requests; fulfillment can force resource refreshes | [commit.rs:1217](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:1217), [queue.rs:109](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/queue.rs:109), [commit.rs:1347](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:1347). |
| Terminal images decode synchronously after layout; its host has no motion engine | [terminal/host.rs:88](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/terminal/src/host.rs:88), including layout/image handling at lines 204–257; [terminal/image.rs:147](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/terminal/src/image.rs:147). |

The incorrect or incomplete claims are addressed below.

**3. Concerns.**

**C1 — BLOCKING — D2: The proposed clock split does not cover subsequent clock operations or their replies.**

**Evidence:** `clock_within` takes its starting time from `host.now()` and rejects targets below it ([agent.rs:802](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/agent.rs:802), lines 814–815). `Presenter::clocked` also returns `host.now()`, including when called by `land_then` ([presenter/clock.rs:20](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter/clock.rs:20)).

Consequently, after a capture leaves runner time **0** and presentation time **1000**:

- The driver’s `clock +100` requests **100**, which Linux rejects as backwards.
- Successful `clock data` and `clock land` replies can report **1000**, despite the runner remaining at **0**. Fixing only the unsettled reply at `agent.rs:740` is insufficient.
- Starting a transition at presentation time 1000 leaves undefined whether subsequent `clock +100` advances that transition or merely advances the runner toward it.

These are deductions from the call paths, not executed reproductions.

**Resolution:** Define both clocks’ updates for numeric `clock`, relative `clock`, `clock settle`, `land`, and capture. Then update clock validation, returned landing times, and time-derived facts—not only dispatch and fulfillment stamps. Explicitly amend LLP 1012’s statement that `clock` moves both clocks to one instant ([1012:296](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/llp/1012-agent-api-v1.spec.md:296)).

**C2 — BLOCKING — D2: `advance_timed(runner_now)` does not guarantee that no timer fires.**

**Evidence:** Timer selection uses `next_ms <= now_ms`, not strictly earlier times. An advance can stop at a timer’s due time because of refusal, leaving another timer due at that same instant. It can also stop after a request-producing timer. See [commit.rs:319](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:319), [commit.rs:468](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:468).

For example, two timers are due at 1000; the first refuses. Runner time is now 1000, but the second remains due. D2’s next dispatch calls `advance_timed(1000)` and fires it.

Thus “everything due by the runner’s clock has fired” is false.

**Resolution:** Enforce timer suppression explicitly in the dispatch path. Use a timer-disabled advance/dispatch policy, preserving the intended continuation ordering. Timestamp equality cannot serve as the timer prohibition.

**C3 — BLOCKING — D3(b): The fixed-point predicate omits queued host work.**

**Evidence:** Commits move host commands into `Presenter::commands`, to be executed later ([presenter.rs:663](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter.rs:663), especially lines 711–714). D3 never calls `run_commands` or checks that queue.

An asynchronous answer’s `then` can issue `focus` or `scrollIntoView`. The first pass observes the commit; the next pass can be quiet while that command remains queued. Executing it afterward can dispatch another handler and start more requests. These commands are real implementations, not hypothetical extensions: [commands.rs:12](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter/commands.rs:12).

Other queues need explicit treatment too: `collection.pending()` checks only its refinement queue, not `scroll_events` ([collection.rs:149](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter/collection.rs:149)); buffered announcements have their own predicate.

**Resolution:** Include command execution and resulting callbacks in each pass. Define the completion check over all relevant buffered queues, including announcements and authored scroll deliveries. Establish a final observation point after these checks, then encode that pass’s pixels.

**C4 — MAJOR — D3(b)2, D3(c): The outer cap does not bound the inner motion loop.**

**Evidence:** D3 repeats motion seeks until no later settle time exists, entirely inside one outer pass. A seek can dispatch geometry callbacks that start another transition—the RFC acknowledges this, and [presenter.rs:1399](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter.rs:1399) provides the path.

Such a cycle never consumes sixteen outer passes. The text does not specify deadline checks or a separate budget inside this loop.

Also, sixteen active passes do not prove a loop. A finite chain of seventeen asynchronous continuations can legitimately exceed the budget.

**Resolution:** Make each motion seek consume the shared pass budget, or specify a separate bounded inner loop with deadline checks. Define checkpoints for every cooperative wait. Describe `exhausted` as “work budget exhausted,” not proof of a cycle.

**C5 — MAJOR — D3(c–d): The reused helpers discard errors that the procedure promises to return.**

**Evidence:**

- `wait_for_replies` prints pump errors and continues: [agent.rs:934](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/agent.rs:934).
- `Presenter::tick` logs geometry callback errors.
- `frame()` logs collection-refinement errors before independently setting `last_frame_succeeded` from the painter’s result.
- Image-report application prints intrinsic/layout errors: [presenter/images.rs:18](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter/images.rs:18).

A successful paint therefore does not establish that the preceding pass succeeded.

**Resolution:** Specify how these errors reach the settle result and include those helper changes in implementation scope. Return activity and failure information sufficient for D3’s decision; checking `last_frame_succeeded` alone only solves backend paint failure.

**C6 — MAJOR — D4: Copying an invocation ID onto newly created entries leaves ownership transfer undefined.**

**Evidence:** Two existing operations do not fit that recipe:

- `enqueue` can retain an existing resource ticket while replacing its arguments, without creating a pending entry: [commit.rs:980](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:980). What happens when an owned commit retargets an unowned ticket?
- `then_due` is one slot per mutation, overwritten when another answer arms it: [commit.rs:535](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/commit.rs:535). App-wide tool serialization does not prevent a person’s commit from replacing an owned continuation.

D4 delegates outcomes for dropped or retargeted **requests**, but these cases also concern adopting work and replacing continuations.

**Resolution:** Specify ownership transitions for ticket retention, continuation replacement, queue-head execution, refusal, rollback, and reload. Queue execution must inherit the queued entry’s owner. Completion must be observed after the commit and continuation-arming stages finish, never during the temporary empty interval after request removal.

**C7 — MAJOR — D4: The boundary around background work, announcements, and timers is missing.**

**Evidence:** LLP 1105’s lifecycle requires refusing invocation-created streams and background storage. Background storage is a module-wide ticket that makes no commit and bypasses `enqueue` and checkpoints ([background.rs:1](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/background.rs:1)). Tagging ordinary runner entries cannot by itself distinguish that work from the person’s background work.

Likewise, announcements are coalesced topic strings without invocation provenance ([time.rs:138](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/runner/src/runner/time.rs:138)). An owned commit can also arm a gated `after`, but timers are absent from D4’s list.

**Resolution:** State explicitly which work classes are owned, refused, or excluded. If the stream/background refusal remains in 1105, cite it and identify the source/executor seam that supplies attribution. If timers and announcements are intentionally outside invocation completion, say so. This cannot all be implemented solely by adding fields to the listed runner entries.

I have not treated 1105’s superseded result-dependency filter as a contradiction; replacing it is the intended improvement.

**C8 — MAJOR — D3/D5: The shared procedure needs explicit mode adapters and preserved API outcomes.**

**Evidence:** D3 contains no runner-time advance, yet the command-line mode and `clock settle` require one. The table expresses that intention but does not specify its ordering with reply waits and continuations.

More concretely, today’s Linux `clock settle` also considers game-world settlement and returns `settled:false, reason:"device"` for held device requests ([agent.rs:830](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/agent.rs:830), lines 875–895). `has_pending` deliberately excludes those holds. D3’s four ends and D5’s predicates do not preserve this behavior.

For the siblings:

- **1101.003 r4:** The column agrees, provided a no-commit pass is followed by its one-shot/busy checks and sleep—not immediately treated as completion.
- **1105:** The broader ownership is the intended change, subject to C6–C7.
- **1106:** Fresh per-capture deadlines need coordination with D11’s child deadline and parent hard deadline. D5 additionally introduces a total-test `--timeout` without defining its units, origin, or relationship to those deadlines.

**Resolution:** Specify each mode’s advancement, hold predicate, and result mapping. Preserve existing device/world results explicitly. Clarify that the timer prohibition applies to presentation-only settlement; explicit `clock settle` remains timer-firing unless LLP 1012 is deliberately changed. LLP 1012.000 expressly rejected removing those timers ([1012.000:128](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/llp/1012.000-attached-agent-sessions.rfc.md:128)).

**C9 — MINOR — Summary/D1/D2: Correct the remaining as-built claims.**

**Evidence and resolution:**

- The Summary says today’s `screenshot` moves presentation time. Ordinary Linux capture calls `frame()` without a seek ([presenter.rs:1412](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter.rs:1412)). Distinguish existing capture from 1106’s proposed seek.
- The driver does not update `s.now` from every reply. `s.op` returns replies unchanged; the cited line 1218 is the `+N real` branch ([agent.mjs:914](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/scripts/agent.mjs:914)). Describe the actual update sites.
- The callback inventory omits ordinary resize delivery, which also calls `Host::dispatch_at` ([transform_geometry.rs:208](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/linux/src/presenter/transform_geometry.rs:208)).
- “Every host has two clocks” should distinguish conceptual clock roles from current independent state. The terminal has no presentation clock; the JS agent currently samples animation from `exact.clock.now` ([web-js/agent.js:197](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv4-astra/host/web-js/agent.js:197)).

**4. Suggestions.**

Add a short executable-style specification of one pass: inputs, clock updates, serviced queues, activity observations, error propagation, and the final completion predicate. This would resolve more uncertainty than additional prose examples.

Prioritize future regression cases covering:

- Presentation ahead of runner, followed by `tap`, `clock data`, and a small `clock +N`.
- Two simultaneous timers with the first refusing.
- A reply’s `then` issuing a host command whose callback starts another request.
- Kept-ticket adoption and continuation replacement during a person/tool overlap.
- A finite chain exceeding sixteen passes, plus a genuine motion cycle.
- Two captures whose combined duration exceeds one capture’s deadline.

**5. Open questions for the author.**

1. When presentation is at 1000 and runner time is 0, what should `clock +100` do to presentation time?
2. Should an invocation adopt an existing unowned ticket that its commit retargets, or refuse that situation?
3. After a tool timeout, does app-wide serialization remain held while owned work still exists?
4. Are timers, topic announcements, and presentation-generated callbacks explicitly outside invocation ownership, or must their causal origin be tracked?

Verdict: NOT READY
## Round 2 (2026-10-08), on r2

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the four drafts (1101.003 r5, 1107 r2, 1105 and 1106 parked) and only Astra's own earlier reviews; 2026-10-08T02:19:20Z to 2026-10-08T02:31:14Z.
- **Method:** one brief (sha256 `0c33b34bcc6be753a91cd4d008376034f8e67d8922f3a20a51de870fbd28dbd7`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("ok sounds reasonable, go ahead" to the capture-as-sample revision and another round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r3 (2026-10-08) takes it by narrowing: a final capture seeks through the real presenter (nothing observes the session after it; the driver refuses a further step), and a mid-test capture samples paint-only motion from an engine clone, refusing `height`, `ghost` and `images` (N1–N3). Units fixed (N8); timers off inside `dispatch_at` for a flag spanning prelude to epilogue (N4); activation and nested callback errors end `error` (N5); deferred refreshes owned (N6); background refused while armed or failed if armed during the call (N7); the lock held after a timeout until owned work drains.

**1. Overall assessment**

**The sample-based clock model is sound; r2’s Linux capture procedure is not yet implementation-ready.** Cloning the engine, advancing the clone and reading its frame can preserve the live engine and both session clocks. The engine’s state and sampling operations support that approach ([engine.rs:240](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/motion/src/engine.rs:240), [engine.rs:524](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/motion/src/engine.rs:524)). This removes r1’s need for clock reconciliation or an amendment to LLP 1012 §2.

The unresolved part is the surrounding presentation state. Layout and image loading are not pure operations; some motion lives outside `Engine`; and sampled geometry can require collection materialization. D4 also leaves two categories of invocation work without sufficient ownership rules.

D5’s command-line column now agrees with 1101.003 r5 D2. Leaving `clock data` and `clock settle` unchanged also preserves their existing distinctions. I have not counted the parked siblings’ old text as contradictions.

This review is source-based at `2ce193976`. No files were changed, and no builds or application binaries were run.

**2. Previous-round concerns**

| Concern | Status and evidence |
|---|---|
| **C1 — Clock divergence** | **Resolved at the design level.** D1 samples a clone; the live clocks need not diverge. The presenter isolation problems below are separate. |
| **C2 — Timers due exactly now** | **Partly resolved.** `land_then` explicitly disables timers ([commit.rs:285](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/commit.rs:285)); the guard’s operation boundary remains unspecified, N4. |
| **C3 — Queued host work** | **Resolved in the stated pass.** D3 now executes commands, delivers scroll events, pumps announcements and includes their queues in `quiet`. |
| **C4 — Unbounded motion-seek loop** | **Resolved.** There is no repeated live motion seek, and `exhausted` is correctly described as a work budget. |
| **C5 — Swallowed errors** | **Partly resolved.** D3 requires error propagation, but its four-helper inventory misses activation and nested command failures, N5. |
| **C6 — Ownership transfer** | **Partly resolved.** Fulfillment capture, queue inheritance, kept-ticket handling and continuation replacement are specified; deferred refresh ownership is missing, N6. |
| **C7 — Background/timer boundary** | **Partly resolved.** Timer and announcement exclusions and the lock owner are explicit; background-work attribution remains unresolved, N7. |
| **C8 — Mode adapters and outcomes** | **Resolved.** D5 delegates CLI behavior to r5 D2 and preserves the existing clock paths, including world and device outcomes. |
| **C9 — Earlier factual corrections** | **Resolved.** Current screenshots do not seek, clock roles are distinguished from fields, and resize delivery is included. New factual problems are identified below. |

**3. New concerns**

**N1 — BLOCKING — D3, sampled frame steps 3–5: the specified isolation mechanism does not preserve session state.**

Two new current-behavior claims are incorrect:

- **`layout_motion` is not pure.** It calls `layout`, which synchronizes height ownership, mutates kernel layout, reports diagnostics and moved-node counts, and changes damage bookkeeping ([height.rs:100](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/height.rs:100)). Recomputing the original geometry does not undo those effects. For example, layout publication consumes flags and records geometry changes, while `Runner::moved` increments counters ([publication.rs:157](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/kernel/src/layout/publication.rs:157), [perf.rs:45](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/perf.rs:45)).
- **Image synchronization is not merely a cache fill.** `sync_visible` changes each live view’s visibility and requested dimensions. Making an image invisible cancels its request, drops its lease and removes its displayed bitmap ([image.rs:181](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/image.rs:181)). `wait_images` then applies reports to the live kernel, clamps live scrolling and refreshes geometry callbacks ([images.rs:13](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/images.rs:13)).

Thus a sampled end state that moves an image offscreen can discard the image still needed by the unchanged live presentation. Restoring the engine and layout does not restore that loader state.

**Resolution:** Define the snapshot boundary after ordinary live settlement. Use capture-local layout, image demand and presentation state, sharing decoded cache entries where safe. Alternatively, specify a complete restoration mechanism, including failure paths. Explicitly distinguish permitted live preparation and cache invalidation from the side-effect-free final sample.

**N2 — MAJOR — D1/D3: the chosen settle time includes motion that the proposed sampler never samples.**

D1 correctly includes press feedback and reorder-ghost return in *t_s*. Neither belongs to the cloned engine:

- Press feedback is evaluated by `Host::presented` using **live `self.now_ms`** ([host.rs:628](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/host.rs:628)).
- The reorder ghost is held in presenter state and the painter’s lift state; its return advances through `land_group` ([group.rs:339](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/group.rs:339), [group.rs:395](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/group.rs:395)).

Copying `host.presented` and applying `sample.frame()` therefore does not produce the complete presentation at *t_s*.

**Resolution:** Add pure capture-time sampling for press feedback and ghost presentation, including the ghost’s disappearance at rest. Specify their inclusion in the overlay without running their live retirement paths.

**N3 — MAJOR — D3: sampled Height geometry can expose collection rows that do not exist.**

Collections are refined during the data pass, using live geometry. Their feedback includes the current scrollport dimensions and can cause the runner to materialize more rows ([collection.rs:899](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/collection.rs:899), [collection.rs:953](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/collection.rs:953)).

Consider a virtualized list inside a panel whose presented Height is small but whose settled Height is much larger. D3 refines for the small port, enlarges it only during sampled layout, and expressly forbids collection queuing there. Repeating the pass still refines at the small live height. The sample can consequently omit rows that a settled presentation would show.

**Resolution:** Decide whether capture supports materializing collections for sampled geometry, explicitly refuses this case, or promises only the currently materialized tree. “What the session would show at *t_s*” currently implies more than the procedure delivers.

**N4 — MAJOR — D2: the pre-operation pump is not timer-free, and the settling guard starts at an unspecified boundary.**

The new claim at D2:106 is false for the complete call path. Before `answer` runs, `agent::handle` polls images, pumps and executes commands ([agent.rs:59](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/agent.rs:59)). `pump` immediately delivers authored scroll events ([presenter.rs:1289](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter.rs:1289)); those use `dispatch_at`, whose runner implementation calls `advance_timed` ([collection.rs:490](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/collection.rs:490), [commit.rs:188](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/commit.rs:188)).

A flag installed only on entering D3 is too late. The operation’s epilogue can also dispatch hover callbacks after painting.

**Resolution:** Specify exactly when the guard is entered and released. It must cover capture-related prelude, activation, callbacks and epilogue work, with errors included in the capture result. Correct the pump claim.

**N5 — MAJOR — D3 activation/errors: changing the four named wrappers is insufficient.**

Activation failure currently sets `activation_failed` and logs the error ([display_frame.rs:293](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/display_frame.rs:293)). `data_activating()` then returns **false**, so “wait while data_activating” does not distinguish successful activation from failure ([delivery.rs:117](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/delivery.rs:117)).

Likewise, returning an error from `run_commands` requires changing its callees: `focus_command` already prints and discards a callback error before returning a view ID ([events.rs:61](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/presenter/events.rs:61)).

**Resolution:** Include activation failure and nested callback delivery in the error contract and implementation scope. A failed activation must terminate as `error`, not merely stop waiting.

**N6 — MAJOR — D4: deferred resource refreshes can outlive the owned set.**

An invocation can force a resource refresh without immediately enqueueing its request. `force_refresh` records only a resource index ([commit.rs:956](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/commit.rs:956)). Settlement explicitly preserves that intention when arguments are pending or the source cannot yet answer ([settlement.rs:768](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/settlement.rs:768)).

D4 tags pending requests, continuations and sends, but not these deferred refresh intentions. The owned set can become empty before the refresh is issued; a later unowned commit can enqueue it under the wrong owner.

**Resolution:** Include deferred refresh intentions in ownership and completion, preserving their owner through settlement and rollback. Alternatively, explicitly refuse this situation. Do not report completion merely because no request has been created yet.

**N7 — MAJOR — D4: background refusal still lacks an attribution mechanism.**

The exclusion is now stated, but the earlier implementation problem remains. `arm_background` polls **module-wide work**, outside ordinary request checkpoints; the source interface supplies no invocation identity ([background.rs:27](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/background.rs:27), [source.rs:252](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/source.rs:252)). The JS executor reports aggregate background state, not the originating invocation ([background.rs:35](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/js/src/background.rs:35)).

An app-wide tool lock does not distinguish a person’s existing background work from work initiated while answering the tool.

**Resolution:** Name the source/executor attribution seam and its required changes. Clarify whether “refused when scheduled” prevents scheduling or reports failure after work has started. Runner ID fields alone cannot implement this policy.

**N8 — MINOR — D1/D3/D4: correct the remaining numerical and cost claims.**

- `settle()` returns **milliseconds**, whereas the motion engine uses **seconds**; specify the conversion in `sample_at(t_s)` ([host.rs:658](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/host/linux/src/host.rs:658)).
- D3 clones on every sampled-frame pass, potentially again after image work—not once per capture.
- Seventeen continuations do not necessarily require seventeen passes: `land_then` drains due continuations within one call ([commit.rs:317](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/runner/src/runner/commit.rs:317)). Qualify the example as seventeen work-producing passes.
- `after(0, …)` is not valid current Contract; timer intervals must be at least 1 ms ([timers.rs:34](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv5-astra/contract/lower/src/timers.rs:34)).

**Resolution:** Correct these statements without changing the central design.

**4. Suggestions**

- Define the invariant as “ordinary live settlement, then an isolated presentation sample,” with explicit permitted cache effects.
- Define `work` independently of painting and cache invalidation. Otherwise the mandatory sampled frame and subsequent dirtying make pass accounting ambiguous.
- Add regression cases for Height restoration, images moving out of the sampled viewport, press release, ghost landing, and expanded virtualized scrollports.
- Test pre-operation callbacks with a due timer, failed activation, a refusing focus callback, deferred refresh ownership, and pre-existing versus invocation-created background work.

**5. Open questions for the author**

1. Exactly which session changes are permitted during image preparation, and where does the side-effect-free boundary begin?
2. Must sampled geometry materialize newly visible collection rows, or should v1 refuse that case?
3. Does the app-wide lock remain held after timeout while owned work survives?
4. Who will implement the revised scope, including the required source/executor changes?

Verdict: NOT READY
## Round 3 (2026-10-08), on r3

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the four drafts (1107 r3, 1101.003 r6, 1105 and 1106 parked) and only Astra's own earlier 1107 reviews; 2026-10-08T06:10:14Z to 2026-10-08T07:08:40Z.
- **Method:** one brief (sha256 `e3a6b3687dd291405e01f12b859b4c239ea28990b33152ebeb0d5e6d3bdba8f9`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("sure do both" to the final-capture/mid-test split and a review of 1107 only). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r4 (2026-10-08) takes it, narrowed by Charlie: mid-test sampling is dropped, so R3-3, R3-4 and the paint-only R3-6 items are moot and recorded under "Isolated sampling (later)". R3-1: the final capture seeks inside each pass until no finite target lies ahead, each seek spending the budget (D3). R3-2: `frame()` and `follow_pointer` run inside the loop, with one final observation point (D3). R3-5: post-step reads run before the final screenshot, teardown runs after encoding, and a failure still uses up the session (D1). Q2: "armed" is an outstanding runner background ticket (D4). The 16/17 wording and citations corrected.

**1. Overall assessment**

**r3 is not ready to implement without further decisions.** The approved split is viable, but the final-capture procedure can report completion with newly started motion still running. The sampled-paint path also leaves important ownership and preparation details unspecified.

I accept sampling press feedback: its evaluator is pure. I also accept sampling `Layout` in principle, but the stated justification is incomplete: it changes surface size and clipping as well as translation.

D5’s command-line column agrees with 1101.003 r6 D2’s authority. I have not treated the parked siblings as competing specifications.

This review used source inspection at `2ce193976`; no files were changed and no Cargo builds or application tests were run.

**2. Round-2 concerns**

| Concern | Status and evidence |
|---|---|
| **N1 — Presenter isolation** | **Partly resolved.** D1 now prohibits sampled layout and live image synchronization, but the preparation boundary and capture-local painter remain underspecified; R3-4 below. |
| **N2 — Press, ghost and paint sampling** | **Partly resolved.** Pure press sampling and ghost refusal address those components; the proposed `PaintMotion` source is incorrect; R3-3. |
| **N3 — Unmaterialized collection rows** | **Resolved by narrowing.** Mid-test Height motion is refused; final capture uses real collection refinement. Collection demand reads kernel frame dimensions, supporting the distinction from paint-only `Layout` ([collection.rs:256](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/presenter/collection.rs:256)). |
| **N4 — Timer guard’s span** | **Resolved in the stated contract.** D2 now brackets the prelude and epilogue and intercepts `Host::dispatch_at`. Completion after those callbacks remains a separate problem; R3-2. |
| **N5 — Activation and nested errors** | **Resolved in the stated contract.** D3 explicitly distinguishes activation failure and requires nested callback errors to propagate, addressing the existing failure paths in [display_frame.rs:293](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/presenter/display_frame.rs:293) and [events.rs:61](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/presenter/events.rs:61). |
| **N6 — Deferred refresh ownership** | **Resolved in the stated contract.** D4 retains ownership while `refresh_next` survives settlement, matching the deferred-work mechanism at [settlement.rs:768](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/runner/src/runner/settlement.rs:768). |
| **N7 — Background attribution** | **Resolved by changing policy.** Refusing/failing on aggregate background activity removes the need to attribute it. The precise meaning of “armed” still needs clarification; §5 below. |
| **N8 — Units and numerical claims** | **Resolved substantially.** Milliseconds-to-seconds conversion, per-pass cloning and the minimum timer interval are corrected. The 16/17-pass explanation still needs a small wording correction; R3-6. |

**3. New concerns**

**R3-1 — BLOCKING — D1/D3: one final seek does not establish motion settlement.**

**Evidence:** D3 specifies “a final capture’s one seek,” while `quiet` contains no motion condition ([RFC:168](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/llp/1107-settled-and-the-two-clocks.rfc.md:168)).

The real seek itself can start another transition. `Host::tick` advances Height, performs layout, then calls `observe_layout` ([host.rs:1175](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/host.rs:1175)). Layout observation feeds a new `Property::Layout` target into `engine.observe` ([layout.rs:166](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/kernel/src/motion/layout.rs:166)).

Consequently, a Height transition finishing at *t_s* can start a sibling’s layout transition **at that instant**. Later data passes at unchanged presentation time can become quiet while that transition remains unfinished. Geometry callbacks and image-driven layout provide additional routes to the same problem.

**Resolution:** Recompute the settle target after each pass. A final capture completes only when ordinary work is quiet **and no later finite presentation target remains**. Make every advancing seek consume the shared work budget, with deadline checks; do not restore an unbounded inner seek loop.

**R3-2 — MAJOR — D2/D3: guarding the epilogue does not include its successful work in completion.**

**Evidence:** `agent::handle` obtains the operation’s reply before subsequent surface synchronization, commands, painting and hover delivery ([agent.rs:73](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/agent.rs:73)). `follow_pointer` can dispatch authored hover handlers and run `after_commit`; it deliberately lives outside `frame()` ([events.rs:192](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/presenter/events.rs:192)).

D2 now suppresses timers and propagates errors across this span. It does not say what happens when an epilogue callback **successfully** changes state or starts a request after D3 selected its pixels. Conversely, a real frame inside D3 can clear `dirty`, preventing the conditional epilogue from delivering hover for that newly painted geometry.

**Resolution:** Put real-frame hover delivery and its resulting work inside the fixed-point procedure. Repaint and recheck completion after it. The isolated sampled frame must not deliver live hover. Define one final observation point after all capture-related callbacks, then encode its pixels.

**R3-3 — MAJOR — D1: the paint overlay cannot obtain `PaintMotion` from the engine clone.**

**Evidence:** D1 proposes “a `PaintMotion` read from the clone.” `PaintMotion` is a separate `Host` field, beside `Engine` ([host.rs:82](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/host.rs:82)). It contains ownership, pending targets, appearance and `currentcolor` information absent from the engine ([paint.rs:11](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/kernel/src/motion/paint.rs:11)).

Moreover, current paint presentation does more than assign each emitted property to its node: it propagates inherited colour, handles `currentcolor` borders and clears settled overrides ([paint_motion.rs:81](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/paint_motion.rs:81)). A literal copy-and-apply implementation can leave a descendant’s old inherited-colour override behind.

**Resolution:** Specify a pure projection using the sampled engine, immutable kernel and host paint ownership, writing only the capture overlay. Require the same inheritance, override removal and appearance behavior as the live projector. This needs no scratch kernel or full scratch presenter.

**R3-4 — MAJOR — D1/D3/D5: the handoff from live preparation to isolated painting is still ambiguous.**

**Evidence:** D1 describes settlement preceding the sample. D3 instead selects `frame_sampled` inside each pass **before** `wait_images`; D5 says images must already be decoded or the capture refuses. These leave unspecified whether a currently visible but pending image is waited for or immediately refused.

The renderer boundary also needs one more concrete sentence. A capture buffer and copied row bookkeeping do not alone isolate the existing painter: `paint_selected` mutates retained damage, accepted text and painter metadata, and calls `finish_text_frame` on the shared text engine ([paint.rs:725](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/paint.rs:725)). Real `frame()` also supplies canvas snapshots and placements before constructing the scene ([presenter.rs:49](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/content_region/presenter.rs:49)).

**Resolution:** Define two explicit phases:

- Complete ordinary live preparation, including its image demand and prepared surface content.
- Establish the snapshot boundary, then project and paint in a capture-local painting context.

Name what may be shared, including any permitted text-cache effects. Evaluate `images` refusal against additional demand exposed by the sample, or explicitly choose the stricter policy. Treat `symbol:` images separately: they are drawn procedurally and intentionally have no decoded bitmap ([image.rs:183](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/image.rs:183)).

**R3-5 — MAJOR — D1/D5: the last authored step is not necessarily the last session observer.**

**Evidence:** After the steps finish, the test driver checks outstanding counted fetch faults. That check calls `s.state()` ([agent-test.mjs:205](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/scripts/agent-test.mjs:205), invoked at [line 343](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/scripts/agent-test.mjs:343)). Thus a screenshot can be the last authored step while a later framework observation remains.

Shutdown is also active: the Linux agent cancels the pointer and pumps teardown work ([agent.rs:53](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/agent.rs:53)); pointer cancellation can deliver an authored `pointerup` ([contact.rs:773](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/presenter/contact.rs:773)).

**Resolution:** Define final capture as consuming the session, including implicit driver reads and failure paths—not merely rejecting another authored step. Complete required harness observations within the final operation before sealing it, or select sampled capture when observations remain. Specify the permitted teardown behavior after the clocks part.

**R3-6 — MINOR — D1/D3: correct the remaining factual descriptions.**

**Evidence and resolution:**

- **`Layout` is not only translation.** Its four values include surface width/height scaling, and that surface controls child clipping ([presented.rs:113](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/paint/presented.rs:113), [paint.rs:1153](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/paint.rs:1153)). Keep the exception, but require the complete projection and correct its rationale.
- **`Property::PAINT` contains eleven properties, not eleven colours.** `BoxShadow` carries offset/blur geometry ([property.rs:79](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/motion/src/property.rs:79)).
- **Sixteen work-producing passes exhaust a sixteen-pass budget.** Seventeen successive request-producing passes are a workload that cannot finish within it, not the number required to trigger exhaustion.
- The combined engine/press/**group** settle helper is [agent.rs:693](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/host/linux/src/agent.rs:693); the cited host helper includes only engine and press.

**4. Suggestions**

Add a short phase diagram or pseudocode covering live preparation, final seek repetition, sampled projection, callback completion, encoding and session consumption. That would resolve more uncertainty than expanding the explanatory prose.

Prioritize these regression cases:

- Height completion starts a sibling’s `Layout` transition.
- Settled geometry triggers hover that starts a request.
- Sampled parent colour updates inherited text and `currentcolor` borders.
- Layout size/clipping changes reveal raster and `symbol:` images.
- A final screenshot is followed by an implicit fetch-fault check.
- A capture fails after its first real seek.

**5. Open questions for the author**

1. Who will implement this scope, and when?
2. Does “background work is armed” mean an outstanding runner ticket, or any queued/in-flight module work? Those differ: arming occurs in `take_requests` ([commit.rs:1053](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv6-astra/runner/src/runner/commit.rs:1053)).
3. Is final capture irrevocably consuming from its first real seek, including when it subsequently errors?
4. Which shared cache mutations are permitted during isolated sampled painting?

Verdict: NOT READY
## Round 4 (2026-10-08), on r4 (as LLP 1113)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `371ada597` (origin/main `ce31a47b6` plus the two commits that add these RFCs), with the other family's review files removed; 2026-10-08T07:33:02Z to 2026-10-08T07:48:47Z.
- **Method:** one brief (sha256 `bed346f232fd6f35bc75d7b5abc2e72478f7e7a75968edf4e3f96968b1035d00`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("yeah sounds good do all that" to the r4 narrowing and a review). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted. This round uses the new numbers (1111, 1112, 1113).
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r5 (2026-10-08) is written from a throwaway prototype, `spike/settled-capture` (`ba1dee19a`, `f7786deda`), that ran these findings. Taken, prototyped: N1 (one bounded hatch turn per pass), N4 (the final capture's own path skips `screenshot()` and the epilogue; 0 pixels differ). N2 confirmed and taken (`pending()` skips symbols). N3 taken (≤50 ms image slices, one loop with a 20 ms idle sleep). Taken on paper: N5 (`faults` frozen into the final reply), N6 (completion after `take_requests`), N7 (owned `refresh_next`, transfer and coalescing), and the N8 minors. Dispositions are in LLP 1113 §7.

**1. Overall assessment**

**r4 is substantially simpler, but still not ready to implement without guessing.** The approved narrowing removes the isolated-presentation problem, and recomputing the finite settle target addresses the original final-seek defect. D5’s command-line column agrees with LLP 1101.003 D2; I have not treated the parked siblings as competing authorities.

The remaining problems concern termination, image waiting, the final observation boundary, and invocation ownership. Most refreshed citations identify the intended code, but several prescribed operations do not provide the guarantees attributed to them.

Reviewed at `371ada597` by source inspection. No files changed; no builds or application tests run.

**2. Round-3 concerns**

| Concern | Status and evidence |
|---|---|
| **R3-1 — One final seek is insufficient** | **Resolved.** D3 recomputes `settle()` after work, requires no later finite target, and charges advancing seeks to the budget. This covers Height completion starting another Layout curve. |
| **R3-2 — Work after the selected frame** | **Partly resolved.** Hover is inside the loop, but D2 still describes the ordinary `handle` epilogue after `answer`; its relationship to D3’s final observation point remains unspecified. See N4. |
| **R3-3 — PaintMotion cannot come from an engine clone** | **Moved.** The cloned-engine painting proposal is removed; the ownership and inheritance requirements are recorded under “Isolated sampling.” |
| **R3-4 — Live preparation versus isolated painting** | **Moved.** There is no isolated painting phase in v1. Ordinary live image preparation still has separate problems, N2–N3. |
| **R3-5 — Last authored step versus last observer** | **Partly resolved.** Session consumption and post-encode teardown are explicit, including failure after a seek. Moving fault checks before capture, however, checks them before capture-generated work finishes. See N5. |
| **R3-6 — Factual descriptions and counting** | **Partly resolved.** The combined settle helper and sixteen-pass threshold are corrected; paint-only descriptions moved. D3’s claim that only work awaiting another pump spends passes contradicts its own seek and hatch accounting. |

The round-3 question about “armed” background work is resolved: D4 now means an outstanding runner background ticket. Its observation boundary still needs work, N6.

**3. New concerns**

**N1 — BLOCKING — D3: the hatch drain reintroduces an unbounded inner loop.**

**Evidence:** D3:130 specifies `while hatch_in_flight() > 0: hatch_turn`, inside one work-producing pass. The existing `hatch_turn()` drains one snapshot and invokes moments; those moments may enqueue another snapshot. It contains no cumulative drain limit. The queue’s capacity limit does not stop a cycle that consumes one act and creates another. See [hatch.rs:79](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/presenter/hatch.rs:79) and [session.rs:197](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/hatches/session.rs:197).

The cited `clock settle` implementation supplies its own counter and stops after sixteen drains; D3 omits that counter. [agent.rs:895](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/agent.rs:895)

A hatch whose `changed` callback clicks a control that changes its dataset can therefore remain inside this step indefinitely, never spending an outer pass. “Check before each step” does not explicitly establish a checkpoint inside this loop.

**Resolution:** Drain one bounded snapshot per pass, include outstanding hatch acts in `quiet`, and charge the work. Alternatively, specify a separate cumulative drain bound and a deadline check before every drain. State its result mapping.

**N2 — MAJOR — D3, images/quiet: procedural symbols cannot satisfy the prescribed image predicate.**

**Evidence:** A `symbol:` image deliberately has `source_id = None`, a nonempty source, and `symbol_size = Some(...)`. `poll()` skips it. However, `Images::pending()` does not exclude symbols: its missing prepared-source branch returns `true`. See [image.rs:195](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/image.rs:195), [image.rs:311](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/image.rs:311), and [image.rs:459](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/image.rs:459).

Consequently, an otherwise settled page containing a symbol cannot reach D3’s `complete` using the specified helpers. Moving symbol handling into the deferred sampling discussion does not remove this live-loader issue.

**Resolution:** Include correcting the live pending predicate in implementation scope. A procedural symbol contributes no raster work. Add a final-capture regression containing only text and a symbol.

**N3 — MAJOR — D3: waiting for images can prevent the data work needed to finish that wait.**

**Evidence:** `wait_images(remaining)` calls `Images::wait`, which polls only the image backend until it finishes or consumes the entire remaining deadline. It does not pump application replies or announcements. [presenter/images.rs:13](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/presenter/images.rs:13), [image.rs:528](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/image.rs:528)

For example, a reply that becomes ready during this wait may remove or replace the image being waited for. D3 cannot apply that reply until the image wait returns, potentially at the deadline. The session could settle, but this schedule prevents it.

Conversely, with requests pending and no image work, D3 specifies no sleep: the busy-only polling exception does not cover that case.

**Resolution:** Specify cooperative waiting across requests, announcements and images. Use bounded polling/wake intervals that return to the data pass; reserve deadline exhaustion for work that remains after servicing those sources. Define no-work waiting for requests as well as `accessibilityBusy`.

**N4 — MAJOR — D1–D3: the final observation point still conflicts with the ordinary operation epilogue.**

**Evidence:** Today `screenshot()` encodes and writes inside `answer`. After `answer` returns, `handle` synchronizes surfaces, checks surface errors, runs commands and hatch moments, potentially paints and delivers hover, then calls `first_pixel`. [presenter.rs:1430](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/presenter.rs:1430), [agent.rs:78](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/agent.rs:78)

D2 explicitly includes this epilogue in the guarded span, while D3 declares the first quiet pass final and says nothing paints afterward. Guarding callbacks suppresses timers; it does not eliminate their successful state changes or errors.

**Resolution:** Specify a distinct final-capture control path. Move every required finalization callback before the last completion check, then freeze pixels and reply metadata and encode. Explicitly bypass the ordinary mutating epilogue afterward. Keep the separate teardown exception. This is a sequencing decision, not merely a flag-placement change.

**N5 — MAJOR — D1: moving counted-fault checks before capture changes their meaning.**

**Evidence:** `unfired()` reads current fault hits and immediately records failure when none matched. [agent-test.mjs:206](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/scripts/agent-test.mjs:206)

D3 can subsequently land a pending reply, run its `then`, deliver hover or execute a hatch act that initiates the matching fetch. Checking before the final screenshot can therefore report “matched no fetch” for a fault that the final operation legitimately exercises.

**Resolution:** Evaluate required harness observations at the final operation’s observation point, before sealing the session. Return a frozen fault summary with the final reply and validate it locally. Do not perform another session operation afterward. Define the available summary on failure too.

**N6 — MAJOR — D4: completion can precede background-ticket creation.**

**Evidence:** D4 permits completion once the owned set is empty after commit and continuation arming. Background tickets are created later, by `take_requests()`. A synchronous `Answer::Now` can start background work without creating an owned pending request. The existing background fixture implements precisely that behavior. [commit.rs:1072](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/runner/src/runner/commit.rs:1072), [background/tests.rs:26](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/runner/src/runner/background/tests.rs:26)

On Apple, taking those requests occurs in the bridge’s subsequent emission phase. [abi.rs:230](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/apple/src/abi.rs:230)

An implementation checking immediately at D4’s stated commit boundary can finish successfully before the ticket appears, evading “fails if one appears during it.”

**Resolution:** Define completion after background arming and request handoff, with the invocation still active through that boundary. Apply the check to synchronous actions and continuations as well as asynchronous fulfilments. This preserves the chosen ticket-based policy without requiring attribution.

**N7 — MAJOR — D4: deferred-refresh ownership needs consumption and collision rules.**

**Evidence:** `force_refresh` coalesces by resource index; it does not necessarily create a new entry. Later settlement consumes retained refresh intentions and enqueues requests under whichever commit made their arguments available. [commit.rs:975](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/runner/src/runner/commit.rs:975), [settlement.rs:771](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/runner/src/runner/settlement.rs:771)

D4 says both “copy the commit’s invocation id at enqueue” and “a retained refresh keeps its owner.” It does not explicitly say how the latter overrides an unowned consuming commit, or what happens when a tool forces a resource with an existing unowned deferred refresh. The kept-ticket rule covers neither case.

**Resolution:** State that consuming a refresh transfers its owner to the resulting request, independently of the consuming commit. Define coalescing between owned and unowned refresh intentions, including rollback and supersession. A small ownership-transition table is sufficient; outcome names can remain in LLP 1111.

**N8 — MINOR — D1/D3/D5: narrow the remaining absolute claims.**

**Evidence and resolution:**

- **“The engine is untouched” / “session unchanged” is stronger than today’s screenshot.** Its prelude pumps replies, and fulfilment commits can change engine targets. Say “no explicit clock seek; ordinary screenshot processing remains.” The acceptance test should not require equivalence to omitting capture when asynchronous work is pending. [agent.rs:61](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/agent.rs:61), [host.rs:1281](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/host/linux/src/host.rs:1281)
- **Unchanged `clock data` is not an operation-wide timer prohibition.** Its unguarded prelude and callback dispatches can reach `advance_timed`. Qualify D5’s cell as the behavior of `land_then`, or deliberately expand the guard. [commit.rs:192](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv7-astra/runner/src/runner/commit.rs:192)
- **D3:171–172’s “only work awaiting a later pump spends passes” is false.** Its own definition also charges seeks, hatch work, hover and image reports.
- **D1’s “failure … reply is D3’s error” conflicts with the separate `deadline` and `exhausted` ends.** Consumption should be independent of which end occurred.

**4. Suggestions**

Keep the narrowing. None of these findings requires reinstating isolated sampling.

Write one end-to-end sequence covering guarded entry, activation, bounded passes, harness observations, frozen pixels and metadata, encoding, and teardown. Include where errors are collected and where the ordinary `handle` path is bypassed.

Extend the existing regression list with a self-requeuing hatch, a symbol-only picture, an image removed by an asynchronous reply, a counted fault first exercised during final capture, synchronous background work, and deferred-refresh ownership crossing an unowned commit.

**5. Open questions for the author**

1. Who implements this scope, and when? The draft correctly records that nobody is assigned yet.
2. What duration and override define a final test capture’s “fresh” deadline? D5 gives no value; LLP 1112 D11 defines the render child’s deadline.
3. Is session consumption unconditional once `final: true` is accepted, or conditional on reaching the first seek? Specify all four D3 ends and encoding failure.
4. Are the sixteen-pass budget and app-wide lock policy now approved, or do the questions for Charlie remain open?

Verdict: NOT READY