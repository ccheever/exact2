# Code review: a batch of list rows alone skips the route projection, round 2 (132b5bd1b), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `132b5bd1b`, plain output.
- **Method:** one brief (sha256 `ae8b3f5216e1fd6adde68a7048f643c1291e651f3fcb424ffe199e9016d9f38d`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition (r3):** 1: `drawOwed` is gone, folded into `settled` (any early stop is owed). Not taken: a first-draw test with a session and a flat-leaf test.

---

LAND

Round 1 holds. The unconditional `.children`/`.content`/`.frame` arm is gone. `rowsList` is only a virtualized list (`collections.owns`) that is not the route owner and has no header, tab, tablist, or Back control above it. `present` syncs under those same nodes. `drawOwed` is in `syncOwed`. The gate is tested through `apply`.

1. **MINOR** — `drawOwed` and flat leaves are untested. `ListRowsIOSTests.swift:79-86` only checks that a row `style` does not enter `sync` and that a style outside the list does. Deleting `drawOwed = true` at `NavigationIOS.swift:345` (and its read at `NavigationIOS.swift:137`) still passes: nothing here has a session, a window, or `firstDrawMs == nil` with `parts.count > 1`. A flat heading under the header is also absent. `inRows` classifies it only via `flats.parent` (`ListRowsIOS.swift:60-62`); treating every flat id as rows would keep a header title stale and the suite would stay green. The pop suite covers `nativeMoved` only. **Fix:** one apply where `firstDrawMs` is still nil, a second row-only batch, and an assertion that `syncCalls` advanced and the deferred sheet was presented; plus a flat leaf under the header whose `style` is not rows-only.
