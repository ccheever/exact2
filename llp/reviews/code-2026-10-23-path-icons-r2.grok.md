# Code review: x2/path-icons: the packages' nonzero path icons as icon tokens, round 2 of 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `7e2b420470`.
- **Method:** one brief (sha256 `36c36fab1320b1bc3ba5af14f9f97293d61196dbd4c4f9ca97880e695032dfb1`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The Astra path fix and the gate sweep are already in hand. I'll finish by comparing the fifteen hardcoded path strings to the pre-move glyphs, then check that the colour test actually builds a path icon.The colour test builds a path icon through `presenter`. Next I’ll compare each hardcoded `d` to the glyph it replaced, and re-read the alert restyle paths.The colour test is a real path icon. I’ll compare the fifteen `d` strings to the old glyphs and check the call sites for anything the first pass missed.The Astra fix is right, and the rest of `HEAD~5..HEAD` matches the plan. No findings.

**Astra.** `ProfileHeader.alert` now reads `text_color` on the image leaf `2/2/{0..3}/0` and on the label `2/2/{0..3}/1` (`scripts/s13c-restyle.mjs:87`). The header's third child is the padded column; its third child is the alerts row; each alert button's children are the image, then the label. `sample-ph-alerts` still has four alerts (ban, eyeslash, warning, info). `color="param.alert"` is the `text_color` row. The old extra `/0` was the deleted `<path>`. `cards.alert-icon` still reads `height,width` on that same image (`s13c-restyle.mjs:143`).

**Gate sweep.** No other script still reads into a moved icon. The remaining `fill` paths are the icons the plan left inline: BarItem, HeaderButton, ComposeFrame's gallery/camera/GIF, and ProfileHeader's two-colour badge. `scripts/stage12-gate.mjs:704` uses `ar-saved`. Nothing in `scripts/` still says `sample-ar-saved` or names a deleted path function.

**Geometry.** All 15 hardcoded `d` strings in `contract/cli/tests/it/library_path_icons.rs` match `packages/{cards,actions}/common.contract` at both `11fc03c159^` and `0277d04737`, and each SVG is exactly `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="…"/></svg>` with no trailing newline. No `fill`, no `evenodd`, no `transform`. The old `Glyph` was a 24-box, `width=size` `height=size`, no transform, default nonzero winding. Call sites keep that square box (`flex-shrink=0`, equal width and height). `IconPath.placed` treats the default `fill` like `contain` for a square box, so the omitted `object-fit` is the old viewport.

**States and access.** Liked and saved still swap outline/filled and colour; reposted still recolours the same path. PostCard's pin/repost ternary and `alertIcon`'s warning/eyeslash/ban/circle-info chain are unchanged, including the `info` fallback. Every moved image is `aria-hidden=true`. Buttons that hold them still have `aria-label` (like and save still have `aria-pressed`). The context and reply icons sit beside visible text, as the old decorative `svg`s did.

**Tokens, rename, leftovers.** Icons are `cards.*` and `actions.*` with `icon(svg "….svg")` and no `sf`. An app line of the same full name wins (`packages.rs` `own_icons`); a package SVG is read from that package's `assets/`. The icon-skip regex drops `icon(` and `icon (`. `ar-saved` is the new name in the app and in `ACTION_ROWS`, and the gallery expectation is 25 cells because `saved` is now a style-row variant. `sample-ar-replydisabled` stays prefixed. Deleted functions and `Glyph` are gone from both packages; the only `component Glyph` left is Harmony's emoji component. Even-odd icons and `VerifiedBadge` stay inline. The Rust test's `d` bytes, the tamber lines, and the plan's `view_box` / empty `sf` are what the comments claim. The Mac test builds a path icon (`symbolPath` set) and checks secondary-label `text_color` against a tint-only case that expects the text colour.

I did not run the tests.

Verdict: round 2 is correct and complete; ship it.
