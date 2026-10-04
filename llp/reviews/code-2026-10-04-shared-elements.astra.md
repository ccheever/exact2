# Code review: shared elements stage 1, LLP 1013.000 (563e39822..c94ea418c), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `c94ea418c`.
- **Method:** one brief (sha256 `560271d57f792e32…`), round 1, blind to the other review, requested by Charlie. The author (Claude) is not a reviewer.
- **Verdict:** DO NOT LAND.
- **Disposition** (fixed in `52faec4db` unless argued):
  - Web: the host's unitless time format parsed, with the delay; each transition clears only the names it gave; the agent's `After` runs again when a transition is ready, before `inflight` drops, so its animations are registered on the clock; arrivers are only elements that were not in the document before the flush; a leaver kept as an exit ghost counts as gone and is removed when it pairs; a name with two leavers is not captured at all; the spring is lowered on the native flight's terms (0 to 1000, rest under 1e-3 in displacement and speed, 240 Hz, 10 s), so `spring(240, 26, 1)` settles at 1113 ms on both.
  - Apple: a flight whose place is inside a view beginning its exit lands first and leaves with it, and the Rust host ends its curve; `reset` ends every flight; a destroyed arriver's view leaves its overlay before the overlay is dropped; `applyBoxLayer` leaves the radius to a flight; a tinted image draws while it flies; containment is the nearest route (a navigation or tab container's child) or parentless controller; the Mac source rectangle goes through the layers; the destination scroll runs after the batch, so a virtualized list hears it; the batch-end loop iterates a copy (Swift's `for` over `flights.values` already iterates a copy, so the reported trap cannot occur, but the copy is now explicit).
  - Kernel: pairing indexes names once (linear).
  - Declared in LLP 1013.000 §3 rather than fixed: a web leaver inside a virtualized list's rows, a destination's own transform during flight, coalesced web commits' later leavers, Mac dialog containment and accessibility.

---

Static review only; no builds or tests run.

- **Must-fix — [shared.js:49](/tmp/rv-1013-code/host/web-js/shared.js:49): ordinary easing flights never run.** The existing emitter writes `300 0 ease` for `300ms ease`; this parser requires `ms`/`s` suffixes. Duration stays zero and the pair is skipped. Parse the existing duration/delay format.

- **Must-fix — [shared.js:123](/tmp/rv-1013-code/host/web-js/shared.js:123): interruption cleanup erases the next transition’s names.** Starting T2 skips T1, whose asynchronous `finished` cleanup clears *every* `exact-se-*` name, including T2’s newly assigned sources. Cleanup must belong to its transition.

- **Must-fix — [shared.js:128](/tmp/rv-1013-code/host/web-js/shared.js:128): View Transitions escape the agent clock.** The agent’s `After` hook runs before these animations exist; `ready` merely decrements `inflight`, without registering or pausing them. A flight can finish in wall time while the agent clock remains unchanged. Existing pending transitions also aren’t awaited before explicit clock jumps.

- **Must-fix — [shared.js:111](/tmp/rv-1013-code/host/web-js/shared.js:111): pairing includes unchanged nodes.** With an unchanged mounted route C sharing A’s name, replacing A with B finds both B and C and refuses the valid pair. Removing A without creating B can instead animate toward C. D1/D3 require actual created arrivers.

- **Must-fix — [shared.js:110](/tmp/rv-1013-code/host/web-js/shared.js:110): exit ghosts prevent handoffs.** `rt.js` still runs `Pres.exit` for named leavers. The retained ghost remains connected, so this branch treats it as unchanged and never names the arriver. D7 explicitly requires suppressing that presence ghost.

- **Must-fix — [host.rs:1183](/tmp/rv-1013-code/host/apple/src/host.rs:1183): an exiting ancestor can orphan a flying child permanently.** Destroying that subtree removes the child’s engine flight but `exit_holds` withholds its destroy. Swift’s exit traversal misses the child because it lives in the flight overlay. The eventual ancestor destroy removes neither the lifted view nor its flight record.

- **Must-fix — [PresenterIOS.swift:317](/tmp/rv-1013-code/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:317), [PresenterMac.swift:545](/tmp/rv-1013-code/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:545): reset leaves flights alive.** Neither reset clears `flights` or removes overlays outside the document root. Reloading mid-flight retains old views; reused IDs can then have new frame operations intercepted by stale flight records.

- **Should-fix — [FlightsMac.swift:58](/tmp/rv-1013-code/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:58), [FlightsIOS.swift:169](/tmp/rv-1013-code/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:169): transformed endpoints jump.** AppKit’s `NSView.convert` ignores the layer transforms used for dragging/scaling, so the source rectangle is wrong. On iOS, destination transforms are removed during flight and restored on landing: a translated destination visibly jumps at the end.

- **Should-fix — [FlightsIOS.swift:171](/tmp/rv-1013-code/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:171), [FlightsMac.swift:153](/tmp/rv-1013-code/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:153): normal painting overwrites the interpolated radius.** Resizing schedules display, where `applyBoxLayer` restores the authored radius without checking flight state. A rounded thumbnail opening into a square viewer loses its rounding instead of interpolating it.

- **Should-fix — [NodeViewIOS.swift:1313](/tmp/rv-1013-code/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1313): tinted raster images disappear during flight.** Drawing now excludes every image with `flightLook`, while `applyImageLayer` still rejects `tint_color`. Neither path paints the image until landing.

- **Should-fix — [FlightsIOS.swift:219](/tmp/rv-1013-code/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:219): containment skips route controllers.** Requiring `vc.parent == nil` bypasses routes inside navigation/tab controllers. Start a flight, then push another route: the old flight remains above the new screen in the outer controller’s overlay.

- **Should-fix — [FlightsIOS.swift:124](/tmp/rv-1013-code/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:124), [FlightsMac.swift:114](/tmp/rv-1013-code/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:114): cancellation leaks empty overlay layers.** `dropEmptyLayer` runs while the flying view is still attached; the presenter removes the view afterward, with no second cleanup. Destroying the last flight leaves its container installed.

- **Should-fix — [shared.js:69](/tmp/rv-1013-code/host/web-js/shared.js:69): spring timing differs from native.** Web uses unit displacement, different rest criteria and a four-second cap; native uses displacement 1000, the engine’s velocity threshold and ten-second cap. Delays are also discarded. The same declared spring therefore settles at different clock times, contrary to §4.

- **Should-fix — [shared.js:30](/tmp/rv-1013-code/host/web-js/shared.js:30): virtualized-list removals bypass detection.** Only effects with `$r` are inspected, but `list.js`’s collection effect has none. Moving a named item from a persistent virtualized list into a newly opened viewer can flush directly without a transition.

- **Should-fix — [flights.rs:82](/tmp/rv-1013-code/host/apple/tests/it/flights.rs:82): risky paths lack executable coverage.** The added interruption test checks batch strings, not presented continuity or cleanup. There are no added Swift/browser tests covering 40% reversal, ancestor exits, reset, crop/radius, virtualized return, modal containment, reduced motion, or asynchronous skip/clock ordering.

- **Consider — [handoff.rs:66](/tmp/rv-1013-code/kernel/src/handoff.rs:66): pairing is quadratic.** Replacing 10,000 uniquely named nodes scans roughly 200 million leaver/arriver candidates on the commit path. Index names once instead of rescanning both collections per leaver.

DO NOT LAND