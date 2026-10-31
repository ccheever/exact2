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

Exact receipts and raw logs are under
`target/t3-mini-coordinator/evidence/content-geometry/`. Home and conversation
layout consumers, selection, rotation, and performance remain unqualified.
