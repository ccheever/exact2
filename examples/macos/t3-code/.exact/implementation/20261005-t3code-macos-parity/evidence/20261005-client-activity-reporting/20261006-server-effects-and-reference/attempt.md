# Activity server effects and source comparison

Captured 2026-10-06 KST (logs in UTC), source `7444d5999` plus concurrent unrelated repair edits. Exact hashes of the five activity production sources are in `captured-source.json`; assertions reject source drift. No activity production code was changed. macOS26.6.2, Xcode27, pinned Bun1.4.2. This extends, and does not replace, the earlier actual-app cadence/focus/input capture.

## Actual transport and server acceptance

`main.swift` links the current production native app files and framework module facade. It creates a real AppKit window, sends real RPC through production `T3Transport`, and exercises production `T3Fleet` with the shared production `T3ActivityReporter`. The foreground window is a400×120 harness window, not the main app UI; no layout claim is made. Actual `NSApplication.nextEvent`/`sendEvent` dispatch is required to deliver activation. Two earlier harness attempts omitted AppKit lifecycle/event dispatch and are preserved as `wire-window-unlaunched.log` and `wire-without-event-pump.log`; they correctly did not establish foreground behavior.

Two reference servers from the dependency-audited `1e2ecbd975` export run at16845/16846 with independent base directories, isolated HOME/CODEX_HOME/CLAUDE_CONFIG_DIR and telemetry disabled. No conversation/provider task is submitted. Health probes can execute installed provider binaries discovered outside PATH; their credentials remain isolated. Server A uses `providerHealthRefreshInterval:1000` and Balanced profile.

1. Connect a production transport without a reporter, as the pre-feature baseline. Four `getConfig` samples across3.6s keep Codex and Claude checkedAt unchanged.
2. Disconnect baseline, activate a real AppKit window, and connect a production transport with the reporter. Server reports one foreground lease; four config samples advance provider checkedAt on successive intervals.
3. Pair B and connect it through production fleet. B receives the same client ID as A. `fleetStop` removes the transport, status is disconnected, and B's lease timestamp remains unchanged after stopping (leases intentionally remain until TTL). Reconnect renews B's accepted lease. Distinct server environment IDs are asserted. The prior80s native cadence capture separately covers no B callbacks after final disconnect beyond a full heartbeat interval; this live capture observes the immediate off/on lifecycle.
4. Subscribe/unsubscribe real resource telemetry. Accepted server scope gains then loses diagnostics.
5. Hide the AppKit app, wait48s, and take four final config/policy samples: zero foreground leases, visible/focused/recentlyInteracted allfalse, and provider checkedAt remains unchanged across all samples.

The assertion command is:

```sh
python3 examples/macos/t3-code/.exact/implementation/20261005-t3code-macos-parity/evidence/20261005-client-activity-reporting/20261006-server-effects-and-reference/assert-capture.py
```

It checks captured domain results; it does not claim to replay the live servers. To recapture, create fresh isolated server base dirs, mint local pairing tokens into each `<port>/pair.txt`, compile `main.swift` with `host/apple/modules/ExactNativeModule.swift`, generated `ExactDataKeys.swift`, and all `modules/apple/*.swift`, and run with the fixture base directory argument. Exact launch argv/environment and recorded PIDs are local under `target/t3-verify/activity-repair/{16845,16846}/launch*.json`. No tokens or user data are included in committed evidence.

## Reference comparison

The real reference `backgroundActivityReporter.ts` is copied in the disposable reference export with only `createActivityReport` made exported. `reference-probe.ts` invokes it under controlled document visibility/focus, interaction timestamps, and an actual retained diagnostics subscription/finalizer. `reference-payload.log` captures active and expired-background outputs. Capture assertions compare them with native accepted server leases: clientKind, visible, focused, recentlyInteracted, appState, scopes, and45,000ms expiry. Identity/timestamps are normalized because installations and clocks intentionally differ. The server lease omits environmentId/observedAt; the native callback evidence covers outbound fields and the live connection descriptors establish distinct environments. This is executed source-function parity, not an Electron process capture or a screenshot oracle.

## Remaining acceptance

The actual root Contract10s retry remains a separate GUI check pending the tab lane's handoff. Main-window silent-failure UI is not established by this harness. Server B was stopped during the second background run without disrupting A; no reporting error was emitted, but the harness has no toast surface. Earlier actual app evidence and independent review remain applicable only where source identities match. No task or framework issue is closed by this component evidence alone.
