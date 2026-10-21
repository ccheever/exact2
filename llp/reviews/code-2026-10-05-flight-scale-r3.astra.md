# Code review: a flying view that is not an image is scaled whole (LLP 1013.000 D4.4 as amended), round 3, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x21`.
- **Method:** one brief (sha256 `196a89249d6a1b85fd8c10bb64292d4a4e287a54dd80d64f92734ddb25c4373a`), shared with grok. Round 3 (the last), blind to the other review. Reviewed the staged diff (with the round 2 fixes) in a worktree at df7dc73a5. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 3 was the last (at most three rounds). The must-fix (both reviews) is the macOS accessibility hide added in round 2: `navigation.sync`'s input gates rewrite it in the same apply, so it never held. Resolved by withdrawing it: macOS goes back to main's behaviour, where a flying view stays in the accessibility tree, the stage-1 gap LLP 1013.000 §3 already records ("on macOS ... the flying view stays in the accessibility tree"); for a scaled flight its AppKit frame is the unscaled one for the flight's 0.3 s. Making it hold needs the input gate to know flights (Grok's fix), a change outside this one, left for stage 2. Astra 2 (the border test) taken: the card carries an opaque border colour, and the test asserts the border's layer stays at the full 402 x 714 layout, its scaled bottom below the clip. These two changes came after round 3 and were not reviewed again; iOS XCTest (308) and macOS XCTest pass on them.

---

NOT READY

1. **[FlightsMac.swift:224](/tmp/x21/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:224) — must-fix:** macOS accessibility hiding is immediately undone for a newly created arriver. `Presenter.apply` calls `navigation.sync` after `flightsBatchApplied`; `NavigationMac.swift:101–104` then writes `false` for an otherwise visible flying node. Round 2’s accessibility finding remains unresolved. **Fix:** include `presenter.isFlying(node)` in the accessibility gate. Add a regression through `Presenter.apply` covering creation, a subsequent style update, and restoration on landing; the current direct flight calls bypass this overwrite.

2. **[FlightScaleIOSTests.swift:26](/tmp/x21/host/apple/tests/ExactKitTests/FlightScaleIOSTests.swift:26) — should-fix:** the claimed painted-border coverage remains incomplete. The fixture supplies width but no border colour, which `applyBoxLayer` defaults to clear. Lines 67–68 assert only view/clip dimensions, never the border’s geometry. **Fix:** supply an opaque border colour, run layout, and assert the actual border layer retains the full layout dimensions and its scaled bottom lies outside the clip. This can use layer assertions without adding screenshot infrastructure.

The paint-rank and non-centred-anchor fixes check out. Both cached macOS flight tests passed. No files changed.
