# Review: 2026-10-27 iPhone device fixes, PR 1056

## Scope and verdict

- Task and parent plan: [LLP 1133.020](../../1133.020-device-testing.plan.md), especially its device evidence in section 8. Relevant contracts: recording [1133.009](../../1133.009-recording.rfc.md) D7/D10; local authentication [1133.010](../../1133.010-local-auth.rfc.md); motion [1133.013](../../1133.013-motion.rfc.md) D6; background [1133.017](../../1133.017-background.rfc.md); photos [1133.019](../../1133.019-photos-image.rfc.md) D15/D16.
- PR: [ccheever/exact2#1056](https://github.com/ccheever/exact2/pull/1056), targeting `main`.
- Reviewed head: `140996287af58a7205c4ec2bd88bd74c927006f4`, verified on the remote at 2026-10-26 16:32:45 UTC (2026-10-27 KST). Merged base: `debb8aa3ed6183b1661b46ad022c127b7f33f4d8`.
- Reviewer: primary agent plus the separate `pr1056_independent_review` agent. The independent reviewer made no implementation changes and reviewed the complete 65-file diff and surrounding lifecycle code.
- Verdict: **changes required**. One confirmed P1 defect; no other concrete findings from the independent review.
- Conflict resolution: `host/apple/build.mjs` now preserves both explicit framework linking and the upstream tvOS package filter. Commit `140996287a` was pushed normally. A subsequent merge-tree check against `origin/main` at `87d48b96f9` also completed without conflicts.
- All six repository checks passed on the reviewed source. The final remote recheck reports `MERGEABLE` / `CLEAN`, both Socket Security checks successful, and no approval recorded.

## Acceptance and verification

All fresh commands below used this worktree and Bun 1.4.2 on macOS 27.2 (26B118). Native builds compiled and linked; they did not install or launch the apps.

| Requirement / downstream promise | Command / recipe | Platform / OS / device | Application / framework revision | Result | Evidence |
|---|---|---|---|---|---|
| Required repository checks | `cargo build --all-targets --keep-going`; `cargo test --tests --no-fail-fast`; `cargo clippy --all-targets --keep-going -- -D warnings`; `cargo fmt --all -- --check`; `bun scripts/caps.mjs`; `bun scripts/boot.mjs`; `cargo check --workspace --all-targets --keep-going` | macOS 27.2 | `140996287a` | Pass | [/tmp/exact-pr1056-review-checks.json](/tmp/exact-pr1056-review-checks.json), with per-command log paths |
| Framework selection and presence build policy | `bun test host/apple/link.test.mjs host/apple/presence.test.mjs` | macOS 27.2 | `140996287a` source | Pass: 12; skipped: 1 assembled-activity-app test | [/tmp/exact-pr1056-conflict-link-tests.log](/tmp/exact-pr1056-conflict-link-tests.log) |
| tvOS excludes iOS-only package frameworks | `bun host/apple/build.mjs --tvos metronome-apple --no-install` | tvOS compile target on macOS | `140996287a` | Pass | [/tmp/exact-pr1056-review-tvos.log](/tmp/exact-pr1056-review-tvos.log) |
| iOS retains category-only PhotosUI dependency | `bun host/apple/build.mjs --device --phone 00008140-001239380A98801C photo-editor-apple --no-install` | iOS device compile target on macOS | `140996287a` | Pass | [/tmp/exact-pr1056-review-ios.log](/tmp/exact-pr1056-review-ios.log) |
| Runner changes preserve Contract semantics | `cargo run -p contract-difftest -- quick` | macOS 27.2 | `140996287a` | Pass | [/tmp/exact-pr1056-review-semantics.log](/tmp/exact-pr1056-review-semantics.log) |
| Interrupted writes preserve the last locked secret | Indexed but absent generation 2; real generation 1; fail the first one or two generation-2 probes; reconcile, then unlock after recovery | In-memory Keychain hook using actual source and compiled runner | `140996287a` | **Fail: last real secret deleted** | [Harness](/tmp/exact-pr1056-presence-review/main.rs), [output](/tmp/exact-pr1056-presence-review/repro.log) |
| Physical-device behavior across the full PR | Recorded app drives and manual observations in plan section 8 | iPhone 16 Pro, iOS 27.0.1 | Earlier implementation revisions, through `9097e9c546` | Earlier evidence retained; unverified on the merged head | [Plan evidence](../../1133.020-device-testing.plan.md#8-results) |

## Findings

**P1; `140996287af58a7205c4ec2bd88bd74c927006f4`; `host/apple/src/presence_secrets.rs:233–236`: do not select an unconfirmed keychain generation.** Local-auth crash reconciliation must preserve the last durable credential. A crash after `add_locked()` indexes `key#2` but before it creates that Keychain item leaves real `key#1` and an empty candidate. If the probe of `key#2` fails, `generations()` nevertheless returns generation 2; `Backend::reconcile()` selects it as newest. When the retirement probe subsequently reports “not found,” `delete_locked()` ignores that successful-call status and deletes `key#1`. If both boot probes fail, generation 1 initially survives, but the backend still selects generation 2; a later unlock reads the missing item and `invalidated()` deletes the surviving credential. Both paths were reproduced without accessing the real Keychain. The result is loss of the user's stored credential. Keep unconfirmed candidates separate from confirmed generations, and defer selection and retirement until existence is established; merely preventing deletion while a probe fails does not protect the later unlock path.

## Risks and limitations

- The reviewer read all 65 changed files and surrounding lifecycle code. Source review and compile results do not replace physical-device observations.
- The earlier iPhone evidence is not a fresh runtime verification of the merged head. No device, simulator, or GUI interaction was performed during this review. The linked plan retains its existing physical-tilt and cloud-only-photo evidence limits.
- The Keychain reproducer uses a deterministic hook to model transient probe failures and a crash between indexing and adding an item. It proves the backend's destructive behavior under those results; it does not measure how often an iPhone encounters that sequence.
- One linking-suite test requires an assembled activity app and was skipped. Platform builds succeeded with warnings; their logs preserve those warnings.
- Compatibility with pre-1.0 stored formats is outside this review's required repairs under `rules/RULES.md` and `rules/DEFERRED.md`. The P1 concerns crashes within the new format itself.

## Repair handoff and attempts

Preserve the silent, per-account Keychain probe, authenticated secret reads, and the guarantee that a failed probe does not erase the last durable value. The issue spans `KeychainVault::generations`, `delete_locked`, `Backend::reconcile`, and the missing-item unlock path. Review local-auth D6/D7 and the existing presence tests before changing that interaction.

The isolated harness is [/tmp/exact-pr1056-presence-review/main.rs](/tmp/exact-pr1056-presence-review/main.rs). It includes the reviewed `presence_secrets.rs` directly and links the worktree's `exact_runner`, `ibex2`, and `serde_json` libraries. Run `/tmp/exact-pr1056-presence-review/repro`; it resets the in-memory stores for each case and makes no real Keychain calls. The observed output is:

```text
transient during reconciliation: after boot generation=2 items=[]
unlock after boot probes recover: after boot generation=2 items=["key#1"]
unlock after boot probes recover: after unlock occupancy=Some(Absent) items=[]
```

Add regression coverage for both failure schedules, including recovery and unlock in the same process. Require the last confirmed credential to remain accessible, without another restart. Re-run the focused presence tests, the six repository checks, Contract differential checks if runner behavior changes, and relevant native authentication/device checks. A changed head needs fresh independent review.

| Round | Head | Change attempted and reason | Result | Same blocker remains? |
|---|---|---|---|---|
| Conflict resolution | `140996287a` | Preserve explicit framework linking and tvOS filtering while merging main | Pushed; required checks and both platform builds pass | Yes: the independent P1 was found during review |
| Initial review | `140996287a` | Read the complete diff and reproduce candidate reconciliation failures | Both credential-loss paths reproduced; no repair dispatched | Yes |

## Next action

Repair the P1 before merging. Remote head was rechecked after the push and matches the reviewed commit. No GitHub review or comment was published. The [pr-review skill](/Users/daehyeonmun/.agents/skills/pr-review/SKILL.md) requires the initial review to “stop here before dispatching repairs”; a repair round or publication of the review needs the user's approval. Repair rounds attempted: zero; consecutive failed repair rounds: zero. A new head requires a new verdict.
