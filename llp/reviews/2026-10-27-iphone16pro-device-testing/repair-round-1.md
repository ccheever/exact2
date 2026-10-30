# Review: 2026-10-27 Keychain repair, round 1

## Scope and verdict

- Task: repair the P1 in [PR 1056’s initial review](review-140996287a-attempt-1.md), under [local-auth D4–D7](../../1133.010-local-auth.rfc.md).
- Repository and base: `ccheever/exact2`, `main`; PR head `140996287af58a7205c4ec2bd88bd74c927006f4`.
- Reviewed implementation: the four local files fingerprinted below, over that head.
- Reviewer: `p1_repair_independent_review`; implementer: `p1_keychain_repair`. The reviewer made no changes and launched no tests.
- Verdict: **changes required**. The credential-loss regression passes; idle recovery lacks a scheduled wake.
- Scope: the repair, presence generation consumers, authentication boundaries, and runner/Apple scheduling. This is not a new review of the entire PR. Root runs the repository and native checks separately.

## Acceptance and verification

| Requirement / downstream promise | Command / recipe | Platform / OS / device | Application / framework revision | Result | Evidence |
|---|---|---|---|---|---|
| Preserve the durable credential after failed probes | `cargo test -p exact-apple --lib presence_secrets -- --nocapture` | macOS; deterministic Keychain hook | Fingerprinted local repair | Pass after repair; fail before | `/tmp/exact-p1-before.log`; `/tmp/exact-p1-final-apple.log`; committed regression in `host/apple/src/presence_secrets_tests.rs` |
| Pending moves, policy cleanup and queue order | `cargo test -p exact-runner --lib presence -- --nocapture` | macOS | Fingerprinted local repair | 17 pass | `/tmp/exact-p1-final-runner.log` |
| Recover without another tap in an idle app | Trace backend retry through `Runner::timer_due_ms` and `SessionClockTimer.wake` | Runner and Apple host source | Fingerprinted local repair | Fail by source inspection | Finding below |
| Full checks | Six repository checks, formatting and Contract differential quick check | macOS | Fingerprinted local repair | Pass; source unchanged | `/tmp/exact-p1-repair-gate-attempt-1/report.json`; native verification belongs to the final repair |

## Findings

**P2; local repair over `140996287a`; `runner/src/presence.rs:503`: expose reconciliation’s deadline to host scheduling.** A failed probe records `reconcile_at = now + 1000`, but `Runner::timer_due_ms()` does not include it. Apple schedules from that result; `SessionClockTimer.wake` returns no wake when it is absent. If boot and an unlock encounter the transient failure, the queued unlock can remain pending after the Keychain recovers while the app is idle. `call_due()` covers `extensions.calls`, not `extensions.presence.calls`. Backend tests manually pump at 1000 ms and bypass this boundary. Schedule a wake and prove that advancing to the returned deadline retries the existing unlock without another request. The regression must exercise the first returned deadline: `advance_steps` currently pumps presence before advancing `self.now_ms`.

## Risks and limitations

The scheduling finding is from source inspection, not a fresh runtime reproduction. The previous credential-loss paths are addressed by confirmed selection and deferred authentication. The reviewer checked every `generations()` consumer and found no other blocking defect. Hook tests do not measure physical Keychain failure frequency. Local logs are machine-local evidence; the regression sources and this review are the repository record.

Reviewed SHA-256 values:

```text
735f4bc2465b2640d48f752b568a70ebbe31d532fda72e4805c78ce67d66049c  host/apple/src/presence_secrets.rs
d9fa8f1736cc0a2b6150ac85327cddb1f8e0b39f9bf6bd6f249ffc33ab69c7d1  host/apple/src/presence_secrets_tests.rs
250762e5779f19a0ddd593e3d0adc5b6889dbb191cae5406bcd1058dceb6dcc4  runner/src/presence.rs
928b3bdfdb85e6dfecf9c854cecdffa414da5528e47e3896926c01f695fd081d  runner/src/presence_tests.rs
```

## Repair handoff and attempts

Preserve pending occupancy, confirmed generation selection, queued operation order, and authentication only after successful reconciliation. Add a runner test with no authored timer: fail probes, queue unlock, recover the vault, obtain the deadline, advance to it, and verify the existing call progresses. Repeated failure must schedule a future deadline; recovery must remove it.

| Round | Head | Change attempted and reason | Result | Same blocker remains? |
|---|---|---|---|---|
| 1 | Local repair over `140996287a` | Defer failed probes and separate intentional deletion from generation selection | Credential preservation passes; idle recovery incomplete | Original loss covered; scheduling repair required |

## Next action

The user authorized the P1 repair. Complete the scheduling repair, repeat affected checks and independent review, and preserve this failed attempt. No publication or merge occurred. The remote head remains the earlier reviewed head; local verification does not qualify it.
