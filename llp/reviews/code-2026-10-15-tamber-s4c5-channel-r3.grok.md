# Code review: stage 4 commit 5: the build channel, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the whole range, the second-round fixes first.
- **Method:** one brief (sha256 `b062928ca6b6c8cdd013071270bec6321f5baebc23d9433e4db195e5ff8b723d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking findings; a P2 (web pins over the app's setScheme) and a P3 (the batch cap in UTF-16 units).
- **Disposition:** applied without re-review (the last round) in the third-round commit.

---

Nothing blocking. The round-2 fixes hold: one build per stream, report-body bounds, the 5-second poll sweep, the duplicate-tab probe, the registration nonce guard, the resident relay not producing batches, Linux polling on its own worker, and `tamber/1.2`. Two defects remain.

**[P2] A scheme pin overwrites `setScheme` on both web hosts — `host/web/dev.js:355`**

Apple and Linux pin the system layer underneath the app (`BuildChannel.swift` `systemScheme`, `preferences.rs` `scheme.0` over `scheme.1`), and status reads the scheme that actually paints. The web page and the JS page write the pin into `documentElement.style.colorScheme`, which is the property `setScheme` writes (`host/web/glue.js:767`, `host/web-js/rt.js:269`; the JS pin is `host/web-js/dev.mjs:239`). `prefer` does not do that: it emulates `prefers-color-scheme` and leaves an explicit `color-scheme` alone. After a light pin, `setScheme("dark")` paints dark only until the next generation: `applied` reapplies `channel.held` (`host/web/dev.js:233`) and puts the pin back. Status then reports `light`, because it reads that same computed `color-scheme` (`host/web/dev.js:344`).

**[P3] The 32 KiB batch cap counts UTF-16 units; Apple’s frame limit is UTF-8 bytes — `host/web/dev.mjs:267`**

`pushDesign` and the JS loop’s `framed` (`host/web-js/dev.mjs:111`) compare `JSON.stringify(batch).length` with 32768. The poll path measures `Buffer.byteLength`. A value-only edit whose JSON is 30,000 CJK characters is under the character cap and about 90 KiB of UTF-8, so it is sent as a batch. `PlanURL.didReceive` cancels the whole dev stream when one chunk plus the buffer exceeds 64 KiB (`host/apple/Sources/ExactKit/PlanURL.swift:785`). Linux never sees that frame: its poll turns it into a notice.
