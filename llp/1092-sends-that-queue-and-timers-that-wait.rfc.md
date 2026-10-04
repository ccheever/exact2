# LLP 1092: Sends that queue and timers that wait

**Type:** RFC
**Status:** Draft (r1)
**Systems:** Contract compiler, Plan (`mutations`, `timers`), Runner (`commit.rs`, `runner.rs`, `agent.rs`), JS target (`rt.js`, `emit.rs`), conformance, Lean and difftest, docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stages 1 and 2 on 2026-10-05, stage 3 on 2026-10-06 (§6)
**Amends:** LLP 1016 §4 (newest-wins, unless queued); LLP 1016.001; LLP 1088 D8; LLP 1017 P4c's deferral of `task … when` (D11); LLP 1073 D4; `rules/DEFERRED.md` **Motion**
**Related:** LLP 1005 §6; LLP 1016 D5; LLP 1035.005.000 D7 and §5; LLP 1041 D2; diaries `~/projects/x2apps/{kanban2,flashcards,spreadsheet,studio,ledger2,chat,chat2,trivia,pomodoro}/DIARY.md`

## Summary

Contract's async model has two gaps that the app diaries hit again and again:

- **A second `send` to a mutation forgets the one in flight.** Kanban2 lost
  a tap during a blur-save; three apps split mutations to work around it.
- **A timer runs for the app's whole life or not at all.** A toast's five
  seconds needs an always-on tick that commits even at rest.

This RFC adds one clause to each declaration:

```
mutation wrote as shape Wrote queue refreshes doc then afterWrote

task hide when toast != "" key=toastSeq
  after(5000, expire)
```

- **`queue`:** one request in flight; later sends wait, in order, each
  asked after the previous reply and its `then`.
- **`when` and `key=`:** the timer exists while its condition holds, as a
  `when` arm's nodes do; a new key restarts it, as a new `each` key makes a
  new row.

Both are opt-in; without them, behaviour is unchanged.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `queue` on the mutation declaration; the default stays newest-wins | kanban2 #1, flashcards F4 | 1 |
| D2 | One request in flight; later sends wait with their send-time arguments | kanban2 | 1 |
| D3 | A waiting send is asked in a commit of its own, after the previous reply's `then` | studio R8 | 1 |
| D4 | `pending`, `then`, `refreshes`, assignment, failure, reload, a bound | — | 1 |
| D5 | What the data module sees | spreadsheet | 1 |
| D6 | The compiler: a plan field; `analyze-send-twice` exempts a queue | spreadsheet | 1 |
| D7 | `task NAME when COND [key=EXPR]` | ledger2 #1, chat F7 | 2 |
| D8 | When a gated task arms, re-arms and drops its timer | — | 2 |
| D9 | Gates and keys lower to derives; three refusals | — | 2 |
| D10 | Clock, frames, hosts and the agent | trivia | 2 |
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
  `edited`. `then afterEdit` saw only the second ack, so the editor never
  closed. LLP 1088 D8 now refuses this.
- **Spreadsheet.** D8 refused `down` and `openSheet`, and the fix was a
  mutation per request. Before D8, "the data module saw both, in order":
  forgetting drops the reply, not the request.
- **Studio R8.** Save-then-close relays through a second mutation (`then`
  may not send its own); each reply carries a `seq` so the view acts once.
- **Chat and chat2** guard their sends with `not pending(…)`.

Today `send` asks the source in the commit (`commit.rs:480–518`), and both
`enqueue` (`:750–754`) and an assignment to the slot (`:567–613`) forget
the request in flight; the JS target's `mut()` holds one ticket
(`rt.js:463–498`). Forgetting can drop the write itself, not only its
reply: a host lets go of the work for a ticket the runner no longer
`holds` (`commit.rs:897–902`; Linux `presenter.rs:772`), and the JS target
dispatches a multi-step answer's next step only while it is held
(`rt.js:473`). A save written as a storage step plus a continuation can
stop between them.

### 1.2 Timers

`task NAME mount` arms its one schedule at boot (`runner.rs:928–941`). Only
the root may declare a task (`types/src/lib.rs:1261`).

| App | Task | Needed only while |
|---|---|---|
| ledger2 (#1, Rough 8) | `every(1000, expire)` | an undo toast shows |
| chat (F7), chat2 | `every(200, tick)` | a bot reply or a toast is due |
| trivia | `every(100, tick)` | `screen == "play"` |
| pomodoro | `every(1000, tick)` | the timer runs |
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
  }, [toast, toastSeq]);
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

A send to a queue mutation `m` asks its source in the sending commit only
when `m` is **idle**:

