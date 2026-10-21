# Code review: the title view's subtitle as a line of symbols and text, round 3 (final) (0a1b13944), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the round-2 artifacts commit.
- **Method:** one brief (sha256 `cf2ec214eb870be7ab26baa4eb767d25b6a9fc96b1c017767f56cbbcdf9aefee`), shared with grok. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition:** No findings to take. Landed.

---

Static review of `0a1b13944`; no files changed or tests run. No new Blocker or Should-fix findings.

Round-2 dispositions are acceptable:

| Finding | Assessment |
|---|---|
| Controls and badges | Resolved for the reported cases: controls are skipped before extraction, the heading exception remains, and traversal stops at filled containers after the heading. [NavigationTitleIOS.swift:70](/tmp/x12-rg/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:70) |
| Last symbol becomes blank | Resolved: symbol-source presence determines recognition independently of emitted glyphs, retaining the texts and container label. [NavigationTitleIOS.swift:54](/tmp/x12-rg/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:54) |
| Explicit RTL direction | Resolved: direction is retained, included in `source`, and applied to the paragraph. Mixed-direction run isolation remains acceptably deferred. [NavigationTitleIOS.swift:84](/tmp/x12-rg/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:84), [NavigationTitleIOS.swift:185](/tmp/x12-rg/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:185) |
| Coverage and return state | The return assertion now requires both a hidden subtitle and a nil accessibility value. Remaining coverage limitations are acceptable deferrals, not demonstrated runtime verification. [NavigationBasicsIOSTests.swift:129](/tmp/x12-rg/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:129) |

**The earlier `control(child)` skip does not break the passive avatar inside a pressable title group.** `isButton` and `actsAsButton` inspect the child’s own kind and properties; handlers likewise come from that node’s declarations. They do not inherit the parent’s pressability. [Accessibility.swift:12](/tmp/x12-rg/host/apple/Sources/ExactKit/Accessibility.swift:12), [Accessibility.swift:35](/tmp/x12-rg/host/apple/Sources/ExactKit/Accessibility.swift:35), [runner.rs:1134](/tmp/x12-rg/runner/src/runner.rs:1134)

Consequently, the fixture’s passive avatar column reaches `BadgeFace` at line 77. An avatar with its **own** control semantics is intentionally excluded. The existing test already asserts the passive avatar’s visibility and size inside the pressable group. [NavigationBasicsIOSTests.swift:87](/tmp/x12-rg/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:87)

Verdict: LAND
