# Code review: release notes and fuller build info in the development menu, round 2 (1a88de7ae), 2026-10-08 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree.
- **Method:** one brief (sha256 `60cf5d9782e914cab3b6ce95cd4af8c031c7f6f493cae735e79d68e018178f51`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: the bytes read (one past the cap) are decoded streaming and fatal, so only a valid incomplete last character is withheld and anything else fails (tested with 16 KB of `0x80` and a stray byte past the cap). 2 taken: the Xcode and SDK parts are composed independently. 3 not taken: no UI test; Grok observed the iOS 27 simulator's sheet scrolling over long notes and Copy taking them.

---

LAND WITH CHANGES

1. **MATERIAL — UTF-8 rejection still fails at the cutoff.** [buildinfo.mjs:38](host/apple/buildinfo.mjs:38) removes arbitrary continuation bytes before validating. Reproduced in memory: 16,385 bytes of `0x80` become `…` without error. Round 1’s rejection guarantee therefore does not hold. Use fatal streaming decoding to withhold only a valid incomplete trailing character; add this regression case.

2. **MINOR — Missing Xcode hides available SDK data.** [buildinfo.mjs:74](host/apple/buildinfo.mjs:74) gates the combined line on `xcodebuild` succeeding, even when `xcrun` returned an SDK version. Reproduced with controlled tool responses. Compose the line from independently available values and test this combination.

3. **MINOR — Round 1’s UI verification remains outstanding.** [BuildInfoIOSTests.swift:17](host/apple/tests/ExactKitTests/BuildInfoIOSTests.swift:17) injects device text and tests formatting helpers. Long notes still rely on [UIAlertController’s message](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:224). Verify reaching the final note on iOS and tvOS with remote input, iOS Copy, and actual simulator identification.

Packaging wiring covers [bare macOS](host/apple/build.mjs:1259), [bundled macOS](host/apple/build.mjs:1288), [iOS/device/archive/tvOS](host/apple/build.mjs:1330), and the [iOS sample host](host/apple/build.mjs:1347). Earlier sidecar and build-kind fixes hold. iOS/macOS Copy includes notes; tvOS retains its no-Copy guard.

Hostname, branches, revision and notes ship in plists; this is now disclosed. Device/OS are obtained at runtime. Notes reads are bounded; stamp Xcode/SDK probes are cached, while Git lookups repeat per stamp.

Caps, one filtered Bun test, plist hostile-character round trips, and an AppKit scrolling probe passed. Other boundary probes covered empty notes, multibyte truncation, absent Git and detached HEAD. No new dead code or rules violation found; `build.mjs` meets the 1,500-line cap. Full builds, XCTest and file-writing fixtures were not run in this read-only checkout.
