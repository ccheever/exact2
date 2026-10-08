# Review: LLP 1107: Settled, and the two clocks (r1, grok)

> **Renumbered 2026-10-08.** These RFCs were drafted as LLP 1105, 1106 and 1107 in a checkout 542 commits behind origin, where those numbers had since been allocated to other documents. They were committed as LLP 1111 (apps as tools), 1112 (a URL under a state) and 1113 (settled and the two clocks); LLP 1101.003 kept its number. The reviews below are transcribed as returned and keep the numbers they were written against: read 1105 as 1111, 1106 as 1112, 1107 as 1113.

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the four drafts and only Grok's own earlier reviews; 2026-10-08T01:49:32Z to 2026-10-08T02:04:19Z.
- **Method:** one brief (sha256 `9a0edf89ff0d5d76983699f7dcf76be52cf8011440bfc45ce03d8e2eda74d6f9`), shared verbatim with the other reviewer; round 1, blind to the other review. Requested by Charlie ("sounds good, do the review with astra and grok"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** r2 (2026-10-08) takes all of it. Blocking 1: one `quiet` predicate at one point, no inner motion loop (a capture samples instead of seeking), `wait_images(remaining)`, `QueueFull`/`Budget` end `deadline`/`images`, and `dirty` read after the frame (D3). Blocking 2: the command-line column cites LLP 1101.003 D2 as its authority, and the wall-clock pass is its own kind (D5). Blocking 3: the clocks never part, so there is no rejoining and LLP 1012 is unamended; `clock settle` keeps world `settleAt` and `reason: "device"` (D1, D5). Major (D4): ids at the four creation sites, fulfilment capture, kept ticket, streams and background refused, timers outside the set, the lock per `ExactApp`, and the tool column cites D4. Minor: citations, `deliver_resizes` and the pre-op pump added, `infinite()` named.

---

I'll review LLP 1107 read-only: binding rules and the RFC first, then the sibling drafts and the code they cite, checking each behavioral claim against this commit.The RFC's line citations are the claims I'll check. Next I'll read those sites and the sibling drafts' settled-clock rules.The two clocks are real in the Linux host. I'll check the cited call sites, the agent clock spec, and whether D3's loop matches what the code actually drains.LLP 1107 is the right split, and the Linux clock split it describes is real. It is not yet a procedure an implementer can build, and D5 does not yet match LLP 1101.003 r4.

## 1. Overall assessment

The two clocks are in the Linux host, and the bug the reviews found is in the code. `Runner::now_ms` (`runner/src/runner.rs:1255`) and `Host::now_ms` (`host/linux/src/host.rs:90`) move on different paths. A presentation seek goes through `Presenter::tick` (`presenter.rs:1399`) into `Host::tick` (`host.rs:1175`), which assigns `self.now_ms` and seeks the engine. It does not call `advance_timed`. The next `Host::dispatch_at` (`host.rs:851`) does: it sets the host clock to `max(argument, self.now_ms)` (`:859`) and then calls `Runner::dispatch_at` with that host time (`:873`), and the runner fires every timer due by then (`commit.rs:188-189`). `after_commit` makes this easy to hit. Every commit ends in `finish_commit` (`presenter.rs:647-654`), which dispatches transform geometry and `resize` at `host.now()`. Agent `handle` also pumps at `host.now()` before every operation (`host/linux/src/agent.rs:66`). Under today's agent server the two clocks still match, because nothing seeks the host clock ahead (`app.rs:561` returns into `agent::serve` and never runs the display loop). They part as soon as a capture calls `tick` with a later time.

What I checked and accept:

- Runner time moves only in `advance_within` (`commit.rs:289`), via `advance_timed` (`:207`), `land_then` (`:285`), `advance_until_request` (`:276`), and `dispatch_at`. A backward target returns the current time (`:301-307`).
- The engine refuses a backward seek (`motion/src/engine.rs:549-555`). Commits seek `max(at_ms, engine.now())` (`host.rs:1268-1272`).
- `clock` / `clock settle` advance the runner, then `Presenter::clocked` ticks the host clock to `host.now()` (`presenter/clock.rs:7-31`, `agent.rs:796-903`).
- `clock data` calls `land_then` only. Its unsettled reply reports `host.now()` (`agent.rs:740`). The 16-pass cap is `:743` and `:869` (motion rounds also stop at `:898`). The bound is 20 s (`:791`).
- `screenshot` paints and returns (`presenter.rs:1412`). It does not tick or advance.
- `land_then` runs due `then`s and queue `next`s with the timer flag off (`commit.rs:285-287`, `:332-345`).
- `has_pending` is in-flight requests plus the background round, excluding device holds and a stream after its first message (`commit.rs:1129-1135`, `runner.rs:301-305`).
- Caltrain's ticker is `every(1000, tick)` (`apps/caltrain/app.contract:137-138`) and `nowMs` is the literal `1787915400000` (`:54`).
- The terminal decodes images inside the commit (`host/terminal/src/host.rs:257` → `image.rs:147-164`). `accessibilityBusy` is the lowered `aria-busy` (`contract/lower/src/tags.rs:532`). The terminal already reads it (`host/terminal/src/host.rs:389`).
- `scripts/agent.mjs` builds `clock +N` from `s.now` (`:1225`). The screenshot return (`:1279`) does not assign `s.now`.

What is wrong or overstated:

- `host.rs:583` is `Host::now()`, the getter. The field is `:90`. `dispatch_at`'s host-clock write is `:859`, and the runner call is `:873`. The dialog return at `:869-871` is a different path.
- `engine.rs:549` is the start of `validate_time`. The backward check is `:553`.
- "The clock field of every agent reply is the runner's" (`runner/src/agent.rs:88-95`) is true of `tags()`. `clock data` writes its own `clock` from `host.now()` on the failure path (`:740`) and from `land_then`'s `landed` on the success path (`:766`). `landed` is `host.now()` after `advanced`, which keeps `max(runner, host)` (`host.rs:987`). `tagged()` fills a missing `clock` from `tags` and leaves an existing one (`scripts/agent.mjs:1324-1328`). Both `clock data` replies keep the host time once the clocks have parted.
- `scripts/agent.mjs:1218` is the `clock +N real` loop. Ordinary clock replies assign `s.now` at `:1233`.
- D1's seek row says the runner stays still. `tick` sets `dirty = true` unconditionally (`presenter.rs:1408`) and, when height changes, dispatches transform geometry at `host.now()`. That dispatch calls `advance_timed`. `deliver_resizes` (`transform_geometry.rs:208-232`) is a fourth `dispatch_at(host.now())` and is not in D1's list of three.
- "advance_timed(runner_now) fires nothing new" is false when a `then`, a queue `next`, or a timer is already due at the runner clock. That is the case `land_then` and the front of `dispatch_at` exist for. It is true only of timers whose due time was crossed solely by the host clock.

D2's choke point is still the right one: those dispatches all go through `Host::dispatch_at` or `fulfill_all`. The procedure around it, the command-line column, and the story of how the clocks meet again are not yet specified tightly enough to implement.

## 2. Strengths

- §0 is an accurate account of why one document exists. The parked siblings still contain the copies this RFC means to replace: 1106 D1a says the capture moves the session clock and that due timers wait for a `clock` step; 1105 D4 owns a resource re-ask only when `returns` or `fails when` reads it; 1101.003 r4 already cites this RFC.
- D1's split matches the code's structure, and the rejected alternative in D2 (advancing the runner to the seek time) would fire Caltrain's one-second task and change `nowMs`.
- D2 names a single Linux edit that covers every current `dispatch_at(host.now())` caller, including ones the inventory misses.
- D4's Fieldnotes example is the real mechanism. `library` is `library(query, saveRevision, serviceRevision)` (`apps/fieldnotes/app.contract:70`). Fulfilment settles inside the commit and enqueues resource re-asks at `settlement.rs:755-764`, after the answered request has been removed (`commit.rs:1217`). A storage ask is not keepable (`commit.rs:976-979` requires `storage.is_none()`), so this re-ask is a new pending entry an invocation id can be copied onto.
- D3 (d) matches the painter. A failed paint sets `last_frame_succeeded` (`content_region/presenter.rs:147`) and returns retained or white pixels (`:160-180`) with no error.
- D5's picture column is the rule 1106's next revision needs: timers frozen, runner clock unmoved, motion sought, images waited, 16 passes, caller deadline, `accessibilityBusy`. Publishable stays in 1106, which is the right cut.
- The end names `exhausted` and `deadline` answer the round-3 gap where a 16-pass stop had no result.

## 3. Concerns

### BLOCKING — D3 is not yet a terminating fixed point

**Section:** D3 (b), (c).

**Evidence:** The loop predicate and the quiet predicate are different lists. Work continues for a receipt, `pending()`, a collection or image, a dirty frame, `settle()` ahead, or `set_intrinsics`. Quiet also requires no due `then`, no due queue `next`, and no `accessibilityBusy`. Busy is absent from the loop predicate. The ends section then says a busy tree with nothing pending is polled every 20 ms until the bound and ends `deadline` / `busy`. An implementer who follows the loop ends `complete` while a node is busy. An implementer who follows the ends section does not.

The motion step loops "until `settle()` no longer lies ahead" inside one pass. That inner loop has no pass count and no deadline check. `Presenter::tick` can dispatch transform geometry, and that handler can start another finite transition, so `settle()` moves forward again. Sixteen outer passes never increment.

Step 4 waits until `image.rs:412-443` is false. `Prepared::Failed` does clear that view (`:439`). `Refusal::QueueFull` and `Refusal::Budget` leave `pending()` true (`:432-436`), so the wait never ends on its own. The step also never applies the decode. Reports reach the kernel only through `poll_images` / `wait_images` → `apply_reports` → `set_intrinsics` (`presenter/images.rs:7-33`, `host.rs:1005`). `wait_images` is already bounded and returns whether anything changed. D3 cites the predicate, and the work list mentions `set_intrinsics`, and the image step calls neither. A capture can paint the pre-decode layout, miss the cascade 1106 D1a (d) described, or block in the image wait.

`Presenter::tick` assigns `dirty = true` on every call (`presenter.rs:1408`). `frame()` later sets `dirty` from `collection.pending()` (`content_region/presenter.rs:226`), before `sync_images` (`:227`). Sampling "the frame left the presenter dirty" is well-defined only after that assignment. `clock settle` turns the frame step off (D5), so a tick's dirty flag has nothing to clear it.

**Resolution:** Make one predicate, evaluated at one point in the pass. Include `accessibilityBusy` in it, and state that device holds stay outside it (`has_pending` already excludes them). Cap the motion seek with the same deadline and the pass budget, and check both inside the seek. Define the image step as `wait_images(remaining deadline)`, then treat a true return as work. State that `QueueFull` / `Budget` end `deadline` / `images`. Say that `dirty` is read after `frame()` returns, and that a mode with no frame step does not treat `tick`'s assignment as work.

### BLOCKING — D5's command-line column does not implement D3, and it does not match 1101.003 r4 D2

**Section:** D5 command-line column; D3 step 1.

**Evidence:** 1101.003 D2 says its list is the column and that "LLP 1107 D5 quotes it, so the two can't drift." D5 does not quote it. The two already disagree on the fixpoint.

D3's data step calls `land_then`, which passes `timers: false` (`commit.rs:286`). Due `every` and `after` tasks do not fire. 1101.003's fixpoint "fires every timer and continuation due at the wall clock," then sleeps until the next wake or due time, then tests `has_pending`, any armed or due one-shot, and `accessibilityBusy`. An armed `after(5000)` holds the run before it is due.

D5 says both "D3 (b) data step" and "timers fire as due," and its round-cap cell is "a fixpoint until a pass commits nothing." A pass that commits nothing while a request is in flight, or while an `after` is armed but not due, stops under that cell. 1101.003 explicitly continues. `timer_due_ms` (`runner/src/runner/gates.rs:95-106`) cannot tell an `after` from an `every`; both have a finite `next_ms`. 1101.003 already asks for a one-shot query. 1107's change table does not mention it, and D5 does not say the data step is replaced by `advance_timed` plus a sleep.

D3's data step also calls `sync_surfaces` and `settle_collections`. The terminal host has neither. D5 says motion is absent and images are synchronous, and it leaves those two calls in the procedure the column says to run.

**Resolution:** Replace the command-line cells with the 1101.003 list, including the sleep, the done test after a quiet pass, and which D3 steps this host skips. Say the data step for this column is "announcements, `advance_timed` to the wall clock, layout," and that `land_then` is the agent-clock data step. One of the two documents has to be the quotation; the sentence in 1101.003 D2 is currently false.

### BLOCKING — D2 changes the agent clock in a way LLP 1012 does not allow, and the clocks never rejoin

**Section:** D2; D5 `clock settle` / test columns; Questions for Charlie, question 1.

**Evidence:** LLP 1012 §2: `clock` moves the runner clock and the motion clock to one instant and lands where the runner says. A `tap`'s animation sits at local time 0 until a later `clock`. Events carry that clock. On the web that is one clock (`llp/1012-agent-api-v1.spec.md` "Freezing on the web").

D2 stamps the runner with `runner.now_ms()` and leaves `Host::now_ms` at the seek. That matches the web for the stamp. It does not match the web for birth time. `commit_effects` seeks to `max(receipt.at_ms, engine.now())` and then applies the new transition (`host.rs:1266-1285`), so the transition is born at the host clock. LLP 1012's web animation is born at the agent clock and paused there.

The reunion is unspecified, and today's `advanced` prevents `clock +N` from creating one. `host.now_ms = a.now_ms.max(self.now_ms)` (`host.rs:987`) keeps the host clock at the seek. After a capture at T+5000 with the runner at T:

- `clock +100` advances the runner to T+100, fires timers due by then, and ticks the engine at T+5000 again.
- The reply `clock` is the runner's, so `scripts/agent.mjs:1233` sets `s.now` to T+100 while the pixels stay at T+5000.
- A timer commit is stamped at its due time and then seeked with `max`, so `now()` and the transition's birth time disagree.

Question 1 asks only whether the next `tap` should refrain from firing crossed timers. The `clock` operation's meaning changes in the same edit. Today's agent behavior does not change until something seeks the host clock, because the two clocks are equal on the agent server today. Shipping the seek without an LLP 1012 amendment ships a second meaning of `clock`.

D5's `clock settle` column also drops behavior 1012 requires. `clock_within` extends the target with canvas `world[].settleAt` (`agent.rs:875-900`) and returns `settled: false, reason: "device"` while a hold remains (`:884-893`). That reason is specified (`llp/1012-agent-api-v1.spec.md` "Held device requests") and the driver branches on it (`scripts/agent.mjs:1236`). The column holds the loop on `pending()` and the settle time. A shared function built from that column reports `settled: true` while a world is moving and while a device ticket is held.

**Resolution:** Keep D2's stamp. Add the reunion rule in this RFC and name the LLP 1012 amendment it requires. The rule has to say what `clock +N`, `clock settle`, and a `tap` do to both clocks after a capture, and at which clock a transition started by that `tap` or by a timer is born. Preserve `reason: "device"` and the world target for `clock settle`, and state that its reply stays `{clock, settled, reason}` rather than D3's `complete` / `deadline` / `exhausted`.

### MAJOR — D4's set is the right shape and is not yet a runner query

**Section:** D4; D5 tool-call column.

**Evidence:** Copying an id from a commit onto new pending entries, `then_due` slots, queue waiters, and `unsent` sends can be done at `enqueue` (`commit.rs:1006`), `arm_then` (`:538`), `wait_turn` (`queue.rs:122`), and the `unsent` pushes (`commit.rs:657`, `:754`). The fulfilment commit has to copy the id off the pending entry before `remove` at `commit.rs:1217`. That sentence is missing.

Three paths do not create a new entry, so "copied onto every entry it creates" leaves them unowned:

- The keep-ticket return (`commit.rs:980-995`) updates arguments on an in-flight HTTP resource and returns. Fieldnotes' storage `library` does not take this path. An HTTP resource whose arguments the tool changes does.
- A stream stays in `pending` after the first message (`runner.rs:301-305`). If membership is "a tagged pending entry," the set never empties. LLP 1105 D4 refuses streams as `failed`. This RFC never says the stream leaves the set, or that the call fails instead of waiting.
- Background work is armed from `take_requests` (`commit.rs:1054`) and counted by `has_pending`. It is not in D4's list. LLP 1105 fails the call when an answer schedules it. The query as written cannot see it.

D5 says the app's timers stay live during a tool call. A timer the tool arms, including `after(0, …)`, is a commit with no id. Completion does not wait for it. That is a coherent choice, and it is the difference between this set and "whatever the tool caused." It needs to be stated, because `then` is in the set and `after` is the other continuation form.

App-wide serialization is not a runner field. Each window has its own runner. LLP 1105 D4 serializes per session. D4 here refuses a second call from any window while one is admitted, consent included, and the change table only adds runner ids. The lock's owner is unspecified. Question 3 leaves the rule open while the section states it as decided.

The summary says all three siblings cite "D3 under D5's column." LLP 1105's parked header cites D4 under the tool-call column. The tool column does not run D3. Citing D3 there would put a tool call back on the whole-tree settle this RFC correctly rejects.

**Resolution:** Define the outstanding set as the tagged in-flight requests, due or armed owned `then`s, owned queue entries, and owned `unsent` sends. Say the keep-ticket path adopts the id onto the existing entry or the call ends with 1105's kept-ticket outcome. Say a stream or a background schedule is reported to 1105 and is not waited on. Say timers and frame tasks the call arms are outside the set. Name the host-level lock if serialization is app-wide, and make the summary cite D4 for the tool column.

### MINOR — Citations and inventories an implementer will trust

**Section:** D1, D2.

**Evidence:** Off-by-a-few citations are listed in §1 (`host.rs:583`, `:858`, `:869-871`, `engine.rs:549`, `agent.mjs:1218`). D1's "read by" cell lists receipts' `at_ms`. Those stamps are written from the host clock; the engine reads them. The three presentation paths are real (`transform_geometry.rs:183`, `collection.rs:490` and `:705`, `images.rs:29-31`) and incomplete (`deliver_resizes` at `transform_geometry.rs:224`; `agent::handle`'s pump at `agent.rs:66`). `settle()` as cited (`agent.rs:691-697`) does include press settle, because the host op chains it (`host.rs:654-661`), and `group_settles_at`. The RFC never says so. There is no engine query that means "an infinite animation is running." `settle_time` omits them (`motion/src/engine.rs:669-677`, `animate.rs:360-368`). `quiescent()` is false for any sampled animation, finite or not (`engine.rs:647-649`). "Reported `motion: infinite`" has no source.

**Resolution:** Fix the line numbers, point the image step at `wait_images`, list `deliver_resizes` and the pre-op pump, and name the new engine read for infinite animations. 1106 already called that read new work.

## 4. Suggestions

- Split D3 into the agent-clock pass (`land_then`, no timers) and the wall-clock pass (`advance_timed`, sleep until the next due one-shot). D5 then selects a pass instead of asking one step to do both.
- Specify the picture reply beside the procedure: `settled` is `complete | deadline | exhausted | error`, `reason` is the step, `motion` is `finite | infinite | none`, `clock` is the runner's. Say `clock data` and `clock settle` keep the boolean `settled` the driver already reads.
- In D4, one sentence on capture-before-remove at `commit.rs:1217` is the difference between a design and a patch.
- Add the web and Apple rows as "same stamp rule, audited when built" only after the reunion rule is host-independent. The web is the clock oracle; a Linux birth time that disagrees with LLP 1012's paused animation needs to be called a deviation.
- The header's "implementer: none yet" matches `rules/RULES.md`. Leave the status Draft until the picture host or the tool-call host is named.

## 5. Open questions for the author

1. After a capture has moved the host clock, does `clock +N` move both clocks by N from the runner, snap the runner up to the host clock (the alternative D2 rejects), or leave the gap `Host::advanced` leaves today? LLP 1012 has to say the same thing.
2. A transition started by the `tap` or the timer after that capture is born at which clock? The web freezes a new animation at the agent clock.
3. Does picture mode report `motion: "infinite"` from a new engine query, and does an infinite-only scene seek nowhere, as `settle()` returning `None` implies?
4. For the command line, is 1101.003 r4 D2's fixpoint normative even where D5's round-cap cell says a pass that commits nothing is done?
5. Does a tool call own a keep-ticket HTTP re-ask, and does an `after` the action arms hold completion the way a `then` does?
6. Of your question 3: per session (LLP 1105 D4 as parked) or app-wide? The lock is outside the runner either way. The section should match the answer.
7. Of your question 2: the 16-pass cap is sound only after the motion seek and the image wait are inside the same budget. A chain of transitions, or a `Budget` refusal, exhausts or blocks regardless of the number.

## 6. Verdict

Verdict: NOT READY

## Round 2 (2026-10-08), on r2

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the four drafts and only Grok's own earlier reviews; 2026-10-08T02:19:20Z to 2026-10-08T02:34:03Z.
- **Method:** one brief (sha256 `0c33b34bcc6be753a91cd4d008376034f8e67d8922f3a20a51de870fbd28dbd7`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("ok sounds reasonable, go ahead" to the capture-as-sample revision and another round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** r3 (2026-10-08) takes it by narrowing: a final capture seeks through the real presenter, and a mid-test capture paints a scratch frame from an engine clone with `PaintMotion` and press at `factor(t_s)`, refusing `height`, `ghost` and `images` (both blockers). `t_s / 1000` and the behind-`engine.now` rule; timers off inside `dispatch_at`, covering `focus` and surface messages; `work` defined and a paint is not work; 17 request round-trips; the command-line cell, `clock settle`'s three reasons, and the new lock taken. `Layout` samples as paint-only, as this review noted.

I'll review the r2 RFC read-only: binding rules first, then the earlier review, the revised document, its sibling, and the code those claims cite.The r2 text is in. Next I’ll read the round-1 review and the code the new claims cite, starting with the motion engine and the settle path.## 1. Overall assessment

The clock decision is sound, and it removes the r1 failure it was written to remove. A capture that clones the engine and calls `advance` (`motion/src/engine.rs:524-547`) shows the same samples a seek would, and because that clone is thrown away, `Host::now_ms` and `Runner::now_ms` stay equal. `clock +100`, `tap`, and `clock data` then behave as they do today, and LLP 1012 §2 does not need a second meaning. `Engine` can derive `Clone` once `Clocks`, `Joining`, and `Timelines` do: `Slot`, `Transitions`, `AnimationPlay`, and the velocity tracker already do.

The picture procedure that is supposed to use that sample is not implementable as written. The pixels are taken after the sampled height layout has been undone, `layout_motion` is not the pure function D3 says it is, and the overlay D3 builds is not what the Linux painter reads. An implementer who follows the numbered steps ships a capture at the session clock for height, color, press, and the reorder ghost, and leaves session damage behind.

## 2. Previous-round concerns

- D3 had two predicates and an unbounded inner seek. **Partly.** One `quiet` predicate, no seek loop, `wait_images(remaining)`, and `QueueFull` / `Budget` end `deadline` (`image.rs:432-436`). The pass's `work` bit is still undefined, and the new sampled frame does not terminate in a correct picture.
- D5's command-line column disagreed with LLP 1101.003. **Resolved.** D5 names 1101.003 D2 as the authority, and r5 D2 says that list is normative for the column. The wall-clock gloss underneath is thinner than that list (suggestion below).
- A capture seek split the clocks and `clock +N` was refused (`agent.rs:814-815`). **Resolved.** Nothing in D1 or D3 assigns `Host::now_ms` or seeks the session engine. `clock data` and `clock settle` stay on today's replies, including world `settleAt` (`agent.rs:875-879`) and `reason: "device"` (`agent.rs:884-893`).
- D4 was not yet a runner query. **Resolved.** The four creation sites match the code (`commit.rs:1006`, `commit.rs:538`, `queue.rs:122`, `commit.rs:657` and `:754`), the fulfilment id is readable before `remove` (`commit.rs:1217`), and the keep-ticket return (`commit.rs:980-995`), `then_due` overwrite (`commit.rs:535-540`), stream (`runner.rs:303-305`), and background arm (`commit.rs:1054-1055`) are the right branches.
- Citations and the missing infinite read. **Resolved.** The r1 line numbers are corrected, `deliver_resizes` and the pre-op pump are listed, and `infinite()` is named against `settle_time` (`engine.rs:672-678`, `animate.rs:360-369`) and `quiescent` (`engine.rs:648-649`).

## 3. New concerns

### BLOCKING — The sampled height layout is gone before the pixels are taken

**Section:** D3, "The sampled frame," steps 3 and 4.

**Evidence:** Paint reads `node.frame` from the kernel (`host/linux/src/paint.rs:862-864`). `Host::present` skips `Property::Height` (`host.rs:1350-1351`); height reaches the picture only through `layout`. Step 3 swaps the sampled engine in, lays out, paints, swaps it back, and lays out again. Step 4, which is the step that returns pixels, runs after that restore. The capture therefore shows the session's current height.

`layout_motion` is not a pure function of its inputs. Lines `height.rs:152-158` are a cache check that calls `layout` (`height.rs:100-148`). That path calls `sync_height_owner`, which `MotionSync::apply`s into the engine (`kernel/src/motion.rs:97-120`), writes kernel geometry in `compute_layout_presented`, accumulates `flow_damage` and `row_dirty` (`damage.rs:38-44`), and increments perf `moved` (`runner/src/runner/perf.rs:45-56`). `observe` also removes the property from `played` before it notices the target is unchanged (`engine.rs:419-427`). A second layout overwrites kernel frames when the height projection differs. It does not undo damage, perf counts, or that `played` removal. `Property::Layout` does not need this swap: the painter applies it as a translate of the presented box (`paint.rs:891-892`), and `layout_motion` only compares the height projection.

**Resolution:** Paint while the sampled layout is installed, then restore. Name every host and kernel field the layout writes, and restore those too, or run the layout against a scratch kernel. Drop `Layout` from the swap. State that `observe_layout` staying off does not make `layout` side-effect-free.

### BLOCKING — The overlay is not what the painter samples

**Section:** D3, "The sampled frame," steps 1, 2, and 4.

**Evidence:** `Host::presented` does not return the `presented` map. It fills press from `self.now_ms` (`host.rs:628-631`). Press settle is part of `t_s` (`host.rs:654-661`, `press.rs:51-56`, 120 ms), so a settled picture that paints press at the session clock shows a press still in flight. The reorder ghost is the same kind of motion: `group_settles_at` (`presenter/group.rs:424-435`) is in `settle()` (`agent.rs:693-697`), and the ghost moves only in `tick_arrange`, which a sample never calls.

Paint colors are not in the `presented` map. `present` sends them through `present_paint` (`host.rs:1355-1357`, `paint_motion.rs:81-113`), which writes `self.paint` and calls `engine.target` and `engine.is_active` on the live engine. An overlay built as a copy of `host.presented` drops color, shadow, and `currentcolor`, and those engine queries answer at the session time.

`frame_sampled` is specified as `frame()` (`content_region/presenter.rs:49-231`) minus `boxes`, `scroll`, `last_region_frame`, `flow_damage`, `dirty`, and `queue_collections`. `frame` also consumes `take_row_dirty` (`presenter.rs:66`, `host.rs:643-648`), calls `sync_canvases` (`host.rs:365-417`), `refine_collections`, `poll_content_region`, and `sync_images`, writes `last_frame_succeeded` (`presenter.rs:147`), and on a failed paint `mem::take`s `boxes` (`presenter.rs:170-180`). The same paragraph says the function writes no `dirty` and that it marks every row dirty afterwards. Marking rows dirty after the paint invalidates the next real frame. Kept row pixmaps are invalidated from the dirty set before the walk (`paint/rows.rs:119-163`), so the capture itself still composites rows rasterized at the session clock.

**Resolution:** Specify one scratch presentation: the `presented` map, `PaintMotion`, press evaluated at `t_s`, and the ghost pose at `t_s`, all read from the sample. `frame_sampled` paints from that scratch and must not call `take_row_dirty`, `sync_canvases`, `refine_collections`, or `sync_images`, and must not write `last_frame_succeeded`. Invalidate rows before the sampled paint. Delete the sentence that says it writes no `dirty`.

### MAJOR — `t_s` is milliseconds and `Engine::advance` takes seconds

**Section:** D1, "The sample."

**Evidence:** `settle()` is milliseconds: the engine value is multiplied by 1000 (`host.rs:656-658`), and press and the ghost are on `now_ms`. `Host::tick` seeks with `now_ms / 1000.0` (`host.rs:1285`). D1 passes `t_s` straight to `sample_at`, defined as `advance(t)`. A clone advanced by the millisecond number seeks thousands of seconds ahead. The sentence that `validate_time` cannot trip (`engine.rs:553`) is about the session engine. On the clone, a `t_s` behind `engine.now` — possible for `group_settles_at` when the computed rest is already past and the phase is still `Landing` — returns `ClockWentBackwards`.

**Resolution:** State that `sample_at` takes engine seconds, `t_s / 1000.0`, and that a settle time behind `engine.now` samples at `engine.now`.

### MAJOR — Timers-off is on five call sites, and the data pass dispatches through others

**Section:** D2.

**Evidence:** `Runner::dispatch_at` fires every timer due at `now` (`commit.rs:188-189`, selection `next_ms <= now_ms` at `commit.rs:319`). D2 switches five sites to `dispatch_settling`. The data pass also runs `run_commands` and `sync_surfaces`. `focus` reaches `dispatch_at` at `presenter/events.rs:94` via `set_focus`. The change-table test is a `then` that issues `focus`. Surface messages reach `dispatch_at` at `surfaces.rs:775` and `:856`. Neither site is in the five, so those events still run `advance_timed(now)`.

**Resolution:** While the settle flag is set, `Host::dispatch_at` itself uses the timers-off path. Name `focus` / `blur` and surface messages as callers the flag has to cover.

### MAJOR — A pass's `work` bit does not count what the budget test counts

**Section:** D3, the pass, the `exhausted` end, and the test list.

**Evidence:** The loop stops on a quiet pass with no work, and each step returns `work`. The sampled frame is inside the pass. If painting counts as work, every pass does work and 16 passes end `exhausted`. If it does not, the document never says so. Separately, `land_then` is `advance_within(now, false, false)` (`commit.rs:285-287`), and that loop runs every `then` armed by an earlier `then` in the same call, up to `TIMER_FIRE_LIMIT` (4096, `runner.rs:563`). A chain of 17 synchronous `then`s is one pass. The test "17 chained continuations end `exhausted`" holds only for continuations that each wait on a later pump.

**Resolution:** Define `work` as a session change from pump, `land_then`, commands, collections, or an image report. The sampled paint is not work. State that `exhausted` counts passes, and that the 17-continuation test is 17 request round-trips.

## 4. Suggestions

- Narrow D1's "the session after a capture is the session before it" to the clocks and the session engine. The image step's `wait_images` calls `set_intrinsics` (`presenter/images.rs:18-23`, `host.rs:1022-1024`), which writes kernel intrinsic sizes and lays out. That change is what makes the revealed image settle, and it survives the capture.
- Point the command-line pass cell at "1101.003 D2's fixpoint" rather than the one-line wall-clock gloss. The gloss omits announcements, the commits-nothing test, and the busy check that D2 spells out.
- D5's `clock settle` bound cell names only `reason: "device"`. Today's reply also uses `"world"` and `"requests"` (`agent.rs:840-843`). The authority row already says the column is unchanged. Repeat those two reasons in the cell.
- `Session.swift:139` is the `ExactApp` class, which is the right owner for the new lock. The citation reads like an existing lock. Say the lock is new.

## 5. Open questions for the author

1. Press and the reorder ghost are in `t_s` and are not engine slots. Should the settled picture evaluate both at `t_s`, or does v1 sample only engine properties?
2. Do image intrinsics applied while settling remain in the session after the reply?
3. For the budget test, is one continuation one `then` inside a single `land_then`, or one request that is pumped on a later pass?

Verdict: NOT READY

## Round 3 (2026-10-08), on r3

- **Family:** xAI. `grok-4.7` via the grok CLI (`--prompt-file`), reasoning effort xhigh, `--no-subagents`, `--always-approve` with Edit and Write denied, `--output-format plain`, `--cwd` a detached worktree at `2ce193976` holding the four drafts and only Grok's own earlier 1107 reviews; 2026-10-08T06:10:14Z to 2026-10-08T06:52:47Z.
- **Method:** one brief (sha256 `e3a6b3687dd291405e01f12b859b4c239ea28990b33152ebeb0d5e6d3bdba8f9`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("sure do both" to the final-capture/mid-test split and a review of 1107 only). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited; the sentences before the assessment are the CLI's progress narration.
- **Verdict:** NOT READY.
- **Disposition:** r4 (2026-10-08) takes it, narrowed by Charlie: mid-test sampling is dropped (a mid-test screenshot paints now and reports `motion`), so the scratch-paint blocker, the `PaintMotion` concern, the mid-test settle and the refusal predicates are moot and recorded under "Isolated sampling (later)". Blocking 1: a final capture seeks inside each pass until no finite target lies ahead, each seek spending the budget, with *t_s* from `settle()` at `agent.rs:755-760` (D1, D3). The flag spans all of `handle`, re-verified at `ce31a47b6` (D2). `infinite()` dropped; the busy poll pumps; citations corrected.

I'll review r3 read-only: binding rules first, then the earlier reviews, the RFC, its sibling, and the code behind the new claims.The r3 text and both earlier reviews are in. Next I’ll check the new behavioral claims against the cited code, starting with paint-only motion, the sampled frame, and the settle flag.The split is the right one, and both departures from Charlie's brief hold. The procedure still cannot be built as written: a final capture's single seek is not a fixed point, and a mid-test sample still has no session-preserving paint.

## 1. Overall assessment

r3 is the right shape. A final capture may seek, because nothing afterwards can observe the session. A mid-test capture may sample only what a clone can paint, and must refuse the rest. The clocks stay together in the case that broke r1 (`clock +100` at `agent.rs:814-815`), and LLP 1012 stays unamended. D4, the command-line column, and the `work` budget are in good enough shape to implement. D5's command-line cells match LLP 1101.003 r6 D2, which says its fixpoint is normative for that column.

Both departures are right:

- **`Layout` is paint-only.** `Host::present` writes it into the presented map and does not lay out (`host.rs:1366-1368`). `layout_presented` turns the sample into an origin offset and a surface scale (`kernel/src/motion/layout.rs:178-192`). The painter applies the offset in `Presented::transform` (`paint/presented.rs:105-110`) and the scale only to the node's own surface (`:117-119`); children keep the kernel box. Height is the property that lays out (`host.rs:1350-1351`, `height.rs:152-158`). Refusing `height` is the layout refusal. The citation `paint.rs:891-892` is only the layer branch of that offset.
- **Press can be sampled.** `PressFeedback::factor` is a pure function of the record and a millisecond time (`press.rs:20-22`; the 120 ms duration is compared with `now_ms` at `:51-56`). The painter multiplies that factor into scale (`paint/presented.rs:109`). At *t_s* the factor matches a real `tick`, which retires a finished release only after the factor has reached identity (`press.rs:59-64`). The overlay has to store `factor(t_s)` itself. `Host::presented()` overwrites press from `now_ms` (`host.rs:628-631`).

What is not buildable is the picture. The final path seeks once and then stops, on a clock that the seek itself moves forward. The mid-test path describes a scratch paint that the Linux painter cannot do without either a copy of the row cache or writes into the live one.

## 2. Round-2 concerns

- Sampled height was undone before the pixels, and `layout` is not pure. **Resolved.** Mid-test refuses `height` and does not call `layout`; a final capture calls `Presenter::tick` (`presenter.rs:1399-1409`). `layout` still writes kernel geometry, damage, and `played` (`height.rs:100-158`, `engine.rs:419-427`).
- The overlay dropped press, the ghost, and paint colours, and the sampled frame wrote session state. **Partly.** Press is `factor(t_s)`; a ghost not at rest is refused. Paint colours and the row cache are still not a session-preserving read (concerns 2 and 3).
- *t_s* in milliseconds was passed to `advance`, which takes seconds. **Resolved.** D1 uses `t_s / 1000`, and a time behind `engine.now` samples at `engine.now` (`engine.rs:549-555`, `host.rs:1178`).
- Timers-off was five call sites, so `focus` and surface messages still advanced. **Partly.** The switch is inside `Host::dispatch_at`, which covers `events.rs:94`, `surfaces.rs:775` and `:856`, and `transform_geometry.rs:224`. The flag's cited span does not match `handle` (concern 4).
- `work` could count the paint, and seventeen synchronous `then`s are one `land_then`. **Resolved.** D3 defines `work` as a session change and excludes a paint. `land_then` is `advance_within(now, false, false)` (`commit.rs:285-287`); `TIMER_FIRE_LIMIT` is 4096 (`runner.rs:563`). The test is seventeen pumped round-trips.

The command-line cell, `clock settle`'s `"world"` / `"requests"` / `"device"` (`agent.rs:840-843`, `:884-893`), and the new `ExactApp` lock (`Session.swift:139`) were taken. The narrowed "engine unchanged" sentence is now false (concern 5).

## 3. New concerns

### BLOCKING — A final capture seeks once, and the seek starts motion after *t_s*

**Section:** D1, "A final capture seeks"; D3's `clock` line and `quiet`.

**Evidence:** *t_s* is computed, then `Presenter::tick(t_s)` runs, then D3 loops until `quiet`. `quiet` has no settle time and no ghost. `Host::tick` seeks, lays out height, and on a change calls `observe_layout` (`host.rs:1175-1190`, `presence.rs:57-63`). For a node with `layout-transition`, that observes a new `Layout` curve born at the clock just seeked (`kernel/src/motion/layout.rs:135-137`, `:166-168`). `settle_time` would then lie past *t_s* (`engine.rs:672-677`). The picture is taken at the start of that curve. When height did change, `Presenter::tick` also clamps scroll, queues collections, and refreshes transform geometry (`presenter.rs:1400-1406`); those dispatches can start further curves. `clock settle` already re-reads the target and seeks again (`agent.rs:875-901`). D3 says "one seek".

The ghost half of *t_s* is cited at the wrong function. `host.rs:654-661` chains engine settle and `press_settle` only. `group_settles_at` is added in `agent.rs:693-698`. An implementer who follows the citation seeks to a time at which a landing ghost is still moving (`group.rs:424-435`), and `quiet` does not notice.

**Resolution:** Seek until a freshly computed *t_s* is not ahead of the host clock, inside the 16-pass budget, and include that test in `quiet`. Name `settle` at `agent.rs:693-698` as the function that produces *t_s*. Say where `tick` sits in the pass: before the loop, after a quiet data pass, or both.

### BLOCKING — The scratch paint has nowhere to put row pixels, and the epilogue paints for real

**Section:** D1, "The scratch frame"; D2's epilogue.

**Evidence:** Kept rows live on the shared painter. `rows_dirty` sets `stale` on `rows.kept` (`paint/rows.rs:149-161`). A walk that misses `row_valid` records a new pixmap into that same map (`:453-473`). `row_valid` checks scale, size, images, and frames (`:312-340`). It does not check opacity, translate, colour, or layout offset, so a sample baked into the cache is what the next real frame replays. There is no copy of `rows.kept` in the code. "The capture's copy" is undefined, and a scratch presenter is deferred.

`frame()` also writes past the exclusion list: it publishes scroll from painted boxes (`content_region/presenter.rs:206-212`), clears `flow_damage` (`:196-198`), assigns `dirty` (`:226`), may `queue_collections` (`:216-225`), and captures action bindings while painting (`:107-121`). Deleting only the named calls leaves those.

D2 then runs the real `frame()` in the epilogue whenever `dirty` is set (`agent.rs:81-84`). The prelude's `poll_images` sets `dirty` when a report arrives (`presenter/images.rs:25-27`). `frame_sampled` is specified not to clear it. The epilogue frame calls `sync_images`, which is the cancel D1 refused the sample for (`image.rs:136-142`, cancels at `:188` and below).

**Resolution:** For a mid-test capture, the walk does not call `row`, `rows_dirty`, `rows_begin`, or `rows_end` on the live painter (row keeping off for that walk). State that the epilogue does not call the real `frame()` or `follow_pointer`. Extend the exclusion list to every write in `frame()` after the paint returns, including scroll, `flow_damage`, `dirty`, `queue_collections`, and action capture.

### MAJOR — Paint colours are not a read of the engine clone

**Section:** D1, "Paint-only".

**Evidence:** `PaintMotion` is host state (`kernel/src/motion/paint.rs:11-21`), not a field of `Engine`. The cited `present_paint` (`paint_motion.rs:81-113`) writes `presented` and `row_dirty`, resolves `currentcolor` sides through `engine.is_active` and `presented(view)`, and walks inheritors (`:115-139`). `PaintMotion::shown` (`kernel/src/motion/paint.rs:179-194`) is the pure per-node read and does not do that walk. A settled paint value is stored as `None`, meaning "use the row"; copying the engine sample in as an override paints a different colour. `Property::PAINT` is eleven entries (`property.rs:128-140`), of which `BoxShadow` is offset and blur (`:79-80`), not a colour.

**Resolution:** Specify the overlay as a copy of the `presented` map. Apply `Host::present`'s match using the clone, including `layout_presented(&clone, …)`. Run `present_paint`'s inheritance against that copy and the clone, writing neither the live map nor `row_dirty`. Say a settled paint property clears the override.

### MAJOR — The flag's span does not match `handle`

**Section:** D2, "The flag's span".

**Evidence:** The prelude is cited as `agent.rs:59-66`. `run_commands` is at `:71` and `sync_surfaces` at `:72`, both before `answer`. The epilogue is cited as `:75-85`. `first_pixel` is at `:86`, outside it, and `activate_first_pixel` commits (`presenter/display_frame.rs:293-302`). `Runner::dispatch_at` fires every timer due by its argument (`commit.rs:188-189`). A flag that covers only the cited ranges leaves those commits on that path. `poll_update` (`:69`) and `poll_development` (`:70`) sit in the gap and are not named.

**Resolution:** Set the flag on entry to `handle` for a picture or a test screenshot and clear it after `first_pixel` returns. Name `:69-72` and `:86` inside the span.

### MAJOR — A mid-test settle can change the engine, and a refusal does not undo it

**Section:** D1, "What stays the same"; D3's data pass and images step; D5's mid-test images cell.

**Evidence:** The data pass refines and settles collections. `settle_collections` calls `refine_collections` (`collection.rs:776-784`), and a scroll that authors `scroll` dispatches through `dispatch_at` (`:703-705`, and `dispatch_authored_scroll` at `:490`). With timers off, the event still commits, and `commit_effects` seeks the live engine and can start a curve (`host.rs:1266-1285`). D3 also runs `wait_images`, which applies intrinsics and lays out (`presenter/images.rs:18-31`). D5 says a mid-test capture uses images already decoded and otherwise refuses. D1 says the clone is rebuilt after image work. A height curve started by that work is then refused, after the session has changed. The "engine exactly as before" sentence survives only if none of those commits start motion.

**Resolution:** Say the pre-sample settle may commit, and that the engine is unchanged only when those commits start nothing. Pick one images rule: either `wait_images` runs and intrinsics stay, or it does not and the refusal is decided before any image report is applied. A `refused` result is still a settle; say whether it rolls back.

### MAJOR — The refusal predicates are not functions

**Section:** D1, the three `refused` reasons.

**Evidence:** "Ghost not at rest" is not `group_settles_at`. That returns `None` during `Phase::Active` (`group.rs:425-427`), while the ghost still needs frames (`:439-444`). "An image the sampled frame shows that is not decoded" has no query. `sync_visible` both computes visibility and cancels (`image.rs:136-142`). Height is named; the other two are descriptions.

**Resolution:** Name the ghost test as `group_needs_frame` (or an equivalent that is true for `Active` and `Landing`). Define the image test as: after the scratch walk, an image it drew whose bitmap is absent, judged without calling `sync_visible`.

### MINOR — `infinite()` is in the change table and nowhere in D1

**Section:** "What changes, where".

**Evidence:** The motion row still lists `infinite()`. `settle_time` already omits infinite animations (`engine.rs:669-671`, `animate.rs:360-369`). No decision says what a capture reports or seeks when that is the only motion.

**Resolution:** Drop `infinite()` from the table, or say that `settle()` returning `None` means *t_s* is now and the capture shows that frame.

### MINOR — "read dirty after" has no consequence

**Section:** D3, the `frame` step.

**Evidence:** `work` excludes a paint. `Presenter::tick` sets `dirty = true` unconditionally (`presenter.rs:1408`), and `frame()` then sets it from `collection.pending()` (`content_region/presenter.rs:226`). Nothing says what the read is for.

**Resolution:** Delete the phrase, or say the read is not `work` and does not cause another pass.

## 4. Suggestions

- Point the layout sentence at `Presented::transform` and `Presented::surface` as well as `paint.rs:891-892`, so the size scale is part of the paint-only claim.
- In D5, the command-line frame cell says "per commit". r6 D2 says the column has no frame step and lays out each commit. The authority cell already wins; make the frame cell say that.
- Name collection settlement next to image intrinsics as session state a mid-test settle is allowed to keep.
- The busy poll should say that each poll pumps announcements. A thread clears `aria-busy` by a commit, and a poll that only re-reads the flag waits until the deadline. Polls still do not spend the 16.
- `source.rs:252` is `discard`. The background method with no invocation id is `runner/src/runner/source.rs:259`. `force_refresh` is `commit.rs:957`. `refresh_next` is assigned at `settlement.rs:771`.

## 5. Open questions for the author

1. After `tick` observes a new `layout-transition`, does the final capture seek again, the way `clock_within` does, or is the picture the first frame of that curve?
2. Is `group_needs_frame` the ghost refusal, including a drag still in `Phase::Active`?
3. On a mid-test capture, does `wait_images` run before the `images` refusal?
4. Does the 20 ms `accessibilityBusy` poll pump?

## 6. Verdict

Verdict: NOT READY
