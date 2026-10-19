# Code review: LLP 1129 D1 on native hosts, and a formatted live time's width without its fallback, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `b17567500`.
- **Method:** one brief (sha256 `91dc0426ebbbd8dac0f0eb9bd08f1c344f74070353b6223b889524bc0f8b2776`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking, no findings.
- **Disposition:** nothing to take.

Nothing is blocking. No native host draws LLP 1129 D1’s own-ink hover for a custom pressable, and no native press path chooses its treatment from a compile-time fill.

## Claims

D1 is web-only in its own text. The decision is the 12% `currentColor` wash over an author fill on a `button` with no `href` and no `buttonStyle`; the last bullet already says “The web only.” The amended Limit is right that a bound fill is invisible when the mark is first made, that `host/web-js/said.js` stamps `data-exact-said~="background-color"` from the page’s resolved fill (alpha, and `light-dark()` if either half is filled), and that the wasm page rebuilds that mark from the live node (`props_for` on create and update, with the author `written` bit set for a bound row).

No native host draws that wash for a custom pressable, so none of them need a bound-fill mark:

- Linux `neutral_face` (`host/linux/src/paint/button.rs:428`) washes neutral **button-style** faces only, and `button_control` runs for `NodeType::Control`, not a bare pressable. `row()` (`host/linux/src/paint/neutral.rs:535`) washes neutral list rows and returns immediately when the look is not neutral. `feedback.rs` only tracks that wash plus an authored `:hover` look.
- Mac `HoverMac.swift` applies an authored `:hover` look and nothing else. `PressLookMac.swift` says there is no hover look; the press wash is AppKit’s pressed effect over the pixels already drawn.
- iOS hover is an authored `:hover` look (`LookHoverIOS.swift`) or a UIKit pointer effect when `hover_effect` is set. Android’s `hover` is accessibility explore-by-touch, not a fill wash.

D1’s press half is LLP 1115.000 D6’s dim, and it does not consult a compile-time fill. iOS `dimPress` multiplies opacity (`PressFeedback.swift:429`). The Mac tints the captured picture. Linux applies an authored press paint from the live style mask, or the neutral-style wash above. The web dim is `opacity: .65` and does not use the `background-color` mark.

The live-time test does discriminate. A `time` is a text leaf: children are not laid out as boxes (`lays_out_children`), and once `text` is the formatted string the paragraph runs skip those children (`arena.rs` `text_runs_after`). `align-items="flex-start"` keeps both times at max-content, so they are not stretched to the same 390 px column. The fallback (“A fallback far longer than the formatted text”) is wider than the relative reservation (“59 min, 59 sec ago”) under the monospace kernel. If that child were measured into the paragraph, `with` would be wider than `without` and the assertion would fail. `datetime=-120000 format="relative"` is the same “2 min. ago” the file already checks.

Scoping the fallback rule under `#exact-root` matches the rest of that sheet. Dropping the decorations-editor queue line matches LLP 1114.007 r3: decorations sit on a `textarea`, and that RFC rules out a `contentEditable` stand-in.
