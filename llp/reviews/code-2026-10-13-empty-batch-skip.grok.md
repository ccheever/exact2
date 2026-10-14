# Code review: applyUnlessEmpty lands published slices, then skips an empty batch, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `416d9e451ad6fcb1f10bfa8bbcedd8a209f30c6db2ee9ec6e5e57c25de53529f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The MAJOR (both: the landing ran the appearance reports before the earlier batch) and Astra's NIT (the QUEUE cause) taken in round 2's code.

---

**MAJOR** — `host/apple/Sources/ExactKit/Session.swift:980` — Landing a published slice from `applyUnlessEmpty` does not hold appearance reports, so a batch the owner already made loses to reports created while that slice lands.

`apply` brackets that same `landFill()` with `holdingReports` (`Session.swift:1007`) so `colorsOwed` and `pendingViewDark` flush only after the batch already in hand (`Session.swift:1055`). This path calls `landFill()` bare, then either skips or `apply`s. The slice's outermost `apply` therefore runs `viewScheme` / `reportColors` first. Those commits are later than the timer or scroll batch: `set_view_scheme` presents and can leave the engine in motion (`host/apple/src/paint.rs:111`, `host/apple/src/host.rs:1192`). `changesNothing` then sees the post-report `frames.motion` / `frames.tasks` / `canvasOwed`. If they differ from the earlier batch, `apply(batch)` writes that batch's flags back and `scheduleClock` can drop the display link while a paint transition is live. If they match, the skip path still assigns `timerDue` and rearms the clock from the earlier batch (`Session.swift:986`). The rest of the outermost block (collection flush, commands, regions) was already ungated; only these two drains are the hole. It applies to a non-empty scroll batch as well as a skipped one, because the flush happens before that choice. `noteAppearance` queues a view whose `drawsDark` differs from the session (`Session.swift:1357`), including a `color-scheme` subtree whose `text_color` changed (`PresenterIOS.swift:893`).

Hold reports across this `landFill()`, as `apply` does. On the skip path, drain `colorsOwed` and `pendingViewDark` after `timerDue` / `scheduleClock` / `scrolledWithoutPass`. Falling through to `apply(batch)` then drains them in the existing outermost block. The new test never queues a report, so it stays green either way.

The slice-versus-scroll clock itself is in order. `runtime.scroll` / `advance` run on the owner after a published slice's commit, and a matching empty batch keeps that later deadline (`IdleTickTests.swift:77`). Re-entering while `applying` or `landing` still goes through `apply` or the old in-flight guard. A destroyed session returns after `landFill` without touching the clock. `drainFill` / `drainAll` still wait on the owner and land on their own; this path only lands what is already published, and an unpublished slice still takes `apply`. Callers are scroll, scrollend, and the clock timer. The agent's seek does not use `applyUnlessEmpty` (`Agent.swift:749` sets `session.clock` after `apply`). `publishTickForTests` is DEBUG-only, internal, and goes through real `publish`. The QUEUE entry matches `/tmp/x2p2-iosbase.log`: those eight tests, each once, on `x2-perf2` (iPhone 17 Pro, iOS 27.0), with no assertion text in the xcodebuild log. `48755525c` is that tree's commit and predates the run.

**NOT READY**
