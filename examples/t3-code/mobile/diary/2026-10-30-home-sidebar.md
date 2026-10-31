surfaces: ios
agent: Codex gpt-6-astra
task: match sidebar row spacing and separators

### Rough

On a fresh machine, the native build reached an offline helper before the root
Cargo dependencies were cached and failed on missing `wat`. Root Cargo fetch
also needed `CARGO_NET_GIT_FETCH_WITH_CLI=true` to use the existing Git credentials.
The mobile tests need a built Contract compiler for the expanded-route check.

Xcode lists an iPhone Duo simulator, but T3's device list is empty and opening
that device fails. EAS reports no selected project. Native screenshots,
interaction checks and performance comparisons remain blocked by device access.

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
- full mobile suite: a run during concurrent builds passed 4,169 tests and timed
  out in two existing tests. A quiet rerun is pending; timeouts remain unchanged.
- original iOS Release: built from the frozen clean reference with Xcode 27.2.
- Exact iOS Release: built and signed; strict signature verification passes.
  Native acceptance remains blocked by device access.

Reference source is frozen at `80a2351fcb909034e65a512dfabeb3cdfd367762`.
The older vendored pin remains provenance. This change does not establish pixel
parity or a performance improvement.
