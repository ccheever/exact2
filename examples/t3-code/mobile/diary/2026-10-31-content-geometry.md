surfaces: ios
agent: Codex gpt-6.1-sol
task: observe route-local platform geometry before adapting controls

### Rough

The Duo system rail overlaps conversation text and controls. Window safe-area
values cannot describe a localized obstruction inside a route. The app now
observes its stationary content column after native containment. The observer
publishes local bounds, residual insets, traits, and clipped reserved regions.
It adds no padding or keyboard policy.

The first normal native build refused the two trait registrations because
UIKit isolates them to the main actor. Registration now asserts the framework's
existing main-thread native-entry contract. The second build passes.

A direct UIKit debugger read and one Root resource sample agree. Reading the
runtime on its owner thread was followed by a libdispatch trap. That inspection
is unsuitable for further lifecycle acceptance. A clean ordinary relaunch then
passes navigation and existing draft recovery without a debugger attached.

Both fold commands report a missing hinge-angle sample. Their advised captures
confirm the active panel changed from outer to inner and back. These captures
do not verify the angle or rotation.

### Lean in

Contract members allow the existing layout state and action to move intact out
of the nearly full Root. The four statements remain byte-identical. The new
packet boundary distinguishes unavailable facts from measured zero insets and
rejects retired route owners and nonadvancing counters.

### Needed

- local platform geometry: by hand through an app-owned passive UIKit view.
- native callback lifetime: provided; the app also owns route and mount counters.
- keyboard geometry: provided by the existing owner, unchanged in this unit.

### Checkpoints

- native lifetime fixture: smooth; 25 production-helper checks pass.
- packet and Root bridge tests: smooth; 6 tests and 72 assertions pass.
- full mobile suite: smooth after correcting a fixture environment variable;
  4,184 tests pass with both optional native bridges enabled. The failed setup
  receipt is retained. Source and timeouts were unchanged.
- Contract, strict TypeScript, caps, and boot: smooth.
- normal iOS build: rough at trait isolation, then passes. All 3,289 recorded
  inputs match and all 92 bundle files pass strict signature verification.
- ordinary Duo drive: smooth after removing debugger interference; Home,
  thread reopening, Settings, and exact existing Unicode draft recovery pass.
- geometry inspection: one outer-thread sample agrees from native view through
  Root. Route transitions, keyboard states, and fold-update packets still need
  an inspection method that respects the runtime owner.

The integrated thread route now applies the measured trailing reservation to
the feed, working control and composer width. The focused geometry and history
tests pass; a fresh Duo drive is still needed to qualify keyboard-visible
reachability and the rail during transitions.

A fresh normal build was then installed on Duo `8AE91541-FA45-4707-89E1-43DB980A2985`.
On the outer panel, opening the existing conversation with the keyboard visible
kept the composer and Send/Stop control inside the measured content column; the
right system rail stayed clear. The same drive scrolled Home to its populated
settled rows, including the synthetic QA thread and unsent drafts, without a
row or status entering the rail. Screenshots are retained in the local
`target/t3-mini-coordinator-geometry-8ae-*.png` evidence; this qualifies the
ordinary outer-panel reachability sample only. Fold angles, rotation, and
transition animation remain open.

The same session received an `open` fold request. Hinge readback returned no
sample, so the pose is unverified, but the subsequent capture was the native
951x669 inner panel. The clone still showed one full-width Home list there;
the signed reference's inner panel has a persistent thread sidebar and a
separate content pane. This confirms the remaining GAP007 presentation
difference without attributing it to an angle or rotation result. The current
one-stack route architecture must be extended as a separate sidebar unit before
that layout is changed.

Exact receipts and raw logs are under
`target/t3-mini-coordinator/evidence/content-geometry/`. Home and conversation
layout consumers, selection, rotation, and performance remain unqualified.
