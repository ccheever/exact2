# Review: 2026-10-27 iPhone fixes after main integration

## Scope and verdict

- Task and parent specification: [device testing, results](../../1133.020-device-testing.plan.md#8-results), [local authentication D4–D7](../../1133.010-local-auth.rfc.md).
- PR: [1056](https://github.com/ccheever/exact2/pull/1056), `ccheever/exact2`, base `main`.
- Reviewed implementation: `397c6b0df23ba7223800dee31d5fe048d497eeee`. Remote head before publication: `1d8fde25ff083c68f553e003b3f44f7d29aaa199`; the published-head review is recorded separately after pushing.
- Integrated main: `8f069c8f4ef2d6f288101170ffe1491df95a9cdc`.
- Reviewer: `p1_repair_independent_review`; implementer: `p1_keychain_repair`; root owns verification and publication.
- Scope: all PR changes and integration overlaps, credential preservation and recovery, Excord navigation, and the two Swift fixture repairs. The [earlier review](review-140996287a-attempt-1.md) and [credential repair](repair-round-2.md) retain their evidence.
- Verdict: **local verification passed**; independent source review found no blocking findings. Review of the published head follows the push.

## Acceptance and verification

Environment: macOS 27.2, Bun 1.4.2, Rust 1.97.0; owned iPhone 18 Pro simulator with iOS 27.0. The browser TLS fixture uses Homebrew OpenSSL 3.6.3 ahead of Apple's LibreSSL on PATH.

| Requirement / downstream promise | Command / recipe | Platform | Tested source | Result | Evidence |
|---|---|---|---|---|---|
| Repository checks and Contract semantics | Commands below | macOS | Final fingerprint below | All pass; 6,856 tests pass, 60 ignored; no semantics divergence | Final gate report |
| Hosted UIKit and WebKit behavior | Rebuilt `ExactKitTests`; `xcodebuild test-without-building`, hosted selection from `hostedTests`, serial, `EXACT_REQUIRED=1` | iOS 27.0 | Final fingerprint below | 199 pass; zero failures or skips | Hosted report |
| Bare iframe helper callers | Same rebuilt bundle, nine `IframePartitionTests` methods excluding the hosted snapshot, serial, required | iOS 27.0 | Final fingerprint below | 9 pass; zero failures or skips | Final gate report |
| Native authentication lifecycle | `bun packages/local-auth/test/apple.mjs ios --sim <fresh-UDID>`; `bun packages/local-auth/test/apple.mjs macos` | iOS 27.0; macOS 27.2 | Pre-fixture-repair source fingerprint below | 13 iOS and 4 macOS checks pass | Native report, checks 1–2 |
| Background lifecycle and overlap | `bun packages/background/test/apple.mjs macos ios --sim <fresh-UDID>` | Apple | Pre-fixture-repair source fingerprint | 23 checks pass | Native report, check 3 |
| Shared Apple source compilation | `bun host/apple/build.mjs --tvos metronome-apple --no-install` | tvOS SDK | Pre-fixture-repair source fingerprint | Pass | Native report, check 5 |
| Game engine | In `game/`: `cargo test --workspace --locked --no-fail-fast` | macOS | Pre-fixture-repair source fingerprint | 1,181 pass; 25 ignored | Native report, check 6 |
| Apple linking and manifests | `bun test host/apple/link.test.mjs scripts/app-apple.test.mjs` | macOS | Pre-fixture-repair source fingerprint | 39 pass; 1 skip | Native report, check 7 |
| Motion readers | `bun packages/motion/test/web.mjs` | Chromium, Firefox, WebKit | Pre-fixture-repair source fingerprint | Pass in all three | Native report, check 8 |
| Recorder | `bun host/web/build.mjs recorder`; `bun test apps/recorder/app.test.mjs` | Web | Pre-fixture-repair source fingerprint | 3 pass | Native report, check 9 |
| Location request failure | `bun host/web/build.mjs weatherlight`; `bun test apps/weatherlight/app.test.mjs -t 'location request failures remain visible'` | Web, Homebrew OpenSSL | Pre-fixture-repair source fingerprint | 1 pass, 8 assertions; 3 tests filtered | Weatherlight report |
| Harmony foreground/background work | `bun packages/background/test/harmony.mjs web` | Chromium | Pre-fixture-repair source fingerprint | Pass | Native report, check 11 |
| Excord compact Back, resize, and wide browser Back | Web fixture at 420×900 and 1100×900; open a channel, scroll, navigate away and back | Chromium | Pre-fixture-repair source fingerprint | Compact control and resize pass; all six history checkpoints pass, saved reader offset 56.40625, resumed visit 2 → 3 | Excord history artifacts |

Repository commands:

```sh
cargo build --all-targets --keep-going
cargo test --tests --no-fail-fast
cargo clippy --all-targets --keep-going -- -D warnings
cargo fmt --all -- --check
bun scripts/caps.mjs
bun scripts/boot.mjs
cargo check --workspace --all-targets --keep-going
cargo run -p contract-difftest -- quick
```

The original Swift run also passed 1,996 unhosted tests and the separate GPU test. The later complete hosted run and nine bare iframe tests qualify the two repaired fixture files. For a fresh build, use `bun host/apple/build.mjs --test --ios --sim <fresh-UDID>`; the serial follow-up uses that build's `.xctestrun` files and the same fixture/module mappings from `compileFixtures`. It does not inject the diagnostic observer.

## Findings

No further source defect was found in the reviewed PR. After integrating main, Excord's compact Back control needed structural conditional rendering to pass the native-button source check. Its wide layout preserves scroll position through navigation leave and browser Back.

Two unrelated Swift fixture defects prevented complete native qualification. The keyboard test hid its old window and opened the next before UIKit's `keyboardDidHide` notification. The diagnostic trace showed the next window becoming key at 05:59:27.210, before the previous dismissal at 05:59:27.439. The following scenario then lost focus and failed its keyboard assertions. The test now awaits dismissal before replacing the window; all responder and layout assertions remain. The iframe fixture now attaches its window to the available hosted scene and mounts through a root controller, retaining a bare-process fallback. Its visible and hidden RGB snapshot assertions remain unchanged. Each repaired test passes in isolation with no skips.

## Risks and limitations

Earlier physical-device observations remain in the device report; simulator checks do not qualify Secure Enclave behavior or repeat the phone's manual drives. The repaired Excord device install remains blocked by a development profile without the Declared Age Range entitlement. Cloud-only Photos downloading, incoming-call interruptions, notification appearance/tap, and the other unverified device cases remain explicitly recorded. Beacons' corrected mapping is covered by its regression and controlled-sensor device proof; the tester's completed tilt and rotation checks are not represented as a new physical drive of this final revision.

The location regression passes; this does not claim the complete Weatherlight suite passes. Its two previously observed contrast failures remain outside the location repair. Apple linking skips the existing assembled widget/push-to-start check. The root and game suites retain their declared ignored tests.

## Repair handoff and attempts

| Attempt | Change / reason | Observed result | Same blocker remains? |
|---|---|---|---|
| Main integration | Preserve PR changes over `8f069c8f4e`; render Excord Back structurally | Gate passes after the source-file cap and stale navigation comment are corrected | No |
| Native run | Exercise shared hosts, packages and consumers | Hosted keyboard assertion, snapshot skip, and Weatherlight TLS setup failure; all other checks pass | Resolved by the following scoped checks |
| Weatherlight fixture environment | Restore the earlier tested OpenSSL PATH | Location regression passes without source changes | No |
| Swift fixture repairs | Wait for keyboard dismissal; attach snapshot window to hosted scene | Focused tests each pass, no skips; complete hosted group (199) and bare iframe group (9) also pass without skips | No |

The first focused wrapper attempt stopped at its default 60-second timeout; the corrected 1,800-second attempt reproduced the keyboard assertion. Neither is described as a passing run. The initial native report retains its failed status; later reports supersede only the affected checks.

The committed tests, commands, and this report are the repository evidence. Recipes and complete logs remain under `/tmp`; they are not remote artifact links:

- Native report: `/tmp/exact-pr1056-merge-native-attempt-1/report.json`.
- Weatherlight report: `/tmp/exact-pr1056-weatherlight-verification-1/report.json`.
- Hosted report: `/tmp/exact-pr1056-hosted-verification-1/report.json`.
- Final gate report: `/tmp/exact-pr1056-final-gate-attempt-1/report.json`.
- Recipes: `/tmp/exact-pr1056-merge-native-recipe.json`, `/tmp/exact-pr1056-weatherlight-recipe.json`, `/tmp/exact-pr1056-hosted-recipe.json`, `/tmp/exact-pr1056-final-gate-recipe.json`.
- Excord history artifacts: `/tmp/exact-pr1056-excord-back-history-web/`.

Pre-fixture-repair source fingerprint: `2c5d975fbeda0d454218eb27884b871eccac0907d59475332f5ebb7c6dc7b0b2`. Comparing every source entry against the final tree finds only `NavigationBarIOSTests.swift` and `IframePartitionTests.swift` changed after that run. No production, package, app, dependency, or fixture-module source changed. Final fingerprint: `e8d268ca5ae93580d1453eaeaa02c0a316d2eef1a55c088db77cf83f5e5bd92f`.

## Next action

Implementation `397c6b0df23ba7223800dee31d5fe048d497eeee` is committed. The final gate and hosted reports both record `status: passed`, `source_unchanged: true`, and the final fingerprint above. Publish the verified branch with ordinary fast-forward protection, review that exact published head, then merge under the user’s explicit authorization. Remote publication and merge are not claimed by this local verification record.
