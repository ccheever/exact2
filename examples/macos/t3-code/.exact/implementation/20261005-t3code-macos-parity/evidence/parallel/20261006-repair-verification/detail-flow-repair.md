# Independent tool disclosure repair

The first mounted delayed-response capture found that the global command busy guard dropped a second tool-row click while the first disclosure awaited its output RPC. The loading state also remained hidden until that RPC completed.

`chatlocal:item-detail` now uses the local mutation path. Disclosure changes are synchronous; the existing awaited root read mutation owns all native requests. Opening another row wakes that read batch so its request can begin while an earlier row waits. Cache keys retain each response identity, and closing a row remains independent from completion.

The new regression opens A with a deferred response, opens B while A waits, completes B first, then closes A before its late response. It verifies immediate disclosure/loading, both request identities, and that the late response does not reopen A. All 1,198 Bun tests pass. Strict TypeScript, Contract compile, root build/clippy/format and boot checks pass through `checks-detail-flow/report.json` with unchanged source; staged source caps and the full macOS bundle also pass. This supersedes the earlier repair source fingerprint; prior reports remain historical evidence.

The final mounted replay runs against this rebuilt source. Its result and independent review determine task eligibility; regression checks alone do not close tasks.
