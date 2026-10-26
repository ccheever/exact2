# Code review: x2/path-icons: the packages' nonzero path icons as icon tokens, round 1 of 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `e58cbaf963`.
- **Method:** one brief (sha256 `3eb8cfe98044f486fb6fae59158759a008a6d56fd9dbd24e835073220a5898c6`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The earlier pass closed the geometry, accessibility, and test checks. I'll confirm the four commits and the load-bearing call sites, then write the review.The range is those four commits, tree clean. I'll read the contract, test, and restyle diffs before rating anything.No findings.

The four commits (`11fc03c159`, `13a3aeef6b`, `0baaf70fa5`, `e58cbaf963`) do what the ruling asked. All 15 nonzero one-colour icons are package icon tokens, the old path functions and both `Glyph` components are gone, and nothing in the packages, the gallery, the gate, or the docs still calls them. Even-odd marks (composer tools, `BarItem`, `HeaderButton`, the Attie mark) and the two-colour badges stay inline paths, which is what the ruling left in place.

Geometry matches the old `Glyph`. Each asset is one `<svg viewBox="0 0 24 24"><path d="…"/></svg>` whose `d` is the old function string, with no fill, fill-rule, transform, or stroke. `Glyph` applied no transform of its own, and the SVG default is nonzero. `library_path_icons.rs` checks those bytes, the `icons` line, an empty `sf`, and plan view box `0 0 24 24`. Every call site sets `width` and `height` to the old `size` and `flex-shrink=0`. On a square box that is the old viewport: a path icon’s default fit meets a square view box into `min(width, height)`.

Accessibility and toggles are unchanged. Every moved image keeps `aria-hidden=true`. Button and alert labels, and `aria-pressed` on like and save, are untouched. Liked, reposted, and saved still swap both the symbol and the colour; `big` still picks the larger box; PostCard still chooses pin versus repost; the alert enum still maps warning, eyeslash, ban, and everything else onto the four alert icons, coloured with `color=`. Commit 2 is what puts that colour on `color=` rather than `-exact-tint-color`.

`saved` is now a real variant because `color=(saved ? …)` is a style row, so the gallery count goes from 24 to 25 and `sample-ar-saved` is `ar-saved` in `apps/library-gate/app.contract:65` and `scripts/stage12-gate.mjs:704`. No `sample-ar-saved` remains. The tokens are `cards.*` and `actions.*` in those packages’ designs, which is the name an app override replaces. `scripts/s13c-restyle.mjs:214` skips `= icon(` and `= icon (`, and that lookahead hits only these icon lines.

The tests check the claim. The Rust test compares each SVG to the old `d` byte for byte and requires the same `d` in library-gate’s plan. `testAPathIconTakesItsColourNotItsTint` (`IconShapeMacTests.swift:489`) resolves `secondaryLabelColor` live in aqua and darkAqua, checks the leaf alpha and the center pixel, and checks that a tint with no colour paints `textColor`.

Verdict: Approve. The move is complete and the four commits leave no defect.
