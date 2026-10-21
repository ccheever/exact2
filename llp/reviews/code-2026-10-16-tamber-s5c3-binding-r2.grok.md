# Code review: the binding core (Tamber stage 5, commit 3), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the first round over the change.
- **Method:** one brief (sha256 `600c538e2f867a29d54216a573fce0cee66fc2c8bb2f1bccc4122dbf897fd136`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the second-round commit.

---

I'll review the binding-core range read-only: the round-1 fixes first, then the whole `HEAD~1..HEAD` diff against the stage 5 brief and LLP 1121 D12.The review stays read-only. I'll read the brief, the design, and the `HEAD~1..HEAD` diff before judging whether the round-1 fixes hold.Nothing is blocking. The round-1 fixes hold, and the rest of `HEAD~1..HEAD` does not add a defect that should stop this commit.

- **`inputs` and `set_token`.** `set_tokens` bumps `design_moves` (`runner/src/runner/theme.rs:63`) on a single edit, a live batch, and a role edit. `inputs` hashes that counter, not the design digest (`runner/src/runner/tamber.rs:116`), so a call no longer walks every token and a `set_token` is no longer invisible. Repeating `setMode` with the mode already set does not bump it.
- **`color-scheme` above a place.** `dark` uses the node's computed scheme, which inherits, and otherwise the window (`tamber.rs:93-102`). A boundary that flips light to dark changes `inputs` for a descendant; the window's scheme does not move when the place has its own.
- **Recorded names.** `color` parses inside `theme::recorded` (`tamber.rs:75-82`). That saves and restores the runner's name bits instead of `take_named`, so a binding read does not wipe the bits a row update still owes.
- **A refused value.** A name the plan lacks and a live value that does not parse as a colour both return `None`, with different journal lines, each once (`tamber.rs:65-67`, `84-86`). `null` on the Apple export is the same pair of cases.
- **The export.** A non-integer, out-of-range, or not-live `view` is refused; `0`, missing, and JSON `null` stay the root (`host/apple/src/tamber.rs:18-24`). One non-string in `names` refuses the request (`tamber.rs:26-30`). View ids start at 1, so `0` does not hide a real node.

Bold Text and an editor swap are still outside this commit, as commit 3 stated; they belong to the native binding. `inputs` can also move when a batch edits a token that binding never reads. That over-invalidates on purpose. The new tests cover a token edit, a scheme above the place, the refused-value line, and an out-of-range view.
