# Code review: design tool: a package token's Set for this app keeps its value edit the app's line (D4 batch 2 #1), round 2 re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `c8f75be923`.
- **Method:** one brief (sha256 `3a06b53acb649cd69e23f26e402b938aeef32f0530db025b4b7c182a109eb352`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

**SHOULD** `apps/design-tool/app.contract:757` — `theirs` is true only when `file == ""`. That sentinel is the app's design for the initial state, for `setForApp` (`:775`), `backToBase` (`:798`), `editFromSelection` (`:601`), and for the menu option "The app's design". `chooseFile` (`:999`) also sets `file` from the menu at `:1148`, and that menu lists every non-contract path in `hello.files`. The server's file list starts with the design file itself (`contract/cli/src/tamber_serve.rs:668`), so once a second `.tamber` file exists the menu offers that path beside the sentinel.

Opening the path is the same design. `loadDesign` treats `!file || file === designFile` as the app's design and still returns package and base rows (`of` is `"package"` or `"base"`). `opOf` puts that path on the op (`apps/design-tool/app.ts:165`). `parse_ops` (`contract/cli/src/tamber_ops.rs:36`) and `ops` (`contract/cli/src/tamber_packages.rs:477`) then treat it as the design file: `set-token` of a package or base name is the app's override, and `add-token` of that name is refused (`tamber_packages.rs:504`). With `file` equal to the path, `theirs` is false, so the form proposes `add-token` and Apply stays off with that refusal. Before this commit the row check alone proposed `set-token` on this path, and that op would have been accepted.

The new case opens `brand.tamber` (`scripts/design-tool-scale.test.mjs:197`) and leaves the design's own path (`app.tamber` in that fixture) unexercised.

Clearing `selToken` when `theirs` is sound. A read-only cell keeps its value field, parts, pin, rename, and remove disabled (`app.contract:1301`, `:1338`), so it holds no typed draft. An editable cell's draft is the edit fields, and `addToken` already replaces those on submit (`:762–768`), the same replacement `setForApp` makes. `selToken = ""` only drops the selection, so `loadBranch` comes back empty and the guard at `:310` (`branch.readOnly != ""` and `editTarget == branch.token` and `editKind == branchOp`) no longer matches. The D4 case drives that sequence: the read-only cell, then the form, then Apply, with `selToken` `""` and the override line written.

VERDICT: findings
