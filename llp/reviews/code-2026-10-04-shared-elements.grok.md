# Code review: shared elements stage 1, LLP 1013.000 (563e39822..c94ea418c), 2026-10-04 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --no-subagents`, headless, `--cwd` a detached worktree at `c94ea418c`.
- **Method:** one brief (sha256 `560271d57f792e32…`), round 1, blind to the other review, requested by Charlie. The author (Claude) is not a reviewer.
- **Verdict:** DO NOT LAND.
- **Disposition** (fixed in `52faec4db` unless argued):
  - Web: the host's unitless time format parsed, with the delay; each transition clears only the names it gave; the agent's `After` runs again when a transition is ready, before `inflight` drops, so its animations are registered on the clock; arrivers are only elements that were not in the document before the flush; a leaver kept as an exit ghost counts as gone and is removed when it pairs; a name with two leavers is not captured at all; the spring is lowered on the native flight's terms (0 to 1000, rest under 1e-3 in displacement and speed, 240 Hz, 10 s), so `spring(240, 26, 1)` settles at 1113 ms on both.
  - Apple: a flight whose place is inside a view beginning its exit lands first and leaves with it, and the Rust host ends its curve; `reset` ends every flight; a destroyed arriver's view leaves its overlay before the overlay is dropped; `applyBoxLayer` leaves the radius to a flight; a tinted image draws while it flies; containment is the nearest route (a navigation or tab container's child) or parentless controller; the Mac source rectangle goes through the layers; the destination scroll runs after the batch, so a virtualized list hears it; the batch-end loop iterates a copy (Swift's `for` over `flights.values` already iterates a copy, so the reported trap cannot occur, but the copy is now explicit).
  - Kernel: pairing indexes names once (linear).
  - Declared in LLP 1013.000 §3 rather than fixed: a web leaver inside a virtualized list's rows, a destination's own transform during flight, coalesced web commits' later leavers, Mac dialog containment and accessibility.

---

I'll review this commit read-only against the shared-elements RFC and the repo rules, starting with the spec and the diff.The kernel pairing and host flight curve are in place. Next I’ll read the presenter, the web commit path, and the tests against D4–D8.The reduced-motion path crashes, and two of the web rules in D7 and D9 do not hold. Do not land this.

## Must-fix

**[must-fix] Reduce Motion, a missing leaver, or a view with no window crashes the presenter — `host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:91`**

`flightsBatchApplied` walks `flights.values` and `liftFlight` removes the entry on the failure path (`FlightsIOS.swift:131`, same loop at `FlightsMac.swift:87` and `:122`). Swift traps if a dictionary changes during enumeration. D8 takes that path for every flight when Reduce Motion is on: the arriver should stay put and the curve should still run. The same trap fires when capture fails (`source.rect` null, view not in a window, no presentation root). The Apple test only checks batch op strings, so it never hits this.

**[must-fix] An exit ghost keeps the shared name, so the pair never flies — `host/web-js/shared.js:110`**

D7 says a leaver with `exit-animation` is not given a presence ghost. `Leave` still calls `Pres.exit` (`host/web-js/rt.js:831`), which leaves the element connected with `data-shared-element`. The update then treats it as "stayed" and never assigns `view-transition-name` to the new holder. The ghost plays its exit in place and the arriver pops in. The same thing happens when an ancestor is the one with `exit-animation` and the named node stays inside it.

**[must-fix] The agent clock seeks the view transition after it has already jumped to the end — `host/web-js/shared.js:128`**

`inflight` drops when `ready` resolves, and nothing registers the new animations at the commit's clock. `document.getAnimations()` does not contain them until after the update callback, so the `seek` inside `settled` (`After`) misses them. `agentSettled` then waits out `inflight`, computes settle time, and `advance`s the clock to that end (`host/web-js/agent.js:281`) before the next `seek` registers the animations. That `seek` stamps their start as the end time and sets `currentTime` to 0, so `clock settle` waits about twice the duration and the flight replays. D7 and D9 say the driver waits for `ready` and then seeks, and §4 says native and web settle at the same clock time.

## Should-fix

**[should-fix] Tearing down a flight leaves the layer, and sometimes the view, behind — `host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:124`**

`forgetFlight` (and the early return in `landFlight` at `:100`) calls `dropEmptyLayer` while the lifted view is still a subview, so the layer is not removed. Destroy then calls `removeFromSuperview`, and an empty full-size `FlightLayer` stays on the presentation root. `flightLook`, hit testing, and accessibility are not restored. A list that evicts the row mid-flight, or a navigation that destroys the arriver with no reverse handoff, hits this. Mac is the same at `FlightsMac.swift:114`. Interruption hides it because the next lift reuses `subviews.last as? FlightLayer`.

**[should-fix] The host scroll that brings the destination on screen is invisible to a virtualized list — `host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:140`**

`scrollIntoView` runs from `flightsBatchApplied`, still inside `apply`, so `batchDepth > 0`. `setContentOffset` fires `scrollViewDidScroll` → `collections.changed(id, user: true)`, and `Collection.swift:446` advances the cursor only when `batchDepth == 0`. The list's model keeps the old offset. The next report corrects the port back, the overscanned row is retired, and the flying view is destroyed. D5's explicit `scrollIntoView` command is a snapshot correction and survives `endBatch`. The host's own scroll for an already-built row does not.

**[should-fix] Native and web springs do not settle together — `host/apple/src/flights.rs:23`**

The flight is a physics `Layout` spring over a displacement of 1000. Rest is an absolute `1e-3` (`motion/src/spring.rs:53`), so a `spring(240, 26, 1)` runs until the value is within 0.001 of 1000, on the order of a second. The web lowers the same spring as a unit oscillator and stops at a relative 0.001 (`host/web-js/shared.js:69`), about half that time. §4 requires one clock time for one curve. `Layout` does spring as physics (`motion/src/property.rs:220`), so the 1000-unit choice changes the duration.

**[should-fix] Two leavers of one name still snapshot the first — `host/web-js/shared.js:90`**

The second leaver sets the name to null, but the first already has `view-transition-name` and the old state is captured before the update clears it. D3 says that name pairs nothing. If another name in the same commit does pair, `skipTransition` is not called, and the duplicate name plays the default cross-fade.

**[should-fix] A Mac flight is not contained in the presentation — `host/apple/Sources/ExactKit/Mac/FlightsMac.swift:127`**

The layer is always the window `contentView`. A shared element inside a dialog (`DialogsMac.swift` lifts the dialog onto `viewport`) flies over the whole window, outside the dialog. D4.3 puts the layer on the presenting controller's root. The Mac view is also left in the accessibility tree, and hit testing is blocked only by `FlightLayer.hitTest` returning nil.

## Consider

**[consider] Coalesced commits only name the first commit's leavers — `host/web-js/shared.js:82`**

A second commit before the update callback is queued and flushed inside it, but `leavers()` already ran. A handoff whose region was not in the first queue gets no `view-transition-name`. The callback is scheduled for the next rendering opportunity, so a second commit in that window is normal.

**[consider] Tests do not reach the paths above.** `kernel/tests/it/handoff.rs` covers pairing, not two leavers or exit-plus-name. `host/apple/tests/it/flights.rs` asserts op order and a mid-flight reverse in the batch string only. Nothing drives Reduce Motion, row eviction, modal containment, the web update callback, `skipTransition`, or `clock settle`.

DO NOT LAND
