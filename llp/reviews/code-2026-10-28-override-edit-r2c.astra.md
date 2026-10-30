# Code review: design tool: a package token's Set for this app keeps its value edit the app's line (D4 batch 2 #1), round 2 re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c8f75be923`.
- **Method:** one brief (sha256 `3a06b53acb649cd69e23f26e402b938aeef32f0530db025b4b7c182a109eb352`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD — [apps/design-tool/app.contract:757](apps/design-tool/app.contract:757):** `file == ""` does not cover every way to open the app’s design. The picker also lists its pathname (`app.tamber` in this fixture), and `chooseFile` stores that unchanged. Selecting it and submitting `reading.measure` now proposes `add-token`, which the server rejects for package/base names. Normalize that selection to `""` or compare the actual design identity. The D4 regression case should cover selecting the app’s explicit pathname.

No separate regression found from clearing `selToken`: submitting the form already replaces the active draft, and clearing the selection removes the read-only branch guard.

Confirmed with in-memory source checks; the full integration test was not run under read-only permissions.

VERDICT: findings
