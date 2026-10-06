# Code review: a followed end's measured row is its motion's one target; a port at a device pixel is at its target (7962a23b7), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `7962a23b7`, plain output.
- **Method:** one brief (sha256 `4b5b943abc10e7904a32a26034e4c031259cb0561d9a72a10eef24337cf5abaf`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken as proposed: 0.01, and half a point only when `follows_end(&anchor)`, in `restore` and `travel_within`; the 0.4 pt case is a test, run twice. 2 taken: `list.js` mirrors it (`atOffset`). 3 taken: `XCTAssertFalse(driver.drawn)` with the driver driven directly; the half-point test asserts the move; the opening test with `at_end`. A port 0.6 pt short of the end was tried and left out: it is a reader off the end (the anchor does not follow), covered by `dom_integer_end_append_follows_but_half_pixel_reader_does_not`. 4 taken: a retarget before the first frame that folds within half a point of the start sets the offset and ends. 5 taken.

---

LAND WITH CHANGES

1. **MATERIAL** — A half-point gap drops a real row-anchor move. `runner/src/instance/collection/mod.rs:738`

`restore` runs after measurements (`mod.rs:1318`) and from `update_data` (`mod.rs:587`). `capture_anchor` (`index.rs:518`) stores `within` before the measurement; `restore_anchor` (`index.rs:569`) adds the new prefix. A row above the viewport measured 40 → 40.4 yields `corrected = offset + 0.4`. That is `≤ AT_OFFSET`, so no correction is emitted and `g.offset` stays at the host report `set_geometry` just stored (`mod.rs:1307`). The anchor row moves 0.4 pt (one device pixel at 2× and at 3×). The next report captures at that same offset, so the miss becomes the baseline and the next 0.4 pt moves it again. A followed end cannot absorb this way: `restore_anchor` returns `max_offset` on every call (`index.rs:557`), which is the 4405.1667-vs-4405.333 resend that blocks `settle_start` (`start.rs:80`). A kept row round-trips, so a gap above the old 0.01 was a single correction. Linux and the wasm web host share this function; Linux stores the correction as `f32` (`host/linux/src/presenter/collection.rs:878`), whose ulp at a transcript offset is far under 0.5, so a 0.4 pt correction is representable there. `scrollIntoView` already treats 0.5 pt as arrived in `settle_target` (`into_view.rs:268`), which runs after `restore`. An insert still corrects: the new row enters at `ESTIMATED_HEIGHT` (32). Horizontal remeasures are applied before the anchor on purpose (`mod.rs:1280`); a first horizontal measurement still goes through this `restore`.

`travel_within` (`mod.rs:1437`) is the right place for `AT_OFFSET`. With an unchanged index the only persistent gap in `(0.01, 0.5]` is a followed end. Widening that skip does not reopen the idle-tick holes: those are `Session.apply` skipping fills, controls, rasters, and regions, and these reports already pass the fast path’s other gates. `at_end` still forces the full path (`mod.rs:1406`), so `settle_start` still runs for an opening list.

Fix: emit when `gap > 0.01`, and skip the half-point band only when `SizeIndex::follows_end(&anchor)`. Test a 0.4 measurement above a non-following anchor, and a second 0.4 on the next report.

2. **MATERIAL** — The web JS collection still re-sends above 0.01 pt. `host/web-js/list.js:264`, `host/web-js/list.js:549`

The web build is the JS target. `restore` and `travelWithin` are the twin of `mod.rs` and still use `0.01`. `settleStart` (`list.js:257`) still requires `!this.correction`. Glue reads `scrollTop` and writes corrections with `place` (`host/web/collection-glue.js:157`). A device-pixel `scrollTop`, the browser case §6.8 names, still produces a correction on every report, so an opening list never clears `atEnd` and a later follow stays snapped. Glue’s `animate` (`collection-glue.js:168`) only skips the smooth motion under 0.5; the correction stays owed.

Fix: use the same follow-end half-point rule in both functions.

3. **MINOR** — The new tests miss the cases that fail a wrong threshold. `runner/src/instance/collection/smooth_tests.rs:53`, `host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:178`, `:199`

The runner test is a follow-end port `1/6` pt short, an unrelated slot bump, and an append. A `restore` that suppressed every gap under 0.5, including a row measurement, still passes. `scroll-start: end` is never set, so `settle_start` clearing `at_end` is untested. The iOS before-frame asserts sit inside `if !driver.drawn`. `wait` (`SmoothCollectionIOSTests.swift:32`) pumps the run loop in 5 ms slices while the driver starts from `DispatchQueue.main.async`, so a display-link frame in that slice skips the assert and the later “began increased” check still passes. The half-point test allows accuracy 0.34, so an offset left at 0 passes `|0 - 0.3| ≤ 0.34`.

Fix: `XCTAssertFalse(driver.drawn)` before the retarget, and assert the offset actually moved. Add the runner cases from finding 1, a port 0.6 pt short that still corrects, and an `at_end` report at `max - 1/6` that ends with `correction == None` and `at_end == false`.

4. **MINOR** — A fold onto the port after the driver has started still runs the link. `host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:373`

`correct` (`:265`) and the deferred start (`:193`) set a sub-half-point target and run no frames. `retarget` before `drawn` only stores `to`. A measured target within 0.5 of `from` after `startOffsetDriver` still eases for 0.3 s. Shrink still reclamps in `frame` (`:395`). A drag still stops the driver via `scrollViewWillBeginDragging` (`NodeViewIOS.swift:633`). Agent-frozen corrections never build a driver (`CollectionIOS.swift:253`).

Fix: in the `guard drawn else` branch, when the new target is within 0.5 of `from`, assign `contentOffset`, `cancel()`, and `done(serial, true)`.

5. **NIT** — `OffsetDriver`’s type comment (`CollectionIOS.swift:351`) still says every retarget is a fresh ease from the current port. The `retarget` comment is the one that matches §6.8.

No earlier review of this change is in `llp/reviews/`. `QUEUE.md` records the host-resolved end. `mod.rs` is 1499 lines.
