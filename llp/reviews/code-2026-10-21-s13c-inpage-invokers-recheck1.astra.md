# Code review: iOS in-page custom invokers: UIKit's pull-down only for a menu-shaped popover (LLP 1021 D3), narrow re-check after round 2, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fc3d755edd`.
- **Method:** one brief (given on the command line). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** nothing blocking

**VERDICT: RIGHT**

1. **INFO — [PointerIOS.swift:370](/tmp/iosfix-rv-ip3/host/apple/Sources/ExactKit/IOS/PointerIOS.swift:370):** The entire file exactly matches the pre-branch version (`f7cb634c2f^`). A handlerless invoker forwards the real touch to `pressResponder`. On release, the ancestor’s handler runs before `activating` invokes the surviving invoker’s target ([InvokersIOS.swift:133](/tmp/iosfix-rv-ip3/host/apple/Sources/ExactKit/IOS/InvokersIOS.swift:133)).

2. **INFO — [MenusIOS.swift:189](/tmp/iosfix-rv-ip3/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:189):** No dependency remains on `hasMenuOverlay`; its only caller was removed. The retained iOS guard uses `pullsDown` independently, preserving menu/confirmation overlays and tvOS behavior.

3. **INFO, non-blocking — [QUEUE.md:3](/tmp/iosfix-rv-ip3/QUEUE.md:3):** Missing pressed feedback is explicitly deferred with the correct separation: visual state must not claim press ownership. No blocking issue found in this commit.

Read-only source review; no files changed or runtime tests run.
