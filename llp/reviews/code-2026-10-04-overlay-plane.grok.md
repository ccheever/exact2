# Code review: a navigation container sits under an authored overlay with a positive z-index (ba0439579), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `ba0439579`.
- **Method:** one brief (sha256 `aa07aa0262f63552fbacba9883b4f144438dcdf610069bef3a28937e9a6f6088`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND.
- **Disposition:** no findings. The residual coverage it names (negative ranks, no authored rank ½, ghosts and lifts) is DEFERRED with astra's nit 2.

---

No findings.

The half-step stays in the open interval between rank ½ and the next higher rank, and the call sites that still need the top plane were left on it.

**Dense mapping.** `rankChildren` places each distinct rank at `(index − index of 0) × 0.001` (`PaintOrder.swift:114-119`). A document-plane view stores plane `1` (`PaintOrder.swift:178`), so it shares rank ½’s slot and then adds `0.0005` (`PaintOrder.swift:124`). No integer rank lies in `(0, 1)`, so that slot is always `0.001` and the container is always `0.0015`, with the next distinct rank at `0.002` or farther. `0.0005` is half the dense step, so it cannot land on another `k × 0.001` or cross it. Checked across negative ranks, a set whose only rank `1` is the container itself, and the ghost (`4_294_967_292`), lift (`4_294_967_294`), and `Int64.max` planes: those three sort above `1` and only change which later slot they occupy. Kernel ranks for a non-zero `z-index` are `2z` (`kernel/src/paint_order.rs:142-148`), so the first positive `z-index` is rank `2`, the first slot above the container.

**Hit-testing.** `hitOrder` sorts by live `zPosition`, later subview on a tie (`PaintOrder.swift:141-155`). `NodeView.hitTest` and `ScrollView.hitTest` both go through that helper, so a positive-`z` sibling is visited before the container. The agent tap path uses `UIWindow.hitTest` and `obscured` (`AgentIOS.swift:526-537`, `624-636`); it does not assume a foreground view is topmost. `syncModal`’s `inFront` compares `zPosition` the same way (`Accessibility.swift:182-186`). Menus and the transition snapshot still call `setPaintForeground()` with the default `aboveAuthored: true` (`MenusIOS.swift:319`, `NavigationIOS.swift:76`, `MenusMac.swift:124`), so they stay on `Int64.max`, above ghosts and lifts.

**Call sites.** All three document mounts pass `aboveAuthored: false`: the tab holder (`NavigationTabsIOS.swift:141`), the primary stack (`NavigationIOS.swift:247`), and the presented stack (`NavigationIOS.swift:349`). A presented stack belongs there. Before `590d73531` it was `zPosition` 0 as the last subview, so a positive `z-index` already painted over it; `bringSubviewToFront` (`NavigationIOS.swift:366`) only wins ties. The viewport moves into the modal with the root, so the call-screen overlay and the presented stack stay siblings, and the overlay stays on top. The route snapshot is parented inside the route view, not beside the root overlay, and keeps the top plane.

**Leaving the plane.** `setPaintForeground(false)` clears the association, sets a non-`NodeView` back to `zPosition` 0, and re-ranks the superview (`PaintOrder.swift:178-183`). `preserveModalContent` does this after moving the outgoing stack into the modal controller (`NavigationIOS.swift:471-472`). A `NodeView` is not zeroed; the re-rank restores `paintRank`. Nothing in this change calls the API on a `NodeView`.

**macOS.** The only caller is the menu layer, on the default top plane. `raisedHit` and the screenshot reorder already follow `zPosition`. No macOS path assumes every foreground view is above authored content.

**Test.** `testANavigationContainerSitsUnderAPositiveZIndexOverlay` (`PaintOrderIOSTests.swift:88-109`) locks the real order for this fixture: back of `subviews`, above rank 0 and rank 1, under rank 60, top plane still above that, and `setPaintForeground(false)` returns to 0. `hitOrder` is the same helper production hit-testing uses. It does not also build a negative rank, a sibling set with no authored rank 1, or a ghost and a lift. Those three stay above or below the container by the sort above, so they are residual coverage, not a wrong assertion.

Verdict: LAND
