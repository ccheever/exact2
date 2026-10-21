# Code review: LLP 1121 stage 2, commit 5 (the Apple hosts and a theme fixture), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `199d7a19dd0cd19103cf1d863a89d6d223bafcfe4c159f6b47f08ff88d944008`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both, MINOR).
- **Disposition:** fixed in round 1's commit

---

The Apple host does not need new code for an app-axis slice. A `setMode` or a pin change rewrites ordinary colour rows, and the existing presenters read those rows again. The fixture's own driver check is wrong for SVG paint.

Re-sent rows reach the screen through the paths D7 names:

- Layers, gradients, shadows, and filters re-read the new style. `applyStyle` displays again; shadow casters compare `shadowColor`; `BoxFilter.set` replaces the program and renders it; a fixed gradient is cached by the source value and the current appearance, so a new slice misses the cache.
- A `light-dark()` value stays a pair on the wire (`push_color_value` in `host/apple/src/style.rs`). Trait changes re-resolve it, including native buttons: iOS keeps the current traits on the button face and reconfigures when they change.
- SVG elements are not views. A touched shape dirties the scene, and the scene is sent again when its paint string changes.
- Text style and paragraph ops clear `cachedTextSpec` and the raster key. The raster key includes the resolved colour.
- Paint motion retargets only properties named by `transition` or `animation`. `sync_paint` runs on the commit receipt before `present`, and a settled transition stops covering the row. With no transition, the row shows directly.
- `colors.rs` is the role and platform table. An app-axis slice is a `Fixed`, `LightDark`, or `Wide` row, so it does not sit in that table. `tamber-role-axis` refuses a role that varies by an app axis, and `ThemeRoles` stays one dynamic colour. The fixture's `AccentColor` is the constant `#1d9bf0`.

The fixture is wired correctly. `apps/theme-fixture/data` and `apps/theme-fixture/apple` are workspace members, `app.json` matches the other Apple fixtures, and `build.rs` watches `app.tamber`: `rerun_if_changed` prints every absolute path from the source graph, and that graph adds `app.tamber` beside `app.contract` after the contract path is canonicalized. The element lint attaches each app's design before checking.

What the driver cannot show is an SVG paint change. The commit says that under `prefer color-scheme dark` and `prefer mode dark-style:dim`, the circle's paint changes to Dim's. The circle is `fill="accent.fill"`, and that token is the constant `#1d9bf0` with no `when`. Dim and Black leave it the same blue. Ground, card, border, the `surface.card` gradient stop, the `border.hairline` shadow mix, and inherited text do follow the axis. The light halves match, so the light scheme stays put, and the pinned previews keep their own tuples. There is no `transition`, no CSS `filter`, and no role or platform colour inside a gradient, filter, or scene, so those three D7 paths are not in this fixture. The host code for them already follows a new row.

VERDICT: fix (MINOR apps/theme-fixture/app.contract:26)
