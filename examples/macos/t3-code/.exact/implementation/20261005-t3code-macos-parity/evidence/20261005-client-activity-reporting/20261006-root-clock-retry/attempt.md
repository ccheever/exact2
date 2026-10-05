# Actual root workspace retry on the Exact clock

2026-10-06 KST. Current integrated native app1280×840, live reference server16843 via transparent verification proxy16844, selected isolated `verify-timeline` thread with Codex provider and fixture project cwd. Codex has no complete workspace snapshot. This is an existing thread's composer, not a new draft; both use the same root discovery resource/mutation and selected-workspace key. Partial snapshot field preservation has separate six-test logic coverage; this live run verifies the missing-snapshot retry scheduler.

The tab acceptance agent owned native process24661 and its tool stdin session12636. Tool sessions are agent-scoped, so it relayed the exact requested commands while the activity verifier independently read the resulting state files and trace counts. No menu was open, no app code changed, and proxy fault rules remained empty. Driver `target/t3-repair/gui/drive.mjs` used platform native timing; the Exact runner clock remained controlled by the driver.

1. `state`: clock0, workspaceDiscovery.neededtrue, retryAt0, no pending workspace mutation.
2. `clock +1000`; `state`: one actual `server.refreshProviders {instanceId:codex,cwd:<fixture>}` request1-33. Mutation completes at clock1000 with retrytrue; root sets deadline11000.
3. `clock +9000`; `state`: clock10000, same deadline11000, exact same request list. No retry before deadline.
4. `clock +1000`; `state`: clock11000, exactly one new request1-37 for the same provider/cwd/key. Mutation completes and next deadline becomes21000.

`capture.json` retains selected state fields, complete sanitized matching RPC request records, original local capture paths and SHA256 hashes, and the behavior-source hashes. Full original state files remain under `target/t3-repair/gui/activity-*.json`. The global proxy trace includes one older16:47 request, which remains in every request list and is not counted as part of this attempt. Assertions compare list prefixes rather than incorrectly treating the historical record as a fresh call.

Run:

```sh
python3 examples/macos/t3-code/.exact/implementation/20261005-t3code-macos-parity/evidence/20261005-client-activity-reporting/20261006-root-clock-retry/assert-capture.py
```

The command checks captured runtime behavior and source identity. It does not claim to recreate the GUI. No task/issue closure is awarded independently by this assertion. This proves the app-local X19 workaround; it does not resolve Exact's timer capability upstream.
