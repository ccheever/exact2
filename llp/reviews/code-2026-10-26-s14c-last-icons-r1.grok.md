# Code review: stage 14 C's last icon moves, round 1 of 2, 2026-10-25..26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `5bbcabe040`.
- **Method:** one brief (sha256 `3404d415a88970f6d60f8ffa1f67d0217baa93710e12ff4d044cb9e0238829af`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The review is still open on placement, colour, and whether the tests match their claims. I’ll confirm those against HEAD, then write the findings.No qualifying defects. The move matches the ruling.

The eight icons keep the old geometry. Each SVG is `viewBox="0 0 24 24"` with the replaced `d` byte for byte (`packages/actions/assets/`, `packages/cards/assets/`, pinned in `contract/cli/tests/it/library_last_path_icons.rs:74`). The boxes stay the old square tokens (`actions.compose-icon` 24, `cards.visit-icon` 20, `cards.badge-size` 12, `cards.profile-badge` 18). Omitted `object-fit` is drawn as aspect-fit (`host/web-js/symbols.js:202`, `host/apple/Sources/ExactCore/IconPath.swift:54`), which on a square is the old `xMidYMid meet`. Badge layers are the same absolute box, ground then check (`packages/cards/common.contract:55` and `:65`), so they stack and align as the old circle/path then check did. A 0.2 grid over each shape shows even-odd and nonzero filling the same samples. The holes wind opposite their outlines.

`cards.verified` is the old circle. `M12 .5a11.5 11.5 0 1 1 0 23 11.5 11.5 0 1 1 0-23Z` is two semicircles of `cx=12 cy=12 r=11.5`; the chord is the diameter, so the large-arc flags do not change the halves, and both sweeps go the same way. Its bounds are `(0.5,0.5)–(23.5,23.5)`. The kernel accepts the glued `0-23` (`kernel/src/svg/path.rs:49`).

Both badge colours follow their params. Ground is `color=ink`, check is `color=mark`, which is `text_color` and the symbol's `currentcolor` (`contract/lower/src/symbols/ink.rs:82`, `host/apple/Sources/ExactKit/Affordances.swift:29`). `cards.badge` is `light-dark(#006aff, #0f73ff)`; `cards.badge-mark` and `cards.visit-ink` are fixed white (`packages/cards/cards.tamber:17`). ProfileHeader passes `param.badge` and `param.badgeMark`; PostCard passes the tokens (`packages/cards/post-card.contract:100`). Restyle reads those image leaves: `scripts/s13c-restyle.mjs:82` and `:89`.

The label sits on the container. Each badge column is `role="img"` with its old aria-label; both images are `aria-hidden` (`packages/cards/common.contract:48`). iOS exposes a labelled `role=img` and not an unlabelled image (`host/apple/Sources/ExactKit/Accessibility.swift:55`, `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1159`). macOS makes that role one image and returns no children (`Accessibility.swift:369`, `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:401`). The web images get `alt=""` (`host/web/src/element.rs:1366`). Toolbar buttons and the visit row keep their own names.

Nothing still calls the deleted functions. `iconGallery`, `iconCamera`, `iconGif`, `attieIcon`, and `ToolGlyph` are gone from packages, docs, gallery, and gate scripts. `BarItem` and `HeaderButton` are outside this commit and still take the caller's `d`. `actions/common.contract` is deleted and was not exported. Library-gate uses the components.

The tests match their claims. `library_last_path_icons.rs` checks the seven `d` strings against the deleted sources, checks the arc numerically against the circle, checks the deletions in both packages, and checks that library-gate's plan keeps all eight nonzero with `viewBox` `0 0 24 24`. `every_package_icon_svg_is_a_nonzero_template` refuses an even-odd package SVG. The compat test is meaningful: `library_gate_keeps_mains_compatibility_id` (`bake/src/icon_rules.rs:231`) requires `cards.verified-check` in the plan, requires `even_odd` false for the whole plan, and requires the id and inputs to match the id computed with the fill-rule input skipped, with no `iconFillRules` key.

Verdict: stage 14 C's last icon move is correct and complete.
