# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) round 3 (abcd157b9), 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `257f05878f19f0f8b92c5d2ea45a4eb408de1c0116e169c3d5ad4a3c949e92a7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MAJOR is taken: a passed-over compiled answer is not reused at equal arguments; the test covers equal and changed arguments.

---

- **MAJOR — [runner/src/runner/settlement.rs:507](/Users/admin/x2p2/apps/exact2-d4/runner/src/runner/settlement.rs:507): compiled-value reuse bypasses suppression when arguments remain equal.** In `plan_switching()`, change account’s baked value from `"x"` to `""`. Setting `use=true` then marks `consume` as a store reader without changing its arguments or store revision. The earlier reuse branch retains the compiled `Held`, bypassing both `secrets_unseen` and the third commit’s equality guard. The resource remains non-placeholder and non-pending; a distinct declared placeholder would never appear. Exclude eligible compiled fallbacks from reuse and cover this equal-argument variant.

The snapshot construction, reload, mirror, restore/checkpoint paths and placeholder-row settlement ordering otherwise look consistent with the amendment.

All 55 focused tests passed using the existing runner binary. The uncovered variant above was traced through the code; no files were changed or rebuilt.

NOT READY
