# LLP 1097: Storage that finishes after the answer

**Type:** RFC
**Status:** Draft (r1), for review.
**Systems:** The TypeScript seam (`js/src/prelude.js`; the Hermes executor `js/src/lib.rs`, `turns.rs`, a new `js/src/background.rs`), Runner (a background target, `DataSource` gains two methods; `runner/src/runner/source.rs`, `commit.rs`, `admission.rs`, a new `runner/src/runner/background.rs`), the composers (`Storage`, `Mixed`, `Placed`), web (`host/web/module-glue.js`, `host/web/storage.js`, `host/web-js/ts-data.js`, `host/web-js/agent.js`), Apple and Linux (lifecycle only), the driver, docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-07, stage 2 on 2026-10-08, stage 3 on 2026-10-09 (§6)
**Amends:** LLP 1027 D10 (a completion is delivered only by its owner's checkpoint; the module becomes an owner); LLP 1027.003.000 §13 (module-wide liveness: the "storage a settled answer left in flight is the module's, claimed by the next answer" rule is replaced); kanban F22's rule in `prelude.js` (an answer waits only for the work it awaits); `docs/reference.md:374–384`
**Depends on:** fix/data6 (`5c8fd1521`, `6c290ee89`, `e3add44c7`, `194ec5dfc`, in `~/projects/exact2-wt-data2`), not yet on main at `b79156175`. Stage 1 starts from main once those land.
**Related:** LLP 1016 D1–D5; LLP 1027.001 (language parity); LLP 1027.002 (worker placement); LLP 1041 D2 (overload is refusal); LLP 1092 D3 (a send stays in flight across "one more round"); LLP 1012 §2 (`clock settle`). Diaries: `~/projects/x2apps/drums/DIARY.md` (R10, R11, Top 5 #3, `repros/R10-deferred-write`); kanban F22, ledger F12, hn-reader F7, minesweeper F10 (as cited in `prelude.js` and `turns.rs`). `QUEUE.md`: "A native data module's `console` never reaches the agent's `logs`" (trivia F7).

## Summary

A web app saves the way a browser lets it. It answers from memory and lets
the write finish behind the answer:

```ts
edit(store, args) {
  song = apply(song, args);
  saving = saving.then(() => storage.fs.atomicWriteFile(PATH, JSON.stringify(song)));
  return song;                       // the answer; the write is not awaited
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
  is between storage steps is deferred until that turn ends (`lib.rs:831–853`).
  Drums gave up saving in its answers and saves from `task every(500,
  autosave)` instead.
- **Before fix/data6, the write was lost.** A write queued in a microtask
  after the reply was never run on macOS, and nothing reported it (R10).

This RFC gives the module an owner of its own for the storage work an answer
leaves behind. The runner holds a ticket for that work, and the hosts run it
as they run any continuation:

- **An answer replies when its value is ready**, as in the browser. Storage it
  started and did not await moves to the module's background work and
  finishes there. An answer that awaits its write still waits: it asked to.
- **Storage keeps one order.** Every storage operation of a module, whether an
  answer's or the background's, runs one at a time, in the order it was
  issued. A later answer that reads therefore reads what an earlier
  background write wrote.
- **Failures are loud and the same everywhere.** Every failed storage
  operation, every unhandled rejection and every `console` line from the
  module reaches the journal (`logs`) on every host.
- **`clock settle` waits for all of it**, on every host. An input step no
  longer has to.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | An answer waits only for the work it awaits; the rest becomes background work | drums R11 | 1 |
| D2 | The module's background owner, in the prelude | drums R10 | 1 |
| D3 | One storage queue per module: issue order, one operation in flight, a bound | — | 1 |
| D4 | Ordering against later answers | drums R11 | 1 |
| D5 | The runner: a background target and two `DataSource` methods | — | 1 |
| D6 | Hermes, Apple and Linux | — | 1 |
| D7 | The web: the wasm target's module realm and the JS target | — | 2 |
| D8 | Failure reporting | drums R10, trivia F7 | 1, 2 |
| D9 | The driver: `clock settle`, `reload`, `state.background` | drums R11 | 1, 2 |
| D10 | Teardown and the app's lifecycle | — | 3 |
| D11 | Rust sources and worker placement | — | deferred |
| D12 | Docs and adoption | drums | 3 |

## 1. Evidence

- **Drums R10** (`repros/R10-deferred-write`). The app answered edits from
  memory and saved with `writes = writes.then(() =>
  storage.fs.atomicWriteFile(…)).catch(note)`, unawaited.
  - On the web, the file was saved and survived `reload`.
  - On macOS, `data/` stayed empty, the `.catch` never ran, and the logs
    said nothing. `test macos` failed both persistence tests.
  - fix/data6's `5c8fd1521` makes every answer reply after its microtask
    checkpoint (tag 3), so the write is now started inside the answer and
    lands.
- **Drums R11.** Once the save started inside the answer, every edit's answer
  waited for its write. Under `test macos`, every second consecutive edit
  read stale (9 of 19 failed); on the web none did. A `persist` mutation sent
  from `then` was no better: the next edit's answer queued behind the write
  in flight. The app now answers from memory and saves from `task every(500,
  autosave)`. fix/data6's `6c290ee89` documents the lag as a pitfall
  ("`clock settle` after the edit in the test, or … save from a `task`").
- **Today's code.**
  - `storageCall` refuses a call when no answer is current, or when the
    current one has replied (`prelude.js:609–617`).
  - `settle` will not reply while the call `owes` work (`:738–797`). Its
    comment gives the reason: "a host only runs an answer's steps while that
    answer is in flight, so an answer that replied first would strand them".
  - The Hermes executor drains microtasks only in `begin`, `resume` and
    `finish_let_go` (`js/src/lib.rs:891`, `:1009`; `turns.rs:84`).
  - An answer asked while a turn is open is parked as `DEFERRED`
    (`lib.rs:831–853`).
  - The wasm target's realm keeps an answer's turn on `tail` until its
    storage lands. "Other answers queue behind it" (`module-glue.js:184–205`).
    `storage.js` drops a completion whose owner was retired
    (`storage.js:30–35`).
  - The JS target runs storage as plain promises (`ts-data.js:150–181`). A
    write nobody awaits runs on the event loop, and nothing counts it, so
    `clock settle` cannot see it.
  - A let-go answer's steps are already finished without an answer
    (`turns.rs:50–91`, ledger F12). That is the one place where the module
    runs storage on its own, synchronously, on the owner's thread.
  - LLP 1027 D10 is the premise: "Only the owning answer's checkpoint
    delivers a browser storage completion, with its store context
    installed; no unsolicited microtask resumes app code."

## 2. What the web does

A page's storage (IndexedDB, OPFS, a SQLite worker) completes on the event
loop whether or not anyone awaits it. An unawaited write finishes after the
code that started it has returned. An operation the app does not catch is an
`unhandledrejection`, which the console prints. Per-object ordering is the
store's own: IndexedDB runs transactions with overlapping scopes in creation
order, and a SQLite worker runs its queue in order. A page being unloaded may
lose what is in flight. This RFC makes Hermes and the wasm realm behave the
same way, and adds what the web leaves to chance: an order, a bound, a
journal line and a wait for the driver.

## 3. Decisions

### D1 — An answer waits only for the work it awaits

An answer replies when its value is ready: its promise has settled, or it
returned a value at once. Any storage steps it started and did not await move,
at that moment, to the module's **background work** (D2). The answer does not
wait for them.

- **A step the answer awaits** is part of its value, so the answer still
  replies after it. This is unchanged.
- **A `fetch` the answer did not await** still holds the reply, as today. A
  background fetch is §7's: it would need a ticket of its own, a retry rule
  and an owner for its reply.
- **A failure of a moved step** cannot fail the answer, which has already
  replied. It rejects the app's own promise, as on the web, and it is
  journaled (D8).

This replaces kanban F22's rule ("the answer is given once the work it
started has landed"). F22's reason was that steps no answer owned would be
stranded. With an owner and a ticket for them (D2, D5), nothing is stranded.

### D2 — The module's background owner

The prelude gains one module-level pseudo-call, `background`. Like an answer,
it has a `storage` count and a place in `storing`. It is never replied to and
never let go.

- **The move.** When `settle` finds an answer finished (`done` or `failed`)
  with `call.storage > 0`, it adds that count to `background.storage`, marks
  the answer `moved`, replies, and asks the executor to run background work
  (D5).
- **Contexts that resolve to it.** A storage completion's `.then` sets
  `currentCall` to the call it captured (`prelude.js:631–640`). For a moved or
  replied call, it sets `background` instead. So a chain such as `saving =
  saving.then(() => write())`, which runs when the previous write lands,
  issues its next write as background work.
- **No refusal after a reply.** `storageCall` with `call.replied` attaches the
  call to `background`; it no longer refuses it. Two refusals stay: during
  module evaluation (no answer has ever begun), and at bake (`code: 'bake'`,
  as today).
- **Liveness.** Background work counts as outstanding (`turns.rs:28`,
  `outstanding`). An answer that awaits a promise chained on background work
  (`await saving` before reading) is `WAITING`, and is asked again after each
  background delivery (LLP 1027.003.000 §13). The rule that "a storage step a
  settled answer left in flight is the module's: the next answer to settle
  without work of its own claims it" (`prelude.js:723–727`) is deleted:
  `background` owns that work.

### D3 — One storage queue per module

Every storage operation of a module enters one FIFO queue in the prelude, and
at most one is in flight at a time. Answers and background issue into the same
queue, and an operation starts when the one before it settles.

- **Why.** Completions never interleave, so each one is delivered to the
  owner that issued it. The order is the order the app wrote, which is what
  "one storage turn at a time, as the browser's worker runs them"
  (`lib.rs:831`) was protecting. The browser's SQLite worker is already
  serial, and so is a single SQLite connection. The parallelism given up is
  between independent files, which no consumer has measured.
- **Bound.** At most 256 operations are queued per module. The 257th
  rejects at once with `code: 'EBUSY'` ("storage queue full: 256 operations
  wait") and is journaled (D8). Refusal is the overload policy (LLP 1041 D2).
  A runaway save loop then fails where it starts, and does not grow without
  limit.
- **The host's wait.** `Session::continuation` (`js/src/storage.rs:64–94`)
  waits until Ibex2's context is idle or a completion arrives. With one
  operation in flight, "idle" means the queue's head has landed. The
  continuation for the background ticket and an answer's continuation both
  wait on the same context; only the issuer's resume delivers (§4, the
  risk).

### D4 — Ordering against later answers

The guarantees, in the order an app relies on them:

1. **An answer never waits for background work to begin or to reply.** A
   background turn does not park answers as `DEFERRED`: `turn_open()` counts
   answers' turns only. A storage-free answer (drums' edit) begins and
   replies in its own turn on every host.
2. **Storage runs in issue order** (D3). A read an answer issues after an
   earlier answer's background write sees that write. A write an answer
   issues lands after every background write issued before it.
3. **Awaiting background work waits for it** (D2, liveness). `await saving`
   in a later answer replies only after the chain it awaited.
4. **A background failure changes no answer.** It re-asks nothing, refuses no
   commit and runs no `then`. The app learns of it through its own promise,
   and the person through what the app's next answer says (D12).
5. **Answer against answer is unchanged.** An answer between its own storage
   steps still defers the next answer (`lib.rs:831–853`). Removing that rule,
   now that D3 orders storage, is Q1.

### D5 — The runner: a background target

The runner gets a third kind of target beside resources and mutations,
`Target::Background`, with no slot, no `pending`, no `then` and no commit. The
new `runner/src/runner/background.rs` keeps it, and `commit.rs` (1,077 lines)
gains call sites only.

```rust
/// Work the module started that no answer waits for (LLP 1097 D5), or
/// `None`. Polled after every `answer`, `fulfill`, `release` and
/// `forgotten`.
fn background(&mut self, store: &Store) -> Option<Request> { None }

/// The outcome of background `token`: its next step, or `None` when done.
/// An `Err` is journaled; nothing commits.
fn background_landed(&mut self, token: u64, store: &Store, outcome: Outcome)
    -> Result<Option<Request>, DataError> { Ok(None) }
```

- **One ticket in flight.** Its steps are a chain of rounds, like a `Later`
  that answers `Later` again (LLP 1092 D3).
- **Enqueued like any request.** The ticket gets a ticket number in
  `requests` and goes to the host through `take_requests()`. `holds(t)` is
  true for it, so no host lets go of its work (`commit.rs:900`), and
  `has_pending()` counts it (`commit.rs:864–868`).
- **Never forgotten** by arguments, assignment or a refused commit. Only
  teardown ends it (D10). `poison()` finishes it first (D10).
- **The journal:**
  - `background: storage (2 waiting)`;
  - `background: done (3 operations)`;
  - `background failed: <message>`, an executor error, as distinct from a
    failed operation, which is D8's line.
- **The composers forward it.** `Storage`, `Mixed` and `Placed` forward
  both methods, translating tokens as they do for `continuation`. These are
  the same three that QUEUE's `take_logs` line names (D8).

### D6 — Hermes, Apple and Linux

The executor's half is the new `js/src/background.rs` (`lib.rs` is 1,480
lines):

- **`background()`.** While `background.storage > 0` and no background ticket
  is out, it returns `Request::continuation(BACKGROUND)`.
- **`dispatch`.** The token maps to `storage::Session::continuation()`, the
  closure answers use, run on the host's I/O worker.
- **`background_landed`.** It calls `deliver_storage_one()` and `drain()`
  with `currentCall = background`, then `progress += 1`, which wakes waiting
  answers (`turns.rs` `wake`). It returns the next round while
  `background.storage > 0`.

Apple (`host/apple/src/abi.rs:229–283`) and Linux
(`host/linux/src/presenter.rs:777–805`) already run any continuation ticket
the runner hands out, park `Held` ones, and release them after each commit.
They need no change for D1–D5. Their lifecycle changes are D10's.

### D7 — The web

**The wasm target's module realm** (`module-glue.js`, the shared prelude):

- `storage.js` gains one owner that is never retired, `background`. Its
  completions queue as an answer's do (`storage.js:30–35`).
- A turn's loop (`module-glue.js:184–205`) ends when the answer's value is
  ready. Its leftover storage moves to `background` (D2), and the answer is
  `finish`ed. The next answer no longer waits on `tail` for that storage.
- A background loop beside `tail` (`storage.deliver(background)`,
  `checkpoint()`, the prelude's `__exact_background()`) runs while the
  wasm runner holds a background ticket. The glue's data source implements
  D5's two methods over it.
- The realm keeps per-answer liveness (LLP 1027.003.000 §13), with one
  exception: an answer awaiting background work is `waiting`, not "pending
  on nothing", while a background ticket is out.

**The JS target** (`ts-data.js`) already runs unawaited storage. It gains what
the other hosts gain:

- D3's queue and bound, inside `storageOf`'s `wrap` and file methods
  (`ts-data.js:170–181`);
- a count of operations in flight, which `exact.inflight` includes, so the
  agent's settle waits for it (`agent.js:266–303`);
- D8's journal lines;
- an `unhandledrejection` listener on the page, attributed to the module
  when its stack is the module's.

The JS target has no answer ownership: every storage operation is simply in
the queue. What it does matches the other hosts in every observable respect
D4 and D9 name.

### D8 — Failure reporting

The same lines appear on every host. They are journaled by the runtime and
read by `logs`:

- **Every failed storage operation**, whether an answer's or the
  background's: `storage failed: atomicWriteFile app:/data/song.json: ENOSPC
  no space left on device`. A failure inside an answer also reaches the
  answer's promise, as today. The line exists so that a failure nobody
  catches is still seen. R10's `.catch` that never ran, and logs that said
  nothing, become impossible.
- **Every unhandled rejection** in the module: `data: unhandled rejection:
  <message>`. Hermes's promise rejection tracker supplies them (the prelude
  enables it); the wasm realm and the JS target use `unhandledrejection`.
- **A module's `console`** reaches the journal on Apple, Linux and the wasm
  target's realm, as the page's console already does on the JS target. This
  is QUEUE's "A native data module's `console` never reaches the agent's
  `logs`": a `DataSource::take_logs()` that the runner drains after each
  answer, fulfill and background round, forwarded through `Storage`, `Mixed`
  and `Placed`. Lines from `Module::take_logs` (`js/src/lib.rs`) are prefixed
  `console:`.
- **`state.background`**: `{queued, inFlight, done, failed, last}`, where
  `last` is the last failure's line. It is printed by the runner on native
  and on the wasm target, and by `agent.js` on the JS target.

A background failure never refuses a commit and never re-asks a resource
(D4.4). There is no Contract-visible signal (Q3).

### D9 — The driver

- **`clock settle`** waits until no storage operation of the module is in
  flight or queued, an answer's or the background's, on every host. Natively
  the ticket is a runner request, so the existing wait loops cover it (Linux
  `agent.rs:697–805`, Apple `Agent.swift:431–515`, the wasm glue
  `glue.js:942–953`), within their existing bounds (20 s, 16 rounds). On the
  JS target it is D7's count.
- **`clock +N`** lists background work in its `inflight` report as
  `background (N operations)`.
- **An input step** ends with what it settled (LLP 1012). Under D1, a
  storage-free answer is there at the input's end on every host, so drums'
  `tap "tempo-up"` then `expect text "tempo" == "113"` passes under `test
  macos` with no `clock settle`. fix/data6's pitfall entry is deleted.
- **`reload`** (the test step, and the interactive operation's
  restart) settles storage first, as `clock settle` would. It then journals
  `reload: waited for 2 storage operations`. A test asks "what persists",
  and a person's app would have had the milliseconds a write takes (Q2).

### D10 — Teardown and the app's lifecycle

- **A dev reload, a `reload()` command, a runner restart or `poison()`**
  finishes the module's background work before the module is dropped, as a
  let-go answer's steps are finished today (`turns.rs:50–91`). The bound is 5
  s in all. What is left after it is dropped and journaled:
  `background: dropped 1 operation at teardown (timeout)`.
- **iOS and tvOS.** While a background ticket is out, the host holds a
  `UIApplication.beginBackgroundTask` assertion and ends it when the ticket
  ends. A suspension then does not cut a write in half.
- **macOS.** `applicationShouldTerminate` answers `.terminateLater` while a
  background ticket is out, and replies when it ends, within 5 s.
- **Linux.** On an orderly exit, the presenter finishes background work
  within 5 s.
- **Web.** A page cannot hold its unload. What is in flight at `pagehide` is
  the browser's to commit or lose, as for any page. The docs say so.

### D11 — Rust sources and worker placement

Both keep today's rule for now, and the docs say so: an answer waits for its
storage.

- **A Rust source** has no promise to leave unawaited. Its storage is an
  explicit `Later` (`data/src/storage.rs:8–24`). The shape it would take is
  `exact_data::storage::after(op, args)` during `answer`. That queues a
  step the `Storage` wrapper returns from `background()`, whose failure is
  journaled and whose result no code sees. Deferred to a Rust consumer (§7).
- **A worker-placed module** (`Placed`, LLP 1027.002) runs a turn to its
  answer on its owner thread (`placed.rs:356–385`). Moving work to the
  background there means interleaving background deliveries between the
  owner's turns. Deferred to a consumer that places a saving module on a
  worker (§7).

### D12 — Docs and adoption

**Docs:**

- `reference.md:374–384` is rewritten: "an answer waits for the storage it
  awaits; what it starts and does not await finishes after it, in order,
  on every host; `clock settle` waits for it; failures are journaled". The
  "refused and logged" sentence goes, except for module evaluation.
- `agent-pitfalls.md` loses fix/data6's "one input late" entry. It gains:
  "a background save fails silently to the person unless the next answer says
  so: keep a `saveError` in the module and answer it".
- `contract-for-humans.md`'s storage section gets the save-in-the-background
  example from the Summary.

**Adoption** (outside the repo, `EXACT_APP_DIR`, on the web and macOS):

- **Drums.** `task autosave`, the `persisted` mutation and `savedRev` go.
  `op`'s answers chain an unawaited `atomicWriteFile` on `saving`. Its
  persistence tests become an edit, then `clock settle`, then `reload`. The
  R11 tests pass without settles on web and macOS.
- **The in-repo storage consumers** are driven unchanged (Fieldnotes,
  Markdown), with their tests on the web and macOS.

## 4. Effect on each implementation

| | Stage 1 | Stage 2 | Stage 3 |
|---|---|---|---|
| prelude | `background`, the move, contexts, D3's queue and bound, the rejection tracker, journal lines | — | — |
| Hermes executor | `js/src/background.rs`; `turn_open` counts answers only | — | — |
| runner | `Target::Background`, two `DataSource` methods, `take_logs`, `state.background`; `background.rs`, call sites in `commit.rs` and `admission.rs` | — | `poison()` finishes background |
| composers | `Storage`, `Mixed`, `Placed` forward | — | — |
| Apple, Linux | none | — | D10's lifecycle |
| web wasm | — | `storage.js` owner, `module-glue.js` background loop | — |
| JS target | — | `ts-data.js` queue, count, journal; `agent.js` | — |
| driver | `inflight` names background; `reload` settles | the same on the web | — |

**The risk** is D3's host wait. `Session::continuation` waits on Ibex2's
module-wide context, so two tickets (the background's and an answer's) can
both wake on one completion. Only the issuer's resume delivers it; the other
finds nothing and goes one more round. Stage 1's first commit proves that
this cannot spin: a ticket whose owner has nothing queued returns at once,
and the runner does not re-ask it until its owner issues again. If it can
spin, the continuation becomes per-owner (a completion carries its owner)
before anything else lands.

## 5. Tests

- **Prelude and executor** (`js/tests/it/storage.rs`, beside
  `storage_an_answer_does_not_await_still_lands`):
  - an answer that starts an unawaited write replies in its own turn, and the
    write lands under the background ticket;
  - a chain `saving.then(write)` over three answers writes three times, in
    order;
  - an answer that awaits its write replies after it;
  - a later answer's read sees an earlier background write;
  - an answer awaiting `saving` is `WAITING` and replies after the chain;
  - the 257th queued operation rejects `EBUSY` and is journaled;
  - a failing background write journals `storage failed:` and rejects the
    app's promise;
  - an unhandled rejection is journaled;
  - storage during module evaluation is still refused;
  - a let-go answer's steps still finish.
- **Runner** (`runner/src/runner/background/tests.rs`):
  - a background ticket is enqueued after an answer, a fulfill and a
    release;
  - one ticket at a time, with its rounds;
  - `holds` is true and `has_pending` counts it;
  - it is not forgotten by an assignment or a refused commit;
  - `background_landed`'s `Err` is journaled with no commit;
  - `poison()` finishes it first;
  - `take_logs` lines reach the journal.
- **Hosts.** Linux's agent test for `clock settle` waiting on background work
  (`host/linux/src/agent/tests.rs`); an XCTest for the iOS background-task
  assertion's begin and end.
- **Web.** `host/web/tests/js-runtime.test.mjs` and the module-glue tests:
  the JS target's queue, bound and count; the realm's background owner
  surviving its answer's retirement.
- **Parity.** A fixture app (`host/web-js/conformance/storage-background`, a
  TypeScript source) driven by the same steps on the wasm target, the JS
  target and Linux, comparing journals and `state.background`:
  - an edit that saves unawaited, its answer at the input's end;
  - `clock settle`;
  - `reload`;
  - the value read back.
- **Driven.** Drums' suite on web and macOS, its R11 tests without settles;
  the five checks after each stage.

## 6. Implementation plan

Each commit passes the five checks. Stage 1 starts once fix/data6 is on main.

1. **2026-10-07, Hermes, the runner, Apple and Linux** (D1–D6, D8, D9).
   - **First commit:** §4's risk settled.
   - **Exit:** §5's prelude, executor and runner tests; drums on macOS, its
     R11 tests passing without settles; the parity fixture on Linux.
2. **2026-10-08, the web** (D7, D8 and D9 for both targets).
   - **Exit:** the parity fixture on the wasm target, the JS target and
     Linux; drums on the web.
3. **2026-10-09, teardown, lifecycle and docs** (D10, D12).
   - **Exit:** the iOS XCTest; macOS quit during a save keeps the file;
     drums adopted on web and macOS.
   - If the macOS terminate wait slips, the rest lands and the slipped piece
     gets a `QUEUE.md` line.

## 7. Deferred, with preconditions

- **A background `fetch`** (a sync to a server after a save). Needs a
  consumer, and a design for its ticket, its retries and who reads its
  reply.
- **Rust sources' `storage::after`** (D11). Needs a Rust consumer that
  saves from an answer.
- **Background work on a worker placement** (D11). Needs a consumer that
  places a saving module on a worker, and a measured owner-thread
  interleaving.
- **A Contract-visible signal** for background state (Q3). Needs a consumer
  whose UI must show "saving…" or "couldn't save" before its next answer.
- **Parallel storage across files** (D3). Needs a measured consumer whose
  writes queue behind each other long enough to matter.

## 8. Considered, not taken

- **Keep today's rule and document the task idiom** (fix/data6's pitfall).
  The web and native would still disagree on when an answer lands, and every
  editor would carry an autosave timer.
- **An explicit `background(promise)` API.** It is a name for what a browser
  does without one. Apps written for the web would still fail natively until
  they learned it, and R10's code would stay wrong.
- **Run background steps synchronously on the owner thread**, as let-go does
  (`turns.rs:50–91`). It blocks the runner for the length of a write, so the
  next input waits for the disk, which is R11 again on the main thread.
- **A background ticket per operation.** The runner's request count would
  grow with the app's write rate. One ticket, with rounds, is bounded.
- **Report a background failure as a commit** (a reserved resource
  re-asked). It is new Contract surface with no consumer yet (Q3).

## 9. Open questions, with the author's recommendations

1. **Answer-to-answer deferral** (D4.5). With D3 ordering storage, an
   answer between its own storage steps need not hold the next one: that
   rule protected an order the queue now keeps. Recommended: keep the rule
   in stage 1, and delete it in its own commit once the parity fixture and
   Fieldnotes pass without it, because R11's `persist`-mutation case is
   exactly that rule.
2. **`reload` settles storage first** (D9). Recommended yes. A test's
   `reload` asks what persists. The alternative (a write in flight lost at
   `reload`) is what a crash does, and it is not what the step means.
3. **A Contract-visible signal.** For example, a reserved
   `exactBackground()` source answering `{pending, failed, last}`, re-asked
   as it changes. Recommended: defer. Drums needs none, and "the module
   answers its own `saveError`" covers a failure at the next answer.
4. **The queue's bound.** Recommended: 256 per module, refused with
   `EBUSY`.
5. **The teardown bound.** Recommended: 5 s, the order of macOS's own
   patience with `.terminateLater` before a person notices.
6. **`rules/DEFERRED.md`.** Recommended: no entry. This makes an admitted
   capability (storage, Fieldnotes; LLP 1027.001's parity) behave the same on
   every host, and adds no surface an author writes. The orchestrator may
   rule otherwise.
7. **fix/data6's `5c8fd1521`** makes every answer reply after its microtask
   checkpoint. Recommended: keep it. D1 relies on the checkpoint to see
   what an answer started before deciding what moves.

## 10. Revisions

- **r1** (2026-10-04): first draft.
