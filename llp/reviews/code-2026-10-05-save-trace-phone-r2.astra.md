# Code review r2: LLP 1079 D5 on a phone (8fca848db..626244172), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `626244172`.
- **Method:** one brief (sha256 `ac856b30fdee689d9dad80dbf76d0aebc8c49f2597bd6f91ea47863cae6f0e1e`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken: `devicectl` runs only when `simctl` knows no such simulator (its exit 148); a simulator it knows but cannot answer for (shut down, the app not installed) is refused with `simctl`'s own words. Checked by hand on a booted simulator with a trace, a shut-down one, and one without the app's trace. 2 DEFERRED: no test file covers `agent-inspect.mjs` and none of the scripts' tests runs in a check; a UIKit test of the share sheet's presentation likewise.

---

LAND WITH CHANGES

1. **MINOR — Simulator lookup failures still fall through to `devicectl`.** At [agent-inspect.mjs:333](scripts/agent-inspect.mjs:333), any failed container lookup selects the physical-device branch. For a named simulator without the selected app installed, this attempts an unsupported copy and discards the useful `simctl` error. Mocked execution reproduced the fallthrough. Determine simulator identity separately; report its container error directly. Round 1’s missing-trace case is fixed, but its broader disposition is incomplete.

2. **MINOR — Driver and sharing behavior remain untested.** [FrameSamplerTests.swift:48](host/apple/tests/ExactKitTests/FrameSamplerTests.swift:48) now correctly pins the literal filename, matching bytes, and replacement. It cannot catch wrong copy arguments, simulator fallthrough, or a broken Share action. Use the injected runner at [agent-inspect.mjs:327](scripts/agent-inspect.mjs:327) for copy/error tests; add UIKit coverage for the saved URL and iPad anchor at [DevMenuIOS.swift:115](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:115). These tests need no physical phone.

Other round 1 fixes hold. `caps` and `git diff --check` pass; `agent.mjs` is 1,500 lines. Both trace files receive identical serialized contents, with atomic replacement of latest. Production writes remain gated; sharing is excluded on tvOS and anchored on iPad. No additional ordering, lifetime, reload, or agent-mode regression found.

Apple builds, UI interaction, and hardware transfer were not verified in this read-only sandbox.