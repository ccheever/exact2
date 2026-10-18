# Code review: Apple's absolute press opacity and looks (LLP 1121 stage 8 B2), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** `e99eda450`, the Apple host.
- **Method:** one brief (sha256 `a99d2f2635b82c222bf9cecc242ad2436e884e9920a7cfc8b66b151bb205985f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: a re-entrant hover's superseded looks, disabled changes; should-fix: a pressed share ignoring a look's opacity).
- **Disposition:** in the round-1 commit, as Astra's 6, 1 and 4 (the share is of what shows, as the web and Linux take it, 0.4 for 0.5 of a hover's 0.8, not the node's own as proposed).

---

**RIGHT WITH CHANGES.** `e99eda450` (LLP 1121 stage 8 B2, Apple) gets the main path right: `:active` lowers to an absolute `press_opacity_to` eased from the opacity actually showing, looks stand in under that paint (focus over hover, unnamed rows stay the base), and a pressed-paint row replaces the platform dim and the Mac wash. Three correctness holes remain.

1. **BLOCKING** — A re-entrant `setHoverPath` paints the superseded path’s `:hover` looks. `host/apple/Sources/ExactKit/Mac/PresenterMac.swift:932` registers `defer { hoverLooks(nodes) }` before `hoverTransition` is incremented at line 933, and that defer does not check the generation the event path uses at lines 942–959. `send` runs `onHover` immediately when a batch is not applying (`PresenterMac.swift:923`). `HoverFollowsLayoutMacTests.swift:185` is that call: `setHoverPath([child, parent])` enters the parent, whose handler calls `setHoverPath([parent])`. Events stay on the parent because the inner call bumps the generation. Looks do not. The inner defer paints the parent, then the outer defer runs `hoverLooks([child, parent])` and turns the child’s look back on.

Fix: capture the generation first, and paint only if this call still owns it.

```swift
hoverTransition &+= 1
inlineHoverTransition &+= 1
let generation = hoverTransition
defer { if hoverTransition == generation { hoverLooks(nodes) } }
```

In the same function, `HoverMac.swift:27` should also require `isDescendant(of: viewport)`, matching `eligible` at `PresenterMac.swift:937`. A view removed from the window (the sibling test at `HoverFollowsLayoutMacTests.swift:201`) is still in the raw path, is not hidden, and would otherwise keep a look.

2. **BLOCKING** — Toggling `disabled` does not drop or restore looks. `looked` (`PressPaint.swift:275`) returns the base for a disabled node, but `lookState` (`PressPaint.swift:302`) returns immediately when the hover and focus flags are unchanged, and `propsChanged` (`PresenterMac.swift:41`) only calls `syncDisabledLook`. `hoverAt` does not exclude `disabled`, so the next-frame `followPointer` calls `lookState(hovered: true)` again and that call is a no-op. A node already showing `:hover` or `:focus-visible` keeps its background, border, shadow, and `lookOpacity` animation after `disabled` becomes true. One created disabled, then enabled while the pointer is still over it, never gains the look until the pointer leaves and re-enters: `hoverLooks` (`HoverMac.swift:33`) sets `hovered` even though `looked` painted nothing. `syncDisabledLook` (`PressLookMac.swift:106`) can also halve `alphaValue` without recomputing the additive delta, so the displayed opacity is the halved alpha plus the stale look offset. `testADisabledNodeTakesNoLook` (`StateLooksTests.swift:104`) only builds the node already disabled.

Fix: on a disabled change, after `syncDisabledLook` so `ownOpacity` is current, reapply even if the flags did not move: `restyleForPress(pressPainted(looked(lookBase)))` and `refreshPressOpacity()` (that already calls `applyLookOpacity`). That covers focus as well as hover. Extend the test to flip `disabled` while hovered and while `focusVisible`.

3. **SHOULD-FIX** — Hand-written `press_opacity` ignores the look’s opacity. `pressDepth` (`PressPaint.swift:81`) is right for `press_opacity_to`: depth is `(lookOpacity ?? ownOpacity) - target`, and the two additive animations land on the target. The share branch at line 85 is still `ownOpacity * (1 - share)` and never reads `lookOpacity`. Both animations are additive, so a full press shows `lookOpacity - ownOpacity * (1 - share)`. Own 1, look 0.8, share 0.5 displays 0.3; D7’s share of the node’s own, with the press replacing the look, is 0.5. `:hover` opacity plus `-exact-press-opacity` is a legal node (one-spelling only refuses `:active` together with the press rows).

Fix: one depth, from what is showing to the destination.

```swift
private func pressDepth(_ s: NodeStyle) -> Float {
    let shown = pressPaint.lookOpacity ?? ownOpacity
    if let to = s["press_opacity_to"]?.number {
        return shown - Float(min(1, max(0, to)))
    }
    let share = Float(min(1, max(0, s["press_opacity"]?.number ?? 1)))
    return shown - ownOpacity * share
}
```

With no look, `shown == ownOpacity`, so this is the existing D7 depth and the current press tests stay valid.
