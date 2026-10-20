# Code review: stage 12 D2, the slot check on writes, round 3 re-check 4 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `48296aa529`.
- **Method:** one brief (sha256 `06fc440c59cd33c92b34639188cd3b9776229fd136f3155b6eab0cf5124d0ae5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** None.  
**Should-fix:** None.  
**Nits:** None.

- [tamber_walk.rs:355](contract/cli/src/tamber_walk.rs:355): Legal Tamber name spellings now remain intact, including trailing `-`, matching `is_ident`/`is_name`.
- [tamber_serve_slots.rs:267](contract/cli/tests/it/tamber_serve_slots.rs:267): The adapted expression test requires one server refusal before comparing compiler/server messages exactly at line 273; equality cannot pass vacuously.
- [tamber_serve_slots.rs:112](contract/cli/tests/it/tamber_serve_slots.rs:112): Attribution now checks the app design, consistent with C2 final.

The permitted test could not run: the read-only sandbox denied Cargo’s target-directory write.
