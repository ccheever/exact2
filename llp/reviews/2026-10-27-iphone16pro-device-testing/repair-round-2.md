# Review: 2026-10-27 Keychain repair, round 2

## Scope and verdict

- Task: repair the P1 in [PR 1056’s initial review](review-140996287a-attempt-1.md), under [local-auth D4–D7](../../1133.010-local-auth.rfc.md).
- Repository and base: `ccheever/exact2`, `main`; published head at source review `140996287af58a7205c4ec2bd88bd74c927006f4`.
- Reviewed implementation: eight local source/test files fingerprinted below, plus D6 documentation.
- Reviewer: `p1_repair_independent_review`; implementer: `p1_keychain_repair`. Root owns final verification. The reviewer made no changes and launched no tests.
- Verdict: **repair verified**. Independent source review has no blocking findings; final repository checks and native verification pass.
- Scope: the complete repair and its generation consumers, runner scheduling, store publication, and Apple authentication delivery. This does not repeat the full-PR review.

## Acceptance and verification

| Requirement / downstream promise | Command / recipe | Platform / OS / device | Application / framework revision | Result | Evidence |
|---|---|---|---|---|---|
| Preserve credentials after interrupted indexed writes and failed probes | `cargo test -p exact-apple --lib presence_secrets -- --nocapture` | macOS; deterministic Keychain hook | Fingerprinted local repair | Pass; the regression failed before repair | `/tmp/exact-p1-before.log`; `/tmp/exact-p1-round2-final-apple.log`; `host/apple/src/presence_secrets_tests.rs` |
| Idle recovery, queue order, store publication, authentication ownership and virtual request stops | `cargo test -p exact-runner --lib presence -- --nocapture` | macOS; deterministic vault and runner | Fingerprinted local repair | 22 pass | `/tmp/exact-p1-round2-final-runner-v2.log`; `runner/src/runner/kept_presence_tests.rs` |
| Required repository checks and Contract semantics | `cargo build --all-targets --keep-going`; `cargo test --tests --no-fail-fast`; `cargo clippy --all-targets --keep-going -- -D warnings`; `cargo fmt --all -- --check`; `bun scripts/caps.mjs`; `bun scripts/boot.mjs`; `cargo check --workspace --all-targets --keep-going`; `cargo run -p contract-difftest -- quick` | macOS 27.2, Bun 1.4.2, Rust 1.97.0 | Fingerprinted local repair | All pass; 6,838 tests pass, 60 ignored; no semantics divergence | `/tmp/exact-p1-repair-gate-attempt-2/report.json` |
| Native authentication lifecycle | `bun packages/local-auth/test/apple.mjs ios --sim 903B96C3-CFA6-44B3-8E06-F554E8A2D56F` | Fresh iPhone 18 Pro simulator, iOS 27.0 | Fingerprinted local repair | 13 checks pass; 0 fail | `/tmp/exact-p1-repair-native-attempt-1/001.stdout.log`; fixture `com.example.lockfixture-efb1f1` |
| macOS capability refusal | `bun packages/local-auth/test/apple.mjs macos` | macOS 27.2, unprovisioned fixture | Fingerprinted local repair | 4 checks pass; 0 fail | `/tmp/exact-p1-repair-native-attempt-1/002.stdout.log` |

## Findings

No blocking defect found in the final scoped source review. Failed probes select no generation and retire no credential. Recovery deadlines reach the host scheduler; advancing to the first deadline retries the waiting operation. Recovery also commits changed occupancy. Pending-to-plain recovery refreshes existing store readers, and its unchanged regression verifies a resulting data request stops virtual advance. Failed deletion retries are bounded, and refused writes wait while authentication owns a generation.

## Risks and limitations

The deterministic failure injection proves recovery logic, not the frequency of physical Keychain failures. Simulator authentication does not qualify Secure Enclave behavior. Its isolated simulator was removed after the suite passed. This repair does not repeat the earlier physical-device tests across PR 1056. The follow-up Excord device install remains blocked by its missing Declared Age Range provisioning entitlement. Logs and detailed verification reports under `/tmp` are machine-local; the committed regression sources and this review provide the reproducible repository record.

Reviewed SHA-256 values:

```text
735f4bc2465b2640d48f752b568a70ebbe31d532fda72e4805c78ce67d66049c  host/apple/src/presence_secrets.rs
3221fe470d5b6fb99f8fdd1769974a5b5d8834f097fa70f3a8d377f5363dd4fa  host/apple/src/presence_secrets_tests.rs
26db3a1480f38ca2e097627dbb28a97f79575710dca7c156790fa0dd1143ecad  runner/src/presence.rs
c9d7b927b22759623ceb2d0d1fd4eb1d3a93fa551ba4102046d293d68bb46304  runner/src/presence_tests.rs
8e7fc79497a9e5445c1330424ae7d05d78fe5c427189eab15d63a5d0e8f7aef8  runner/src/runner/advance.rs
a139d5cbffd900431c87778a5626ed3f1fbcdcec40a60f0b357bd7a1ae883d32  runner/src/runner/gates.rs
32a7ac7cd49f06bc04501da89be0f6ebe9c154081a9d7bc2dd82c638308b48c6  runner/src/runner/kept_presence_tests.rs
b8057aec426e10e5bc8ae847ab67b22ec3f1573dfc4c56643b70816d36e4dcc6  runner/src/runner/modules/presence.rs
23fc4b2b8967b6c7a41fe0f6ac8a65221955fd8a9c61c9a967db480c42218b99  llp/1133.010-local-auth.rfc.md
```

## Repair handoff and attempts

The [first repair review](repair-round-1.md) preserves the missing-deadline finding. Three runner regressions then failed before scheduling was connected (`/tmp/exact-p1-round2-before.log`). The virtual-stop regression exposed missing reader refresh in the intermediate implementation (`/tmp/exact-p1-round2-final-runner.log`); it passes unchanged after that correction.

| Round | Head | Change attempted and reason | Result | Same blocker remains? |
|---|---|---|---|---|
| 1 | Local repair over `140996287a` | Defer failed probes and separate intentional deletion | Credential preservation passes; idle retry wake missing | Scheduling remained |
| 2 | Fingerprints above | Schedule and execute retries, publish recovery, bound failed deletions, retain authentication ownership | Focused tests pass; independent source review has no blocking findings | No |

## Next action

All nine reviewed file hashes match. The final gate and native reports both record `status: passed`, `source_unchanged: true`, and source digest `499857dd0a7f4535215717b87384ef7cb5695a7204d21a6fa29bc48abab86c63`. Their recipes are `/tmp/exact-p1-repair-gate-recipe.json` and `/tmp/exact-p1-repair-native-recipe.json`. The native fixture creates its own stores; to repeat iOS verification, create an iPhone 18 Pro simulator with iOS 27.0 and pass its new UDID to the recorded command.

The remote branch was rewritten to `1d8fde25ff083c68f553e003b3f44f7d29aaa199` during verification. Its tree differs from the reviewed base only by removal of one unrelated document; no implementation file changed. The local repair was rebased onto that head and preserves the deletion. The repository aliases `ccheever/exact47` and `ccheever/exact2` resolve to repository ID `1348429835`. Publication and merge remain unperformed; a changed implementation requires fresh verification.

Verified implementation commit: `1aabfea946537049a9a664ffcb1ea9e61db04b42`. A separate checkout of this exact revision matches both passing verification reports, including their recipe identities. The comparison record is `/tmp/exact-p1-committed-proof.json`. Only this verification record changes in the subsequent bookkeeping commit.
