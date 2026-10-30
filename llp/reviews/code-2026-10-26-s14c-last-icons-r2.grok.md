# Code review: stage 14 C's last icon moves, round 2 of 2, 2026-10-25..26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `5bbcabe040`.
- **Method:** one brief (sha256 `25ac2b25bc0a47a36d8dbb318ad714a274844609f8874b43762a55679ef6c231`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Round 2 is a fresh pass on HEAD only. I'll recheck the places round 1 was most likely to have assumed: how each host paints the stacked badge layers, dark-mode colour, the compat-id comparator, and whether docs or QUEUE still describe the old icons.No findings.

HEAD `5bbcabe` matches the rulings. The eight shapes are package icon tokens drawn as `image "symbol:…"` with `color=`, the badges are two stacked one-colour layers, and `BarItem` and `HeaderButton` still take the caller’s `d`.

The fresh pass on the places round 1 was most likely to have assumed:

- **Stacking and size.** Each badge is a `position="relative"` column sized to `size`, with the ground image then the check image, both `position="absolute" top=0 left=0` at that same box (`packages/cards/common.contract:47`). That column is the containing block, so the layers sit on the badge rather than a higher one. Default `object-fit: fill` is drawn as `contain` on web, macOS, and iOS, and a square 24×24 viewBox in a square box fills it. The edge gate passing is consistent with that alignment and paint order.
- **The arc.** `M12 .5a11.5 11.5 0 1 1 0 23 11.5 11.5 0 1 1 0-23Z` is two semicircles of the circle centered at (12, 12) with radius 11.5. Both sweeps match, so they form one disk. The cubic conversion’s center math is exact for those radii, and the gate’s edge tolerance covers the cubic approximation on Apple.
- **Colour, including dark mode.** `color=` is the image’s `text_color`, and lowering also sets the symbol tint to `currentcolor`, so each layer keeps its own ink. `cards.badge` is `light-dark(#006aff, #0f73ff)` and `cards.badge-mark` is fixed white. Profile headers pass those through `param.badge` and `param.badgeMark`; post cards pass the tokens. Toolbar buttons are custom buttons, so their icons are not a native face that would keep the control tint. The same path is what the light and dark gate shots paint.
- **Accessibility.** The label and `role="img"` stay on the column. Both layers are `aria-hidden`. On the web that is `aria-hidden`; on the Mac a `role="img"` element exposes no children; on iOS the symbol views are hidden and the labelled column is the image element.
- **Deleted names.** `attieIcon`, `iconGallery`, `iconCamera`, `iconGif`, `ToolGlyph`, and `packages/actions/common.contract` have no remaining call sites in packages, docs, gallery, or gate scripts. The stage-12 gate line about `common.contract` is a generic helper comment.
- **Tests.** `library_last_path_icons.rs` checks the seven `d` strings byte for byte, checks that the arc is that circle (radius, diameter ends, equal sweeps, closed), checks that the deleted names are gone, and checks that library-gate’s plan rows are nonzero with an empty `sf`. The compat test compares library-gate with and without the `iconFillRules` input. Sibling tests show an even-odd icon changes the id, and path bytes are not a compat input, so a matching id means this app does not gain the rule. `s13c-restyle.mjs` reads the badge layers at `2/1/0/1/0/0` and `…/1`, and the three toolbar icons at `>2/0/0/{0,1,2}/0`, all on `text_color`.
- **Docs and QUEUE.** `docs/contract-for-agents.md` lists the new tokens, the stacked badge, and the two icons that stay paths. QUEUE drops the three lines this closes.

Verdict: correct and complete.
