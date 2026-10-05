# Code review: shared-element flights on Apple, the drawn image in points, the flight layer's rank, the enclosing presentation, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `223be43c32fc33715ecb871aa002327f8fdf8120f37969b7288a35a2052187ee`), shared with grok. Round 1, blind to the other review. Reviewed the uncommitted diff in a detached worktree at `536a054e1`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. The noted coverage gaps (an interrupted image flight through `showFlight`, real navigation/tab/modal containment, macOS execution, both ends inside one modal) are DEFERRED; the Signal Clone agent films cover the route/overlay case end to end.

---

**READY — no concrete regression found** in the uncommitted changes against `536a054e1`.

- **Interpolation and interruption:** [FlightsIOS.swift:292](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:292) correctly interpolates each endpoint’s drawing in points. Adding the interpolated box origin preserves linear motion in the container. Recapturing `flightLook / bounds`, then multiplying by the new source size, preserves the interrupted drawing, including conversion between differently scaled containers. Existing caveat: the `max(bounds, 1)` denominator at [line 65](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:65) already breaks continuity for dimensions below one point; this change does not introduce it.

- **macOS coordinates:** [FlightsMac.swift:185](/tmp/x20/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:185) converts both rectangles into the flight layer before using their sizes. Flipping changes origins, not these dimensions; the new interpolation introduces no additional flip requirement.

- **Paint order:** [FlightsIOS.swift:197](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:197) and its macOS counterpart are appropriate. `FlightLayer.hitTest` returns `nil`. [Accessibility.swift:320](/tmp/x20/host/apple/Sources/ExactKit/Accessibility.swift:320) excludes helper layers from occlusion checks. Removing the layer leaves no retained view in `DensePaintRanks`; its cached numeric ranks are harmless and recomputed when membership next changes.

- **Root selection:** [FlightsIOS.swift:188](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:188) checks physical containment in the correct direction. Equal roots remain unchanged; sibling presentations and separate windows fall back to B. This preserves the existing boundary for native sheets/fullscreen presentations outside A’s subtree—it does not extend flights across those boundaries. The explicit inequality handles `isDescendant` including identity. [Apple reference](https://developer.apple.com/documentation/uikit/uiview/isdescendant(of:)). A weak class reference inside a struct is valid Swift and avoids retaining A’s presentation. [Swift reference](https://github.com/swiftlang/swift/blob/main/docs/ABIStabilityManifesto.md#data-layout).

- **Tests:** [FlightsIOSTests.swift:29](/tmp/x20/host/apple/tests/ExactKitTests/FlightsIOSTests.swift:29) catches the crop regression; the hierarchy tests distinguish the previous container and paint-order behavior. Coverage gaps are nonblocking: interrupted image flights, real navigation/tab/modal containment, and macOS execution.

Validation: `git diff --check` passed. An independent numerical model reproduced **376.25 → 273.25 pt** at midpoint and preserved 10,000 interruption recaptures within floating-point error. **XCTest was not rerun in this read-only sandbox.**
