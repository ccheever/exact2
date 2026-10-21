# Code review: build info in the development menu, round 1 (2685abe21), 2026-10-08 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree.
- **Method:** one brief (sha256 `faba537bad1c849d50579309c29d09f0655de540866eb7f47d2fee73db1b734a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** All taken: build.mjs back within the cap (the stamp re-exported from assets.mjs); the test's age and local time hold in any zone; a bare macOS executable reads its sidecar (`ExactEnv.appMetadata`); the iOS sample host is stamped (the macOS host is not: it has no dev menu); a distributed build is `release`; a new untracked file makes a tree dirty.

---

DO NOT LAND

1. **BLOCKING — Line-cap check fails.** `bun scripts/caps.mjs` reports `host/apple/build.mjs` at **1,501 lines**. The [checker counts the trailing newline](scripts/caps.mjs:154). Trim the file below the enforced cap.

2. **BLOCKING — The new Swift test fails.** Its timestamps differ by 7,199.877 seconds; [the formatter truncates to hours](host/apple/Sources/ExactKit/BuildInfo.swift:32), producing “1 h ago,” while [the assertion expects two](host/apple/tests/ExactKitTests/BuildInfoTests.swift:17). The fixed date assertion also fails in UTC−11. Correct the fractional-boundary expectation and make the date assertion timezone-aware.

3. **MATERIAL — Bare macOS executables show no build metadata.** [The menu reads only `Bundle.main`](host/apple/Sources/ExactKit/Mac/DevMenuMac.swift:273), bypassing the stamped sidecar. Use the existing [ `ExactEnv.appMetadata` fallback](host/apple/Sources/ExactKit/Session.swift:73) and test this launch path.

4. **MATERIAL — Sample-host builds miss stamping.** [The iOS host plist](host/apple/build.mjs:1348) receives none of the new keys. Mac host publication [removes the standalone sidecar](host/apple/build.mjs:1208) without providing a host equivalent. Stamp host metadata too, preserving its distinct identity; test both packaging paths.

5. **MATERIAL — Distributed Mac builds can report “debug.”** [Kind depends only on production trust](host/apple/buildinfo.mjs:29), but [Swift selects release for `production || distribution`](host/apple/build.mjs:751). Default `exact release` takes this mismatched path. Derive the label from the actual configuration, with archive taking precedence, and test distribution with development trust.

6. **MATERIAL — Untracked source changes report clean.** [`--untracked-files=no`](host/apple/buildinfo.mjs:16) excludes newly added source files that the build can consume. Include nonignored untracked files and pin that case in the dirty-state test.

Copy includes the displayed information, and tvOS retains its pasteboard guard. Stamping occurs after link-reuse decisions and before signing; wall-clock timestamps make packaged outputs non-reproducible byte-for-byte. Custom plist keys are [permitted by Apple](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/AboutInformationPropertyListFiles.html).

Read-only Bun, Foundation, plist, missing-git and non-repository probes ran. Full Apple builds/XCTest were not run.
