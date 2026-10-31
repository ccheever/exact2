surfaces: ios
agent: Codex gpt-6-astra
task: match sidebar row spacing and separators

### Rough

On a fresh machine, the native build reached an offline helper before the root
Cargo dependencies were cached and failed on missing `wat`. Root Cargo fetch
also needed `CARGO_NET_GIT_FETCH_WITH_CLI=true` to use the existing Git credentials.
The mobile tests need a built Contract compiler for the expanded-route check.

T3 initially listed no local devices despite Xcode listing an iPhone Duo; EAS
reported no selected project. T3 now exposes a coordinator-owned iPhone 18 Pro.
Home awaits its own device window, and Duo remains unavailable through T3.

### Lean in

The existing authored-view fixture reproduces the sidebar separator mismatch
without a new test harness. The same Home component serves the screen and sidebar.

### Needed

- sidebar: by hand; row padding and gutters now follow the reference surface.
- foldable layout: missing from the app's width-and-height-only layout decision;
  native headers and workspace ownership need coordination.

### Checkpoints

- install: rough; generated app tables and the viewer dependency are required.
- editing home.contract: smooth; the separator regression fails the old condition
  and passes the correction. Home tests pass: 184 before, 185 after.
- types: smooth after generating the Contract declarations; strict ES2024 passes.
- full mobile suite: the quiet run passes all 4,171 tests in 6.52 seconds with
  unchanged timeouts. The concurrent-build run's two timeouts are retained in evidence.
- root checks: all six pass. The workspace-only retry passes with the verified
  `wasm-bindgen-cli 0.2.127` on PATH and `CARGO_INCREMENTAL=0`; its original missing-tool
  failure is retained in evidence.
- original iOS Release: built from the frozen clean reference with Xcode 27.2.
- Exact iOS Release: built and signed; strict signature verification passes.
  Native acceptance remains blocked by device access.

Reference source is frozen at `80a2351fcb909034e65a512dfabeb3cdfd367762`.
The older vendored pin remains provenance. This change does not establish pixel
parity or a performance improvement.
