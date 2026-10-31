# Code review: Linux focus on a control a commit disables, round 3-recheck3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `16adda3519`.
- **Method:** one brief (sha256 `fb554e815fcb1f17806d996eb9d0a6acf1c306f7b481095e1348443a28ffd428`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

No blocking findings. I confirm approval of `16adda3519`.

- **(1)** [Enter handling](host/linux/src/presenter/field.rs:1039) now blurs between change and submit. `commit_text` clears the edited marker first, preventing a duplicate change. The test checks `cbs` and cleared focus.
- **(2)** [Hover dispatch](host/linux/src/presenter/events.rs:703) now checks after every leave/enter event. The test covers an outer hover disabling the control before an inner hover re-enables it.
- **(3)** The GPU batching gap can remain a stated follow-up. Those loops are unchanged; transient disable/re-enable remains an existing limitation. Final servicing still catches a control left disabled. Closing the gap requires bringing presenter focus handling into that dispatch boundary.

Tests inspected, not rerun: this read-only checkout has no build artifacts. `git diff --check` passed.

APPROVE
