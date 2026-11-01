# Code review: Linux focus on a control a commit disables, round 3-recheck2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `28d6bc52c1`.
- **Method:** one brief (sha256 `95188dbb34449021275fe8ac172c3f95e6a3462b4386dbd63d30d58037a832b0`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

The `set_control_value` fix is correct, but I revise approval. Three paths still have the same gap:

1. **Enter: `change` → `submit`** — [field.rs:1039](host/linux/src/presenter/field.rs:1039). `commit_text()` dispatches without servicing; `submit_event()` services only after submitting. If change disables the focused field and submit re-enables it, blur never runs and focus survives. Service the change before submit.

2. **Batched hover events** — [events.rs:709](host/linux/src/presenter/events.rs:709). All leave/enter handlers run before one `after_commit()`. Entering nested boxes whose handlers disable then re-enable a focused input skips its blur. Run fixup between dispatches.

3. **Batched GPU messages** — [surfaces.rs:807](host/linux/src/surfaces.rs:807), also the restore loop at line 888. Multiple message handlers commit before presenter servicing. Two messages that disable then re-enable a focused control likewise preserve focus without blur. These dispatches also need intervening fixup.

`deliver_picker` dispatches one event and services it immediately. Tests were not rerun: this read-only checkout has no built test artifacts.

CHANGES