- no request of `m` is in flight;
- no send of `m` is waiting;
- no `then` of `m` is armed;
- no earlier send of `m` was made in this commit.

Otherwise it joins `m`'s queue as its source and argument values,
evaluated where the send stands (LLP 1005 §6): kanban's tap sends the label
it was tapped for. The queue is in the commit's checkpoint (`commit.rs:35`),
so a refused commit adds nothing.

### D3 — A waiting send is asked in a commit of its own

`m` becomes **free** once its request ends (its reply lands, or it fails
with no answer and is released) and, if that armed a `then`, once the
`then`'s commit ends, stood or refused. If a send is waiting, the runner
then arms `next_due[m] = now`, beside `then_due` (`commit.rs:395`).

At any one time, `advance` runs the armed `then`s, then the armed `next`s,
then the timers, each group by index. This extends LLP 1016.001 D3's rule
that a `then` goes before a timer.

- **A `next` commit** asks the head's source as an action's send does: an
  answer now lands there and arms `then`; a request goes out when it
  stands. Journaled `wrote next (1 waiting)`.
- **`land_then`** (`commit.rs:207`) runs everything due now except timers,
  so it runs `next`s too. An input step against a source that answers at
  once ends with every queued send answered and every `then` run.
- **A refused `next`** (the source refuses the ask, or the answer has the
  wrong shape) drops that send, journaled as `wrote queued send refused: …`.
  The advance stops at that time, as for a refused `then`, and the
  following send is armed.

Asking the head inside the reply's commit was not taken: two answers given
at once would land in one commit, and `then` would see only the second.

### D4 — What each part of the model does with a queue

- **`pending(m)`** is true while a request is in flight or a send waits.
- **`then`** runs once per reply, in send order, reading the slot as that
  reply set it. The 2026-09-27 ruling (once per advance, the latest)
  agrees: no second reply can land before the `then` runs. Studio's `seq`
  goes; `analyze-then-self-send` stays (§8 Q3).
- **`refreshes`** re-reads when a send is asked (in the action or in
  `next`), not when it joins the queue, and forces again at each reply.
- **Assignment.** `m = none` writes the slot. It forgets nothing and drops
  no waiting send, because the queue holds writes a person made. No
  cancellation is offered (§8 Q2).
- **A failure that brings no answer** leaves the slot as it was. `then`
  does not run, and the queue moves on.
- **Bound.** At most 64 sends wait per mutation; a 65th refuses its action
  (`RunnerError::QueueFull`). Refusal is the overload policy (LLP 1041 D2):
  dropping a write silently is the bug being removed.
- **Reload and poison** drop the waiting sends along with the ticket in
  flight, as LLP 1016 D5 drops tickets, journaled as `forgot`.

### D5 — What the data module sees

- One request at a time, in send order. Each `answer(source, args, store)`
  is called when its send is asked, so the store holds the previous
  reply's writes; the arguments are the send-time values.
- No seam change. A send never asked (a reload, poison) never reaches it.
- Two queue mutations do not order each other; to order two kinds of
  write, send both through one (kanban folds `aside` into `wrote`).

### D6 — The compiler

- **AST and plan.** `MutationDecl` gains `queue: bool`
  (`syntax/src/parser.rs:799`). Plan `mutations` gains `queue: bool`, and
  `FORMAT_DIGEST` changes.
- **`analyze-send-twice`** (`analyze/src/sends.rs:118`) skips queue
  mutations. For the rest, its message names the new fix:

  > `commitEdit` sends `edited` twice; only the last send's reply reaches
  > `then afterEdit` (LLP 1016 D5). Send once, use a mutation per request,
  > or declare `mutation edited … queue` to run both in order.

- **The JS target** emits `mut(…, queue)`; `rt.js` keeps `m.queue` and
  `m.next` under the commit's `undo` and orders `advance`/`drive` as D3.

### D7 — `task NAME when COND [key=EXPR]`

```ebnf
task = "task" IDENT ( "mount" | gate ) block(schedule) ;
gate = "when" expr [ "key" "=" expr ] | "key" "=" expr ;
```

```
task hide when toast != "" key=toastSeq     -- the undo toast (ledger2)
  after(5000, expire)
task autosave when draft != saved key=draft -- a debounce
  after(800, save)
task ticker when status == "running"        -- pomodoro
  every(1000, tick)
task fly when flying                        -- Messages' flight phases
  every(frame, step)
```

`mount` is the ungated task, unchanged. `key=` alone means
`when true key=…`, and it is spelled as `each` spells it. Tasks stay
root-only, and the schedules and their literal intervals do not change.

