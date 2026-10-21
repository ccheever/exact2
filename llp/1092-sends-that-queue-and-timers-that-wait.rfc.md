# LLP 1092: Sends that queue and timers that wait

**Type:** RFC
**Status:** Accepted (r4); stages 1–3 built 2026-10-04 ("As built", §6). Accepted (r4, by the orchestrator under Charlie's delegation after three review rounds; Grok 4.7 only — Codex budget exhausted; round-3 findings folded unreviewed — the implementation review checks them). Every review is Grok 4.7 (xhigh), one family: r1 with two scopes, semantics (`llp/reviews/1092-r1.grok-a.md`, READY WITH CHANGES) and implementation (`llp/reviews/1092-r1.grok-b.md`, NOT READY); r2 a delta review (`llp/reviews/1092-r2.grok.md`, NOT READY); r3 the final delta review (`llp/reviews/1092-r3.grok.md`, NOT READY, three MATERIAL), whose fixes r4 folds as given (§9). The orchestrator decided r1's open questions under Charlie's 2026-10-04 delegation (§8); the `rules/DEFERRED.md` waiver is recorded in its own commit (`3555dc4e6`).
**Systems:** Contract compiler, Plan (`mutations`, `timers`), Runner (`commit.rs`, `admission.rs`, `lists.rs`, `runner.rs`, `agent.rs`, new `queue.rs` and `gates.rs`), JS target (`rt.js`, `agent.js`, `emit.rs`, new `schedule.js`), conformance, Lean and difftest, docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Revised:** 2026-10-05 (r2, r3, r4)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stages 1 and 2 on 2026-10-05, stage 3 on 2026-10-06 (§6)
**Amends:** LLP 1016 §4 (newest-wins, unless queued); LLP 1016.001; LLP 1088 D8; LLP 1017 P4c's deferral of `task … when` (D11); LLP 1073 D4; `rules/DEFERRED.md` **Motion**
**Related:** LLP 1005 §6; LLP 1012 §2; LLP 1016 D5; LLP 1027.005; LLP 1035.005.000 D7 and §5; LLP 1041 D2; LLP 1054.000.000 D1; diaries `~/projects/x2apps/{kanban2,flashcards,spreadsheet,studio,ledger2,chat,chat2,trivia,pomodoro}/DIARY.md`

## Summary

Contract's async model has two gaps that the app diaries hit again and again:

- **A second `send` to a mutation forgets the one in flight.** Kanban2 lost
  a tap during a blur-save; three apps split mutations to work around it.
- **A timer runs for the app's whole life or not at all.** A toast's five
  seconds needs an always-on tick that commits even at rest.

This RFC adds one clause to each declaration:

```
mutation wrote as shape Wrote queue refreshes doc then afterWrote

task hide when toast != "" key=toastUntil
  after(5000, expire)
```

- **`queue`:** one request in flight; later sends wait, in order, each
  asked after the previous reply and its `then`.
- **`when` and `key=`:** the timer exists while its condition holds, as a
  `when` arm's nodes do; a new key restarts it, as a new `each` key makes a
  new row. Nothing runs when the gate changes.

Both are opt-in; without them, behaviour is unchanged.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `queue` on the mutation declaration; the default stays newest-wins | kanban2 #1, flashcards F4 | 1 |
| D2 | One request in flight; later sends wait with their send-time arguments | kanban2 | 1 |
| D3 | When a mutation is free; a waiting send is asked in a `next` commit on the wake path | studio R8 | 1 |
| D4 | `pending`, `then`, `refreshes`, assignment, failure, reload, the bound | — | 1 |
| D5 | What the data module sees | spreadsheet | 1 |
| D6 | The compiler and the JS target | spreadsheet | 1 |
| D7 | `task NAME when COND [key=EXPR]` | ledger2 #1, chat F7 | 2 |
| D8 | When a gated task arms, re-arms and drops its timer | — | 2 |
| D9 | Gates and keys lower to plan code; refusals | — | 2 |
| D10 | Frames, the clock, hosts and the agent | trivia | 2 |
| D11 | A gated timer is not the deferred reaction | LLP 1017 P4c | 2 |
| D12 | Lean and difftest | — | 3 |
| D13 | Docs and adoption | all | 1, 2 |

## 1. Evidence

### 1.1 Sends

- **Kanban2 #1** (`DIARY.md:33`, `:71`). A label tap during a blur-save
  through `wrote` checked `not pending(wrote)` and did not send; sending
  would have forgotten the save's reply (`contract-for-agents.md:323`).
  The workaround is a second mutation, `aside`. The diary asks for "a
  write queue, or a click that does not cancel the save already in
  flight".
- **Flashcards F4.** `commitEdit` sent `saveCard`, then `setImage`, to
  `edited`; `then afterEdit` saw only the second ack. LLP 1088 D8 now
  refuses this, and the app sends `setImage` through `imaged`.
- **Spreadsheet.** D8 refused `down` and `openSheet`; the fix was a
  mutation per request (`committed`). Before D8, "the data module saw
  both, in order": forgetting drops the reply, not the request.
- **Studio R8.** Each `op` reply carries a `seq`, so `afterOp` acts once
  per reply.

Today `send` asks the source in the commit (`commit.rs:480–518`), and both
`enqueue` (`:750–754`) and an assignment to the slot (`:567–613`) forget
the request in flight; the JS target's `mut()` holds one ticket
(`rt.js:463–498`). Forgetting can drop the write itself: a host lets go of
the work for a ticket the runner no longer `holds` (`commit.rs:897–902`;
Linux `presenter.rs:772`), and the JS target dispatches a multi-step
answer's next step only while it is held (`rt.js:473`). A save written as a
storage step plus a continuation can stop between them.

### 1.2 Timers

`task NAME mount` arms its one schedule at boot (`runner.rs:926–941`). Only
the root may declare a task (`types/src/lib.rs:1261`).

| App | Task | Needed only while |
|---|---|---|
| ledger2 (#1, Rough 8) | `every(1000, expire)` | an undo toast shows |
| chat (F7), chat2 | `every(200, tick)` | a bot reply is due, or a toast shows |
| trivia | `every(100, tick)` | `screen == "play"` |
| pomodoro | `every(1000, tick)` | always: it advances `nowMs`, the day clock behind `dayNum` |
| studio | `every(15000, beat)` | always (a real heartbeat) |

Every tick is a commit even when it writes nothing: ledger2's `logs` show
`t=5000 advance → 4 timers fired, epoch 12`, and chat's `perf` 10 epochs
for 10 ticks. LLP 1035.005.000 moved the answer-pollers to `then` and left
time over (Bluesky's toast, Messages' flight phases: "an action cannot
start a one-shot timer"); this RFC is that recount. A replaced toast's five
seconds must also restart ("an `after` that state re-arms"), as a
debounce's must.

## 2. What the web and React do

**Sends.**
- Two `fetch`es both run and both resolve, unordered; `AbortController`
  cancels when asked.
- SWR's `useSWRMutation` keeps the latest trigger's data: exact2's default.
- TanStack Query v5 runs mutations sharing a `scope: { id }` serially, in
  order: the FIFO chosen here. React 19's `useActionState` likewise runs
  dispatched actions one after another.

**Timers.**
- The web has imperative `setTimeout`/`clearTimeout`; React puts them in an
  effect:

  ```js
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(expire, 5000);
    return () => clearTimeout(t);
  }, [toast, toastUntil]);
  ```

  The timer lives while its condition holds and restarts when a dependency
  changes. D7 is that effect without code: the condition is `when` and the
  dependency list is `key=`.

## 3. Decisions

### D1 — `queue` on the declaration; the default stays newest-wins

```ebnf
mutation = "mutation" IDENT "as" "shape" type [ "queue" ]
           [ "refreshes" IDENT { "," IDENT } ] [ "then" IDENT ] NL ;
```

`queue` is a keyword only in that position. It belongs on the declaration
for the reason `then` does (LLP 1016.001 D1): every send of a mutation
means the same thing.

The default stays newest-wins: LLP 1016 §4's logout rule depends on it (an
assignment drops a late sign-in reply), and a sign-in or a superseded draft
save wants the latest answer. A write log wants every send.

### D2 — One request in flight; later sends wait

A send to a queue mutation `m` is **asked** in the sending commit only when
`m` is **free** (D3) and no earlier send of `m` was made in this commit.
Otherwise it joins `m`'s queue as its source and argument values, evaluated
where the send stands (LLP 1005 §6): kanban's tap sends the label it was
tapped for. Only an asked send calls the source and hands out a
request. The queue is in the commit's `Checkpoint`
(`commit.rs:18–30`), so a refused action adds nothing to it.

### D3 — When a mutation is free; the `next` commit

**In flight.** A send of `m` is in flight from its ask until its source
answers `Now`, or its request ends with no answer (`release_failed` in
`admission.rs`, including admission refusals since issue #286). A further `Later` round
(`commit.rs:1036–1042`, "one more round") is not a reply: the send stays in
flight and runs no `then`. A storage step and its continuation therefore
finish before the next send is asked.

**Free.** `m` is free when no request of `m` is in flight, its `then_due`
is not armed, its `next_due` is not armed, and it is not stalled (below).
The runner checks this from state, not from the kind of commit
(`queue.rs`, `arm_next`), after every commit concludes, whether it stood or
was refused. Admission failure now also commits the release of its ticket
(issue #286, 2026-10-08); it has no separate early-return path.

When `m` is free and a send waits, `next_due[m] = now`. A `then` is checked
only after its commit concludes, so `then_due` being cleared before the
`then` runs (`commit.rs:271`) never frees `m` early.

**The wake path.** `timer_due_ms` (`runner.rs:1199`) reports `next_due`
exactly as it reports `then_due`; Apple, Linux and the wasm glue read it
after every batch. The JS target's wall clock has no such read: `drive()`
is called only from a commit that armed a `then` (`rt.js:184`), so arming
`next` calls `drive()` too, whether or not the mutation has a `then` (D6).
A queue with no `then` (studio's `rec`, kanban's `wrote` after the fold) is
then asked as soon as its reply lands, with no other timer. `clock settle`'s
advance to now runs every due `next`, then waits on the request it hands
out, so it does not return while a send waits that is due. A stalled send
(below) is an error stop, not a send `settle` holds: it is neither due nor
in flight, and `settle` returns.

**Order.** At one time, `advance` runs every armed `then`, then every armed
`next`, then the timers, each group by index (`land_then`, `commit.rs:207`,
runs the first two). This extends LLP 1016.001 D3.

**A `next` commit.** `next_due[m]` is cleared before the commit, outside
the checkpoint, as `then_due` is (`commit.rs:271`). Inside the commit the
head is popped and asked as an action's send is: an answer now lands there
and arms `then`; a request goes out when the commit stands. Journaled
`wrote next (1 waiting)`. The queue is in the checkpoint, so a refused
`next` commit restores the head, and what happens next depends on what
refused it:

- **The ask's own refusal** (the source errors on `answer`, or a `Now`
  answer misses the shape): the send can never be asked, so the head is
  popped again after the restore, journaled `wrote queued send refused: …`,
  and the scan arms the successor at that time. The advance stops with the
  error, as for a refused `then` (`what` = `wrote next`); the next advance
  asks the successor before any timer.
- **Any other refusal** (settlement, a router value, D8's `TaskKey`): the
  head stays first, and `m` is **stalled**. The scan does not arm a stalled
  mutation, so no host wakes in a loop (`timer_due_ms` does not report it).
  A stall is cleared only by a standing commit that changed a slot, a
  resource or a derive value, a superset of the state the refusal saw; a
  commit that changed none of them (a frame task that writes nothing, a
  timer whose action is a no-op) leaves it stalled, so a permanently
  failing ask is not retried every frame. Journaled `wrote next refused
  (…); waits for a change`.
- **`frame()` and a stall.** `frame()` returns before firing frame tasks
  when its prelude `advance` refused (`commit.rs:158–161`). A refusal of a
  `next` in that prelude is journaled and does not stop the frame: armed
  frame tasks still fire at that frame. The stalled `next` runs only on a
  later wake, after those frame tasks, and only once a change has cleared
  the stall. With stalls cleared only by a change, one `advance(60_000)`
  and sixty `advance(1_000)`s stop at the same fire and resume at the same
  commit.
- **Poison** clears the queues (D4); nothing runs on a poisoned runner.

A shaped failure is an answer (LLP 1016 D4) and runs `then`.

Asking the head inside the reply's commit was not taken: two answers given
at once would land in one commit, and `then` would see only the second.

### D4 — What each part of the model does with a queue

- **A waiting send's write** shows from its send, as a send in flight does,
  when `m` declares `refreshes` (LLP 1054.000.000 D1);
  one whose ask is refused at its `next` ends, stops showing in that commit,
  and the resources it showed in are asked again.
- **`pending(m)`** is true while a send of `m` is in flight or waits. It is
  false once a reply has landed and nothing waits, even while that reply's
  `then` is armed. Sending while `pending` is how a queue is fed: `not
  pending(m)` means the spinner is off, not that a send may be skipped
  (kanban drops its guard).
- **`then`** runs once per reply, in send order, reading the slot as that
  reply set it. The 2026-09-27 ruling (once per advance, the latest) agrees,
  because no second reply can land before the `then` runs.
  `analyze-then-self-send` stays (§8).
- **`refreshes`** asks nothing at a send, asked or waiting: its write shows
  through the data module's overlay (LLP 1054.000.000 D1). It forces the
  resources at each reply, with their arguments in that commit, not the
  queued body's.
- **Assignment.** `m = none` writes the slot and forgets nothing. The reply
  in flight and every waiting send's reply still land and overwrite it.
  A mutation that must drop a late reply (a session) does not declare
  `queue`. No cancellation (§8).
- **A failure that brings no answer** leaves the slot as it was, runs no
  `then`, and frees `m`.
- **Bound.** At most 64 sends wait per mutation; the send in flight does not
  count. A send that would be the 65th waiter refuses its action
  (`RunnerError::QueueFull`). Refusal is the overload policy (LLP 1041 D2):
  dropping a write silently is the bug being removed.
- **Reload, poison, a dev restart** drop the waiting sends along with the
  ticket in flight. They were never asked, so there is no ticket and no
  source call (no `discard`): one line, `forgot 2 waiting sends (wrote)`.
  `poison()` (`commit.rs:662–669`) clears the queues. The queue is not
  carried. A kept answer (LLP 1027.005) is not a record of a send that was
  never asked, so a carried draft can be ahead of the resource, with
  `pending` false.

### D5 — What the data module sees

- One request at a time, in send order. `answer(store, source, args)`
  (`source.rs:280–285`) is called when the send is asked, so the store holds
  the previous reply's writes; the arguments are the send-time values.
- No seam change.
- Two queue mutations do not order each other; to order two kinds of
  write, send both through one.

### D6 — The compiler and the JS target

- **AST and plan.** `MutationDecl` gains `queue: bool`
  (`fn mutation`, `syntax/src/parser.rs:815`, moves with `fn task` into
  `parser/decls.rs`). Plan `mutations` gains `queue: bool`; `FORMAT_DIGEST`
  changes.
- **`analyze-send-twice`** (`analyze/src/sends.rs:118`) skips queue
  mutations. For the rest, its message adds the fix, except when the same
  action assigns the mutation's slot (a reply it means to drop):

  > `commitEdit` sends `edited` twice; only the last send's reply reaches
  > `then afterEdit` (LLP 1016 D5). Send once, use a mutation per request,
  > or declare `mutation edited … queue` to run both in order.

- **The JS target** emits `mut(…, queue)` and keeps the queue in
  `schedule.js`. For a queue mutation:
  - an assignment's `forget` (`rt.js:152`) does nothing;
  - `pending` stays true while the wait list is non-empty, even with
    `ticket == null`;
  - the commit's `undo` pushes and restores the wait list and the stall
    bit, beside `m.ticket` (`rt.js:164`);
  - `gated()` runs inside the commit's undo `try`, after `settle()`
    (`rt.js:157–170`), and pushes `clock.timers` onto that undo, so a gate
    refusal is a refusal, not a poison from the tree update
    (`rt.js:176–179`);
  - `ask` and `refreshes` (`rt.js:483–493`) run only when a send is asked;
  - `advance` (`rt.js:263–275`) runs every due `then`, then every due
    `next`, then timers, each group by index;
  - arming `m.next` calls `drive()` when `!clock.agent`, as arming a `then`
    does (`rt.js:184`), with or without `m.then`, and `drive()`'s minimum
    includes `m.next`;
  - the agent's jump predicate (`agent.js:313`, `clock.timers` only today)
    includes every mutation's `due` and `next`.

### D7 — `task NAME when COND [key=EXPR]`

```ebnf
task = "task" IDENT ( "mount" | gate ) block(schedule) ;
gate = "when" expr [ "key" "=" expr ] | "key" "=" expr ;
```

```
task hide when toast != "" key=toastUntil     -- ledger2's undo toast
  after(5000, expire)
task autosave when draft != saved key=draft   -- a debounce
  after(800, save)
task round when screen == "play"              -- trivia
  every(100, tick)
task fly when flying                          -- Messages' flight phases
  every(frame, step)
```

`mount` is the ungated task, unchanged. `key=` alone means
`when true key=…`, spelled as `each` spells it. Tasks stay root-only, and
the schedules and their literal intervals do not change.

### D8 — When a gated task arms, re-arms and drops its timer

Each task is **idle** or **armed**. Idle is an infinite deadline, as a
spent `after` is today, so neither `advance_within` (`commit.rs:240–245`)
nor `frame()` can fire it. The gate step (`gates.rs`) evaluates each gated
task's gate and key code (D9) right after the commit's settlement
succeeds, inside the rollback a settlement failure takes, and never at the
start of `Runner::update`: there a refusal would leave row slots,
`requests` and `into_view` applied (`commit.rs:546–645`; `lists.rs:44–98`).
In `run_action_inner` the step joins the settlement-error path
(`commit.rs:592–602`): on `TaskKey` or a trap it takes that path
(`discard_later`, undo the row slots), before `enqueue` and `into_view`.
In `fulfill_inner` it joins `router_change().and_then(settle)`
(`commit.rs:1074`), and in `commit_again` it joins `settle(false)`
(`settlement.rs:75`, a one-line call site); neither has row writes. Timer
state joins the `Checkpoint`, so a refused commit restores what the step
changed. The checkpoint is taken after `advance`'s
own bookkeeping (a fired timer's next deadline, a cleared `then_due` or
`next_due`), so a refused fire is not fired again. At boot, which makes its first frame with `Tree::create` and
not `update`, the step runs where timers are armed today: after `settle`
and `init_late_slots`, before `Tree::create` (`runner.rs:923–945`).

| Gate | Key | Result |
|---|---|---|
| false | — | idle; an armed timer is dropped and nothing runs |
| true, was false (or boot) | — | armed: `after` and `every` at now + ms; a frame task as D10 says |
| true | changed | re-armed from now |
| true | unchanged | as it was: an `every` keeps its phase; a spent `after` stays spent |

- **Keys compare by `key_text`** (`instance.rs:1030`, made `pub(crate)`),
  as an `each` key does: `-0` is `0`. A non-finite number is not a key
  (`key_text` gives `None`): that commit is refused
  (`RunnerError::TaskKey`) and the timer is unchanged.
- **"Now" is the commit's time.** A commit that a timer makes at its due
  time arms from that due time.
- **A task's own fire** is a commit like any other: an `every` whose action
  clears its gate stops, and a debounce whose `save` makes `draft == saved`
  goes idle.
- **An `after` fires at its due time exactly,** so its action sees
  `now()` equal to the deadline it was armed for
  (`semantics/corpus/timers/after.contract`: `at == 500`). An action that
  re-tests the deadline with a strict `>` does nothing; the timer has
  already measured the interval (D13).

### D9 — Gates and keys lower to plan code

`lower` (`timers.rs`, moved out of `lower/src/lib.rs`) compiles a gate
(`bool`) and a key (string, number or bool) as plan code, the way a slot's
initializer is (`format.json:121`, `code`). Plan `timers` gains `gated:
bool`, `gate: code`, `keyed: bool` and `key: code`; an ungated task's gate
is the constant `true`. The runner evaluates them with `Runner::eval`
(`runner.rs:1365`) in D8's step, reading the commit's settled derives,
resources and `pending`; a trap refuses that commit. Nothing is a derive,
so `state().derives` and difftest's `observe` are unchanged, and Lean
evaluates the same expressions (D12). The JS target compiles each to a
function the gate step calls.

Refusals:

- **`type-task-gate`**: "`when` takes a bool; `toast` is a string; write
  `toast != ""`".
- **`type-task-key`**: a key that is not a string, number or bool.
- **`analyze-task-gate-clock`**: a gate or key that reads `now()`, directly,
  through a derive, or through a `fn` body. "A gate is read at commits, not
  as the clock moves, so an `after` gated on `now()` cannot be dropped
  before it fires, and an `every` stops up to an interval late. Gate on
  state (`toast != ""`) and let `after(5000, …)` measure the time."

### D10 — Frames, the clock, hosts and the agent

- **Frame tasks.** While a host presents frames, arming lists the task:
  `wants_frames()` (`runner.rs:1191`) becomes "a frame task is armed", and
  `frame(now)` (`commit.rs:156–187`) fires only armed frame tasks, once,
  at that frame, as LLP 1073 D4 says; an idle one is skipped. While the
  agent or a test seeks, arming sets the task's next virtual frame from the
  commit's time (`virtual_frame(now, 1)`), and dropping sets it infinite, so
  a seek never fires an idle frame task. In the JS target, `gated()`
  removes an idle frame task from `clock.timers`, which is what stops
  `paint()` (`rt.js:297–306`); arming it inserts it with
  `due = vf(now, 1)` and, off the agent, calls `paint()` (which returns at
  once under the agent, `rt.js:301`).
- **The clock.** `timer_due_ms` reports only armed tasks (and D3's
  `next_due`), so an idle gated task keeps no host awake.
- **Hosts.** No change: Apple reads `timer_due_ms` and `wants_frames` each
  batch (`host/apple/src/host.rs:1128–1132`), Linux each loop
  (`display.rs:525`), Windows through the Linux presenter, and the wasm
  glue each turn.
- **The seek rule.** One `advance(60_000)` equals sixty `advance(1_000)`s
  with no reply outstanding, virtual frames included. A driver jump stops
  at a commit that hands out a request and lands its reply first
  (`advance_until_request`, `commit.rs:192–197`; LLP 1012 §2). Under the
  driver, `clock +60000` and sixty `clock +1000` agree with each other.
- **The agent's `state`** gains `tasks` (due time or `null`) and `queued`
  (waiting counts), printed by `runner/src/agent/schedule.rs`. No operation
  is added.
- **A dev reload** restarts tasks over the carried state, as today.

### D11 — A gated timer is not the deferred reaction

LLP 1017 P4c moved "general `task … when [dep]`" to DEFERRED after diary
0102's hydration race, in which a reaction copied a query's answer into a
draft and raced the typing. LLP 1035.005.000 §5 kept it out.

That form ran an action when its dependency changed. A gated task runs
nothing then: the change starts a timer, and the action runs at least 1 ms
later as its own commit, reading the state as it is then (`then`'s
discipline). `after(1, seed)` bends it toward a reaction, as
`task boot mount after(1, start)` already does; the docs name `then` as the
reaction to an answer. The waiver is recorded under **Motion** in
`rules/DEFERRED.md` (`3555dc4e6`).

### D12 — Lean and difftest

Lean asks every send now (`Runtime.lean:477`); its oracle answers at once,
so nothing is ever in flight (`Big.lean`, `pendingSettled`), and it has no
`then` or frame tasks.

- **Queues.** `runAction` puts a queue mutation's later sends in a commit
  into `Config.queued`. `dispatch` does not drain it, because the runner's
  `dispatch` does not and difftest observes `dispatch` without `land_then`
  (`observe.rs:191–193`). `advance` drains it at its start and again after
  every commit it makes, before the next timer, a commit each, at the
  clock's time, as the runner's `next`s run after each commit and before
  later timers (D3). `observe` prints each queue's waiting count, on both
  sides. Each drain commit is a D3 `next` commit: on success it applies
  D8 after its update; on the ask's own refusal it drops the head and stops
  the advance; on any other refusal it keeps the head, records a stall, and
  is not drained at the start of a later `advance` until a standing commit
  changes a slot, resource or derive value.
- **Gates.** `contract lean` emits a task's gate and key as expressions on
  the task, as D9 lowers them. `boot` applies D8's table after `lateSlots`, not in
  today's `startTimers` (`Runtime.lean:548–555`); every `runAction` that
  stands applies it after `update`. Keys compare as `rowKey` does
  (`Runtime.lean:273–281`).
- **Proofs.** `StepSound`, `Invariant` and `Soundness` carry the new
  fields.
- **Corpus.** `mutations/queue-*` (a tap that sends twice, observed again
  after a clock step; a timer that sends twice inside one advance) and
  `timers/gated-*`, all `then`-free and frame-free. Per-reply `then` order and a send behind one in flight are
  runner and conformance tests (§5); Lean cannot express them.
  `mutations/two-sends.contract` is refused on main today
  (`analyze-send-twice`, by probe), so `difftest corpus` is red apart from
  this RFC: stage 1 splits its sends over two mutations, and stage 3 adds
  `queue-two-sends`.
- **The generator** emits queue mutations and gated tasks, never two sends
  to a mutation without `queue`, and never a `now()` gate.

### D13 — Docs and adoption

**Docs.** `contract-for-agents.md` gains table rows (`:300`, `:730`):
"writes that must all land, in order → `mutation … queue`" and "a timer
while something shows → `task … when cond`, restarted by `key=`". `:323`
and `:535` follow, with D4's `pending` and assignment sentences. The
grammar, its contextual words and `contract-for-humans.md:1043–1066`
change; `agent-pitfalls.md` gains "a gate reads state at commits".

**Adoption** (outside the repo, `EXACT_APP_DIR`; driven on web and macOS).
Stage 1:
- **Kanban2** folds `aside` into `wrote queue`. `afterWrote` gains
  `afterAside`'s `if not w.ok` → `dueFor = ""`. The label tap's `pending`
  guard goes.
- **Flashcards** sends `saveCard` and `setImage` to `edited queue`.
  `imaged` goes. `afterEdit` has no `setImage` arm, so that ack falls
  through. The editor still closes in the action.
- **Spreadsheet** folds `committed` into `edited queue`. The commit op's
  reply carries `select = false`, and `afterEdit` already shows a failure's
  message.
- **Studio** declares `op … queue` and drops `seq` and `handledOp` from
  `afterOp`. The `file` relay stays, because `closeBoard`'s arguments come
  from the file reply (`studio/app.contract:472–512`).

Stage 2:
Each toast task's action clears without re-testing the time (D8: the
`after` fires with `now()` equal to the deadline, and today's strict `>`,
`ledger2/app.contract:65`, `chat/app.contract:149`, would leave the toast
up forever; shown by running a probe of ledger2's shape through
`contract-difftest verify`, runner and Lean agreeing):
- **Ledger2**: `task hide when toast != "" key=toastUntil after(5000,
  hideToast)`, where `hideToast` sets `toast = ""` and `undoId = ""`;
  the `every(1000)` task and `expire` go.
- **Chat**: two toast tasks, `when toast != "" and not toastUndo
  key=toastAt` with `after(1500, clearToast)` and `… and toastUndo
  key=toastAt` with `after(4000, clearToast)`; `clearToast` sets
  `toast = ""` and `toastUndo = false`. The bots become
  `when box.nextDue > 0 every(200, tick)`, still polling while a reply is
  due; `tick` keeps only the bot branch and its `not pending(stepped)`
  guard.
- **Chat2** (`chat2/app.contract:117–124`, `:149`, `:167`): one toast task,
  `when toast != "" key=toastUntil after(1600, clearToast)`, where
  `clearToast` sets `toast = ""` (chat2 has no `toastUndo`,
  `chat2/app.contract:30–31`), and
  `when phase == "wait" every(200, tick)`; `tick` keeps only the wait
  branch and its `not pending(changed)` guard.
- **Trivia**: `when screen == "play"`; the settings pause stays in `tick`.
- **Pomodoro** is not adopted: its tick is the day clock.

## 4. Effect on each implementation

Every file stays under the 1,500-line cap (`rules/RULES.md`). The work lands
in new files, and files near the cap gain only call sites:

| | Stage 1 (D1–D6) | Stage 2 (D7–D11) |
|---|---|---|
| syntax | `parser/decls.rs`: `fn mutation`, `fn task` from `parser.rs` (1,443) | `when`/`key=` there |
| types, analyze | `sends.rs` exemption and hint | `types/src/tasks.rs`; `analyze/src/gates.rs` |
| lower, plan | `mutations.queue` | `lower/src/timers.rs`, from `lib.rs` (1,458); `timers.gated`, `.gate`, `.keyed`, `.key` |
| runner | `runner/queue.rs`: queues, `next_due`, stalls, the scan, `QueueFull`; call sites in `commit.rs` (1,077: `conclude`'s callers, the `next` commit) and `admission.rs` (the early return, `:67–68`) | `runner/gates.rs`; call sites on the settlement paths of `run_action_inner` and `fulfill_inner` (`commit.rs`), `commit_again` (`settlement.rs:75`, one line: 1,452), `runner.rs` boot (`:924–945`) and `frame()` |
| agent | `agent/schedule.rs` (`agent.rs` is 1,408) | the same |
| JS target | `schedule.js` (`rt.js` is 1,338); `src/timers.rs` from `emit.rs` (1,409); `agent.js`'s jump predicate | `gated(…)` there |
| hosts | none | none |

Stage 3 is D12, Lean and difftest only.

## 5. Tests

- **Compiler** (`contract/cli/tests/it/mutation.rs`, `time.rs`,
  `contract/corpus/rejects.txt`): `queue` in D1's position; send-twice
  accepted for a queue, refused otherwise, D6's message asserted whole with
  and without the hint; `when`, `when … key=`, `key=` alone; D9's refusals,
  `now()` through a `fn` included.
- **Plans.** Decode every in-repo app's plan before and after: equal, with
  `queue == false` and no gate or key everywhere, and a changed digest. The
  bytes change, since every row gains the fields.
- **Queues** (`mutation.rs`):
  - two sends in one action, answered now and later;
  - a send while one is in flight;
  - one `then` per reply, in order;
  - a queue with no `then` asked by `timer_due_ms` alone;
  - a `next` refused by its own ask: dropped, its successor asked before a
    due timer;
  - a `next` refused by settlement or `TaskKey`: the head kept, the
    mutation stalled (no due time reported); a standing commit that changes
    nothing leaves it stalled, one that changes a slot asks it;
  - with an armed frame task and a stalled `next`, every `frame()` fires
    the frame task;
  - a gate refusal in an action that wrote a row slot and sent a `Later`:
    the row slot, `requests` and `into_view` are all rolled back;
  - a refused `then` that does not stall;
  - assignment overwritten by a later reply;
  - a failure, released, that moves on;
  - a `Later` round that is not a reply;
  - the 65th waiter refused;
  - poison's and reload's journal line and no source call;
  - a two-step continuation completing behind a second send (today's
    dropped save).
- **Timers** (`time.rs`):
  - on then off before the due time fires nothing;
  - a key change re-arms, a NaN key refuses its commit, `-0` equals `0`;
  - a spent `after` stays spent, and an `every` that clears its gate stops;
  - a boot gate that reads a derive and a resource's baked answer;
  - an `after` whose action re-tests its deadline with `>` does nothing
    (the strict compare), and one that clears unconditionally clears;
  - `frame()` skips an idle frame task, a seek never fires one, and
    `wants_frames` follows the gate;
  - a gate armed inside an advance fires within it;
  - one `advance(60_000)` equals sixty `advance(1_000)`s;
  - an idle gated task reports no due time;
  - a driver case: a gated tick sends and its `then` clears the gate, and
    `clock +60000` and sixty `clock +1000` stop together.
- **Conformance.** `host/web-js/conformance/queue.contract` and
  `gates.contract`, with `.steps`, compare journals and state across the
  wasm runner, the JS runtime and the Linux host (`conform.mjs`), a jump
  over a due `next` included.
- **The JS wall clock.** `host/web/tests/js-runtime.test.mjs` gains a queue
  with no `then`: a reply lands, and the waiting send is asked by `drive()`
  alone, off the agent.
- **Lean.** `lake build`; difftest `corpus` and `random --count 500`
  (async lane).
- **Driven.** D13's adoptions, and the five checks after each stage.

## 6. Implementation plan

Each commit passes the five checks.

1. **Stage 1, 2026-10-05: queued sends (D1–D6, D13), with §4's files.**
   Exit: §5's queue tests and conformance; kanban2's blur-and-tap test
   (type notes, tap the label, no settle) on web and macOS.
2. **Stage 2, 2026-10-05: gated tasks (D7–D11, D13).** Exit: §5's timer
   tests and conformance; ledger2 makes no commit at rest over
   `perf during "clock +60000"`.
3. **Stage 3, 2026-10-06: D12.** A slipped proof lands as a restriction
   named in `semantics/README.md` with a `QUEUE.md` line; no `sorry`.

### As built (stages 1–3, 2026-10-04)

- **Stage 1** (`220d614fe`). As D1–D6. `queue.rs` holds the queues, `next_due`,
  the stall (with the slots, derives and resources its refusal saw, a `Basis`)
  and the scan, which runs after every commit concludes and on
  `release_refused`'s early return; `run_next` is the `next` commit. Beyond the
  text:
  - `land_then` (an agent input's end) runs due `next`s as well as `then`s:
    they are due at now and are not timers.
  - A `next` refused by its own ask that leaves nothing waiting or in flight
    commits again (`a refused queued send`), so the view hears `pending` end,
    as after a failed reply.
  - `frame()` keeps the prelude's refusal in `Advanced.error` and still fires
    the armed frame tasks.
  - The JS target's answer now lands before the action's own writes, as the
    runner's does (a send answered now and an assignment of its slot in one
    action: the assignment wins). That was a divergence for every mutation;
    `queue.contract` found it.
  - The journal lines: `wrote next (1 waiting)`, `wrote queued send refused:
    …`, `wrote next refused (…); waits for a change`, `forgot 2 waiting sends
    (wrote)`. A reload's line is the new runner's, from `Carried.forgot_waiting`.
  - `has_timers` counts a queue mutation.
- **Stage 2** (`eadd390a7`). As D7–D11. Beyond the text:
  - Plan `timers` also gains `name`, for the agent's `state.tasks`.
  - A settlement that stood has already published its caches and handed out
    its resources' requests when the gate step runs, so a commit's checkpoint
    also keeps what a settlement publishes (`Published`, taken only for a plan
    with a gated task) and lets go the requests it handed out.
  - The gate step reads the commit's own sends as pending, as the settled
    derives did (settlement's flags follow tickets, handed out after the step).
  - A task that starts at neither `mount` nor a gate is `syntax-task-start`.
  - The JS target keeps `clock.timers` in plan order (`i`), so ties break as
    the runner's do, and a frame task armed or dropped by a frame task's own
    commit fires only if it was armed at the frame's start.
  - Lean's checker types gates and keys too (`taskGate`, `WellTyped.taskGates`).
- **Stage 3** (`908ce0f23`). As D12, with the restrictions in
  `semantics/README.md` ("Queued sends and gated tasks") and a `QUEUE.md`
  line: the one-slot invariants are stated for slots that are not a queue
  mutation's; Lean's `pending` of a waiting queue is false; the
  component-level semantics refuses both, so `difftest expansion` leaves them
  out. `nextCommit_sound`, the drain branch of `advance_sound`, boot's gate
  step and the frame lemmas are proved; no `sorry`.
- **Tests.** `contract/cli/tests/it/queue.rs` (§5's queue list, the stall by
  settlement and by `TaskKey`, a gate refusal on a reply's commit and on a
  commit made again), `time.rs` (§5's timer list, D9's refusals whole),
  `contract/corpus/rejects.txt`, `host/web/tests/js-runtime.test.mjs` (a queue
  with no `then` asked by `drive()` alone; a gate refusal rolled back, timers
  included). Conformance: `queue.contract` and `gates.contract`, every step
  equal on wasm and JS, and on Linux up to the step where it stops (an advance
  refused by a queued send's own ask); the whole `--synthetic --build` run has
  one failure, `bootpress`'s RealWorld type check, unrelated.
- **Difftest.** `quick` agrees; `corpus` 254 of 256 agree (2 outside); `random
  --seed 1 --count 500` 500 agree; `types --count 200`, `expansion` (corpus and
  `--count 100`), `lowering --count 100` and `apps` agree.
- **Plans.** Every in-repo app compiles to the same tables before and after,
  apart from the new fields (`queue == false`, no gate or key) and the pools
  they shift (code offsets, interned names); the digest changed.
- **Size.** Trivia's `app.js` and `rt` chunk, brotli: 38,955 B before, 39,201
  after unadopted (+246 B), 40,047 adopted with a gate (+846 B, `schedule.js`).
- **Adoption** (scratch copies, never `~/projects/x2apps`):
  - kanban2 (`aside` folded into `wrote queue`): `test web` 7/7 and `test
    macos` 7/7, the label tap while notes save included; its journal shows
    `wrote next (0 waiting)`.
  - spreadsheet (`committed` folded into `edited queue`): `test web` 15/15.
  - chat (two toast tasks, bots `when box.nextDue > 0`): `test web` 9/11, the
    same two failures as the unadopted app on this branch; at rest over
    `clock +60000` it commits nothing (300 commits before).
  - trivia (`when screen == "play"`): `test web` 9/9, `test macos` 9/9, no
    commit at rest on the menu on either host.
  - flashcards, studio, ledger2 and chat2 use LLP 1091's modules from main
    and do not compile on this branch's base; see the final report.
- **Found, not fixed.** The JS target's agent jump waits for the reply of a
  request a timer sent at the jump's last instant and runs its `then`; the
  wasm, Linux and Apple hosts' jumps do not (`QUEUE.md`).

## 7. Considered, not taken, and deferred

- **Queue by default.** It breaks the logout rule and changes what
  `pending` means for every existing mutation.
- **Pipelined sends** (all asked at once, replies held in send order). The
  source sees concurrent writes and must order them itself.
- **`then action(answer)`** (LLP 1016.001): every reply, but still
  forgotten requests and an unordered source.
- **Cancel on assignment.** It drops a person's writes silently.
- **An action-started timer** (chat F7). Hidden state outside the slots,
  not carried, needing a handle to cancel; D7 derives the timer from state
  the view already shows.
- **Component-scoped tasks.** Instance-bound timers in the runner, and a
  restart needs a keyed single instance Contract lacks (no `key=` on a
  component use, no list literals, LLP 1088 §9.1). Trigger: a per-row
  timer.
- **`animationend=expire`.** It ties app state to presentation, and reduced
  motion changes when it fires.
- **Free empty commits** (chat F7): QUEUE's "Presenter.apply runs its
  post-pass for a batch that changed nothing", independent of this RFC.

**Deferred, with preconditions:**
- **A computed interval, `after(expr)`.** Chat's and chat2's bots still poll
  every 200 ms while a reply is due. Precondition: a measured cost of that
  poll after stage 2, and a rule for an interval that changes while armed.
- **`then` in Lean,** and with it per-reply order in difftest.
  Precondition: QUEUE's "What the Lean semantics still leaves out" closes
  `then`.

## 8. Questions decided (the orchestrator, for Charlie, 2026-10-05)

1. **The waiver for D7–D11:** granted under Charlie's 2026-10-04
   delegation, recorded in `rules/DEFERRED.md`'s **Motion** entry as its own
   commit (`3555dc4e6`). No take.
2. **Cancelling a queue:** no `cancel` yet.
3. **A `then` that sends its own queue mutation:** stays refused.
4. **A minimum gated `after`:** none beyond `mount`'s 1 ms.
5. **The bound:** 64 waiting sends.

## 9. Revisions

- **r4** (2026-10-05, accepted). Grok 4.7 xhigh's final delta review of r3
  (`llp/reviews/1092-r3.grok.md`, NOT READY: three MATERIAL, two MINOR,
  one NIT). After three rounds, its fixes are folded as given, unreviewed;
  the implementation review checks them. The code confirmed each one.
  - D3: a stall is cleared only by a standing commit that changed state, and
    `frame()` still fires armed frame tasks after a `next` refusal in its
    prelude; `clock settle` treats a stall as an error stop.
  - D8: the gate step runs on each commit's settlement path, inside its
    rollback, not at the start of `Runner::update`.
  - D6: the JS `gated()` runs inside the undo `try`, which also restores
    `clock.timers` and the stall bit.
  - D12: Lean's drain commit is a D3 `next` commit, stall included.
  - D13: chat2's `clearToast` body.
- **r3** (2026-10-05, round 2 of 3). Grok 4.7 xhigh's delta review of r2
  (`llp/reviews/1092-r2.grok.md`, NOT READY). Each finding was checked
  against the code; the strict-compare finding was also shown by running a
  probe through `contract-difftest verify`.
  - D3: `next_due` cleared outside the checkpoint and the head restored by
    it; only the ask's own refusal drops the head, and any other refusal
    stalls the mutation until a commit stands. The JS wall clock calls
    `drive()` when it arms a `next`.
  - D6: the agent's jump predicate includes mutation dues.
  - D8: idle is an infinite deadline, so a seek never fires an idle task;
    the step is the start of `Runner::update` and, at boot, after
    `init_late_slots`; `key_text` becomes `pub(crate)`; an `after` fires
    at its deadline exactly.
  - D9: gates and keys are plan code evaluated in the step, not derives.
  - D12: Lean drains the queue after every commit of an advance, and
    `observe` prints waiting counts.
  - D13: chat2's own adoption; every toast's action clears without
    re-testing the time.
  - §4 names the runner's call sites; §5 adds the cases.
- **r2** (2026-10-05, round 1 of 3). Two Grok 4.7 xhigh reviews of r1, one
  family with two scopes. Each finding was checked against the code; the
  dispositions are in `llp/reviews/1092-r1.grok-{a,b}.md`.
  - D3 rebuilt: in flight until `Now` or a release; free is a state check
    after every commit and on `release_refused`; `next_due` on the wake path;
    the head popped before the checkpoint.
  - D8 runs inside the commit, under the checkpoint, after
    `init_late_slots` at boot; keys compare by `key_text`.
  - D10 splits frame arming between presenting and seeking, and `frame()`
    skips idle tasks.
  - D9's clock refusal walks `fn` bodies, and D9 hid the synthesized
    derives (r3 replaces them with plan code).
  - D12 drains in `advance` only, applies gates after `lateSlots`, and
    names the corpus's limits.
  - D4 restates `pending`, assignment, `refreshes`, reload and the bound.
  - D13 corrects studio, chat, chat2 and pomodoro, and writes the folded
    `then`s.
  - §4 names the new files under the line cap, and §5 compares decoded
    plans.
- **r1** (2026-10-05): first draft.
