# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 18 after round 3, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `394134ab86`.
- **Method:** one brief (sha256 `c56c33a08bf8e5840c112d33620e7e66a36eb9bb935ae857e72ccdbca229f812`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

**BLOCKING — `.exact` bypasses definition (c).** Its new check calls only `in_package`; the `package_roots` check applies only to requested files. [Guard](contract/cli/src/tamber_packages.rs:133)

Concrete reproduction, using the first setup in the [existing resolved-root test](contract/cli/tests/it/tamber_serve_packages.rs:600):

1. Replace `vendor/p` with `.exact`, including both `node_modules/p` per-file symlink targets.
2. Keep `.exact/package.json` as `{}` and the Contract importing `P` from `"p"`.
3. Omit the `app.json` design override, leaving `app.tamber` as the app’s design.
4. Start the server, obtain `hello.ok.files`, then transact `set-token brand = #654321` with those digests.

The resolver records real `app/.exact` as package `p`’s root. However, `in_package` finds neither a named manifest nor a directory link covering the journal; the subsequent root check examines only `app.tamber`. The transaction therefore reaches [commit](contract/cli/src/tamber_serve.rs:1157), which writes its temporary files and journal inside `.exact`—package files under **(c)** during an ordinary transaction.

Source-traced reproduction; not executed because it would write files. No files changed.
