# Code review: release notes and fuller build info in the development menu, round 1 (35d531d6b), 2026-10-08 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree.
- **Method:** one brief (sha256 `2ba72529bd5098dde3d5be855aaec05ded5818622dd1e9c07a15db9a26d8f443`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: at most 16 KB is read, a cut lands on a character and ends with `…`, non-UTF-8 fails the build naming the file (tested). 2 taken: the Xcode and SDK probes run once a build. 3 taken: exact2's branch is stamped and shown beside its commit, so an in-tree app shows its branch. 4 taken: the SDK and its version join the Xcode line. 5 taken: the author guide (`docs/contract-for-agents.md`, Inspection and testing) names the file, the limit, `EXACT_DISTRIBUTION_REVISION` and that a shipped binary carries the host name and commits. 6 taken in part: the boundaries are tested; the sheet's scrolling on iOS and tvOS is UIKit's and not tested.

---

LAND WITH CHANGES

1. **MATERIAL — Notes are silently corrupted/truncated, without bounding input cost.** [buildinfo.mjs:47](host/apple/buildinfo.mjs:47) reads the entire file before slicing UTF-16 units. Reproduced: 7,999 ASCII characters followed by an emoji produces a replacement character; remaining notes disappear from both display and Copy. Invalid UTF-8 is also silently replaced. Enforce a documented size limit before decoding, reject invalid UTF-8 clearly, and test these boundaries.

2. **MINOR — Repeated synchronous Xcode probes.** [buildinfo.mjs:29](host/apple/buildinfo.mjs:29) launches `xcodebuild` on every stamp: three times for macOS `--bundle` ([build.mjs:1259](host/apple/build.mjs:1259), [1285](host/apple/build.mjs:1285), [1288](host/apple/build.mjs:1288)); twice for iOS `--host`. Compute the stamp once per build and reuse it.

3. **MINOR — In-tree apps never show their available branch.** [buildinfo.mjs:45](host/apple/buildinfo.mjs:45) conditions branch lookup on having a separate app repository; no exact2 branch is stamped instead. Read the app directory’s branch independently and display it beside the applicable commit. Continue omitting detached `HEAD`.

4. **MINOR — SDK version is never displayed.** The new probe only captures Xcode’s version; [BuildInfo.swift:21](host/apple/Sources/ExactKit/BuildInfo.swift:21) never reads SDK metadata. Archives already have `DTSDKName` ([build.mjs:464](host/apple/build.mjs:464)). Pass the selected SDK metadata through and display it when available.

5. **MINOR — Author documentation and disclosure are missing.** The filename, limit and `EXACT_DISTRIBUTION_REVISION` convention exist only in implementation comments. [buildinfo.mjs:52](host/apple/buildinfo.mjs:52) also unconditionally ships the builder’s hostname—including potentially personal names or internal DNS suffixes—in distributed plists. Document these conventions and shipped metadata in the existing guide, as [DEFERRED.md:815](rules/DEFERRED.md:815) requires.

6. **MINOR — Tests bypass the platform behavior.** [BuildInfoIOSTests.swift:17](host/apple/tests/ExactKitTests/BuildInfoIOSTests.swift:17) injects the device string; subsequent assertions test text helpers, not actual scrolling or Copy. Add boundary cases to existing tests and verify long notes on iOS/tvOS, including remote scrolling and simulator identification.

All listed packaging paths reach the stamp. Missing/blank notes, absent git/Xcode and detached HEAD behaved correctly in probes; Apple plist parsing accepted hostile-character samples. AppKit scrolling passed; iOS/macOS Copy includes notes, while tvOS retains its existing no-Copy guard. Changed files remain below 1,500 lines; no new dead code found.

One filtered Bun test passed. Full native builds and fixture-writing tests were not run in this read-only checkout.
