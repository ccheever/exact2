# Code review: the date at launch (time.utcOffset read 0 at boot), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `bf8b5bdc6821e74041af246b65a2ece4ed6d2449e4952aa957ba6e22cbc231ec`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The cap, the panic leak, the fresh re-boot on the previous date and store-before-validate fixed in round 1's commit (the re-boot fix revised in round 2).

---

The first Apple settle reads the real offset. The commit is not ready to land: `host/apple/src/abi.rs` is over the 1,500-line cap.

## MAJOR

**`host/apple/src/abi.rs:1512` — the file is over the source-line cap.** `scripts/caps.mjs` counts 1,512 lines (`split` on `\n`). The parent of this commit counted 1,497. `rules/RULES.md` caps every source file at 1,500, with no baseline allowance, so `cap:lines` fails. The two `launching` wrappers (`abi.rs:541` and `abi.rs:842`) are what crossed it. `Session.swift` was split into `SessionTime.swift` for the same cap; `abi.rs` was not.

## MINOR

**`runner/src/time.rs:52` — a panic inside `launching` leaves the date on the thread.** The cell is replaced at line 51 and restored only after `boot()` returns at line 53. Nested boots save and restore `previous` on the success path. If the closure panics, that restore never runs. `cargo test` and the dev profile unwind, and the test harness reuses the thread, so the next `Runner::boot` on that thread reads the leaked `WallTime`. That includes boots that never call `launching`. `release` and `host-dev` use `panic = "abort"`, so a shipped Apple build does not survive the panic. Restore in a `Drop` guard.

**`host/apple/Sources/ExactKit/SessionTime.swift:47` — a fresh re-boot settles on the previous epoch.** `primeTime` returns once `booted` is true. The dev menu's baked reload (`DevMenuMac.reload` / `DevMenuIOS.reload` → `session.boot(size:)`) takes `runtime.boot` when `lastPlan` is nil, which is `boot_fresh` with a new clock at 0 (`abi.rs:546`) and the epoch last stored. `finishBoot` calls `tellTime()` only after that settle. Offset-keyed kept seeds still match. A seed whose identity includes the absolute date is decided against the old epoch, and a mismatch is consumed before `tellTime` installs a fresh one. A carried candidate boot (`prepare_plan_carried`) keeps `now_ms`, so the stored epoch still lines up with the clock.

## NIT

**`host/apple/src/abi.rs:1311` — `set_time` stores the fact before it validates.** An invalid tell replaces a good `launch_time`. The next `launching` drops that fact (`time.rs:50`) and boots at 0, while the live runner has refused it and kept the previous date. `dateNow()` stays inside the range the runner accepts. Store the value only after `WallTime::validate` succeeds.

## Checked

The reported launch path holds. `Runner` copies `launch_time()` into `self.time` before `settle()`. `exactTime` is answered from that value. A resource argument that reads it stays `Pending` until that answer exists, then the kept-seed comparison uses those arguments. A kept Discover identity of `-420` is asked as `-420` on the first settle, and a same-fact `set_time` commits nothing. `primeTime` runs at both session boot sites (`Session.swift:779` and `:806`) before the runner exists.

`dateNow()` is the old `followOffset` and `tellAgentOffset` arithmetic. With `launchPlace.epoch` set (`EXACT_AGENT`, fixed for the session), the frozen epoch and the drive zone's offset at `epoch + now()` are what get stored. Without it, the machine clock and `TimeZone.autoupdatingCurrent` are used.

The not-booted return is the existing `{"error":"not booted"}` batch. `set_time` writes `launch_time` before that return. `primeTime` discards the batch, as `primePreferences` discards `setPreferences`, and does not call `apply`, so the error does not fail the session.

Boots that never enter `launching` still read `WallTime::default` (zero). That is Linux (`host/linux/src/app.rs`, time set after the presenter boots, and `restore_time` on an already-built candidate), the web host (`host/web/src/abi.rs` `set_time` only forwards to a live host), the render host (`host/render/src/lib.rs` `Runner::boot_at`), the content-region host (`boot_region` / `boot_native_region`), and ordinary tests. Those can stay for a later change.

The new test shows the first frame's text, an invalid fact booting at zero, and a later boot outside `launching` still at zero. It does not cover a kept seed or a panic inside the closure.

NOT READY
