# Code review: JS dev loop fixes (atomic save, startup lock, stale reader), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `b20890b78`, `300b875c1`, `0b351a502` (host/web-js/dev.mjs), as first written.
- **Method:** one brief (sha256 `797d8635fe65e119553cd99c07a1f075dedaf7357700f8a41f08bc8e4755f4d2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES on all three (substring temporaries; timing of a temporary; wrong crates and a shared target; the cleaner outliving stop; backend-origin deletion; the test).
- **Disposition:** each fix amended into its commit (`525b91c1a`, `ab217a323`, `263c8ad4c`). Lock: a bare `.exact` event is an edit when `backend-origin` came, went or changed since the last build (existence tracked). Atomic save: a temporary is recognised by an anchored name of a design beside it (`.tmp`[suffix], `.swp`/`.swx`, `~`, JetBrains', Emacs's), at event time whenever it comes, so neither substring matches nor burst timing decide; the earlier substring rule is gone. An empty burst (a stale timer) is no rebuild. The test puts `.exact` aside and back, restores the design by rename after the loop stops, waits for the lock, and waits out the verifying build for any reload. Reader: only the `referenced from` / `in function` lines are read, crates mapped through `cargo metadata` (std dropped), Linux's `undefined reference` too; the clean is its own process group, ended by stop and the listen failures, never started while stopping, and a failed clean is said, not retried. Cargo's target lock serialises the clean with another build there.

---

1. **P0 — A gone file whose name merely contains the design’s name is treated as an editor temporary, so a real delete never reaches the page.** `host/web-js/dev.mjs:308`

   `temporary()` is true when a missing path sits beside a `.tamber` that is still in this burst and `basename(path).includes(basename(d))`. That path then satisfies `designOnly` (`:309`). The follow-up build’s `keep()` (`:365`) throws the new stage away, so `dist` stays as it was.

   Failing input: `app.json` has `"tamber": "assets/brand.tamber"`, and in the same watch burst the design is saved and `assets/brand.tamber.png` is deleted (the JS build copies `assets/` with `cpSync`). Same for deleting `icons.app.tamber.ts` next to a root `app.tamber` while that design is still in `edited` (including after a failed build put it back). The png/ts is not a `.tmp`, a swap file, or a `~` backup, but it is dropped, and the page keeps serving it.

   Fix: treat as a temporary only names that are that design’s temp, anchored, for example `` `${base}.tmp` ``, `` `.${base}.swp` ``, `` `${base}~` ``, `` `#${base}#` ``. Do not use `includes`.

2. **P1 — The same temporary is still a plan rebuild when it is not in the burst `rebuild` already closed.** `host/web-js/dev.mjs:308` and `:472`

   The watcher arms a 5 ms timer and `rebuild()` classifies `edited` synchronously, then clears it. `temporary()` only sees `.tamber` paths still in that set, and only if the temp is already gone.

   Failing input: an editor writes `app.tamber.tmp`, fsyncs, then renames it onto `app.tamber`. If the create event is delivered and the timer fires before the rename, `app.tamber.tmp` still exists, so it is an ordinary non-design edit and `designOnly` is false. If the design event closes the burst first and the delete event arrives later, `edited` holds only the missing temp, `designs` is empty, and it is classified as a deleted asset. Either way the loop logs `edit → plan ready` and reloads.

   Fix: remember design paths saved in the last few hundred milliseconds even after `edited` is cleared, and ignore a sibling temp of one of those whether or not it still exists. Do not start the verifying rebuild on a burst that is only such a temp.

3. **P1 — Stale-link recovery cleans the wrong crates in the shared `host-dev` tree, and often cleans nothing.** `host/web-js/dev.mjs:200` and `:226`

   `staleCrates` takes every `lib<name>-<hex>.rlib` in the captured stderr. Rustc’s link failure includes the full `cc` command, so that list contains the sysroot (`libstd-6f725e084c564b8a.rlib` becomes `-p std`) and every dependency, not only the object that referenced the missing symbol. The usual “referenced from” line is an object (`exact_kernel-<hash>.<hash>.rcgu.o`), which this regex does not read. `cargo clean -p std -p exact-kernel …` then fails package selection and deletes nothing, and the one retry links the same stale rlib. `slice(-64000)` on stderr (`:210`) can keep `std` and drop `exact-kernel`.

   A clean that does match workspace crates runs `cargo clean --profile host-dev` in the one `target/` every JS loop, the terminal host, and the Linux host share (`:354`). That removes `exact-kernel` (and whatever else matched) out from under the other build.

   Failing input: macOS `ld` text `Undefined symbols` plus `_ZN12exact_kernel…E.llvm.15874425473274385624`, referenced from `exact_kernel-0c8e4f2a1b3d4e5f.9ab.rcgu.o`, with `libstd-….rlib` and `libexact_kernel-….rlib` on the `cc` line. On Linux the same failure says `undefined reference`, so `staleCrates` returns `[]` and never cleans.

   Fix: read only the undefined-symbol block; take the crate from the `referenced from` object or rlib; map it through the workspace (so `exact_kernel` stays package `exact-kernel` and `std` is dropped); pass only those `-p` flags. Do not clean a profile another live `cargo` is building into.

4. **P1 — The clean keeps running after the loop has stopped.** `host/web-js/dev.mjs:226` and `:755`

   The `cargo clean` child is not stored and is not detached, so `stop()` cannot signal it. `stop()` sets `stopping`, kills `tool.child` only, and then `process.exit(0)`. The clean is reparented and keeps deleting `target/host-dev`. The listen-failure paths (`:770`, `:772`) call `process.exit` without setting `stopping`, so a clean that exits in that window still calls `buildTool()`.

   Failing input: start the loop, hit the stale link, then SIGTERM (as `devLoop` does) while `cargo clean` is still in `target/`. A second loop’s `cargo build --profile host-dev` races that orphan.

   Fix: spawn the clean detached, keep it on `tool`, and kill that process group from `stop()` and from both listen-failure exits. Set `stopping` before exiting. In the `exit` callback, rebuild only if this clean is still the current one and `stopping` is false. The `cleaned` flag already limits the retry to one, so this is not an infinite loop.

5. **P1 — Deleting `.exact/backend-origin` is ignored when the watcher reports the directory.** `host/web-js/dev.mjs:453`

   A bare `.exact` event is an edit only when `.exact/backend-origin` exists and its mtime is at least `since`. The `catch` returns when the file is missing. That is the same bare `.exact` event the lock uses. After `rm .exact/backend-origin`, the directory mtime moves and the file is gone, so this path drops the event. A replacement whose mtime stays old (`mv` of a file written before `since`) is dropped here and again at `:464`, which stats the file rather than the directory.

   Failing input: `rm apps/theme-fixture/.exact/backend-origin` with the event filename `.exact`. The bake’s backend selection flips from present to absent (`EXACT_BACKEND_SELECTION`) and the loop does not rebuild.

   Fix: remember whether `backend-origin` existed. If it was there and lstat now fails, keep the event as an edit. Return only when it is absent and was already absent, or present with `mtimeMs < since`.

6. **P1 — The new test can pass while the lock still rebuilds, and its restore of `app.tamber` is a truncate of the shared fixture.** `scripts/tamber-server.test.mjs:693`, `:699`, `:704`, `:707`

   `plan ready` is logged before the design server creates `.exact/tamber.lock`, and `edit → plan ready` is logged only when that rebuild finishes. The test sleeps 5 seconds and checks once. A theme-fixture rebuild that is still running at 5 seconds has not printed `edit →` yet, so the assertion passes. `until(/edit → \w+ ready/)` then returns on the first `edit → batch ready`, and `finally` stops the loop, so a following `edit → plan ready` from the temp in finding 2 is never seen.

   `finally` does `writeFileSync(file, was)` while the loop is still watching. That truncates `apps/theme-fixture/app.tamber` before the new bytes are written. `scripts/tamber-dtcg.test.mjs` reads that same file in parallel and can observe a short file. A kill in that window leaves the fixture empty. `rmSync` of `apps/theme-fixture/.exact` is outside `try`, so a failure in `devLoop` leaves it deleted.

   Fix: point the loop at a copy of the app. On the real fixture, restore by writing a temp and renaming it over `app.tamber` after `stop()`, both inside `finally`. After the lock file exists, fail if `edit →` shows up before the save. After the batch line, wait out one more quiet rebuild interval and require that `edit → plan ready` is absent.

Commit `b20890b78` (lock is no edit): **RIGHT WITH CHANGES**.

Commit `300b875c1` (atomic save is a value edit): **RIGHT WITH CHANGES**.

Commit `0b351a502` (clean and build the reader once): **RIGHT WITH CHANGES**.
