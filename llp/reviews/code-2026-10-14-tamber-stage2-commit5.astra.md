# Code review: LLP 1121 stage 2, commit 5 (the Apple hosts and a theme fixture), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `199d7a19dd0cd19103cf1d863a89d6d223bafcfe4c159f6b47f08ff88d944008`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both, MINOR).
- **Disposition:** fixed in round 1's commit

---

Two MINOR findings:

- **MINOR — [app.contract:26](/tmp/rv-s2c5/apps/theme-fixture/app.contract:26): SVG recolouring is not exercised.** The circle uses `accent.fill`, which is constant `#1d9bf0` in [app.tamber:21](/tmp/rv-s2c5/apps/theme-fixture/app.tamber:21). Neither scheme nor mode changes its paint, so this fixture cannot demonstrate the SVG change claimed by the commit. Use an axis-varying fill.

- **MINOR — [app.contract:21](/tmp/rv-s2c5/apps/theme-fixture/app.contract:21): several claimed refresh paths are absent.** There is no `filter`, no `transition`, no contrast-dependent token, and the preview pins never change. Add cases that exercise filter colour replacement, transition retargeting—including an interrupted transition—and changing a mounted subtree’s pin.

I found no stale-value defect for re-sent rows in the Apple paths reviewed. Text invalidation, grouped-cell/control refresh, SVG caches, and transition targets account for changed values. `ThemeRoles`’ boot-only table is consistent with the compiler’s `tamber-role-axis` rejection.

Manifest validation, workspace/lock membership, and diff whitespace checks passed. `build.rs`’s compiler helper includes `app.tamber` through the source graph. Native builds and execution were not verified: this copy is read-only and has no build artifacts.

VERDICT: fix
