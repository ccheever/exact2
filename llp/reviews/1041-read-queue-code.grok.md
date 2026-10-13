I'll review the LLP 1041 §8.4 amendment read-only against the agreed design, starting with the commit range, the design notes, and the diff.The range is three commits on `2eca0ad80`. I'll read the admission path, the amended LLP text, and the tests against the agreed design.There is one blocker. The ordered lane’s sixteen, `counts[0]`, and `ordered_idle` match the amendment; a re-ask behind a full refusal fence does not.

## Blocker

**Re-asks fill the refusal fence’s 128 slots, and the next real read is then refused for good while read capacity is free.**

`Fenced::Again` counts as held:

```104:107:host/apple/src/executor_core.rs
    fn held(&self) -> bool {
        !matches!(self, Fenced::Refused(..))
    }
```

While a refusal is outstanding, `place_waiting` pushes re-asks into that fence until 128 are held (`executor_core.rs:745-751`). The next ordered request, including a `GET`/`HEAD`, does not wait in the read backlog. Admission records it as `Fenced::Refused` (`executor_core.rs:330-350`) with “earlier ordered admission refusal must settle first”, and drops its work. On lift, `resume_ordered` does not try admission again. It returns that ticket as a refusal and raises the barrier (`executor_core.rs:617-624`). The host’s `refuse_request` (`abi.rs:312-315`) then fails the answer: “keeps its last value”.

At that moment the read window can be empty. Fenced re-asks are not in `counts[0]` yet, and when the fence lifts they settle into the marker window (`agains`), not the 128 real tickets. The separate marker window exists specifically so markers cannot refuse the shared load’s next fetch (`llp/1041-graceful-overload.rfc.md:609-614`, found in build). The fence undoes that whenever any ordered refusal is already up and a wave of 128 re-asks arrives before the next read. Re-asks past that 128 stay pending (`executor_core.rs:661-665`); a read in the same spot does not.

`a_re_ask_behind_a_refusal_settles_after_it` never builds this case. It fills the fence mostly with `GET`s and lets one extra re-ask wait (`executor_again_tests.rs:285-307`).

## Should-fix

**Past 1024 waiting records, a re-ask fails permanently and raises the ordered fence.** `again` returns `"native executor re-ask backlog full"` (`executor_core.rs:26-27`, `661-663`). Apple and Linux then `refuse_request` on an ordered ticket (`abi.rs:312-315`, `presenter.rs:816-818`), so the answer takes the `release_failed` path and later ordered work is fenced. Render marks the page `Busy` (`lib.rs:870-872`). The amended text documents this carve-out (`llp/1041-graceful-overload.rfc.md:648-650`). It is still the failure mode Q7/Q8 forbid for a capacity-blocked re-ask: the answer is not left pending.

**The 512 MiB lane bound is not checked for a handed-off turn’s outcome.** `next_job` adds no reservation for a handoff (`executor_core.rs:806-812`); `complete` adds `retained` bytes with no aggregate check (`858-863`). Sixteen such turns can pass 512 MiB. The amendment and `QUEUE.md` leave this on purpose and the sixteen still caps how many are outstanding. It is not enforced.

## What holds

`Dispatch::Again` carries no closure (`turns.rs:12-13`, `request.rs:636-641`). With room and no barrier, `settle_again` appends the ticket already complete, charges `size_of::<Completed>()` to the 512 MiB lane, and does not start a job (`executor_core.rs:761-780`). `counts[0]` drops only in `drain` and `forget`, not in `complete`. `ordered_idle` is still `counts[0] == 0` (`579-580`). A finished read joins `light` and leaves the sixteen; a finished write does not (`864-865`, `369-374`). Queued or running `GET`/`HEAD` still count toward sixteen. `POST`, storage, opaque work, ordered native, handed-off turns, ordered auth, and background rounds stay on the effect side. The core tests cover that split, including a background round admitted beside 20 re-asks and refused behind 16 writes (`executor_again_tests.rs:81-125`, `313-341`).

A re-ask inside the marker window or the 1024 waiting records stays pending: `again` returns `Ok`, one record per ticket, oldest first. `place_waiting` runs from `drain`, `forget`, and `resume_ordered`. Resuming that ticket is the same parked call (`lib.rs:900-920`); it does not enter `answer()` again. The fixture’s `#1` begun-count and “each URL once” check that (`shared-load.ts:28-32`, `admission.rs:219-236`). A terminal failure unlinks the call with `forget_calls` before `release_failed` (`lib.rs:900-908`). That is bookkeeping, as Q5 says. Forget drops a re-ask in the window, behind the fence, and in `waiting` (`executor_again_tests.rs:251-278`). `retire` clears completed markers and `waiting` (`executor_core.rs:702-705`).

The readiness record is executor-owned, not on the host `parked` map. That matches the amended text: `Module::release` and the storage composer drop the continuation unless dispatch is `Held` (`mixed.rs:631-633`, `data/host/src/lib.rs:303-305`), so the source cannot retry the token. Apple, Linux, and render share `Core::again`. Web pushes the empty success on the immediate path (`host/web/src/host.rs:1104-1105`). Terminal replies at once (`requests.rs:85-86`). Render’s page enumerator has an arm (`pages.rs:66`). Render checks the deadline after a non-empty drain, so a waiter that re-asks forever ends `Deadline` instead of spinning (`lib.rs:776-779`, test at `1385-1405`).

The separate marker window (not the shared 128 / 64 MiB backlog) is the recorded found-in-build deviation. It is what keeps `/labelers` from being refused when 128 markers are already admitted and the fence is down. The blocker above is that same crowding, moved onto the fence.

## Tests and the repro

The new assertions fail on the old no-op re-ask. An opaque `Work::Now` still hits sixteen, `refuse_request` fails that answer, and `twenty_answers_awaiting_a_shared_load_all_load` requires every row’s text and an empty refusal log (`admission.rs:251-259`, `219-229`). Nothing in the tree runs the old path beside the new one. “Rows 15 and 16” in the LLP is a manual claim, not an assertion.

The 20-row repro is deterministic for that result. `boot` emits and dispatches before the first pump (`abi.rs:413-414`), and `/prefs` blocks until the gate opens after that (`admission.rs:63-68`, `172-179`). URLs are compared as a set, and `DRIVE` keeps the process-global transport to one test. The 100 ms `recv_timeout` (`admission.rs:186`) can add extra pumps if a wake is missed, so this test does not prove wake-only progress. The Linux test does: 200 re-asks, `poll` on the executor fd, fail if no wake (`events_tests.rs:911-946`). Pump counts 38 / 62 / 430 are printed, not asserted (`admission.rs:237-238`), so the LLP’s evidence numbers are not locked.

## Nit

`llp/1041-graceful-overload.rfc.md:639` says waiting records are placed “ahead of newer arrivals.” The next bullet (`643-647`) and `re_asks_past_the_window_wait_and_settle_in_order_with_no_delivery` (`executor_again_tests.rs:202-221`) do the opposite: a write and a read admitted while re-asks wait enter ahead of them. Entry order is the rule the code implements. `js/tests/it/main.rs:25-29` also wraps `Again` back into a no-op `Work::Now` for the JS harness, so those tests never see executor admission.
