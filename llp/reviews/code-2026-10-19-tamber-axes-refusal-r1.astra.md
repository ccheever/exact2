# Code review: Tamber axes bound before the fold (the JS loop's when-token edit), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `1e9ae466bf`.
- **Method:** one brief (sha256 `570b358ca2c3d575a3c6185fdf7e918f410f1a691c376821e684224fff10feab`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one should-fix (a pre-existing setMode refusal hides an axis declared twice), one nit (a fixed sleep)
- **Disposition:** both taken in f8ad6c5658: setMode's refusal is pushed beside the design's, a second refusal check before the plan finishes, the duplicate axis tested, the JS test waits on its refusal line

**Nothing is blocking.** HEAD fixes the reported token-fold failure; I found no path that returns a new plan with a refused design.

1. **Should-fix follow-up — invalid axis declarations can still hide their own refusal (pre-existing).** An invalid `under scheme=light` fails parsing, so the new binding never runs and `setMode` still reports “declares no axes.” A duplicate axis with a value appearing only in its second declaration also triggers `tamber-mode`: [set_mode_args](/tmp/rv-axes-1/contract/lower/src/theme.rs:899) selects the first declaration, and the fatal [`?`](/tmp/rv-axes-1/contract/lower/src/stmts.rs:117) discards the accumulated duplicate-axis error. Preserve accumulated errors or suppress dependent checks when axis declarations are invalid. These deserve regression cases, but do not block this narrower fix.

2. **Nit — replace the fixed sleep in the JS test.** The [500 ms delay](/tmp/rv-axes-1/host/web-js/dev-builds.test.mjs:195) should use a diagnostic/completion condition, consistent with the repository’s testing guidance.

The other requested checks look sound:

- Parsed axes can satisfy a mode/pin check despite a refused design, but [accumulated errors prevent plan completion](/tmp/rv-axes-1/contract/lower/src/lib.rs:638). Plan axes are emitted only after folding succeeds.
- Folding and parameter expansion preserve axis names, values and ordering, so the later AXES assignment remains consistent.
- `declare_theme` clears AXES before the no-design return; lint’s `bind_axes` also replaces it with an empty list. Server paths use checked designs or successful plans and do not independently consume stale AXES.
- Keeping checks for valid axes during a token-only refusal is reasonable; blanket suppression is unnecessary.
- The Rust test covers the refusal, pin validation and repaired runtime behavior. The JS test waits for a completed build and checks the served edit and refusal. A same-thread design→no-design case would strengthen lifetime coverage.

Read-only review only; no files changed, builds or tests run.
