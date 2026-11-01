surfaces: ios
agent: Codex gpt-6.1-sol
task: add a native workspace container for the iOS route

### Rough

The workspace source was already staged when this pass began, so the review had to
cover generated Contract output, the full mobile suite, strict TypeScript, the
workspace compiler and the native container together. The first workspace check
failed because `wasm-bindgen` was absent from PATH; rerunning with the pinned
0.2.127 tool already present in the checkout passed without a source change.

### Lean in

The tab-container seam accepts the three authored panels as separate UIKit
navigation controllers while leaving route content and its existing navigation
keys in place. The native owner can animate sidebar and inspector transitions,
settle reduced-motion changes, and reject stale configuration packets without
adding another scroll surface.

### Needed

- split workspace presentation: by hand through an app-owned UIKit container.
- native tab-panel handoff: provided by the host seam once the authored tab panels
  are present.
- pinned compiler tools: provided when the checkout's tool directory is added to
  PATH.

### Checkpoints

- generated sources: smooth; 527 outputs match their inputs.
- mobile tests: smooth; 4,224 pass, 2 intentional native skips, 37,598 assertions.
- Contract build and strict TypeScript: smooth.
- caps and boot: smooth.
- workspace check: rough on the initial missing-tool PATH, then smooth with the
  pinned wasm-bindgen binary.
- native iOS build: smooth; the staged source had already produced a signed
  simulator artifact before this final source verification.
