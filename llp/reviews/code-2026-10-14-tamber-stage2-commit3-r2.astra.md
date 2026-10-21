# Code review: LLP 1121 stage 2, commit 3, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `af9199d98092eb367879ae6d12c32971a36c51e4dbd14faf5d1c7c4b1fac9952`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 2's commit (72790081)

---

- **MAJOR — [runner/src/instance/theme.rs:267](/tmp/rv-s2c3/runner/src/instance/theme.rs:267):** An implied row’s cache survives an authored override. With ancestor `color="ink"` and a pinned boundary carrying `color=(override ? "#ff0000" : none)`, toggling false → true → false clears the boundary’s colour. Because `want == implied[k]`, restoration is skipped, so descendants inherit the ancestor’s slice instead of the pinned slice. The token-index entry is also lost. Reapply the implied row when the boundary’s own binding clears or becomes invalid. The added test places that binding above the boundary, missing this case.

- **MINOR — [runner/src/instance/theme.rs:210](/tmp/rv-s2c3/runner/src/instance/theme.rs:210):** When the tuple changes, `||` skips `mem::take`, leaving `colour_moved` set. A pinned container with its own themed colour therefore forces another full descendant refresh on its next unrelated update. Consume the flag unconditionally. This is a redundant later traversal, not a permanent every-update loop; the create reset and unchanged-binding guard prevent that.

`around_moved` handles the nested-pin case correctly. The recomputed schema digest matches.

HEAD’s regression tests were not run: the cached binary predates the fixes, and rebuilding requires writes. Worktree unchanged.

VERDICT: fix
