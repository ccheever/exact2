# Code review: design tool: a package token's Set for this app keeps its value edit the app's line (D4 batch 2 #1), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3db023ed51`.
- **Method:** one brief (sha256 `7b541e89a01ca2b1cef59706bddd6c484baf0852559c6cf71ebde0b526258695`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the design-tool fix in commit `3db023ed51` read-only, starting with the review skill and the diff.This is a focused commit review with a fixed report format. I'll read the review-agent instructions and the commit before judging the fix.The review target is one commit and a specific stale-state question. I'll inspect the diff and the token-edit paths around it.**SHOULD:** `setForAppName` is a session latch, so a later Add of that name is `set-token` after the name stops being a package's or the base's — `apps/design-tool/app.contract:753`

`setForAppName` is written only in `setForApp` (`app.contract:768`) and is never cleared. Apply (`app.contract:874`), Cancel (`app.contract:840`), and a later cell or panel edit clear `editKind` and leave the name in place, and the Add form keeps `newName`. `addToken` then treats any submit of that trimmed name as `set-token` for the rest of the session.

While the name is still a package's or the base's, that is the right operation. An app override of it is still theirs: `add-token` is refused (`contract/cli/src/tamber_packages.rs:504`) and `set-token` updates the app's line. Removing the override (Back to the package or the base) leaves the name theirs and undefined by the app, and `set-token` is rewritten into the override line (`tamber_packages.rs:487`). Those two cases the question names are sound.

It is the wrong operation once the row is no longer one of those. `sourceOf` (`apps/design-tool/base.ts:48`) sets `of` to `"package"` or `"base"` for the read-only row and for the app's override, and to `""` when the token is only the app's. After the package drops the name:

- The app does not define it either. `set-token` fails with `no token` (`tamber/src/edit.rs:1200`), the proposal is not ok, and Apply stays disabled, so that name cannot be added until the tool is reloaded.
- The app still has the line, now a plain app token (`of == ""`). `set-token` proposes a value change of that token, and Apply can write it. `add-token` would have been the duplicate refusal (`a token is declared already`).

Decide it from the live row: `set-token` when some token in `design.tokenGroups` has this trimmed name and `of == "base"` or `of == "package"` (`shapes.contract:597`), otherwise `add-token`. That matches the server's `theirs` check, including an override, and it follows a package token disappearing. The same check also covers a different package or base name typed into the form. The last clicked name does not.

The new test does prove the red and the cell edit-again path. On the parent, `addToken` always assigns `add-token`, so the first `state` expect (`scripts/design-tool-scale.test.mjs:185`, `set-token` / `reading.measure` / `700px`) fails. The refused proposal would fail the next expect as well. The second half (`:189`) types `branch-value`, which commits through `change=cellText` to `set-token`, and checks the `720px` line. That is the override's cell, and it does not submit Add again, so it stays green if the latch is wrong.

VERDICT: findings
