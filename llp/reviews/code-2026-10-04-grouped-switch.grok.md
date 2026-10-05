# Code review: a grouped-list switch that "does not flip" (d82c5dd55), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, no subagents, `--cwd` a detached worktree at `d82c5dd55`.
- **Method:** one brief (sha256 `8fdf2ae19f73806d768314bee7671cb905d189c6a706e5c0f8531c1fa1bc0c65`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the CLI's running narration before it is omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** 1 taken: rows 12 and control 13 are framed so the hidden `ControlHost` switch covers the accessory's middle; the test asserts they are distinct, overlap, the scroll is hidden, and the hit is the accessory. 2 taken: `axe touch -x <x> -y <y> --down --up`. 3 taken: the cause says a coordinate tap resolves no element; the fragment is a sentence.

---

The coordinate-tap conclusion is right. A finger at the accessory's middle hits that `UISwitch`, and a zero-duration `tapAt` is what iOS 27's switch ignores. The new test does not actually place the hidden `ControlHost` switch there, and one of the documented axe recipes does not parse.

1. **should-fix** — The hit-test assertion never sees the hidden row switch it names — `host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:131`

`testAToggleFlipsItsControl` hit-tests the accessory switch's midpoint and accepts that switch or a descendant. That predicate matches the iOS 27 trace (`_UILiquidLensView` inside the accessory). The fixture does mount a second switch: control 13 is `type=checkbox` with `accessibilityRole=switch`, `ControlHost.sync` builds a `UISwitch` targeted at `ControlHost`, and `GroupedListView.mount` hides the authored scroll. The batch only frames the list and row 21 (`GroupedListIOSTests.swift:53`). Control 13 stays on a zero box, so `ControlHost` centers its switch on the scroll origin (`ControlsIOS.swift:180`). The accessory is the third row's trailing control. Those rects do not meet, and a hidden ancestor is skipped by `hitTest` anyway. The assertion still fails if this fixture puts some other view over that midpoint (the collection is not frontmost, or the cell or switch is not interactive). It passes if the authored switch is later laid out on the accessory and wins the hit, which is the covering case the comment describes.

Fix: after layout, frame row 12 and control 13 so the `ControlHost` switch's window rect covers the accessory. Assert the two switches are distinct, their frames intersect, the scroll is hidden, and `window.hitTest` is still the accessory or a descendant.

2. **should-fix** — `axe touch --down --up` is not a valid command — `docs/agent-pitfalls.md:183`

AXe's `touch` requires `-x` and `-y`. The recipe as written fails validation, so an agent following the pitfall never produces the touch that flips the switch. `axe tap -x -y --tap-style physical` and `axe tap --id <testId>` are the working forms.

Fix: write `axe touch -x <x> -y <y> --down --up`.

3. **nit** — The cause line can be read as "no element at that point" — `docs/agent-pitfalls.md:178`

A coordinate tap never resolves an element: AXe sets `isSwitchLikeControl` false and `--tap-style automatic` sends `FBSimulatorHIDEvent.tapAt`. "Knows no element there" sounds like the hit test missed. The switch is there; the invocation just does not look it up. "Looks broken, its app logic fine" is also a fragment beside the full sentences in this section.

Fix: say a coordinate tap does not resolve an element, so automatic sends `tapAt`, a touch with no duration, which iOS 27's `UISwitch` ignores. Keep the bare-UIKit sentence.

Hit-testing, interaction, the hidden scroll, paint ranks, delayed touches, and `ControlHost` versus the accessory all agree with that account. The collection is added after the scroll and both stay at `zPosition` 0; `PaintOrder` only re-ranks `NodeView`s and `setPaintForeground` views, so `7a2ecf2ef` does not put the scroll or a navigation container over the switch. `mount` hides the scroll, and `hitTest` skips it. The cell stays interactive unless the row is inert. The collection's delayed-touches path does deliver `touchesBegan` and `touchesEnded` together, and the switch's recognizers do not move on a zero-duration touch. The same `tapAt` fails on a bare `UISwitch`, and a physical touch on this accessory flips it through `flip` into `presenter.checked`. Agent `tap` calls `groupedLists.activate` before `controls.activate`, so it toggles the accessory rather than the hidden control. No product change is missing.

Verdict: LAND WITH FIXES
