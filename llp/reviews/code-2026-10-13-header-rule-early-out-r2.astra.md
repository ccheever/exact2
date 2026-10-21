# Code review: PresenterIOS.swift under its cap, and HeaderRule's early answer pinned, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `dbb8642cfeeab23a08b375ae3465184407b5587bb2d318048ec1432092d1dd3d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

No BLOCKER / MAJOR / MINOR / NIT findings.

- [FirstResponderIOS.swift:6](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/IOS/FirstResponderIOS.swift:6): byte-identical move. Both `fileprivate` members remain together; guard, UIKit import, selector, and external references remain valid.
- [HeaderRule.swift:76](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/HeaderRule.swift:76): the counter is acceptable here. It adds one allocation-free increment per visited node, with none on the early-return path. `nonisolated(unsafe)` provides no synchronization, but current callers run on the main thread.
- [HeaderMacTests.swift:263](/Users/admin/x2p2/apps/exact2-comp/host/apple/tests/ExactKitTests/HeaderMacTests.swift:263): counts are correct—41 feed nodes and six header nodes. The post-setup baseline and synchronous assertions avoid test-order dependence; grouped classes execute serially within their process.

`caps`, `git diff --check`, and the new XCTest from the existing built bundle passed. No files changed; no fresh iOS/tvOS builds performed.

READY
