surfaces: ios
agent: Codex gpt-6-astra
task: match sidebar row spacing and separators

### Rough

On a fresh machine, the native build reached an offline helper before the root
Cargo dependencies were cached and failed on missing `wat`. Root Cargo fetch
also needed `CARGO_NET_GIT_FETCH_WITH_CLI=true` to use the existing Git credentials.
The mobile tests need a built Contract compiler for the expanded-route check.

T3 omitted unused simulators from discovery. Creating an actual iPhone Duo through
Xcode Device Hub made it available through T3 without restarting shared services.
Both Release apps launch through the returned launcher. Fold requests switch the
visible panel, but hinge-angle readback fails. Orientation commands succeed without
an observed orientation change in either app.

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
- actual Duo: both Release apps respond to initial Home, Add environment, and
  Settings controls. Screenshots show outer, inner, and outer panels in sequence.
  The reference has a split sidebar on the inner panel and a right rail on both
  panels. Exact retains full-width Home and bottom search. Shared layout remains
  coordinator-owned.
- acceptance: populated rows, separators, clipping during motion, accessibility,
  appearance changes, and verified rotation remain open. The reference remains in
  Loading environments during this drive, so settled empty states are not matched.
  Screenshots, trees, and raw tool errors are in `target/t3-home-evidence/duo-initial`.

Reference source is frozen at `80a2351fcb909034e65a512dfabeb3cdfd367762`.
The older vendored pin remains provenance. This change does not establish pixel
parity or a performance improvement.
