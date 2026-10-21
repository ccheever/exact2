# Code review: the date at launch (time.utcOffset read 0 at boot) round 2, 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `92e73dc68c9375e15f846819a63135603f885963390cf036a60778f72247b417`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The dropped commit and the stale date for prepare's reloads fixed by exact_set_launch_time, told in prepare; set_time no longer keeps the date, so a refused recommit leaves nothing behind.

---

The first launch now settles on the real offset. The re-boot path does not: while a session is already booted, `primeTime` can commit a time change on the live runner and throw that batch away.

## MAJOR

**A re-boot can commit a new date on the live runner and drop the batch** — `host/apple/Sources/ExactKit/SessionTime.swift:51`

`primeTime` now runs while the session is booted, and it always calls `setTime`. The bridge stores that date and, when a host exists, applies it (`host/apple/src/abi/preferences.rs:31`). The runner treats the fact as unchanged only when the offset is equal and the epoch moves by less than a second (`runner/src/runner/time.rs:61`). The epoch `dateNow` computes is `Date()` minus `CACurrentMediaTime` (`SessionTime.swift:29`, `Session.swift:580`). That media clock stops while the machine sleeps and `Date()` does not, so a reload after sleep (or after any step of the wall clock) is a real commit: `exactTime` is asked again, `persist` runs (`host/apple/src/host.rs:1352`), and `emit` dispatches the new requests (`host/apple/src/abi.rs:283`) before Swift sees the batch. `_ =` then discards it.

If the replacement boot succeeds, those requests are already in flight on a runner that is about to be dropped. If it fails, the old host is left on the new date and the presenter never receives the ops, so later diffs apply to a tree it does not have. The comment at `SessionTime.swift:47` describes the case where the date did not change. The update is needed only when it did.

## MINOR

**File-watch and dev-server reloads never refresh the date** — `host/apple/Sources/ExactKit/Session.swift:284`

`primeTime` is only called from `boot` and `boot(plan:)`. A live connection reload and an `EXACT_DEV_PLAN` reload go through `applyTogether` → `prepare`, which boots the candidate under whatever `set_time` last stored. While the wall clock and the media clock still agree, that anchor matches a carried clock. After sleep, or after a zone change that `followOffset` has not sent yet, the candidate's first settle still uses the previous epoch and offset. `tellTime` in `presentCommitted` corrects it only after that settle, so a kept answer whose arguments include the timestamp is asked at the old instant first.

**A refused time update stays stored for the next boot** — `host/apple/src/abi/preferences.rs:32`

`launch_time` is written before `Host::set_time`. Validation rejects a bad fact and leaves the stored date alone, which is what round 1 asked for. A fact that validates and then fails inside `recommit` is different: the runner puts the previous time back (`runner/src/runner/time.rs:72`), and the bridge keeps the new one. The next `launching` boots under the time the live runner just refused.

## What round 1 fixed

- `host/apple/src/abi.rs` is 1497 lines. `set_time` lives in `abi/preferences.rs`.
- `launching` restores the thread-local in a `Drop` guard (`runner/src/time.rs:51`). An unwind, a nested boot, and a boot outside `launching` all see the outer date again. The test boots outside the guard both before and after a panic (`contract/cli/tests/it/time.rs:83`).
- Only a date that passes `WallTime::validate` is stored.
- The first Apple boot does read the date. `primeTime` runs before the host exists, the ignored return is `{"error":"not booted"}` with no ops, and both boot sites wrap the runner in `launching(self.launch_time)`. The first settle therefore has the offset the kept answer was stored under. Under the agent, `dateNow` uses `launchPlace.epoch` and the drive zone's offset at that instant (`SessionTime.swift:28`).

A boot that never enters `launching` still starts at zero: the Linux host sets the date after the presenter exists (`host/linux/src/app.rs:332`), the web page calls `followOffset` after `exact_boot` (`host/web/glue.js:1359`), and the render host, the content-region host, and direct `Runner::boot` never set the thread-local. That is the same as before this change.

NOT READY
