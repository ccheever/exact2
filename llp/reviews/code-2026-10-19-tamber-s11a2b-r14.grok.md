# Code review: LLP 1121 stage 11 A2 part 2, package designs on the wire (tamber/1.14), re-check 11 after round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents`, a throwaway worktree at `1fc2382b64`.
- **Method:** one brief (sha256 `6d43f30fe180f319bc1b765ac0c4d98a89c64b7d798cd889a876f3e4bef3507c`), blind to the other reviewer; the commit as amended after the previous pass. Past round 3, narrow checks under the lead's 14:09Z cap and theme2's 20:38 and 21:32 rulings (only a write path to a package file, as 21:32 defines one, blocks; watcher-late-push and out-of-definition layouts go to QUEUE). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (a BLOCKING finding).
- **Disposition:** each finding taken in the next amend, as the commit message's review notes list; layouts outside theme2's 21:32 definition queued (QUEUE: "Package-file layouts outside the read-only guard's definition").

## BLOCKING

**1. `node_modules` nested inside a package is never scanned.** `contract/cli/src/tamber_packages.rs:177`

The walk records a `node_modules` directory and does not enter it, then only canonicalizes that directory's direct entries (and one `@scope` level). A link at `node_modules/<pkg>/node_modules/@acme/reading` → `vendor/reading`, with no `package.json`, is invisible. `app.json` `"tamber": "vendor/reading/reading.tamber"` still starts (the file canonicalizes inside the app), and `set-token` writes that file. The same walk skips symlink directories (`file_type().is_dir()` does not follow them, line 175) and `.git`, `target`, and `.exact`, so a `node_modules` only reachable those ways is missed too.

**2. Contract ops never use that check.** `contract/cli/src/tamber_serve.rs:886` and `contract/cli/src/tamber_contract.rs:79`

`set-row`, `set-state`, `add-param`, and `remove-param` are split out before `tamber_packages::ops`. They are admitted by `owned()`, which only canonicalizes the app's own top-level `node_modules`. A `.contract` under a directory linked from `src/node_modules` or from a parent's `node_modules` (no manifest), imported with a relative `use`, is still an app source. Those ops write it. Undo then calls `read_only` and refuses, so the package file is already changed and the server will not put it back.

**3. Crash recovery renames without `read_only`.** `contract/cli/src/tamber_serve.rs:481` (called from `start` at line 359)

A journal committed after the check and before the rename is finished on the next start. `files()` still lists the app design, so a link added in that window (`theme/` linked from `node_modules/@acme/theme`, the re-check 8 case) is renamed into. Undo is refused; recovery is not.

Watcher timing is unchanged and stays out of this bar.
