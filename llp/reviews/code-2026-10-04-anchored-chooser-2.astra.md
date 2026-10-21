# Code review: the anchored chooser, part 2 — macOS, position-area, hr, compile-time refusal, 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree.
- **Method:** two rounds, each on one brief shared with grok, blind to the other review. Round 1 (sha256 `114640ba005da39373d4a67806cb44ad58604206d5a3bf22afc6256437e3f576`) covered a64127df6, three commits. Round 2 (sha256 `a0ed44ef0a92977d86da6ad116906e3cddaa2fae3155371924ee4a919d4b9820`) was a delta over the fixes, at 76e117cd8. Requested by Charlie. The author (Claude) is not a reviewer.
- **Transcription:** each run's final message, unedited.
- **Verdicts:** round 1 LAND WITH FIXES; round 2 DO NOT LAND.
- **Disposition, round 1:** all four fixed.
  1. *Hidden macOS actions choosable.* Fixed by the shared `shown(_:in:)` check.
  2. *Cancel rows identified by span.* Fixed: rows are identified by the inlined node and its `when`/`match` arm ancestry. A component regression test covers it.
  3. *Placement drops margins.* Fixed: the margin box is placed, as CSS places it.
  4. *Inset thresholds.* Fixed: Blink's `CalculateInsetOutsetColor`, with 30 cases measured in Chrome.
- **Disposition, round 2:**
  1. *Unclosed test function.* Fixed; it was a merge error of the author's.
  2. *Margins never reach Apple.* Fixed: the encoder carries them when `position-area` is set. A production-path Rust test covers it, and the drives measure a 12-point gap on iOS and in Chrome.
  3. *Pending chooser choices escape validation.* Fixed: every batch revalidates them, any invalidation cancels them for good, the invoker's visibility is checked, and dispatch is bound to a presentation count.
  4. *Plain-menu picks lose their invoker.* Fixed: each pick holds its invoker weakly and is bound to a presentation, and revalidated the same way.

## Round 1

1. **should-fix — [ChooserMac.swift:55](/tmp/chooser-review2/wt/host/apple/Sources/ExactKit/Mac/ChooserMac.swift:55): Hidden actions remain selectable.** While a chooser is tracking, a batch can set an action to `display:none` without changing its title, handler, or disabled state. `choosable()` still returns true, so validation keeps the menu open and selection dispatches through `fromNativeMenu`, which bypasses visibility checks. iOS would invalidate this owner. Check action visibility and ancestor eligibility, exempting only the popover’s intentional native-presentation hiding; cover this change during tracking.

2. **should-fix — [menus.rs:204](/tmp/chooser-review2/wt/contract/lower/src/menus.rs:204): Source spans cannot identify expanded cancel rows.** Put the same reusable Cancel component in the two mutually exclusive arms of a `when`. Expansion preserves the component’s source span, so both rows have identical spans. `exclusive()` locates both in the first arm and reports “a second cancel,” rejecting an app that always presents exactly one. Track each expanded row’s identity and branch ancestry instead of using spans as identity; add a component-based regression case.

3. **should-fix — [PositionArea.swift:24](/tmp/chooser-review2/wt/host/apple/Sources/ExactKit/PositionArea.swift:24): Painted placement discards authored margins.** A fixed-size popover with `position-area="top" margin=0 margin-bottom=12` should leave a 12-point gap above its invoker. This function places its border edge directly against the invoker on both painted Apple hosts. CSS aligns the **margin edge**, so the web result differs. Pass resolved margins into placement and clamping, or explicitly refuse unsupported combinations. [CSS alignment rules](https://drafts.csswg.org/css-anchor-position-1/#area-align)

4. **should-fix — [border.rs:65](/tmp/chooser-review2/wt/kernel/src/style/border.rs:65): The inset shading thresholds disagree with Chromium for colored borders.** For `border="2px inset #000040"`, squared RGB distance classifies the color as outside the near-black threshold, producing a black top/left edge. Chromium uses relative luminance, classifies that blue as near-black, and lightens those edges instead. The mostly grayscale fixtures conceal this difference. Implement Chromium’s luminance thresholds and add dark saturated-color cases. [Chromium’s shading implementation](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/paint/box_border_painter.cc#L574)

LAND WITH FIXES

## Round 2

1. **Blocker — Unclosed function breaks the test target.** [contract/cli/tests/it/styles.rs:1461](/tmp/chooser-review3/wt/contract/cli/tests/it/styles.rs:1461) inserts the new test before closing `inherit_unset_and_currentcolor_are_csss`. The final brace closes only the new function. Read-only `rustfmt --check` confirms an unclosed delimiter originating at line 1412. Restore the preceding function’s closing brace before `#[test]`.

2. **Should-fix — The margin fix receives no margins in real apps.** [PositionArea.swift:18](/tmp/chooser-review3/wt/host/apple/Sources/ExactKit/PositionArea.swift:18) reads four style values that [host/apple/src/style.rs:52](/tmp/chooser-review3/wt/host/apple/src/style.rs:52) explicitly strips from Apple batches. Consequently, `position-area="top" margin-bottom=12` still opens flush against the invoker. The new Swift tests inject margins directly and bypass this omission. Export resolved popover margins through the production bridge and cover that path.

3. **Should-fix — Pending chooser selections escape lifecycle validation.** [ChooserMac.swift:161](/tmp/chooser-review3/wt/host/apple/Sources/ExactKit/Mac/ChooserMac.swift:161) validates only the state at dispatch, while [MenusMac.swift:214](/tmp/chooser-review3/wt/host/apple/Sources/ExactKit/Mac/MenusMac.swift:214) checks only open entries. Tracking completion has already removed the entry. A queued batch hiding the sibling invoker therefore still permits dispatch: `valid(owner)` checks its window, disabled state and inertness, but not visibility. Likewise, a title changed and restored across intervening batches never permanently cancels the choice. Reopening the same popover also leaves the old choice alive, whose hide command can close the new presentation. Keep pending owners under batch validation, cancel permanently on invalidation, and bind dispatch to a presentation generation.

4. **Should-fix — Plain-menu picks lose their invoker’s identity.** [MenusMac.swift:335](/tmp/chooser-review3/wt/host/apple/Sources/ExactKit/Mac/MenusMac.swift:335) captures only the row and popover. If a completion removes the invoker after selection but before dispatch, while a shared popover remains mounted, every guard at line 360 can still pass and the old selection executes. Retain a weak invoker reference and presentation identity, and cancel the pending pick when that owner becomes invalid.

The schema fixup checks out: 177 contiguous rows, `order` at 175, `position_area` at 176, and independently recomputed digest `0x26c863a4f45d85f6`. The no-reset argument agrees with the [CSS specification](https://drafts.csswg.org/css-anchor-position-1/#position-area) and [Blink’s margin resolution](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/layout/absolute_utils.cc). Chrome was not rerun; no edits or builds were performed.

**DO NOT LAND**
