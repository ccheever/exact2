# Code review round 2: shared elements stage 1, LLP 1013.000 (563e39822..dd6a7940c), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `dd6a7940c`.
- **Method:** round 1's brief with the range and round 1's dispositions named; blind to the other review. The author (Claude) is not a reviewer.
- **Verdict:** DO NOT LAND.
- **Disposition** (fixed in the commit after `dd6a7940c`, "code review round 2", unless argued):
  - Flights capture before exits: the Rust host emits `flight` ops before `exit` ops, so a leaver inside an exiting subtree is captured while it is still in the presenter's maps.
  - Web: a leaver inside an ancestor's exit ghost counts as gone (`closest('[data-exiting]')`), is hidden and its arriver named; every candidate is captured under its own name and a name pairs when exactly one candidate actually left and one arrived (so a retained duplicate no longer blocks the pair); a candidate that stayed is shown as it is (`animation: none`), not moved on the browser's default curve; a refused commit does not start a transition (`Sh && ok`); an explicit clock jump awaits a pending transition's `ready` (`exact.viewTransition`).
  - Apple: a document root does not lift (it lands in place, so a roots change cannot strand it), and a flight whose place went restores the view's look, input and accessibility; reduced motion is `DisplayPreferences.reducedMotion`, which the agent's `prefer` sets; a flight that does not lift still scrolls its place into view; Mac measures both ends through the layers.
  - Kernel: a test for two leavers of one name.
  - Declared (LLP 1013.000 §3): a tinted image's crop does not interpolate; the radius in flight is the largest corner's.

---

Static review only; no builds, tests, or file changes. The time parser, transition-owned cleanup, reset/direct-destroy cleanup, and linear pairing fixes look sound. Exit handling and clock ordering remain incomplete. Declared stage-2 gaps are excluded below.

- **Must-fix — Native exits precede source capture.** [host.rs:1172](/tmp/rv-1013-code2/host/apple/src/host.rs:1172). Replacing a container with `exit-animation` emits `exit` before its child’s `flight`. `beginExit` removes descendants from `views`, while `leaving` indexes only the container; the child’s capture therefore fails. Reversing a flight also lands it before capture. Emit captures before exits as well as destroys.

- **Must-fix — Changing document roots can permanently disable a flying view.** [FlightsIOS.swift:100](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:100), [PresenterIOS.swift:837](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:837). While a root element flies, adding another root removes its placeholder and reparents the flying view. Landing then takes the missing-parent return without clearing `flightLook` or restoring interaction/accessibility. Preserve flying roots’ slots and make cancellation restore surviving views. Mac has the same root-reconciliation problem.

- **Must-fix — Web ancestor exit ghosts still prevent pairing.** [shared.js:122](/tmp/rv-1013-code2/host/web-js/shared.js:122). When a named image’s parent exits, only the parent receives `data-exiting`. The image remains connected and is classified as unchanged, so its replacement never receives the transition name. Detect retained exiting ancestors and suppress the paired child’s ghost.

- **Must-fix — Explicit web clock jumps still overtake pending transitions.** [shared.js:148](/tmp/rv-1013-code2/host/web-js/shared.js:148), [agent.js:307](/tmp/rv-1013-code2/host/web-js/agent.js:307). If a data reply starts a transition immediately before `clock +100`, the jump does not await that existing transition. Its later `ready` hook registers animations at the advanced clock, showing progress zero instead of 100 ms. Await pending update/readiness before advancing; the new hook alone does not close round 1’s race.

- **Should-fix — Mac landing ignores destination ancestors’ transforms.** [FlightsMac.swift:172](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:172). Source capture now converts through layers, but destination measurement still uses `NSView.convert`. An untransformed image inside a translated/scaled parent flies toward the wrong rectangle, then jumps when reattached. This is separate from the declared gap for the arriver’s own transform.

- **Should-fix — Candidate holders are counted as actual leavers.** [shared.js:99](/tmp/rv-1013-code2/host/web-js/shared.js:99). An `each` containing two holders of the same name replaces one and retains the other. The scan counts two leavers and suppresses the valid one-destroy/one-create pair that the kernel reports. Resolve ambiguity using actual removals.

- **Should-fix — Native flights ignore the agent’s reduced-motion setting.** [FlightsIOS.swift:147](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:147), [FlightsMac.swift:140](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:140). `prefer prefers-reduced-motion reduce` updates `DisplayPreferences`, but flights read the OS directly. They still animate on a machine without Reduce Motion, and results depend on machine settings. Use `DisplayPreferences.reducedMotion`.

- **Should-fix — Tinted images now draw, but their crop does not interpolate.** [NodeViewIOS.swift:1325](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1325), [NodeViewMac.swift:1285](/tmp/rv-1013-code2/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1285). Tint excludes the image-layer path; fallback drawing ignores `flightLook.image` and uses the destination’s `object-fit` throughout. A tinted cover thumbnail opening into contain immediately changes framing instead of preserving frame zero.

- **Should-fix — Presenter/browser regression coverage remains absent.** [flights.rs:82](/tmp/rv-1013-code2/host/apple/tests/it/flights.rs:82). The reversal test checks batch strings, not presented continuity or cleanup. Add executable coverage for ancestor-exit reversal, root changes during flight, transformed landing, reduced-motion overrides, and pending/skip/explicit-clock ordering.

DO NOT LAND