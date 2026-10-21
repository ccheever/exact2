# Code review round 2: shared elements stage 1, LLP 1013.000 (563e39822..dd6a7940c), 2026-10-04 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --no-subagents`, headless, `--cwd` a detached worktree at `dd6a7940c`.
- **Method:** round 1's brief with the range and round 1's dispositions named; blind to the other review. The author (Claude) is not a reviewer.
- **Verdict:** LAND WITH FIXES.
- **Disposition** (fixed in the commit after `dd6a7940c`, "code review round 2", unless argued):
  - Flights capture before exits: the Rust host emits `flight` ops before `exit` ops, so a leaver inside an exiting subtree is captured while it is still in the presenter's maps.
  - Web: a leaver inside an ancestor's exit ghost counts as gone (`closest('[data-exiting]')`), is hidden and its arriver named; every candidate is captured under its own name and a name pairs when exactly one candidate actually left and one arrived (so a retained duplicate no longer blocks the pair); a candidate that stayed is shown as it is (`animation: none`), not moved on the browser's default curve; a refused commit does not start a transition (`Sh && ok`); an explicit clock jump awaits a pending transition's `ready` (`exact.viewTransition`).
  - Apple: a document root does not lift (it lands in place, so a roots change cannot strand it), and a flight whose place went restores the view's look, input and accessibility; reduced motion is `DisplayPreferences.reducedMotion`, which the agent's `prefer` sets; a flight that does not lift still scrolls its place into view; Mac measures both ends through the layers.
  - Kernel: a test for two leavers of one name.
  - Declared (LLP 1013.000 §3): a tinted image's crop does not interpolate; the radius in flight is the largest corner's.

---

I'll review the shared-elements change read-only: the RFC and rules first, then the diff and the round-1 dispositions.The range is four commits on top of the RFC. I'll read the kernel pairing, the Apple flight path, and the web commit hook next.The review is still open. I'll confirm the crash path and the round-1 fixes against the current tree, then write the verdict.The exit path looks like the crash. I'll confirm batch order and the web cases before ranking them.Round 1's fixes hold. Unitless time and delay, per-transition name cleanup, `ready` re-running `After` before `inflight` drops, arrivers limited to elements that were not in the document, a direct exit ghost removed when it pairs, duplicate leavers left unnamed, the spring lowered on the native 0–1000 / 1e-3 / 240 Hz terms, `resetFlights`, the overlay dropped after the lifted view leaves it, radius left alone while `flightLook` is set, tinted images still drawn, iOS containment, Mac capture through layers, and the destination scroll running after `endBatch`. `for f in flights.values` copies the dictionary on Swift 5.9, so `landFlights` removing entries is safe. The four gaps named in RFC §3 are unchanged.

## Should-fix

**A reverse handoff out of a subtree that exits in the same commit captures nothing.** `host/apple/src/host.rs:1172` emits the exit before the flight, and `PresenterIOS.swift:814` applies them in that order. `PresenceIOS.swift:19` lands every flight whose slot is inside the exiting view (`FlightsIOS.swift:133`), which snaps the view to its layout frame, clears `flightLook`, and `release`s it. `leaving` is keyed by the exit root (`PresenceIOS.swift:33`), so `beginFlight` (`FlightsIOS.swift:55`) does not find a descendant leaver and stores a null source. The new flight is then dropped. Close at 40% while the viewer's card has `exit-animation`: the thumbnail pops in at its slot, and the photo finishes inside the card's exit at the full layout rect. Mac is the same path (`PresenceMac.swift:18`, `FlightsMac.swift:54`, `FlightsMac.swift:126`). A plain close with no exit still captures the interpolated rect.

**A named descendant of an exit ghost is treated as stayed.** `host/web-js/shared.js:122` checks `data-exiting` only on the named node. `host/web/presence-glue.js:290` sets that attribute on the exit root. The descendant stays connected, keeps `view-transition-name`, and the new arriver never receives it. Open a photo whose thumbnail sits inside a card with `exit-animation`: the card ghosts in place and the viewer pops in. A leaver that is itself the ghost is removed and does fly.

**Reduce Motion, and a failed capture, skip the destination scroll.** `FlightsIOS.swift:147` drops the flight record and returns before the `afterBatch` scroll at `FlightsIOS.swift:158` (Mac: `FlightsMac.swift:140` and `:147`). An arriver whose row is off-screen stays off-screen for the whole curve. The curve still runs in the engine; Swift has discarded the record, so `present` and `land` do nothing and the view sits at its layout slot.

**Staying names animate on the user-agent curve.** `shared.js:123` leaves `view-transition-name` on an element that is still in the document and pushes a duration rule only for a new arriver (`shared.js:132`). On the tasks screen every chip has `sharedElement` and `spring(240, 26, 1)`. Moving one task view-transitions the chips that only reflowed, at the default ~250ms ease. `rt.js:176` runs presence playback inside that same update, so those chips are also measured for a layout FLIP.

**A view transition hides a refused commit from the clock.** `shared.js:112` initializes `result` to `true` and returns it at `shared.js:150` before the async update runs. `rt.js:176` returns false for a refusal or a poisoned flush, and `rt.js:266` stops `advance` only when `commit` returns something other than true. An action that dirties a `when` holding a shared element and then refuses: the transition still starts, the false result is discarded, and later timers keep firing.

## Consider

**A tinted image flies, but its crop does not morph.** `BoxLayerIOS.swift:59` will not build a flight image layer when `tint_color` is set, so `NodeViewIOS.swift:1313` draws the destination `object-fit` for the whole flight. Mac matches that guard in `BoxLayerMac.swift:276`.

**`cornerRadii(...).max()` (`FlightsIOS.swift:194`, `FlightsMac.swift:178`) rounds every corner to the largest one for the duration of the flight.** A thumb with a radius on one corner only is fully rounded in the air and square again on land.

**The risky paths above have no executable test.** `host/apple/tests/it/flights.rs` asserts batch op strings (flight before destroy, one mid-flight reverse). Nothing drives an exit during a flight, Reduce Motion, crop or radius, modal containment, or the web update callback, `skipTransition`, an ancestor exit, or a refusal under `clock settle`. `kernel/tests/it/handoff.rs` never builds two leavers of one name.

LAND WITH FIXES
