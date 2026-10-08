# LLP 1097: Storage that finishes after the answer

**Type:** RFC
**Status:** Built (stages 1–3, 2026-10-04; As built in §6). Accepted (r4, by the orchestrator under Charlie's delegation after three review rounds; Grok 4.7 only — Codex budget exhausted; round-3 findings folded unreviewed — the implementation review checks them).
- r1 (`96f781472`) was reviewed by Grok 4.7 (xhigh) with two scopes: semantics (`llp/reviews/1097-r1.grok-a.md`, NOT READY) and implementation (`llp/reviews/1097-r1.grok-b.md`, NOT READY).
- r2 (`9db90ce57`) had a delta review (`llp/reviews/1097-r2.grok.md`, NOT READY: five MATERIAL, two MINOR, two NIT).
- r3 (`c54843797`) resolved round 2, rejecting one finding with reasons, and had the final round (`llp/reviews/1097-r3.grok.md`, NOT READY: three MATERIAL, six MINOR, two NIT).
- r4 folds round 3's fixes as given, with no further review (§10).
- The orchestrator accepted r1's recommendations under Charlie's 2026-10-04 delegation, and confirmed r2's two changes: the answer-to-answer deferral is deleted in stage 1, and the queue's refusal code is `full` (§9).
- No `rules/DEFERRED.md` entry (Q6).
**Systems:** The TypeScript seam (`js/src/prelude.js`; the Hermes executor `js/src/lib.rs`, `turns.rs`, a new `js/src/background.rs`), Runner (a background ticket, `DataSource` gains three methods; `runner/src/runner/source.rs`, `commit.rs`, `admission.rs`, a new `runner/src/runner/background.rs`), the composers (`Storage`, `Mixed`, `Placed` on `Main`), web (`host/web/module-glue.js`, `host/web/storage.js`, `host/web-js/ts-data.js`, `host/web-js/agent.js`), Apple and Linux (lifecycle only, in new files), the driver (`scripts/agent.mjs`, `scripts/agent-test.mjs`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3, r4)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-07, stage 2 on 2026-10-08, stage 3 on 2026-10-09 (§6)
**Amends:**
- LLP 1027 D10: a completion is delivered only by its owner's checkpoint, and the module becomes an owner.
- LLP 1027.003.000 §13: the rule "storage a settled answer left in flight is the module's, claimed by the next answer" is replaced.
- LLP 1092 D3, one sentence: unawaited storage does not keep a send in flight, and the background ticket is not that send's request.
- kanban F22's rule in `prelude.js`: an answer waits only for the work it awaits.
- The answer-to-answer turn deferral, `js/src/lib.rs:831–853`, is deleted (D4).
- `docs/reference.md:374–384`.

**Depends on:** fix/data6 (`5c8fd1521`, `6c290ee89`, `e3add44c7`, `194ec5dfc`, in `~/projects/exact2-wt-data2`). It is not on main as of r2. Stage 1 starts from main once it lands.
**Related:** LLP 1016 D1–D5; LLP 1027.001 (language parity); LLP 1027.002 (worker placement); LLP 1041 D2 (overload is refusal); LLP 1012 §2 (`clock settle`). Diaries: `~/projects/x2apps/drums/DIARY.md` (R10, R11, Top 5 #3, `repros/R10-deferred-write`); kanban F22, ledger F12, hn-reader F7, minesweeper F10 (as cited in `prelude.js` and `turns.rs`). `QUEUE.md`: "A native data module's `console` never reaches the agent's `logs`" (trivia F7).

## Summary

A web app saves the way a browser lets it. It answers from memory and lets
the write finish behind the answer:

```ts
edit(store, args) {
  song = apply(song, args);
  storage.fs.atomicWriteFile(PATH, JSON.stringify(song)).catch(note);  // started now, not awaited
  return song;
}
```

On the web build this works. On Hermes, and in the wasm target's module realm,
a host runs an answer's storage steps only while that answer is in flight. So
the prelude holds the reply until every step the answer started has landed
(kanban F22). The consequences:

- **The answer is late.** A saving edit's answer is a reply on real time. It
  lands at the next `clock` step, so a native test reads one input behind the
  web (drums R11; fix/data6's pitfall).
- **The next answer waits behind the write.** An answer asked while another
  is between storage steps is deferred until that turn ends
  (`lib.rs:831–853`). Drums gave up saving in its answers and saves from
  `task every(500, autosave)`.
- **Before fix/data6, a write could be lost.** A write queued in a microtask
  after the reply was never run on macOS, and nothing reported it (R10).

This RFC gives the module an owner of its own for the storage an answer leaves
behind. The runner holds one ticket for that work, and the hosts run it as
they run any continuation:

- **An answer replies when its value is ready**, as in the browser. Storage it
  started and did not await moves to the module's background work. An answer
  that awaits its write still waits, because it asked to.
- **Storage keeps one order.** Every storage operation of a module enters one
  queue, in the order it was issued, and runs one at a time. Call storage in
  the answer, not in a `.then` chain: the queue is the serializer (D1).
- **Failures are loud and the same everywhere.** Every failed storage
  operation, every unhandled rejection and every `console` line from the
  module reaches the journal (`logs`) on every host.
- **`clock settle` and the driver's `reload` wait for all of it**, on every
  host. An input step no longer has to.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | An answer waits only for the work it awaits; what it started and did not await becomes background work | drums R11 | 1 |
| D2 | The module's background owner, in the prelude: the move, its counts, its contexts, no `fetch` | drums R10 | 1 |
| D3 | One storage queue per module; only the issuer of the operation in flight holds a ticket | — | 1 |
| D4 | Ordering against later answers; the answer-to-answer deferral goes | drums R11 | 1 |
| D5 | The runner: a background ticket, its own path, three `DataSource` methods | — | 1 |
| D6 | Hermes on the main thread; a worker placement keeps today's rule | — | 1 |
| D7 | The web: the wasm target's realm and the JS target | — | 2 |
| D8 | Failure reporting | drums R10, trivia F7 | 1, 2 |
| D9 | The driver: `clock settle`, `inflight`, `reload`, `state.background` | drums R11 | 1, 2 |
| D10 | Teardown and the app's lifecycle | — | 1, 3 |
| D11 | Rust sources and worker placement | — | deferred |
| D12 | Docs and adoption | drums | 3 |

## 1. Evidence

- **Drums R10** (`repros/R10-deferred-write`). The app answered edits from
  memory and saved with `writes = writes.then(() =>
  storage.fs.atomicWriteFile(…)).catch(note)`, unawaited.
  - On the web, the file was saved and survived `reload`.
  - On macOS, `data/` stayed empty, the `.catch` never ran, and the logs said
    nothing.
  - fix/data6's `5c8fd1521` makes every answer reply after its microtask
    checkpoint, so a write started there is inside the answer and lands.
- **Drums R11.** Once the save started inside the answer, every edit's answer
  waited for its write. Under `test macos`, every second consecutive edit
  read stale (9 of 19 failed); on the web none did. A `persist` mutation sent
  from `then` was no better, because the next edit's answer was deferred
  behind the write in flight. fix/data6's `6c290ee89` documents the lag as a
  pitfall.
- **Survey #3** (x2apps survey, reproduced on `ff9bcbc1d`, after fix/data6).
  The answer that is late need not save at all. `extendSurvey` returns a plain
  object, with no storage and no promise. The tap that sends it blurs the
  title field, whose `change` first sends `saveSurvey`, which awaits a SQLite
  `UPDATE`. On macOS `extendSurvey` is then parked as `DEFERRED` behind that
  open turn (`lib.rs:831–853`), so `tap "add-short"` then `type "prompt" …`
  finds no `prompt` (`test macos`: 2 of 4 failed); on the web it passes.
  Isolated with three one-test drives: the add passes with no save in flight,
  fails while the title's save is in flight, and passes once
  `type "survey-title" key "Enter"` and `clock settle` landed the save first.
  The data6 checkpoint did not change this; D4.5's deletion of the deferral
  does.
- **Issued is not chained** (a simulation of D3's queue, run for r2 with Bun:
  one operation in flight, two edits, then a read):
  - With `saving = saving.then(() => write(v))`, the queue saw
    `write:song1, read, write:song2`. The read saw `song1`; the disk ended
    as `song2`.
  - With `write(v)` called in the answer, the queue saw `write:song1,
    write:song2, read`. The read saw `song2`.

  A `.then` on a promise that has not settled issues nothing during the
  answer's checkpoint (`js/src/shim.cc:352–356` drains only what is queued).
  The chain's later write is issued only when the earlier one lands.
- **Today's code.**
  - `storageCall` refuses when no answer is current or the current one has
    replied (`prelude.js:609–617`).
  - `unstore` decrements the call its completion captured
    (`prelude.js:419–422`).
  - `settle` returns ticket 0 while `call.storage > 0` (`:770`), before it
    consults `finished` (`:764`, `:771`). An answer is never observed
    finished with storage outstanding.
  - `fetch` refuses only `currentCall == null`, and pushes its ticket onto
    the current call (`prelude.js:479–487`, `:520`). The next answer that
    settles with nothing of its own claims an unclaimed ticket (`:779–782`).
  - The Hermes executor drains microtasks only in `begin`, `resume` and
    `finish_let_go` (`js/src/lib.rs:891`, `:1009`; `turns.rs:84`).
  - An answer asked while a turn is open is parked as `DEFERRED`
    (`lib.rs:831–853`). `turn_open()` is any parked call with ticket 0
    (`turns.rs:11–13`).
  - `Session::continuation` returns when Ibex2's shared context is idle or
    any completion is queued (`js/src/storage.rs:63–92`). `is_idle` and
    `wait` are module-wide, and `admit` wakes every waiter
    (`vendor/ibex/crates/ibex2/src/task.rs:201`, `:273–282`, `:728–731`).
    `deliver_one` pops the head with no owner
    (`vendor/ibex/crates/ibex2/src/engine/ibex2_jsi.cc:1004–1015`).
  - The wasm target's realm keeps an answer's turn on `tail` until its
    storage lands (`module-glue.js:184–205`). A storage call captures the
    answer's owner at issue (`module-glue.js:130`, `storage.js:53–56`).
    `finish` retires that owner (`module-glue.js:167`), and a retired owner's
    completion is dropped (`storage.js:30–35`).
  - The JS target runs storage as plain promises (`ts-data.js:150–181`).
    `storage-fs.js` chains mutations (`:335–347`), but its reads call the
    store directly (`:349–350`). A write nobody awaits is invisible to
    `clock settle`.
  - LLP 1027 D10: "Only the owning answer's checkpoint delivers a browser
    storage completion, with its store context installed; no unsolicited
    microtask resumes app code."

## 2. What the web does

A page's storage (IndexedDB, OPFS, a SQLite worker) completes on the event
loop whether or not anyone awaits it. An unawaited write finishes after the
code that started it has returned, and an unhandled rejection reaches the
console. Order is the store's own. IndexedDB runs transactions with
overlapping scopes in creation order, and a SQLite worker runs its queue in
order. But `storage-fs.js`'s reads do not wait for an earlier write
(`readFile` does not wait on `mutate`). A page being unloaded may lose what is in flight; D10's web row
(2026-10-08) journals the filesystem's share of it.

This RFC makes Hermes and the wasm realm finish storage as a browser does. It
adds what the web leaves to chance, on every host including the JS target:
one order for reads and writes, a bound, a journal line, and a wait for the
driver.

## 3. Decisions

### D1 — An answer waits only for the work it awaits

**Started** means `storageCall` has run, either in the answer's body or in its
microtask checkpoint. That is the checkpoint fix/data6's `5c8fd1521` adds
before every reply (Q7). When the answer's value is ready (its promise has
settled, or it returned a value at once), the storage it has started and not
awaited moves to the module's **background work** (D2). The answer replies
then.

- **A step the answer awaits** is part of its value, so the answer replies
  after it. Its promise is still `pending` while the step is, so `settle`
  sees an unfinished answer and keeps today's ticket-0 wait.
- **A `.then` chain** issues its storage when the earlier promise lands. That
  is after the answer, so the chain's storage runs as background work. It
  persists, but its order is its issue time, not the order of the edits that
  scheduled it (§1). The docs say to call storage in the answer and let the
  queue (D3) serialize it.
- **An unawaited `fetch`** still holds the reply, as today. A background fetch
  is §7's.
- **A failure of a moved step** cannot fail the answer, which has replied. It
  rejects the app's own promise, as on the web, and it is journaled (D8).

This replaces kanban F22's rule ("the answer is given once the work it
started has landed"). F22's reason was that steps no answer owned would be
stranded. With an owner and a ticket for them (D2, D5), nothing is stranded.

### D2 — The module's background owner

The prelude gains one module-level pseudo-call, `background`, with a
`storage` count and a place in `storing`. It is never replied to, never let
go and never claimed.

**The move.** In `settle`, ahead of today's line 770, the order is:

1. `finished` is computed first.
2. If the answer is `done` or `failed`, `call.storage > 0`, the call is not
   `lost`, and the executor is on the main thread (D6), then:

   ```js
   background.storage += call.storage; call.storage = 0;
   storing.delete(call); storing.add(background);
   call.moved = true;
   ```

   It asks the executor to run background work (D5), and continues to the
   reply.
3. Otherwise, if `call.storage > 0`, today's single ticket-0 return
   (`prelude.js:770`) is replaced by two cases (D3). This covers an answer
   that is still `pending` because it awaits, and also a finished answer
   whose storage was not moved because the flag is unset (a worker, D6):
   - if the queue's head operation is this call's, it returns ticket 0;
   - otherwise it returns `{tag: 1, call: call.id, ticket: 0, waiting: true}`.
     That is today's waiting reply (`prelude.js:786`), which the executor
     decodes to `WAITING` only with `call` present (`js/src/lib.rs:774–784`;
     without it the reply is "pending on no ticket").

`call.storage` counts operations issued and not yet landed, whether queued or
in flight, because D3 increments it at issue.

**The owner of a completion.** A completion's handler
(`prelude.js:631–640`) resolves its owner as `owner(call)`: the call it
captured, or `background` when that call is `moved` or `replied`. It sets
`currentCall` to that owner and `unstore`s it. So the count lives in one
place, and a chain that runs when a moved write lands issues its next
operation as background work.

**`storageCall`** with `call.replied` attaches to `background`; it no longer
refuses. Two refusals stay:
- during module evaluation, when no answer has ever begun;
- at bake (`code: 'bake'`), as today.

**`background`'s status is `"pending"`**, so `hostWork` (`prelude.js:159–164`,
which joins only a `pending` call) joins a digest started in background work
to it.

**No `fetch` and no native call from background work.** `fetch` (and
`native.later` and `openAuthSession`, which go through it, `prelude.js:554`,
`:838–839`) rejects while `currentCall` is `background`. So does
`native.call`, whose own check (`prelude.js:812`) a `pending` background would
pass. Each rejection is journaled, for example `data: fetch() called from
background work: it was never run`. No ticket is pushed onto `background`, and
`settle`'s unclaimed-ticket scan never sees one.

**Liveness.** Background work counts as outstanding (`turns.rs` `outstanding`,
`:28`). An answer that awaits a promise chained on background work is
`WAITING`, and it is asked again after each background delivery. The host's
`release_work` after the delivery is what reaches `wake` (D5). The rule that
"a storage step a settled answer left in flight is the module's: the next
answer to settle without work of its own claims it" is deleted:
`background` owns that work. The comment is `prelude.js:723–727`; the claim
itself is the scan at `:783–785`, which returns ticket 0 for any member of
`storing` that is not in `calls`. `background` is in `storing` and never in
`calls`, so **the scan skips `background`**. Without that, every pending
answer with `call.storage == 0`, including one awaiting a promise chained on
background work, would take ticket 0. Two such answers would park two
continuations on Ibex2's shared context. Such an answer instead takes the
waiting reply, with `call` (step 3). Only `background()` holds the storage
ticket while its operation is the head.

### D3 — One storage queue per module

Every storage operation of a module, whether an answer's or the background's,
enters one FIFO queue in the prelude. At most one is in flight. An operation
starts when the one before it settles. `hostWork` (a digest,
`store.keepKey`, `prelude.js:159–164`) counts in `call.storage` and moves with
it, but it is not storage and does not enter the queue.

- **Order.** The order is the order operations were issued. A read issued
  after a write sees that write. On the JS target this is new for reads
  (`storage-fs.js:349–350`), and the parity fixture checks it.
- **Bound.** At most 256 operations wait behind the one in flight. The next
  call rejects at once with `code: 'full'` ("storage queue full: 256
  operations wait") and is journaled (D8). `EBUSY` already means a locked
  database (`prelude.js:596`, `:601`), and the app branches on `code`
  (`docs/reference.md:357–365`), so the refusal has its own code. `full` joins
  the code list in `reference.md`. Refusal is the overload policy (LLP 1041
  D2).
- **One ticket waits on Ibex2: the issuer's.** With one operation in flight,
  the prelude knows its issuer, `queue.head.owner`.
  - **An answer whose own operation is in flight** settles with ticket 0 and
    holds the one continuation.
  - **An answer whose operations are still queued behind another owner's**
    settles as `WAITING` (D2 step 3's second case). It is asked again after
    each delivery and takes ticket 0 when its operation reaches the head.
  - **Background** holds the ticket only while its operation is the head
    (D5).

  So no two continuations wait on Ibex2's shared context, one completion
  wakes one waiter, and the issuer's resume delivers it. This is what makes
  `Session::continuation`'s module-wide `is_idle() || wait()` sound: it is
  only ever waited on by the head's issuer. It also removes r1's spin
  (round-1 review B, finding 1).
- **The first stage-1 commit proves it:** a test counts continuation rounds
  over a background write, an answer's read behind it, and an idle module.
  The rounds equal the deliveries, and an idle module hands out no ticket.

### D4 — Ordering against later answers

1. **An answer never waits for background work to begin or to reply.** The
   background ticket is not a parked answer, so it is not a `turn_open()`.
2. **Storage runs in the order it entered the queue** (D3). D4's ordering is
   stated only for operations that have been issued. A chain's later
   operation is issued when it is issued (D1). A read forced by a mutation
   answered `Now` (fix/data6's `e3add44c7`, `landed_now`) runs after the
   answer's own write, because the answer issued that write first.
3. **Awaiting background work waits for it** (D2, liveness).
4. **A background failure runs no Contract `then`, re-asks no resource and
   refuses no commit.** The app's own promise reactions run in the
   background drain (D1). The person learns of a failure through what the
   app's next answer says (D12).
5. **The answer-to-answer deferral is deleted in stage 1** (`lib.rs:831–853`,
   `DEFERRED`). It is deleted in the commit after D3's issuer-only ticket
   lands. The orchestrator confirmed this (§9).
   - **What changes: answers interleave at their awaits, as on the web.**
     Suppose answer A awaits `write1` and then issues `write2`, and answer B
     begins while `write1` is in flight and issues `writeB`. The queue then
     runs `write1`, `writeB`, `write2`.
     - That is the order a browser gives two async calls. The JS target asks
       each answer as it comes and returns its promise, with no turn rule
       (`ts-data.js:207–218`), so it already runs this order.
     - The queue orders operations issued. It does not keep an answer's
       not-yet-issued second step ahead of another answer, and nothing here
       claims it does.
     - An app whose two writes must be contiguous puts them in one
       `transaction` (SQLite) or one operation, or chains its answers on its
       own promise, as it must on the web. The docs say so (D12).
   - **The context a continuation runs in.** Work chained behind another
     answer's promise runs in that answer's context while it has not
     replied. That answer owes the work and replies after it, and the
     waiting answer is `WAITING` and woken by liveness. It costs the first
     answer latency, and it loses no operation.
   - **Why it goes.** Under D3, an answer whose own awaited operation is the
     queue's head holds ticket 0, which is a `turn_open()`. With the rule,
     every later answer is deferred for the length of that operation. That
     is R11's `persist`-mutation case, and the web has no such wait.

### D5 — The runner: a background ticket

The new `runner/src/runner/background.rs` keeps it. `commit.rs` (1,090 lines)
and `admission.rs` gain call sites only.

**One `PendingReq` in `self.pending`.** Its target is
`Target::Background`, with no slot. Because it is in `self.pending`, `holds`
(`commit.rs:913`) and `has_pending` (`:877`) see it, so no host lets
its work go (`host/apple/src/abi.rs:229`, `host/linux/src/presenter.rs:772`),
and `clock settle` waits on it. It is absent from `pending_res` and
`pending_mut`. It has no `then` and makes no commit.

**Its own path.** A fulfill whose ticket is the background's goes to
`background.rs`, never to `fulfill_inner` (`commit.rs:1005`, whose `Later`
commits and checkpoints):

- it calls `DataSource::background_landed`;
- on `Some(next)`, it keeps the same runner ticket and the same `PendingReq`.
  It pushes a new `RequestOut` for that ticket, carrying `next`'s continuation
  token, onto `self.requests`, so `take_requests()` (`commit.rs:809`) hands
  the host the next round. Apple reads it at `abi.rs:236`, Linux at
  `presenter.rs:778`. It does not go through `enqueue` (`commit.rs:734`, whose
  push is `:783–788`): it sets no `forgot`, assigns no new ticket and runs no
  `forgotten`;
- **a host may run a ticket it has just completed**: the next round's work
  arrives under the same ticket after the fulfill;
- on `None`, it removes the entry;
- on `Err`, it journals `background failed: <message>`, an executor error, as
  distinct from D8's failed operation;
- it calls `update()` never.

The host's existing `release_work` after the fulfill still runs
(`abi.rs:259`, `presenter.rs:801`, `host/web/src/host.rs:1041`). That is what
asks `WAITING` answers again (`js/src/lib.rs:1163`, `:1186`).

**Checkpoints.** A commit's checkpoint restores `pending` on refusal
(`commit.rs:65–79`). The restore keeps the live background entry, as it
stands after the restore, rather than the one checkpointed. A background
round that landed between checkpoint and restore is not undone.

**The methods:**

```rust
/// Work the module started that no answer waits for (LLP 1097 D5), or
/// `None`. Polled after every `answer`, `fulfill`, `release`,
/// `forgotten` and background round. It arms only when the module's
/// operation in flight is the background's, and returns `None` while a
/// background `RequestOut` is queued or the host still holds the ticket
/// (`enqueue` would drop the pending entry and set `forgot`,
/// `commit.rs:763–767`).
fn background(&mut self, store: &Store) -> Option<Request> { None }

/// The outcome of the background ticket: its next round, or `None` when
/// the head is no longer the background's or nothing was delivered.
fn background_landed(&mut self, store: &Store, outcome: Outcome)
    -> Result<Option<Request>, DataError> { Ok(None) }

/// The module's journal lines since the last take (`console`, D8).
fn take_logs(&mut self) -> Vec<String> { Vec::new() }
```

**Tokens.** The executor's background continuation token is reserved, as
`WAITING` and `DEFERRED` are (`js/src/lib.rs:140–147`): `BACKGROUND = u64::MAX
- 2`. Prelude call ids start at 1, and ticket 0 means storage
(`prelude.js:669`, `:770`).

**The composers.**
Each round's token is consumed by its dispatch (`Storage::dispatch` and
`Storage::continuation` remove a `Pending::Child`, `data/host/src/lib.rs:287–294`,
`:331–332`). So **both** `background()`'s request and `background_landed`'s
`Some(next)` go through each composer's remap, which inserts a fresh child
token for that round:

- `Storage` remaps through the same path `step` uses
  (`data/host/src/lib.rs:77–133`).
- `Mixed` polls its JavaScript child only, and maps the token in
  `self.continuations` (`data/src/mixed.rs:841–848`).
- `Placed` forwards all three methods while it is inline (`Placement::Main`,
  `owner.is_none()`, `data/src/placed.rs:205–208`), the way
  `continuation` already forwards (`:681–686`). It forwards none while a
  worker owns the module (D6). `Main` is the default TypeScript placement
  (`js/bake/src/lib.rs:312–319`), and Fieldnotes is `Storage<Data<Placed<Module>>>`
  (`apps/fieldnotes/apple/src/lib.rs:8–14`). So without this, every
  TypeScript app would lose both the ticket and `take_logs`.
- At the bottom, `Module::dispatch` and `Module::continuation` gain a
  `BACKGROUND` arm. Today `Module::continuation` finds only a parked call
  whose ticket is 0 (`js/src/lib.rs:1241–1244`).

**The `Target` matches** that gain an arm, each a no-op or a name:
- `commit.rs`: `target_name` (`:695`), `sync_pending_flags`'s match
  (`:797–800`), and `fulfill_inner`'s matches (from `:1005`);
- `admission.rs`: `release_refused` (`:48–70`) and `release_failed`
  (`:82–108`);
- `source.rs`: `Target::text` (`:162–165`).

**The journal:**
- `background: storage (2 waiting)`;
- `background: done (3 operations)`;
- `background failed: …`.

**LLP 1092 D3, amended.** A send's unawaited storage does not keep it in
flight. Its answer is its `Now` reply, and the background ticket is not that
send's request. So a queue mutation is free as soon as its value is ready.

**`poison()`** (`commit.rs:675–683`) drops the background entry with the
other requests and journals `background: dropped (poisoned), N operations
waiting`. An operation already handed to Ibex2 completes on disk whether or
not anyone delivers it. Only the operations still queued in the prelude are
lost, and they are named. `poison()` does not wait: waiting is session
teardown's (D10).

### D6 — Hermes on the main thread; a worker placement keeps today's rule

`js/src/background.rs` is the executor's half (`lib.rs` is 1,480 lines; its
`DataSource` methods are one-line forwards):

- **`background()`.** It returns `Request::continuation(BACKGROUND)` when the
  queue's head is the background's and no ticket is out.
- **`dispatch(BACKGROUND)`.** It maps to `storage::Session::continuation()`,
  run on the host's I/O worker.
- **`background_landed`.** It calls `deliver_storage_one()` and `drain()`
  with `currentCall = background`, then `progress += 1`. It returns `None`
  when `deliver_storage_one` delivered nothing, or when the head is no
  longer the background's. Otherwise it returns the next round.

The prelude moves storage (D2) only when it is told it runs on the main
thread.
- **The flag is a field on `Module`, `main_thread`**, which `load` reads.
  `load` sets the prelude's flag only when `main_thread` is true. The inline
  module's load reaches it through `Placed::activate` on `Main`
  (`placed.rs:481–484`).
- **A worker never sets it.** `Module::build` (`lib.rs:490–523`) sets
  `main_thread = false` on the instance a worker owner builds, before its
  `activate()` call at `:522`, which reaches `load`. A flag set
  unconditionally in `load` would be set on the worker instance too.
- The flag defaults unset, so a prelude that no host sets it in behaves as
  today.
- A worker placement's turns run to their reply on the owner thread
  (`data/src/placed.rs:356–385`), and `Placed` forwards nothing while the owner
  holds the module. So there the answer keeps waiting for its storage, as
  today (D11).

Apple and Linux already run any continuation ticket the runner hands out,
park `Held` ones, and release them after each commit
(`host/apple/src/abi.rs:229–283`; `host/linux/src/presenter.rs:777–805`). They
need no change for D1–D6.

### D7 — The web

**The wasm target's module realm** (`module-glue.js`, the shared prelude):

- `storage.js` gains one owner that is never retired, `background`.
- **A mutable owner cell per operation.** Today `enqueue` captures `owner` by
  value at issue (`storage.js:53`). `active()` and `completion()` check
  `retired.has(owner)` (`:32`, `:54`). A completion reaches `queues` only
  when the backend resolves (`:31–35`, `:55–59`). So while an operation runs,
  its queue is empty and nothing could be re-pointed.
  - Each issued operation gets a cell `{owner}`, registered in
    `issued.get(owner)` at `enqueue`, and removed when its completion runs or
    is cleaned up.
  - `active()` and `completion()` read `cell.owner` when they run, not at
    issue.
- **Re-homing before retirement.** Before `finish` retires an answer's owner
  (`module-glue.js:167`), every cell of that owner is pointed at
  `background`, and that owner's queued completions move onto `background`'s
  queue. Then `retire(owner)` (`storage.js:110–116`) finds nothing to drop.
  An owner with a live cell is never retired. `scope()` resolves to
  `background` for any later issue from that answer's context.
- **The flag is set in the wasm module realm in stage 2**, in the same
  commit as the cells and the re-homing. Until then the realm leaves it
  unset, the turn loop keeps its `ticket === 0` wait
  (`module-glue.js:199–204`), and stage 1's prelude change is inert there.
- A turn's loop (`module-glue.js:184–205`) ends when the answer's value is
  ready. The next answer does not wait on `tail` for moved storage.
- A background loop beside `tail` runs while the wasm runner holds a
  background ticket: `storage.deliver(background)`, `checkpoint()`, then the
  prelude's `__exact_background()`. The glue's data source implements D5's
  methods over it.
- The realm keeps per-answer liveness (LLP 1027.003.000 §13), with one
  exception: an answer awaiting background work parks.
  - **Today it cannot.** `moduleWide` is `bytesDoor !== undefined`
    (`prelude.js:730`), and `__exact_bytes` exists only in the Hermes shim
    (`js/src/shim.cc:159`). So in this realm the waiting return (`:786`)
    never runs, and the answer falls through to "pending on nothing"
    (`:792–796`).
  - **In this realm, `settle` returns the waiting reply, with `call`, for
    such an answer** while a background ticket is out, even though
    `moduleWide` is false.
  - **The turn loop** (`module-glue.js:199–204`) and `finish`
    (`:160–167`) treat `waiting: true` as "park, and settle again after a
    background delivery".
    - They do not call `storage.deliver` for it. That would wait on the
      answer owner's queue (`storage.js:102–105`), where nothing will
      arrive.
    - They do not look for ticket 0 in `context.requests`. That would throw
      `module awaits a fetch it never made` (`:160–161`).
    - The owner is not retired while the answer is parked.
- **The worker realm** (`module-worker.js:101`, `:116–129`) is a worker
  placement and keeps today's rule (D6).

**The JS target** (`ts-data.js`) already runs unawaited storage. It gains what
the other hosts gain:
- D3's queue and bound around every `storageOf` method, reads included
  (`ts-data.js:170–181`);
- a count of queued and in-flight operations, which `exact.inflight`
  includes, so the agent's settle waits for it (`agent.js:266–303`);
- D8's journal lines;
- an `unhandledrejection` listener on the page, attributed to the module
  when its stack is the module's.

There is no ownership there and no deferral to delete. What it does matches
the other hosts in every respect D4 and D9 name.

**Open handles of work that failed (Charlie, 2026-10-07).** Background work
owns a let-go answer's operations, not its database handles: an app may keep
or share a handle across answers, so no host closes one for it, on failure or
on settling. When a failure happens (an answer's own rejection, or a rejection
nothing handled) while a database opened by work that now runs in the
background (an answer that replied, failed or was let go) is still open, each
executor journals once per handle `storage: <path> is still open after a
failure in background work that opened it: if that work owns it, close it in a
finally (finally { db.close() })` (the shared prelude, Hermes and the wasm
module realm; `ts-data.js` on the JS target). The host cannot know the failing
chain held that handle, so the line says "if". The chain closes its own handle
in a finally.

### D8 — Failure reporting

The same lines appear on every host, journaled by the runtime and read by
`logs`:

- **Every failed storage operation**, whether an answer's or the
  background's: `storage failed: atomicWriteFile app:/data/song.json: ENOSPC
  no space left on device`. A failure inside an answer also reaches the
  answer's promise, as today. The line exists so that a failure nobody
  catches is still seen.
- **A queue refusal:** `storage refused: full (atomicWriteFile app:/data/song.json)`.
- **Every unhandled rejection** in the module: `data: unhandled rejection:
  <message>`.
  - On Hermes, the prelude calls
    `HermesInternal.enablePromiseRejectionTracker`. It is defined
    unconditionally at the pinned commit (`6badada76`,
    `lib/VM/JSLib/HermesInternal.cpp:633`, `:835`).
  - Stage 1's first test proves that it fires in the lean VM `js/build.rs`
    links. If the lean runtime hides `HermesInternal`, `shim.cc` installs the
    tracker as a host function instead.
  - The wasm realm and the JS target use `unhandledrejection`.
- **A module's `console`** reaches the journal on Apple, Linux and the wasm
  target's realm, as the page's console already does on the JS target. This
  is QUEUE's `take_logs` line.
  - `DataSource::take_logs` (D5) calls the JavaScript child's
    `Module::take_logs` (`js/src/lib.rs:709–714`).
  - `Mixed`'s existing inherent `take_logs` (`data/src/mixed.rs:512`, filled by
    `logs.extend` at `:477`) returns envelope lines and does not satisfy it.
    So `Mixed`'s trait method calls the JavaScript child's, by its trait
    path, so that a same-named call does not reach the inherent method.
    `Storage` and `Placed` on `Main` forward too.
  - The runner writes the lines with `Runner::log` after each answer, fulfill
    and background round, prefixed `console:`.
- **`state.background`**: `{queued, inFlight, done, failed, last}`, where
  `last` is the last failure's line. It is printed by `runner/src/agent.rs`
  `state()` (`:770`; the file is 1,415 lines) on native and the wasm target,
  and by `agent.js`'s state reply on the JS target.

### D9 — The driver

- **`clock settle`** waits until no storage operation of the module is queued
  or in flight, an answer's or the background's, on every host. Natively the
  background ticket is a runner request, so the existing wait loops cover it,
  within their bounds (20 s, 16 rounds):
  - Linux `agent.rs:697–805`;
  - Apple `Agent.swift:431–518`;
  - the wasm glue's settle loop `glue.js:1219–1250`, which already tracks a
    continuation ticket (`:712–718`).

  On the JS target it is D7's count.
- **`clock +N`'s `inflight` stays a number.** LLP 1012 and the driver read it
  as a count (`scripts/agent.mjs:1237–1238`). The reply gains a separate
  field, `background: N` (operations queued or in flight), on every host.
- **An input step** ends with what it settled (LLP 1012). Under D1, a
  storage-free answer is there at the input's end on every host. So drums'
  `tap "tempo-up"` then `expect text "tempo" == "113"` passes under `test
  macos` with no `clock settle`. fix/data6's pitfall entry is deleted.
- **The driver's `reload`** (the test step, `scripts/agent-test.mjs:88–93`,
  and the interactive restart) settles first, in the driver, as `clock
  settle` does and within its bound. It does this before `carrier.reset({keep:
  true})` on the web (`scripts/agent.mjs:290–309`) and before `close()` on
  native.
  - The old session's journal goes with it, so the driver records
    `reload: waited for N storage operations` itself. It is in the reload
    step's reply, and is the first line of the next `logs` it prints.
  - A test's `reload` asks what persists. A write lost at `reload` is what a
    crash does, and that is not what the step means.

### D10 — Teardown and the app's lifecycle

One rule per event:

| Event | What happens to background work | Bound |
|---|---|---|
| The driver's `reload` | The driver settles first (D9). | `clock settle`'s, 20 s |
| A dev edit on the web (a navigation: the edit pushes `{rebuilt}`, `host/web/dev.mjs:919`; `:1036–1055` is the `/__dev` stream and the reloaded beacon) | Nothing waits; it is the browser's, like `pagehide`. | — |
| A native dev restart, a `reload()` command, `DevMenu.reload` (`DevMenuMac.swift:232–242`, `Session.swift:1018–1022`), Linux `reload` (`presenter.rs:531–545`) | Teardown finishes it, as a let-go answer's steps are finished (`turns.rs:50–91`). | 1 s, then dropped and journaled |
| `poison()` | Dropped and journaled; what Ibex2 already has completes on disk (D5). | — |
| macOS quit | `applicationShouldTerminate` returns `NSApplication.TerminateReply.terminateLater` while a ticket is out. The app calls `NSApp.reply(toApplicationShouldTerminate: true)` at the ticket's end, or when its own 5 s timer fires. | 5 s |
| iOS and tvOS suspension | `UIApplication.beginBackgroundTask` is held while a ticket is out and ended when it ends. The expiration handler calls `endBackgroundTask`. A suspension can still cut a write once the assertion expires. | the system's |
| Linux orderly exit | The presenter finishes it before exit. | 5 s |
| Web `pagehide` (amended 2026-10-08) | The filesystem journals its uncommitted mutations to `localStorage`, synchronously, and the next page replays them first, exactly once (below). SQLite, a worker realm and a killed browser still commit or lose; the docs say so. | the event's own (synchronous) |

- **The web filesystem across a closed tab** (amended 2026-10-08, Charlie: "look
  at the exact2 side first"; the authoring bench, t5-pomodoro on Android's web
  cell). A `storage.fs` mutation is one IndexedDB transaction, and Chrome drops a
  transaction still running when the page goes away: a setting committed with
  Enter and the tab closed 4 ms later was lost 4 of 4 times.
  - **The JS target only.** The JS target's filesystem (`ts-data.js`, one per
    app, its module's grants) opts in; the wasm realm's (`storage.js`) and the
    Rust requests' per-scope filesystems (`storage-request.js`) neither journal
    nor replay. Apps ship on the JS target (LLP 1071).
  - **A journal per realm (a session).** `storage-fs.js` keeps each mutation as
    an entry from the moment it is accepted until it commits. The JS target's
    storage queue (`ts-data.js`) accepts it when the app calls (`admit`; a write
    queued before the filesystem module loaded is admitted, in queue order, when
    it does), so a write waiting behind others is an entry too. Only a mutation
    the grants admit is an entry. The mutation's own transaction adds its
    sequence number to the session's applied set (the last 1,024, under a key no
    listing reaches), so whether an entry committed does not depend on the order
    entries ran in. A mutation that fails once the page is going (its
    transaction dropped) stays in the journal.
  - **Written when the page may go.** While the page is hidden, and from
    `pagehide`, the session's entries are in `localStorage` under its own key
    (`exact-storage-journal:<app>:<session>`), rewritten synchronously as
    entries come and go. A phone may discard a hidden tab with no `pagehide`.
  - **Replayed only when its page is gone.** Each session holds a Web Lock for
    its realm's life. A later realm's filesystem takes the app's recovery lock
    (one recovery at a time; a page opening during one waits for it), takes the
    lock of each session whose page is gone, and replays their entries
    interleaved by the time each was accepted, skipping any its session applied
    and checking each against this launch's grants. Each replayed entry adds
    itself to its session's applied set, so a replay cut short resumes and an
    append lands once. Then it drops what was replayed. This filesystem's own
    reads and mutations wait for that.
  - **Failures (D8).** A replayed entry that fails for good (`ENOENT`,
    `EISDIR`, `ENOTDIR`, `EEXIST`, `ENOTEMPTY`, a refusal) is skipped; a busy
    path is retried; any other failure ends the recovery and drops every entry
    left, reported, because kept it could later land over a newer save this
    launch makes. Each is a `storage failed:` line in the runtime's journal
    (`fs.recovery`) and a console warning.
  - **Bounded.** At most 512 entries are pending in a journal (half the applied
    set), so a committed entry's sequence number is always still in its
    session's set; a write past that is not journaled.
  - **Measured after it:** the same close kept the setting 10 of 10 times on the
    JS target.
  - **Not covered**, and the pitfall says so:
    - the wasm realm and Rust storage requests (above);
    - SQLite (its worker's transaction is its own);
    - a source placed in a worker (no `pagehide`, no synchronous storage);
    - writes still queued inside a wasm-realm source (`prelude.js`) behind the
      one running;
    - `compressImage` before its encoded bytes reach the filesystem;
    - two dead tabs' entries accepted in the same millisecond replay in session
      order, which may not be the order they ran in;
    - entries past the journal's 2 MB bound (the prefix that fits is kept);
    - a browser that is killed rather than closed (no event fires);
    - `createFileStore`'s direct users (the picker, SQLite's worker), which do
      not wait for a replay.
- **A dev edit and the 100 ms budget.** A dev edit never waits on the web. A
  native dev restart waits only for work actually in flight, which is usually
  none, so the p50 budget (`rules/RULES.md`) is untouched.
- **The macOS delegates** gain `applicationShouldTerminate`. They are
  `host/apple/Sources/ExactMac/main.swift` and `ExactHostMac/main.swift`,
  which today implement only `applicationShouldTerminateAfterLastWindowClosed`
  (`:374`, `:178`).
- **The iOS delegates** gain the assertion: `ExactIOS/main.swift` and
  `ExactHostIOS/main.swift`.
- **Linux.** The exit drain is a new `host/linux/src/teardown.rs`.
  `presenter.rs` (1,488) and `abi.rs` (1,496) gain one call each at most.

### D11 — Rust sources and worker placement

Both keep today's rule for now, and the docs say so: an answer waits for its
storage.

- **A Rust source** has no promise to leave unawaited. Its storage is an
  explicit `Answer::Later` built by `storage::request` and read back by
  `storage::response` (`data/src/storage.rs:8–24`). The shape it would take is
  `exact_data::storage::after(op, args)` during `answer`. That queues a step
  the `Storage` wrapper returns from `background()`; its failure is journaled
  and no code sees its result. Deferred to a Rust consumer (§7).
- **A worker-placed module** (`Placed`, LLP 1027.002; the worker realm on the
  web) keeps today's rule (D6). Moving work to the background there means
  interleaving background deliveries between the owner's turns. Deferred to a
  consumer that places a saving module on a worker (§7).

### D12 — Docs and adoption

**Docs:**
- `reference.md:374–384` is rewritten to say four things:
  - an answer waits for the storage it awaits;
  - storage it starts and does not await finishes after it, in the order it
    was issued, on every host;
  - call storage in the answer and let the queue order it, rather than
    chaining it on a promise;
  - `clock settle` waits for it, and failures are journaled;
  - two answers interleave at their awaits, as on the web, so writes that
    must be contiguous go in one `transaction` or one operation.

  The "refused and logged" sentence goes, except for module evaluation and
  `fetch` from background work. The `full` code joins the code list.
- `agent-pitfalls.md` loses fix/data6's "one input late" entry. It gains three:
  - "a background save fails silently to the person unless the next answer
    says so: keep a `saveError` in the module and answer it";
  - "a `.then`-chained save is issued when the previous one lands, so a read
    can overtake it: call storage in the answer";
  - "another answer's write can land between two awaited writes of yours:
    put writes that must stay together in one `transaction`".
- `contract-for-humans.md`'s storage section gets the Summary's example.

**Adoption** (outside the repo, `EXACT_APP_DIR`, on the web and macOS):

- **Drums.**
  - `task autosave`, the `persisted` mutation and `savedRev` go.
  - `op`'s answers call `atomicWriteFile` unawaited, with `.catch(note)`.
  - Its persistence tests become an edit, then `reload` (which settles).
  - The R11 tests pass without settles on web and macOS.
- **The in-repo storage consumers** are driven unchanged (Fieldnotes,
  Markdown), with their tests on the web and macOS.

## 4. Effect on each implementation

| | Stage 1 | Stage 2 | Stage 3 |
|---|---|---|---|
| prelude | `background`, the move, owners, the queue, issuer-only ticket 0, `full`, no background `fetch`, the rejection tracker, journal lines | — | — |
| Hermes executor | `js/src/background.rs`; the main-thread flag; `DEFERRED` deleted (second commit) | — | — |
| runner | `background.rs`; the background `PendingReq`; three `DataSource` methods; `take_logs`; `state.background`; the `Target` arms; `poison()` drops | — | — |
| composers | `Storage`, `Mixed` and `Placed` on `Main` forward, with their remaps | — | — |
| Apple, Linux | none | — | the delegates (Mac and iOS, app and host); `teardown.rs`; native dev-restart drain |
| web wasm | — | `storage.js` owner and re-homing; `module-glue.js` background loop | — |
| JS target | — | `ts-data.js` queue, bound, count, journal; `agent.js` | — |
| driver | `background` in `clock`'s reply; `reload` settles and records | the same on the web | — |

## 5. Tests

- **Prelude and executor** (`js/tests/it/storage.rs`, beside
  `storage_an_answer_does_not_await_still_lands`):
  - an answer that starts an unawaited write replies in its own turn, and the
    write lands under the background ticket;
  - an answer that awaits its write replies after it;
  - **two unawaited edits, then a read, with no settle**: the read sees the
    second edit (the direct-call form). The chained form is asserted as
    documented: the read sees the first;
  - an answer awaiting a promise chained on background work is `WAITING`
    and replies after it;
  - **no spin:** continuation rounds equal deliveries over a background
    write and an answer's read behind it; an idle module hands out no
    background ticket;
  - with 256 operations waiting behind the one in flight, the next call (the
    258th) rejects `full` and is journaled;
  - `fetch` from background work rejects and is journaled, and no answer
    claims a ticket for it;
  - a failing background write journals `storage failed:` and rejects the
    app's promise;
  - an unhandled rejection is journaled through the tracker in the lean VM;
  - storage during module evaluation is still refused;
  - a let-go answer's steps still finish;
  - with `DEFERRED` deleted, an answer asked while another's awaited write is
    the head begins and replies at once;
  - two answers interleaving at an await produce D4.5's order, `write1,
    writeB, write2`, on Hermes and on the JS target;
  - an answer whose operation is queued behind another owner's settles
    `WAITING`, not ticket 0;
  - `native.call` and `fetch` from background work reject and are
    journaled;
  - a background round's second `RequestOut` reaches `take_requests()` under
    the same ticket, through `Storage`, `Mixed` and `Placed` on `Main`.
- **Runner** (`runner/src/runner/background/tests.rs`):
  - a background ticket is in `pending`, `holds` is true and `has_pending`
    counts it;
  - its rounds replace it without `forgot`;
  - a refused commit's restore keeps the live background entry;
  - `background_landed`'s `Err` is journaled with no commit and no
    `update()`;
  - `release_work` after a background fulfill asks a `WAITING` answer
    again;
  - `poison()` drops it with its line;
  - `take_logs` lines reach the journal through `Storage` and `Mixed`.
- **Hosts.**
  - Linux's agent test: `clock settle` waits on background work, and `clock
    +N` reports `inflight` as a number and `background` beside it
    (`host/linux/src/agent/tests.rs`).
  - An XCTest for the iOS assertion's begin, end and expiration.
  - A macOS quit during a save keeps the file.
- **Web.**
  - The module-glue tests: a write still in flight when its answer finishes
    resolves its promise after the answer's owner is retired, through the
    re-pointed cell. A queued completion is re-homed the same way. The test
    asserts the app's promise resolves, not only that an owner exists.
  - `host/web/tests/js-runtime.test.mjs`: the JS target's queue (a read
    behind a write), bound and count.
- **Parity.** A fixture app (`host/web-js/conformance/storage-background`, a
  TypeScript source) is driven by the same steps on the wasm target, the JS
  target and Linux, comparing journals and `state.background`:
  - two edits that save unawaited, each answer at its input's end;
  - a read;
  - `reload`;
  - the value read back.
- **Driven.** Drums' suite on web and macOS, its R11 tests without settles;
  survey's suite on macOS without the `clock settle` after each add (survey
  #3); the five checks after each stage.

## 6. Implementation plan

Each commit passes the five checks. Stage 1 starts once fix/data6 is on main.

1. **2026-10-07, Hermes, the runner, Apple and Linux** (D1–D6, D8, D9, D10's
   `poison()`).
   - **First commit:** D3's issuer-only ticket and the no-spin test.
   - **Second commit:** `DEFERRED` deleted (D4.5), with its test.
   - **Exit:** §5's prelude, executor and runner tests; drums on macOS, its
     R11 tests passing without settles; the parity fixture on Linux.
2. **2026-10-08, the web** (D7, D8 and D9 for both targets).
   - **Exit:** the parity fixture on the wasm target, the JS target and
     Linux; drums on the web.
3. **2026-10-09, teardown, lifecycle and docs** (D10, D12).
   - **Exit:** the iOS XCTest; the macOS quit test; drums adopted on web and
     macOS.
   - If the macOS terminate wait slips, the rest lands and the slipped piece
     gets a `QUEUE.md` line.

### As built (stages 1–3, 2026-10-04)

All three stages landed on `impl/1097` in one day: the mechanism and the
no-spin test, the deferral's deletion, the driver, the web, then teardown,
lifecycle and docs. Where the build differs from the text above, it says so.

- **The background round is not a `PendingReq`.** It is a field of its own
  (`runner/src/runner/background.rs`): `holds`, `has_pending`, `in_flight`
  and `pending` see it (listed as `background`), and it is outside the
  checkpoint, `enqueue` and `forgotten`, so a refused commit's restore keeps
  it and a commit that lets requests go cannot drop it. No `Target` variant,
  so none of D5's match arms. Each round goes out under a **fresh ticket**,
  so a host never sees a ticket it completed come back; one request is still
  in flight at a time.
- **The token passes through, unremapped.** `exact_runner::BACKGROUND`
  (`u64::MAX - 2`) is forwarded unchanged by `Storage`, `Mixed` (to its
  JavaScript child), `Placed` on `Main`, `Swappable`, the `configured!`
  entry and the render host's `Anonymous`, so no forwarder consumes a
  round's token or prunes it in `forgotten`.
- **A fourth method, `background_state`**, carries `state.background` and
  `clock`'s `background` count. Its operations are counted **module-wide**
  (`queued` behind the one in flight, `inFlight`, `done`, `failed`, `last`),
  an answer's and the background's alike, because the JS target has no
  owners to tell them apart; `background: storage (N waiting)` and
  `background: done (N operations)` mark a background run's start and end.
- **Polling** is in `take_requests()`, which every host calls after each
  answer, fulfil, release and commit; the module's journal lines are taken
  there too, and after each round.
- **The rejection tracker.** Hermes's tracker schedules its report with the
  global `setTimeout`, which the prelude refuses; the prelude's `setTimeout`
  runs the tracker's own callback (`bound onUnhandled`) at the next
  checkpoint and refuses every other. The test proves it in the lean VM.
- **Journal lines** go through a host op (13) natively and in the wasm
  realm; `console` lines are `console: …`. Every failed storage operation is
  journaled, so an app's expected first-launch `ENOENT` is a line too.
- **Between answers** (a background round, a let-go call's steps, teardown)
  there is no store, and storage is not refused as at bake. Without the
  deferral, a targeted continuation no longer replaces a call parked on its
  key: the runner names the one in flight (`forgotten`) or drops the new one
  (`discard`), which keeps files F18's re-read case working.
- **The wasm realm.** A background round waits for its completion outside
  the realm's turns and takes a turn to deliver it; parked answers are asked
  again in that turn. The realm's `console` and `unhandledrejection` reach the
  journal. The JS target journals every unhandled rejection on the page (the
  runtime handles its own), without reading stacks.
- **Teardown.** `Module::unload` finishes the module's storage within a second,
  so a dev restart, a reload and a drop all do. Linux's `teardown.rs` waits
  five seconds at an agent's quit and a headless exit (the display loop has no
  orderly exit). macOS (both delegates) and the agent's stdio end hold the quit
  through `StorageHold` (`ExactKit/StorageHold.swift`): five seconds, and 1.5
  for the driver, which kills at two. iOS and tvOS hold a background task from
  `sceneDidEnterBackground` (both delegates). The XCTest is the platform-
  neutral rule (`StorageHoldTests`: begin, end, bound, expiration), run by
  `build.mjs --test`.
- **The driver's `reload`** lands the storage with `clock data`, which fires
  no timer; a full `clock settle` would. There is no interactive restart in
  `agent.mjs`, so only the test step does it.
- **Not built: the parity fixture** (§5), for want of a TypeScript app with
  storage on a Linux host (`QUEUE.md`). The same steps were driven by hand on
  drums on the JS target, the wasm target and macOS: `state.background` and
  the storage lines matched, with host-specific messages under equal codes.
- **Verified.** Drums at `7bad882` (R10's chain) persists on macOS and the
  web; adopted drums (§D12: no `task autosave`, `persist` or `savedRev`; saves
  unawaited in the answers) passes a new test of consecutive edits read with
  no settle (R11) on the JS target, the wasm target and macOS, and its
  persistence tests through `reload`; a macOS quit and a drive's end keep the
  last write. Its velocity test fails before and after on every host (a
  context-menu tap's delivery), unrelated.

## 7. Deferred, with preconditions

- **A background `fetch`** (a sync to a server after a save). Needs a
  consumer, and a design for its ticket, its retries and who reads its reply.
- **Rust sources' `storage::after`** (D11). Needs a Rust consumer that saves
  from an answer.
- **Background work on a worker placement** (D11). Needs a consumer that
  places a saving module on a worker, and a measured owner-thread
  interleaving.
- **A Contract-visible signal** for background state. Needs a consumer whose
  UI must show "saving…" or "couldn't save" before its next answer.
- **Parallel storage across files** (D3). Needs a measured consumer whose
  writes queue behind each other long enough to matter.

## 8. Considered, not taken

- **Keep today's rule and document the task idiom** (fix/data6's pitfall).
  The web and native would still disagree on when an answer lands, and every
  editor would carry an autosave timer.
- **An explicit `background(promise)` API.** It is a name for what a browser
  does without one. Apps written for the web would still fail natively until
  they learned it.
- **Run background steps synchronously on the owner thread**, as let-go does
  (`turns.rs:50–91`). It blocks the runner for the length of a write, which
  is R11 again on the main thread.
- **A background ticket per operation.** The runner's request count would
  grow with the app's write rate. One ticket, with rounds, is bounded.
- **Filtering Ibex2's completions by owner.** `deliver_one` has no owner
  (`ibex2_jsi.cc:1004–1015`). With one operation in flight and only its
  issuer waiting (D3), there is nothing to filter.
- **Keeping the answer-to-answer deferral** (r1's Q1). See D4.5.
- **Making the chained form order by schedule time.** That would mean
  reordering the queue by the app's intent, which no store does. The docs
  steer to the direct call instead (D12).

## 9. Questions decided (the orchestrator, for Charlie, 2026-10-04), and two changed in r2

The orchestrator accepted every r1 recommendation. r2 keeps them, except two
that the round-1 reviews showed could not stand as written. Each is flagged
for the orchestrator's confirmation:

1. **Answer-to-answer deferral.** r1 recommended keeping it in stage 1, and
   that was accepted. **Changed in r2, confirmed by the orchestrator:** it is
   deleted in stage 1's second commit (D4.5). Answers then interleave at their
   awaits, as on the web. The round-2 review's case against deletion is
   answered in D4.5 and in its disposition.
2. **The driver's `reload` settles storage first:** kept. It is implemented
   in the driver (D9).
3. **No Contract-visible signal yet:** kept (§7).
4. **The queue's bound: 256,** kept. **Changed in r2, confirmed by the
   orchestrator:** the refusal's code is `full`, not `EBUSY`, which already
   means a locked database (round-1 review A, finding 6).
5. **The teardown bound: 5 s,** kept for macOS quit and Linux exit. A native
   dev restart takes 1 s (D10).
6. **No `rules/DEFERRED.md` entry:** kept.
7. **Keep fix/data6's `5c8fd1521`:** kept. D1 defines "started" by its
   checkpoint.

## 10. Revisions

- **r4** (2026-10-04, accepted). Grok 4.7 xhigh's final review of r3
  (`llp/reviews/1097-r3.grok.md`, NOT READY: three MATERIAL, six MINOR, two
  NIT). Three rounds are done, so its fixes are folded as given, unreviewed;
  the implementation review checks them. The code confirmed each one
  (`js/src/lib.rs:772–785`, `:515–524`; `prelude.js:783–786`). Round 3 also
  reproduced D4.5's interleaved order in headless Chrome.
  - **D2:** the waiting reply carries `call`; the claim scan skips
    `background`; step 3 covers a finished, unmoved answer.
  - **D7:** the wasm realm parks a waiting answer and settles it again after
    a background delivery, without `storage.deliver` or a ticket lookup.
  - **D5:** `background()` returns `None` while its `RequestOut` is queued or
    held. Current `poison()` locator.
  - **D6:** a `main_thread` field that `build` sets false before
    `activate()`.
  - **§4, D8, D10, D11:** `Placed` in the composers row; `Mixed`'s trait
    path; the dev-edit locator; the Rust storage wording.

- **r3** (2026-10-04, round 2 of 3). It resolves Grok 4.7 xhigh's delta
  review of r2 (`llp/reviews/1097-r2.grok.md`, NOT READY). Each finding was
  checked against the code; the dispositions are in that file.
  - **D2, D3:** an awaiting answer takes ticket 0 only when its operation is
    the head, and settles `WAITING` otherwise. `background` is `pending`, and
    `native.call` is refused there.
  - **D4.5:** kept deleted, as confirmed. The interleaving it allows is
    stated as the web's order, with the transaction idiom for contiguous
    writes, and the reason now matches D3. Finding 2 is rejected with these
    reasons.
  - **D5:** each round's `RequestOut` is pushed under the same ticket; both
    `background()` and `background_landed` remap through the composers;
    `Placed` forwards on `Main`; the executor has a `BACKGROUND` arm; current
    `commit.rs` and `admission.rs` locators.
  - **D6, D7:** the flag is set by the inline load and never by a worker;
    the wasm realm gets mutable owner cells and its flag in stage 2.
  - **NIT:** the 258th call; `NSApp.reply(toApplicationShouldTerminate:)`.

- **r2** (2026-10-04, round 1 of 3). It resolves both Grok 4.7 xhigh reviews
  of r1, whose dispositions are in `llp/reviews/1097-r1.grok-{a,b}.md`. Each
  finding was checked against the code. Review A's ordering finding was
  reproduced by simulation (§1), and the Hermes rejection tracker was found
  in the pinned source.
  - **D1:** "started" is defined by the checkpoint; the Summary and D12
    call storage in the answer.
  - **D2:** the move ahead of today's ticket-0 return; the count zeroed and
    owned in one place; no `fetch` from background work.
  - **D3:** issuer-only ticket 0, which removes the spin; the `full` code;
    the bound's denominator; the JS target's reads queued.
  - **D4:** the guarantees narrowed to issued operations; "no Contract
    `then`"; the deferral deleted.
  - **D5:** the background ticket's own path, in `pending` without `forgot`,
    kept across a checkpoint restore; the `Target` arms; the token reserved;
    the remaps; LLP 1092 D3 amended; `poison()` drops.
  - **D6, D7, D11:** the main-thread flag, so a worker placement keeps today's
    rule; the wasm realm re-homes completions before retiring an owner.
  - **D8:** the tracker confirmed at the pin; `take_logs` from the
    JavaScript child.
  - **D9:** `inflight` stays a number; the driver's `reload` settles and
    records.
  - **D10:** one rule per event, with the delegates and files named.
  - **NIT:** citations corrected (`storage.rs:63–92`, `Agent.swift:518`,
    `glue.js:1219–1250`), and the main SHA dropped.
- **r1** (2026-10-04): first draft.
