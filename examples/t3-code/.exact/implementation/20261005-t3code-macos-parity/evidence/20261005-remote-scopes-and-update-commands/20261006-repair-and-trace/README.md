# Repair and complete T0 capture — 2026-10-06

This ticket's acceptance criteria pass on implementation commit
`759779342dd34fa516bec36924923e6beeb4c23a`, integrating base `587798ca8c0f5c2c761a4219263edce8256d50d8`.
Full RPC equality is **not** claimed. The full comparison was performed and its differences are retained.

## Changes

- The AppKit custom button supplies `focusRingMaskBounds` and explicitly uses the exterior focus ring.
  Its regression test checks focus, local mask coordinates after resize, and exclusion of a plain box.
- Integration with merged environment routes preserves the standard five-scope request on Add route
  and SSH route pairing. Both new branches have tests; the route and remote-scope Swift tests remain registered.
- U12 is resolved by the user's explicit answer: retain existing three-scope sessions without a notice.
  Extra permissions arrive only after the user pairs again. No session migration was added.

## Correcting the focus finding

`focus-screen-region.png` shows the blue focus ring around the update icon. Independent review confirmed it.
`keyboard-focus.png`, captured immediately after keyboard input, does not show it; the later
window capture `focus-window-settled.png` does. The screen-region capture also shows it.
This is not evidence that ScreenCaptureKit cannot capture focus rings. Capture timing/state was
not controlled well enough to infer an absent on-screen ring from the first image.
The prior failed evidence remains unchanged; this attempt supersedes its focus conclusion for the repaired implementation.

The diagnostic log records the correct first responder, key window, nonempty 24×24 mask and calls to
`drawFocusRingMask`. The screen-region capture used that instrumented build. Only temporary logging
was removed afterward; the focus implementation is unchanged in the final normal-app T0 drive and
four passing native button tests. No speculative changes to layer backing or paint order were made.
Tab reaches the control, and Space and Return copy the expected command and show the success toast
(`keyboard-results.json`). Clipboard text was restored during that keyboard check.

**Capture rule:** do not diagnose a missing focus indicator from the immediate post-input image
alone. Confirm the focused control and inspect an existing later capture or one settled capture.
A screen-region capture is a cross-check, not evidence that window capture excludes the indicator. The capture used here was
`screencapture -x -R36,38,1280,840 focus-screen-region.png`; coordinates came from this fixture's window.

## Full protocol comparison

Both clients used the actual reference backend at `1e2ecbd975` through the redacting loopback proxy.
The native client was the final normal bundle; the oracle was the actual Electron 44.4.2 desktop build.
The stopped backend data directory was snapshotted and restored between clients. Both scenarios contain:

1. Standard pairing.
2. Opening the new-thread view in the fixture project.
3. Launching the thread with the verification prompt.
4. Accepting the deterministic provider's command-approval request.
5. The final `T0_READY_FOR_APPROVAL` assistant output.

`report.json` confirms matching ordered scopes in **both request and granted response**, HTTP 200,
successful launches and `accept` responses, nonempty thread streams and the assistant message.
The requested/granted order is `orchestration:read orchestration:operate terminal:operate review:write relay:read`.
Credentials, bearer tokens and WebSocket tickets are redacted; the raw capture was additionally scanned
for the two minted credentials and fixture owner seed.

The original five-minute pairing credential expired before the oracle exchange after restoration.
That failed setup attempt is preserved in `trace-full.ndjson.gz`. A fresh link with the same label and
five scopes succeeded. `trace-selection.json` records the exact raw hash and each client's last
`T0-pair` boundary used for `trace-scenario.ndjson.gz`; every subsequent record for that client is retained.
Earlier native marks had no network calls. No unsuccessful app operation was relabeled successful.

The normalized comparator reports **176 differences, zero allow-listed**. Both clients pass scenario
coverage. Differences include browser CORS preflights, client identity strings, polling, subscriptions,
thread-visit calls, timestamps inside opaque payloads, and initial-message whitespace: native retains a
trailing newline while the oracle trims it. Core launch fields otherwise match, as does the approval
`accept` decision. The two full protocol streams are not equivalent. These differences have not been
shown to regress the scope or update-command changes; this ticket's trace acceptance row requires the
ordered scope exchange, not equality of every unrelated RPC. Independent review agrees with that scope.

The checker treats `previewAutomation.connect` as streaming because reference `packages/contracts/src/rpc.ts:1412–1417`
explicitly declares `stream: true`. An earlier checker incorrectly required a terminal unary `Exit`.
Synthetic checks reject empty scenarios, missing results, changed scopes/results/writes and unknown frames.

Artifacts include the redacted raw and selected traces, full normalized native/oracle events,
full difference report, and `trace-diff-final-tally.md` for all reference RPC method names.
Read an archived artifact without modifying it, for example:

```sh
python3 -c 'import gzip,json; print(json.load(gzip.open("trace-diff-final-report.json.gz"))["coverage"])'
```

## Checks and reused evidence

| Check | Result |
| --- | --- |
| TypeScript | 1,829 pass, 1 skip, 0 fail; strict typecheck passes |
| Root Rust tests | 2,928 pass, 0 fail |
| App Rust tests | 10 pass, 0 fail |
| Full Apple host suite, after mask repair | 796 tests, 2 skipped, 0 failures |
| Final targeted native-button tests | 4 tests, 0 failures |
| Final merged transport tests | 47 tests, 2 live skips, 0 failures |
| Build, clippy, formatting, caps, boot | Pass |
| Final normal macOS bundle | Built, launched and driven through T0 |

The initial root/app Rust commands inherited Bun 1.3.14 in subprocesses and failed the repository's
version requirement. Their failure logs are preserved. The runs with Bun 1.4.2 on PATH pass.
The earlier Apple test invocation using the T3 app's resources failed Caltrain-fixture assumptions;
the default Caltrain-backed suite passed. The final change to explicit focus-ring type was checked by
the targeted button suite instead of another full suite run.

[Previous complete verification](../20261006-complete-verification/README.md) retains the passing
28 installation/size/theme cases on each app, narrow-link refusal, update-copy checks and legacy
session/message/approval evidence. Those unchanged command helpers were not driven through the matrix
again. This repair adds route integration tests, the corrected focus evidence and complete trace comparison.

## Delivery and cleanup

PRs [147](https://github.com/ccheever/exact2/pull/147),
[148](https://github.com/ccheever/exact2/pull/148) and
[155](https://github.com/ccheever/exact2/pull/155) were confirmed merged and their integration base is
included locally. Historical clone-on-main and reusable-oracle task records do not identify separate
merged PRs; this feature's direct evidence does not mark those separate tickets delivered.
No remote push, PR update or merge was performed for this repair.

Only recorded fixture processes were stopped. Both fixture Keychain accounts and fixture preference
domains were removed. The real `~/.t3` directory's mtime is unchanged; this is a metadata check, not a
recursive content audit. `cleanup.json` records the exact scope. Temporary capture tools and private
fixture data remain ignored under `target/`; no new maintained verification tool was added.
