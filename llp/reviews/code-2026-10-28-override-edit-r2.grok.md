# Code review: design tool: a package token's Set for this app keeps its value edit the app's line (D4 batch 2 #1), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `6c3fe3210d`.
- **Method:** one brief (sha256 `c5764e719da72e45f6a668d035a67da12e8cfbf2461a0fcd990758dea994956b`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

**SHOULD** `apps/design-tool/app.contract:755` — `set-token` is chosen from the open file's rows and then written to that same `file`. Another open design file's grid still contains every package token (`fold_with` joins them) and every unset base name (`of == "base"` in `apps/design-tool/base.ts:48` and the merge in `apps/design-tool/app.ts:813`). Those rows are not lines of that file. `set-token` is the app override only when the op's file is the app design (`contract/cli/src/tamber_packages.rs:477`); on the other file the server answers `no token`. `setForApp` switches to the app design first (`file = ""` at `apps/design-tool/app.contract:769`). `addToken` does not. `of` cannot tell a line of the file you are editing from a base or package row only shown beside it.

**SHOULD** `apps/design-tool/app.contract:760` — Once the kind is `set-token`, it is the cell editor's `branchOp`. Apply stays off while that same token's read-only cell is still selected (`apps/design-tool/app.contract:310`: `branch.readOnly != ""` and `editTarget == branch.token`). That is the package token's own row and an unset base row. The proposal itself is the valid app override, and nothing in the dock says why Apply is disabled. `setForApp` closes that cell first (`selToken = ""` at `apps/design-tool/app.contract:776`) so the guard does not treat this edit as the read-only branch. The new clear covers `draftFor`, `rawFor`, and `partFor` only.

Checked and not filed:

- On the app design, `sourceOf` sets `of` to `"base"` for a base row and for the app's override of one, `"package"` for a package token's own row and for the app's override of one, and `""` for an app-only token. That matches what `tamber_packages.rs` accepts.
- `n` is bound as a local before the filter. Each filter appends its item and index as later locals, and the inner parameters are `t` and the index, so they do not hide `n`.
- The cell and the form share one edit. Text typed in the cell lives in `editValue` (and `partTexts` for the cut). `addToken` replaces `editValue` with the form value; the three clears are what stop that replacement being judged a stale branch draft. `partTexts` and `rawBase` stay set and are unread once `partFor` and `rawFor` are empty. A committed cell value is left as it was.

The new test drives Set for this app, which already clears the selection and stays on the app design, so it does not reach either gap.

VERDICT: findings
