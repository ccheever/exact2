# Code review: a navigation container sits under an authored overlay with a positive z-index (ba0439579), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `ba0439579`.
- **Method:** one brief (sha256 `aa07aa0262f63552fbacba9883b4f144438dcdf610069bef3a28937e9a6f6088`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition:** nit 1 is taken. A window-mounted regression test over the native fixture's real tabs (`NavigationTabsIOSTests.testARootOverlayWithAZIndexIsOverTheNativeTabs`) checks the layer order, the window's hit, and the agent's tap. DEFERRED: nit 2's boundary matrix (no authored rank ½, negative ranks, ghosts and lifts with navigation), and fullscreen present and dismiss in that test. The out-of-scope note (an overlay inside a wrapper container: scroll, clip box, material) stays as LLP 1080.003 D2 describes.

---

No Blocker or Should-fix defect found. Two nonblocking test gaps:

1. **Nit — The test checks sorting, not touch delivery or navigation integration.** [PaintOrderIOSTests.swift:93](/tmp/x14-review/host/apple/tests/ExactKitTests/PaintOrderIOSTests.swift:93) creates zero-sized UIViews and calls `hitOrder`; it would pass even if a navigation call site still selected the top plane. **Fix:** add a window-mounted navigation/tab fixture with an overlapping overlay, assert `window.hitTest` reaches the overlay, and assert the agent rejects a covered route control. Include fullscreen presentation and dismissal.

2. **Nit — The new plane’s boundary and restoration cases are untested.** [PaintOrderIOSTests.swift:92](/tmp/x14-review/host/apple/tests/ExactKitTests/PaintOrderIOSTests.swift:92) always supplies an authored rank-1 sibling and tests clearing only a plain UIView. **Fix:** extend it with no authored rank 1, negative ranks, authored maximum/ghost/lift ranks, sibling removal, and clearing the override on a ranked NodeView. Existing adjacent tests cover several ingredients separately, but not their interaction with navigation.

The requested checks otherwise look sound statically:

- **Dense mapping:** [PaintOrder.swift:111](/tmp/x14-review/host/apple/Sources/ExactKit/PaintOrder.swift:111) includes the navigation view itself at encoded rank 1 and always includes zero. Consequently navigation lands at **0.0015**, including when no authored sibling has rank 1. Negative ranks do not shift that position relative to zero. Positive authored ranks start at **0.002**. Ghosts and lifts are compressed by distinct-rank count, so their large integer ranks introduce no collision with the half-step.
- **Hit-testing and comparisons:** `hitOrder`, the flat-layer hit path, accessibility occlusion, and capture ordering compare actual `zPosition` values with tree-order tie-breaking. None assumes the foreground flag means “above everything.” The agent’s [occlusion check at AgentIOS.swift:629](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:629) uses the window’s hit result.
- **Call sites:** The primary stack, tab holder, and presented stack consistently select the document plane at [NavigationIOS.swift:247](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:247), [NavigationTabsIOS.swift:141](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:141), and [NavigationIOS.swift:349](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:349). Fullscreen therefore remains below positive-z root overlays deliberately: [ModalIOS.swift:300](/tmp/x14-review/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:300) moves the whole viewport into the presentation. This matches the amendment and pre-590d73531 behavior. Route snapshots and menu top layers retain the default top plane.
- **Clearing:** [PaintOrder.swift:181](/tmp/x14-review/host/apple/Sources/ExactKit/PaintOrder.swift:181) fixes the stale depth on plain views leaving ranking. NodeViews remain eligible for ranking and recover their current authored/ghost/lift rank. This correctly supports `preserveModalContent` placing retained navigation behind the outgoing snapshot.
- **macOS:** Its only production `setPaintForeground` caller is [MenusMac.swift:124](/tmp/x14-review/host/apple/Sources/ExactKit/Mac/MenusMac.swift:124), which retains the default top plane. No changed assumption found.

One existing scope limitation remains: navigation mounts directly on `root`, while authored children can live in `root.container` wrappers. This fix orders actual siblings; it does not solve overlays trapped inside those wrappers.

Static review only; no files modified or tests run.

Verdict: LAND
