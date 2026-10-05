# Activity verification: native component and real timer

Source: `8498fdc8a0a4b7b5b310f4be1a9f3a38fcfee0e2`, on `daehyeon/t3code-parallel-features`.
macOS 26.6.2 arm64; Xcode 27.0 (27A266a); Bun 1.4.2; Python 3.9.6.
Captured 2026-10-06 Asia/Seoul (logs use UTC, previous calendar day).

No implementation edits. The coordinator owns GUI interactions; this attempt deliberately
uses the real reporter with `observeWindows: false`, in-process sender callbacks and
fresh UUIDs. It makes no backend connections and reads no personal preferences.

## Reproduce native and workspace checks

From repository root:

```sh
python3 /Users/daehyeonmun/.agents/skills/verify/scripts/run_checks.py examples/macos/t3-code/.exact/implementation/20261005-t3code-macos-parity/evidence/20261005-client-activity-reporting/20261006-native-and-cadence/acceptance-recipe.json --project-root . --output /tmp/new-activity-attempt
```

The installed skill and pinned Bun paths are host-specific; substitute those executables
on another machine. `run-native.py` compiles all current app native sources, framework
module facade and freshly generated data keys before running seven XCTest assertions.
The second command runs six workspace discovery regression tests. Both pass.
The original `checks/` report covers only those executable assertions. The subsequent
`acceptance-checks/` report additionally fingerprints both capture helpers and records
the unavailable mandatory oracle check; its blocked status is the appropriate acceptance
verdict, regardless of passing component tests.

## Real-time component capture

Copy `cadence-main.swift` to `target/t3-verify-activity/cadence/main.swift`, then:

```sh
xcrun swiftc -swift-version 5 examples/macos/t3-code/modules/apple/T3ActivityReporter.swift target/t3-verify-activity/cadence/main.swift -o target/t3-verify-activity/cadence/run
target/t3-verify-activity/cadence/run
```

The actual reporter is unmodified. A and B connect at launch, a VCS scope is retained
at 2 seconds and released at 3, B disconnects at 4, reconnects at 5 and disconnects at 6.
The process runs 80 seconds, recording every real callback payload and elapsed wall time.
Assertions require reports around 0.25, 25.25, 50.25 and 75.25 seconds, scope removal,
no B reports after its final disconnect and expired interaction after 45 seconds.
This supports native timer/queue/scopes/environment logic, not AppKit focus/pointer,
wire transport, provider server policy or Electron parity.

## Acceptance coverage

| Requirement | This attempt |
| --- | --- |
| Four named pure activity tests, queue/payload/identity regression checks | Seven native tests pass |
| A10 partial workspace discovery and completion/reset regression checks | Six Bun tests pass |
| Real25-second cadence, disconnect and scope release | Actual native reporter callback capture; see cadence.log |
| Focus, occlusion, activation within0.3s | Unobserved; coordinator GUI lane required |
| Real pointer after50s inactivity | Unobserved; no synthetic input substituted |
| On-wire scopes and several environment lifecycle | Component evidence only; actual app/backend trace still required |
| Provider checkedAt baseline versus new app, deactivated expiry | Not run; no controlled baseline/new provider-interval fixture supplied |
| Silent failure stopping backendB | Unobserved at integrated app level |
| Root clock retry at+9000/+10000 | Logic passes; actual Contract clock drive still required |
| Electron trace comparison | Blocked: referenced trace-proxy/trace-diff/electron-oracle files absent in current checkout and mc-orch target |
| Framework main pin and oracle dependency | Pending in ticket, not resolved by these tests |

No issue resolution or task closure is claimed. X19 workaround's root-clock behavior
requires integrated observation. X21 and X28 app-side workarounds do not resolve framework
capabilities upstream.

Observed real timer callbacks:25.272s,50.272s,75.272s. Interaction false at50.272s and75.272s; final cadence/scopes/disconnect/expiry assertions PASS, process exit0.
