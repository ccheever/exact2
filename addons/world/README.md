# X1: install a world as an ordinary data source

`exact-world` and Tally are verbatim copies from `core/world-final`.
`world-source` owns the simulation below exact2's existing `DataSource` seam.
One ordinary Contract app publishes text and dispatches buttons. No GPU crate,
app JavaScript, or second Wasm module is installed.

The reference exact2 worktree is `../..`. Its only change is the
explicitly permitted ureq lock repair. See [REPORT.md](REPORT.md) and
[evidence/summary.json](evidence/summary.json) for the experiment's results.

Run these commands **from this directory**:

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
bun exact.mjs web
bun exact.mjs dev                          # live web server; prints its URL
bun exact.mjs agent web tree 'clock +1000' state

bun exact.mjs macos
bun exact.mjs agent macos tree 'clock +1000' state
# For the ordinary live window:
bun exact.mjs macos --run

bun exact.mjs linux
bun exact.mjs agent linux tree 'clock +1000' state
# On a Mac, the Linux host's live headless presenter uses this app-owned loop:
X1_TRACE=1 target/release/tally-linux-live
```

The `agent` commands launch and drive a fresh application. The experiment scripts
below add explicit PID recording, SIGKILL, and child reaping. A person using
`dev`, `--run`, or `tally-linux-live` owns that long-running process and stops it
when finished.

```sh
cargo test -p exact-world -p exact-world-derive -p tally-data -p tally-logic
cargo clippy -p world-source -p tally-data -p tally-logic -p tally-web -p tally-apple -p tally-linux --all-targets -- -D warnings
node scripts/proof.mjs web
node scripts/proof.mjs macos
node scripts/proof.mjs linux
node scripts/web-measure.mjs               # ten cold Chrome processes + live sample
node scripts/native-measure.mjs macos     # ten cold app processes
node scripts/native-measure.mjs linux
node scripts/macos-store.mjs              # real store, controlled clock, relaunch
node scripts/native-live.mjs              # live macOS timer + real store
node scripts/linux-live.mjs               # app-owned headless Linux timer
cargo run --release -p tally-data --example plans
node scripts/web-measure.mjs 100           # 10 Hz publication, still 60 Hz simulation
cargo test --release -p tally-data --test runner -- --nocapture --test-threads=1
node scripts/summarize.mjs
```

Measurement scripts use Node because Bun's named `child_process.spawn` exports
do not follow `syncBuiltinESMExports`; the PID-recording wrapper refuses Bun.
App builds still use Bun and exact2's own scripts. Run native UI measurements
serially: AppKit's ready announcement precedes usable button dispatch, so the
measurement harness allows 250 ms after ready before querying its counters.
That delay is outside the stored first-tick timestamp measurement.

`Draw`, `Hold`, and healthy `Reset` invoke the unchanged game's actions at tick
boundaries. A failed game's `Reset` reconstructs the simulation. `Save` writes the
runner store and exposes the checkpoint in `state.slots.exported`. A checkpoint
can be pasted into the input and restored; this works even in agent mode, where
stores are normally disposable. `Inspect` exposes hash, tick, up to 64 entities,
components, and the kernel's bounded logs as an ordinary typed mutation result.

Limitations and proposed hooks are recorded in the report. iOS was not run.