### D8 — When a gated task arms, re-arms and drops its timer

Each task is **idle** or **armed** with a due time. The runner reads every
gated task's gate and key, in plan order, after each commit that stands
(an action, a reply, a timer, a `then`, a `next`, a router change) and at
boot after the first settlement:

| Gate | Key | Result |
|---|---|---|
| false | — | idle; an armed timer is dropped and nothing runs |
| true, was false (or boot) | — | armed: `after` and `every` at now + ms; a frame task at the next virtual frame |
| true | changed (compared as an `each` key) | re-armed from now |
| true | unchanged | as it was: an `every` keeps its phase; a spent `after` stays spent |

"Now" is the commit's time. A commit that a timer makes at its due time
arms from that due time, so the seek rule holds (LLP 1005 §6): one
`advance(60_000)` equals sixty `advance(1_000)`s.

A task's own fire is a commit like any other: an `every` whose action
clears its gate stops (pomodoro's `status = "done"`), and a debounce whose
`save` makes `draft == saved` goes idle.

**Placement.** The step goes in `conclude` (`commit.rs:56`), where all four
kinds of commit end (`commit.rs:385`, `:943`, `:983`; `settlement.rs:77`).
It reads only values that stood, so nothing arms in a refused commit.
`arm_then` and D3's arming of `next` move there too.

### D9 — Gates and keys lower to derives

`lower` turns a gate (`bool`) and a key (string, number or bool) into hidden
root derives, `hide#when` and `hide#key`, settled like any derive: a trap
refuses the commit that caused it, and the JS target gets signals. Plan
`timers` gains `gate` and `key`, both `opt:derives`.

Three refusals:

- **`type-task-gate`**: "`when` takes a bool; `toast` is a string; write
  `toast != ""`".
- **`type-task-key`**: a key that is not a string, number or bool.
- **`analyze-task-gate-clock`**: a gate or key that reads `now()`, directly
  or through a derive. "A gate is read at commits, not as the clock moves;
  gate on state (`toast != ""`) and let `after(5000, …)` measure the time."
  A reader of `now()` is re-evaluated only at commits (`rt.js:229–233`), so
  `when now() < toastUntil` would never turn false by itself.

### D10 — Clock, frames, hosts and the agent

- **The clock.** `timer_due_ms` (`runner.rs:1199`) reports only armed
  tasks, so an idle gated task keeps no host awake: no tick, no commit, no
  epoch. `clock settle` is unchanged.
- **Frames.** `wants_frames()` (`runner.rs:1191`) becomes "a frame task is
  armed". Apple reads it each batch (`host/apple/src/host.rs:1128`), Linux
  each loop (`display.rs:525`), and the JS `paint()` stops once no frame
  timer is listed. No host changes.
- **The agent's `state`** gains `tasks` (due time or `null`) and `queued`
  (waiting counts). No operation is added.
- **A dev reload** restarts tasks over the carried state, as today.

### D11 — A gated timer is not the deferred reaction

LLP 1017 P4c moved "general `task … when [dep]`" to DEFERRED after diary
0102's hydration race, in which a reaction copied a query's answer into a
draft and raced the typing. LLP 1035.005.000 §5 kept it out.

That form ran an action when its dependency changed. A gated task runs
nothing then: the change starts a timer, and the action runs at least 1 ms
later as its own commit, reading the state as it is then (`then`'s
discipline). `after(1, seed)` bends it toward a reaction, as
`task boot mount after(1, start)` already does, so the docs name `then`
as the reaction to an answer (§8 Q4). It needs **Motion**'s task entry in
`rules/DEFERRED.md` expanded, by Charlie's waiver (§8 Q1).

### D12 — Lean and difftest

- **Mutations.** Lean asks every send now (`Runtime.lean:477`) and has no
  `then`. A queue mutation's later sends in a commit go to a new
  `Config.queued`, which each `dispatch` ends by asking, a commit each, and
  `advance` drains before timers.
- **Timers.** `Timer` gains `gate`, `key` (derive names) and `armed`.
  `startTimers` and every `runAction` that stands apply D8's table.
- `StepSound`, `Invariant` and `Soundness` carry the new fields. The
  corpus gains `mutations/queue-*` and `timers/gated-*`, and the random
  generator emits both.

Frame tasks and `then` stay outside Lean (`semantics/README.md:582`).

### D13 — Docs and adoption

**Docs.** `contract-for-agents.md` gains table rows (`:300`, `:730`):
"writes that must all land, in order → `mutation … queue`" and "a timer
while something shows → `task … when cond`, restarted by `key=`"; `:323`
and `:535` follow. The grammar, its contextual words and
`contract-for-humans.md:1043–1066` change; `agent-pitfalls.md` gains "a
gate reads state at commits".

**Adoption** (outside the repo, `EXACT_APP_DIR`; driven on web and macOS):
- Stage 1: kanban2 folds `aside` into `wrote queue` and drops the guard.
  Flashcards and spreadsheet fold their split mutations into
  `edited queue`. Studio's close becomes two queued sends.
- Stage 2: ledger2's toast, chat's and chat2's bots, trivia's round and
  pomodoro's ticker become gated tasks.

## 4. Effect on each implementation

| | Stage 1 (D1–D6) | Stage 2 (D7–D11) |
|---|---|---|
| compiler | `queue`; send-twice exemption | `when`/`key=`; D9 |
| plan | `mutations.queue` | `timers.gate`, `.key` |
| runner, agent | queues, `next_due`, `QueueFull`; `state.queued` | gate arming, `wants_frames`; `state.tasks` |
| JS target | `m.queue`, `m.next` | `gated(…)` at a commit's end (`rt.js:184`) |
| hosts | none | none |

Stage 3 is D12, Lean and difftest only.

## 5. Tests

- **Compiler** (`contract/cli/tests/it/mutation.rs`, `time.rs`,
  `contract/corpus/rejects.txt`): `queue` in D1's position; send-twice
  accepted for a queue and refused otherwise, D6's message asserted whole;
  `when`, `when … key=`, `key=` alone and D9's refusals; every in-repo
  app's plan byte-identical apart from the digest.
- **Queues** (`mutation.rs`): two sends in one action, answered now and
  later; a send while one is in flight; one `then` per reply, in order; a
  refused `then` that does not stall; an assignment that forgets nothing;
  a failure that moves on; the 65th send refused; a reload dropping the
  queue; a two-step continuation completing behind a second send (today's
  dropped save).
- **Timers** (`time.rs`): on then off before the due time fires nothing; a
  key change re-arms; a spent `after` stays spent; an `every` that clears
  its gate stops; `wants_frames` follows a frame gate; a gate armed inside
  an advance fires within it; `advance(60_000)` equals sixty
  `advance(1_000)`s; an idle gated task reports no due time.
- **Conformance.** `host/web-js/conformance/queue.contract` and
  `gates.contract`, with `.steps`, compare journals and state across the
  wasm runner, the JS runtime and the Linux host (`conform.mjs`).
- **Lean.** `lake build`; difftest `corpus` and `random --count 500`
  (async lane).
- **Driven.** D13's adoptions, and the five checks after each stage.

## 6. Implementation plan

Each commit passes the five checks.

1. **Stage 1, 2026-10-05: queued sends (D1–D6, D13).** Exit: §5's queue
   tests and conformance; kanban2's blur-and-tap test (type notes, tap the
   label, no settle) on web and macOS.
