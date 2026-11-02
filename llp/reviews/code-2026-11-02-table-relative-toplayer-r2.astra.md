# Code review: a popover table row's relative position, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `dc2d098ac5`.
- **Method:** one brief (sha256 `4e29dd3ecdcd7f43f46ad758c4d93670dba86e22eb1136a7c2b931a8fc0d729b`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** Astra's MAJOR (a popover row laid out before any viewport, with no block of its own) is put to the lead; see the landing note.

- **MAJOR — [kernel/src/layout/table.rs:192](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/layout/table.rs:192): flattened cells lose positioning before a viewport exists.** On a fresh kernel’s first `Offer::MAX_CONTENT` layout, `imply_blocks()` deliberately gives popovers no block ([block.rs:365](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/kernel/block.rs:365)). Consider a positioned table containing `tr popover="manual" position:relative; left:10px; top:5px`. `top_layer::apply()` makes the row absolute, and without a block its insets remain active in the engine. However, publication discards the row’s engine origin for flattened children and uses `relative()` instead. This change returns zero, leaving cells behind while the row moves; the parent propagated `(10,5)`. This is a source-derived regression, not an executed reproduction.

  **Fix:** retain `layout_position()`, but propagate the engine’s resolved displacement for absolute flat rows/groups without an independent block. Add a fresh-kernel `MAX_CONTENT` regression and its transition to a definite viewport. The new fixture meaningfully exercises an implied top-layer block, but establishes a viewport beforehand and therefore misses this path.

No additional introduced issues found in ordinary rows, platform starts, authored sticky, anonymous tables, clearing `popover`, or Swift parity. Absolute classification agrees with [top-layer styling](https://drafts.csswg.org/css-position-4/#top-layer-styling); the issue is descendant translation. Existing RTL/percentage offset defects are unchanged and already recorded in `QUEUE.md`.

Validation: Cargo was blocked by sandbox write restrictions. Bun’s two repository checks passed; its scanner self-test failed because temporary-directory creation was denied. Worktree unchanged.

VERDICT: CHANGES
