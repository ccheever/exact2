surfaces: web, ios
agent: Codex
task: render thread error banners

## Rough
The mobile app's frontend contract had an error value but no visible error surface, so parity required tracing the shared error projection and adding presentation fields without changing the protected shared client files. The existing app-level exact wrapper does not expose the feedback verb, so the repository-level contract and test commands were used directly.

## Lean in
The shared `threadErrorView` seam already centralizes usage-limit suppression, which let the mobile banner reuse the desktop error ownership rules while adding only provider-message and expansion presentation.

## Needed
- files: provided
- accessibility: provided
- haptics: none

## Checkpoints
- install / first build: smooth
- editing app.contract / app.ts: smooth
- iOS build: smooth
- driving the app: smooth on the iOS simulator; the empty Threads route launched and exposed its controls
- properties: smooth
