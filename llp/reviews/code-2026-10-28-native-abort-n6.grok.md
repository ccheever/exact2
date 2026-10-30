# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 6, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `b4cc675603`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The dedupe holds, the native let-go claim holds, and the web cap is the bound already written on the record. Nothing new blocks.

**(a) Dedupe.** `note_retired` returns immediately when that runner ticket is already recorded, and only then applies the 256 cap:

```161:169:runner/src/runner/admission.rs
    pub(super) fn note_retired(&mut self, p: &super::PendingReq) {
        let Some(fetch) = p.fetch else { return };
        if self.retired_fetches.iter().any(|r| r.1 == p.ticket) {
            return;
        }
        if self.retired_fetches.len() == 256 {
            self.retired_fetches.remove(0);
        }
        self.retired_fetches.push((fetch, p.ticket));
```

A refused commit is the path that used to multiply records. The replacing loop records the request (`commit.rs:580-584`) before settlement. On refusal, `conclude` puts `pending` back from the checkpoint (`commit.rs:180`) and does not touch `retired_fetches`. Rolled-back ids are never reused (`commit.rs:166-169`), so the next refusal of the same request hits the ticket check and adds nothing. `a_request_a_refused_commit_retires_again_is_recorded_once` (`abort_tests.rs:124-145`) calls `note_retired` 300 times on one pending request and expects a single `(fetch, ticket)` pair. While that request sits back in `pending`, an abort matches the live request first and does not consume the record (`admission.rs:118-130`). One phantom slot remains until `fulfill_measured` drops it by runner ticket (`commit.rs:1187`).

**(b) Native.** Yes. After the let-go reaches the module, that call's fetch can no longer make a door-22 call. `__exact_forget` (`prelude.js:1285-1300`) settles every pending fetch whose `p.call` is the let-go call, and `settled` (`prelude.js:504-507`) calls `p.release`. That unsubscribe removes the hook (`vendor/ibex/crates/ibex2/src/bindings/abort.js:114-117`), so a later `abort()` does not run `abortRequest` (`prelude.js:563-566`), which is the only door-22 call. A stream returns before the door (`prelude.js:564`).

Every runner let-go sets `forgot` and then `hear_forgotten` (`commit.rs:210`, `commit.rs:216-229`): the replacing loop, `forget_ticket` (`commit.rs:783-789`), and `enqueue` (`commit.rs:858-864`). That calls `DataSource::forgotten`. `Module::forgotten` is `forget_in_flight` (`js/src/source.rs:247-249`), which drops a targeted parked call whose key is absent and calls `forget_calls` (`js/src/turns.rs:115-142`). `forget_calls` calls `__exact_forget` (`turns.rs:70-71`).

On the main placement (the default, `contract/cli/src/manifest.rs:107`), `Placed::forgotten` runs the module inline before the commit returns (`data/src/placed.rs:811-812`). The only JavaScript after `note_retired` and before that is the same commit's settle and gate (`commit.rs:580-612`). The record was just appended.

On the worker placement the same commit queues `Job::Forgotten` (`placed.rs:61-72`, `798-810`) and returns. The replacement turn is `Job::Answer`, sent later by dispatch, so it sits behind Forgotten. The owner is FIFO (`placed.rs:385-397`). `forget()` strips continuation tokens to `None` (`placed.rs:449-466`), and `forget_in_flight` would keep a same-key call for `Some(None)` (`turns.rs:127-130`). The not-yet-dispatched replacement is still `Recorded::Answer`, and `Placed::forgotten` omits those entries from the view (`placed.rs:799-807`), so the old key is absent and the old call is dropped in Forgotten, before the replacement turn runs. `Mixed` drops the same recorded answer before the child hears it (`data/src/mixed.rs:816`).

A fetch stays abortable only until that Forgotten job runs: during the owner turn already in progress, and during jobs already queued ahead of it. The retired record is the newest one, so removing it takes 256 later retirements. That is the same cap. After Forgotten, the subscription is gone.

A fetch another answer has claimed is skipped on purpose (`prelude.js:1290-1295`). The runner request for that fetch is the claimer's yield (`claim` at `prelude.js:1057-1063`, then `take_request`), so it is still pending and an abort matches pending. It is not a retired record waiting on the cap.

Apple and Linux share `complete` (`host/linux/src/executor.rs:8-9`, `host/apple/src/executor_core.rs:951-960`): a forgotten completion is counted and dropped, never drained, so `fulfill_measured` never runs and the record waits for the cap. By then it names a fetch that can no longer call door 22.

**(c) Web.** Agree. The page fulfills a let-go request on both success and failure (`host/web/glue.js:757-758`) through `safelyFulfill` (`glue.js:972-974`) and `exact_fulfill` (`host/web/src/abi.rs:1217`), which calls `runner.fulfill` (`host/web/src/host.rs:1234`) and thus `fulfill_measured`. The retain by runner ticket runs before the missing-pending drop (`commit.rs:1187`, `1214-1219`). `letGo` aborts only GET and HEAD (`glue.js:754`, `954`); a superseded POST runs to completion and fulfills, which removes the record. A record is evicted while its request is still live only once more than 256 superseded requests are outstanding. That is the bound in the record's own comment (`admission.rs:154-160`).

**(d)** Nothing new. The new test calls `note_retired` directly rather than driving a refused commit; that is the function the refuse path calls. One restored request can occupy one phantom slot until its real reply. `poison` still clears `pending` without a record; that path is fatal and outside this fix.

clean
