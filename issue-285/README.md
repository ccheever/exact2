# Issue #285: monotonic real-time clock evidence

Issue: https://github.com/ccheever/exact2/issues/285

Baseline: `fa965d3e2b36417a22228900678f7abc68e72270`.
Verified implementation: `30d3143aaa1e397a121f24203ea5b7bbdf7ef6b1` on `daehyeon/fix-285-agent-clock`.

The driver now subtracts wall-clock samples before adding elapsed time to the app clock, then clamps each request to the last clock reported by the host. The host's strict monotonicity guard and timer scheduling remain unchanged.

| Run | Actual result | Evidence |
| --- | --- | --- |
| macOS before | Fails after 3,313 completed calls: requested `3312.999999999998` after `3313`; one backwards request | [before-macos.log](before-macos.log) |
| macOS after | 40,000 calls, 80,000 seeks, zero backwards requests, exact final clock 40000 | [after-macos.log](after-macos.log) |
| iOS simulator after | 40,000 calls, 80,000 seeks, zero backwards requests, exact final clock 40000 | [after-ios.log](after-ios.log) |
| Quantized wall clock before | All six driver carrier branches request `249.99999999999997` after `250` | [before-deterministic.log](before-deterministic.log) |
| Quantized wall clock after | All six branches request `[250, 250.5, 251]` | [after-deterministic.log](after-deterministic.log) |
| New regression before | Fails on the first unchanged wall sample | [before-regression.log](before-regression.log) |
| Driver suites after | 61 passed, 0 failed, 116 assertions | [verification/001.stderr.log](verification/001.stderr.log) |

The native reproduction instruments every sent clock request and asserts every call ends exactly at its starting clock plus 1. The issue's Clock fixture has a frame task and a 7 ms timer. Both completed runs report 2,400 frames and 5,714 timer ticks. Screenshots are supplemental rendered-state evidence: [before macOS](before-macos.png), [after macOS](after-macos.png), [after iOS](after-ios.png).

## Reproduce

Use Bun 1.4.2 on PATH and Rust 1.97.0. Run from the framework checkout at the baseline or implementation revision. `CLOCK_EVIDENCE` is this evidence directory's absolute path.

```sh
bun install --frozen-lockfile
bun host/apple/build.mjs
cargo run --profile host-dev -p contract -- build "$CLOCK_EVIDENCE/clock.contract" -o "$CLOCK_EVIDENCE/clock.plan"
bun "$CLOCK_EVIDENCE/repro-native.mjs" macos before
# Run on the implementation revision for the after result:
bun "$CLOCK_EVIDENCE/repro-native.mjs" macos after
EXACT_SIM=FB4FCE92-4127-4ED8-8A51-9EA3BAAACC5C bun host/apple/build.mjs --ios
EXACT_SIM=FB4FCE92-4127-4ED8-8A51-9EA3BAAACC5C bun "$CLOCK_EVIDENCE/repro-native.mjs" ios after
bun "$CLOCK_EVIDENCE/repro-deterministic.mjs"
bun test scripts/caps.test.mjs scripts/agent-drag.test.mjs scripts/agent-keys.test.mjs
```

Choose an available simulator UUID on another machine. The recorded environment is macOS 26.6.2 arm64, Bun 1.4.2, Rust 1.97.0, and iPhone 17 Pro simulator on iOS 26.5; details in [environment.json](environment.json). The Clock plan is loaded through the built Caltrain presenter, with no Caltrain resources in the fixture.

## Validation and identity

All five repository checks passed:

- `cargo build --all-targets --keep-going`: [build.log](build.log), exit 0.
- `cargo test --lib --bins --tests --no-fail-fast`: [verification/002.stdout.log](verification/002.stdout.log), 3,539 passed, 0 failed, 34 ignored across 94 suites.
- `cargo clippy --all-targets --keep-going -- -D warnings`: [verification/003.stderr.log](verification/003.stderr.log), exit 0; `cargo fmt --all -- --check`: [fmt.log](fmt.log), exit 0.
- `bun scripts/caps.mjs`: [verification/004.stdout.log](verification/004.stdout.log), exit 0.
- `bun scripts/boot.mjs`: [verification/005.stdout.log](verification/005.stdout.log), exit 0.

The [verification report](verification/report.json) is `passed` with `source_unchanged: true`. Its declared source digest is `1cdd83ac5384e69df462a6916a6aac684d37db161c1ee509066a4936f158a164`. The exact committed files were extracted with `git archive` and compared using the same recipe: [committed-comparison.json](committed-comparison.json) reports `matches`, `source_matches`, and `recipe_matches` all true. [Committed identity](committed-identity.json) records the SHA and method. Independent review: [independent-review.md](independent-review.md), no findings.

No physical iPhone test or actual browser/Linux runtime drive is claimed. iOS coverage uses a simulator; carrier branches are additionally covered by the deterministic regression. The raw failed baseline, native logs, command helpers, fixture source, gate outputs, and source fingerprints are retained here. Initial unpinned-Bun setup failed; all reported validation and acceptance runs used pinned Bun 1.4.2.
