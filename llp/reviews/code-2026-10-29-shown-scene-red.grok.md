# Code review: ShownIOSTests main red (scene guard per test), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `34802a8ca5`.
- **Method:** one brief (sha256 `8721837b290c758f0338f031c657f2e8b860e9bc8df2abacca785ccc46e398e7`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The scene guard now sits in each of the 16 `ShownIOSTests` methods (for example `ShownIOSTests.swift:63`), and each one passes that scene into `presenter(_:in:)`. The helper still makes the session, builds the 400×400 scene window, attaches the viewport, and keys it. `hostedTests` lists all 16 `ExactKitTests/ShownIOSTests/…` methods and reports no `missing("scene")` outside a test method. The only edit in the commit is that move.
