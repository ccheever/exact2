# Code review r3: a list's rows moved by the list play no layout-transition (66f3b8b51..0b0e34c8f), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `0b0e34c8f`, plain output.
- **Method:** one brief (sha256 `09b07dcfcb768e6985705b90d2f172f0bed3e72b5e4601b6a3a6f833ec9e1220`), shared with astra. Round 3, the last, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition (landed):** no findings. Astra's round-3 changes (cross-axis moves, tolerance, the nudge, the early return) were made after it.

---

LAND

No findings.

Round 1 and 2 dispositions hold. Key changes alone move `data_generation` (`fresh || shared || rekeyed` in `runner/src/instance/collection/mod.rs:544`); a body or clock edit does not. Each layout records generations before the empty-tracker return (`host/apple/src/presence.rs:187-189`), so the boot layout is a baseline. A list stays settled only when that generation matches and no row wrapper changed size against the mirror, and no row box moved or resized inside its wrapper (`presence.rs:205-215`). Only that list's wrapper and row box take `observe_unauthored` (`presence.rs:236-246`); an idle property snaps, a running one retargets (`motion/src/engine.rs:507-517`). Insert, same-count swap, growth of a row with no transition, and a margin nudge all play, and the other list stays idle (`host/apple/src/collection_tests.rs:533-573`). A measurement that only shifts later rows through the index does not (`collection_tests.rs:429-455`). Edge actions and nested lists share that per-list rule; nothing outside a settled row is settled. The generation walk is one map lookup per mounted list. Touched sources stay under 1,500 lines.
