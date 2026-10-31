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

The first reference artifact used `CODE_SIGNING_ALLOWED=NO` and failed to read its
connection catalog. A separate simulator Release with normal Xcode ad hoc signing
reaches settled empty Home. One private pairing attempt times out waiting for
Environments; its cause remains unqualified. The keyboard covers the code field;
the app's Dismiss keyboard control allows clearing it.

### Lean in

The existing authored-view fixture reproduces the sidebar separator mismatch
without a new test harness. Home has separate screen and sidebar styling branches;
this checkout mounts only the screen branch.

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
- original iOS Release: the separate ad hoc signed build passes strict signature
  verification and seals its resources. Xcode's generated simulator entitlements
  and the binary's `__TEXT,__entitlements` contain the expected application and
  keychain group prefix. The code-signature entitlement dictionary is empty.
  Reference source, generated project, team, and source entitlements are unchanged.
  Binary SHA-256 starts `d16a4b8c5998`; JavaScript SHA-256 starts `d671fc4e2637`.
  Full command, log, hashes, and signature receipts are retained in
  `target/t3-home-evidence/upstream-signed-build`.
- Exact iOS Release: built and signed; strict signature verification passes.
- actual Duo: both Release apps respond to initial Home, Add environment, and
  Settings controls. Screenshots show outer, inner, and outer panels in sequence.
  The reference has a split sidebar on the inner panel and a right rail on both
  panels. Exact retains full-width Home and bottom search. Shared layout remains
  coordinator-owned.
- populated Exact Home: search, no-results recovery, project filtering, opening the
  QA thread with the search keyboard visible, and returning with the query intact
  pass. The QA Unicode draft survives relaunch. A row menu opens with a blank
  preview. A partial swipe reveals actions, but pressing Snooze opens system
  Control Center. Reachability fails; dispatch cause and thread mutation remain
  unqualified. Captures and receipts are in `target/t3-home-evidence/duo-populated`.
- signed reference drive: actual Duo outer Home settles at No environments connected
  before and after ordinary relaunch. Add environment opens. The one fresh private
  pairing attempt does not reach Environments or leave a visible saved environment.
  The code is cleared before capture. Only the reference app is installed and
  driven; Exact's container and QA draft remain untouched. The owned session is
  closed without shutdown. Evidence is in `target/t3-home-evidence/upstream-signed-drive`.
- acceptance: connected reference pairing, populated visual comparison, separators,
  clipping during motion, scrolling, accessibility, appearance parity, and verified
  rotation remain open. Earlier loading-state captures remain in
  `target/t3-home-evidence/duo-initial`; they do not establish settled-empty parity.
  No performance comparison has run.

Reference source is frozen at `80a2351fcb909034e65a512dfabeb3cdfd367762`.
The older vendored pin remains provenance. This change does not establish pixel
parity or a performance improvement.
