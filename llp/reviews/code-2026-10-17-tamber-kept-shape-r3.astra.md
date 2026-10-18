# Code review: One retained-shape check for propose and the dev loops (theme2's ruling, Tamber stage 5), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `3303d38a24fa8511e80c5e4f86b62709c03745593086417d9c6397e5da66dd48`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a custom-property value edit; the accent counted per token, not per tuple).
- **Disposition:** applied without re-review (the last round).

---

Two P2 findings remain:

- **P2 — Value edits can still change `Plan::structure`.** [tamber/src/session.rs:359](/tmp/rv-ks-3/tamber/src/session.rs:359): change a kept token from `-exact-platform-color(web var(--ink), #111111)` to the same expression with `#222222`. The retained shape stays equal and the `color(--` guard misses it, so `propose` says `live`. But `Plan::structure` preserves values containing `(--`, so its digest changes. Both loops rebuild or refuse the offered scrub.

- **P2 — The accent flag loses tuple information.** [tamber/src/session.rs:348](/tmp/rv-ks-3/tamber/src/session.rs:348): swap a kept token’s two axis values from `[AccentColor, CanvasText]` to `[CanvasText, AccentColor]`. Both `varies` and `accent` remain true, so `propose` says `live`. The JS loop compares the full accent vector, which changes from `[true, false]` to `[false, true]`; it rebuilds and refuses scrubbing. Record the per-tuple flags.

Read-only design-reader probes confirmed both mismatches. The round-2 regression test passes, and the role metadata wiring and T2 fixture change look sound. No files changed.
