# Resize failure and reconnect reproduction

Confirmed with unchanged production `T3TerminalSessions.swift` and `T3TerminalOutput.swift`, compiled together with `main.swift` in this directory. The substitute transport injects the same Disconnected refusal implemented in `T3Transport.rpc` at line 506; the substitute view avoids launching WebKit or interfering with the live verification application.

Run from repository root:

```sh
swiftc examples/t3-code/modules/apple/T3TerminalOutput.swift examples/t3-code/modules/apple/T3TerminalSessions.swift target/terminal-verification/resize-repro/main.swift -o target/terminal-verification/resize-repro/check
target/terminal-verification/resize-repro/check
```

Actual result: exit 1. See `result.json`.

1. A session attaches and successfully resizes to 100 columns × 30 rows.
2. Its connection is lost. Resize to 120 × 40 fails with Disconnected.
3. The connection returns and the registry attaches again (two total attaches).
4. Even explicitly reporting the same 120 × 40 grid once more sends no correcting RPC.
5. Session `lastSize` says 120 × 40; the simulated server remains 100 × 30.

Root cause: `T3TerminalSessions.resize` sets `lastSize` before the RPC result, ignores failures, and suppresses later identical sizes. Reattachment resets output state without retrying the desired grid.

Minimal recommendation: distinguish the latest desired size from the last successfully acknowledged size, coalesce concurrent resize requests to the latest desired value, and synchronize that desired size after a successful attach snapshot. A failure must not record the request as successfully applied. A disconnected resize needs one retry on restored attachment even if the viewport never changes again. Avoid an unconditional failure retry loop (authorization/validation failures should not spin).

Regression cases for the existing native tests: disconnected resize followed by reconnect without any new fit event; retry of an identical size after failure; latest-size coalescing while an earlier resize completes; no repeated successful identical-size RPC. This executable proves the session-state defect under deterministic transport failure, not a real-PTY screenshot or whole-app reconnect trial.

No production or tracked files changed for this reproduction.

## Authorized repair and validation

After the initial read-only reproduction, the parent assigned the repair. Production now retains desiredSize separately, records lastSize only after success, invalidates that acknowledgment on reattach, and reapplies desiredSize when its snapshot arrives. The existing transport/terminal-streams.swift suite gained two regressions covering failed identical retry, snapshot recovery without another fit, coalescing, deduplication, and absence of an automatic refusal loop.

`result-fixed.json` records the corrected behavior: three resize RPC attempts (initial success, disconnected failure, connected recovery), two attaches, final server grid 120 × 40, exit 0. The unchanged reproduction previously exited 1 with server grid 100 × 30.

Full native transport suite: 51 tests, two expected live-server credential skips, zero failures. All four TerminalStreamTests passed. Logs: build.log and transport-tests.log. `git diff --check` also passed. No real-PTY reconnect claim is made from these deterministic tests.