2. **Stage 2, 2026-10-05: gated tasks (D7–D11, D13),** after the waiver
   (§8 Q1). Exit: §5's timer tests and conformance; ledger2 makes no
   commit at rest over `perf during "clock +60000"`.
3. **Stage 3, 2026-10-06: D12.** A slipped proof lands as a restriction
   named in `semantics/README.md` with a `QUEUE.md` line; no `sorry`.

## 7. Considered and not taken

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
  component use, no list literals, LLP 1088 §9.1); the expiry still writes
  root state. A `when` arm's condition, as the gate, gives the same
  lifetime. Trigger: a per-row timer.
- **`animationend=expire`,** the web's declarative timer. It ties app state
  to presentation, and reduced motion changes when it fires.
- **Free empty commits** (chat F7): QUEUE's "Presenter.apply runs its
  post-pass for a batch that changed nothing", independent of this RFC.

## 8. Open questions

1. **The waiver for D7–D11.** It reverses LLP 1017 P4c's deferral of
   `task … when`, in D11's timer-only form. No take is offered. The
   consumers are D13's five apps, plus Bluesky and Messages.
2. **Cancelling a queue.** `cancel m` (drop the waiting sends, forget the
   one in flight) waits for a consumer, such as an upload's Cancel.
3. **`then` sending its own queue mutation** (studio R8). It still loops if
   the `then` always sends, and two queued sends in the action do the same
   job. Keep the refusal?
4. **A minimum `after` under a gate.** Should a gate refuse anything under
   16 ms, so it cannot be written as a near-reaction? r1 admits 1 ms, as
   `mount` does.
5. **The bound.** Is 64 the right number of waiting sends?

## 9. Revisions

- **r1** (2026-10-05): first draft.
