# Code review: the terminal's autofocus taken once (QUEUE), round 1 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `bf9b4b1b18`.
- **Method:** one brief (sha256 `57704b8a4d700ac25736284119ebf51e40d681cf52e9708e78ee1d640d9a270a`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a small host fix, the lead's 21:00Z task). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with nits (2 NIT).
- **Disposition:** both taken (the commit message's "Round 1").

**Verdict: APPROVE WITH NITS. No BLOCKING or SHOULD-FIX findings.**

1. **NIT — [host/terminal/src/host.rs:787](host/terminal/src/host.rs:787):** “On insertion” slightly overstates the behavior. `focusables()` excludes disabled and `display:none` controls, so these remain unprocessed. A control inserted while another field holds focus can therefore autofocus later when enabled/shown after a blur. Linux retains this limitation too; HTML consumes candidates during its flush even when they cannot receive focus. Clarify the comment or document this boundary. [HTML autofocus algorithm](https://html.spec.whatwg.org/multipage/interaction.html#the-autofocus-attribute)

2. **NIT — [host/terminal/tests/it/screen.rs:979](host/terminal/tests/it/screen.rs:979):** The test correctly checks boot autofocus, a fresh arrival after blur, and suppression of subsequent refills. Both candidates arrive with nothing focused, however. Add a case mounting an autofocus field while another control holds focus, then blur and commit: the new field must remain spent. The current test uses synchronous rendering and explicit frame counters; I found no timing-dependent flakiness.

Static inspection found no introduced regression in route replacement that removes the focused node, keyed rerenders, or dialog/popover opening, reopening and saved-focus restoration. Layer opening selects autofocus independently of the processed set. Stable node keys preserve consumption across rerenders; generation-checked pruning removes destroyed nodes, bounding entries by live processed nodes. HashSet capacity retains its historical peak.

`cargo test -p exact-terminal --test it` could not start because the read-only sandbox denied creation of Cargo’s temporary target directory. Working tree unchanged.
