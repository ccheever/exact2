# Code review: a grouped-list switch that "does not flip", round 2 (4cc252edb), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, no subagents, `--cwd` a detached worktree at `4cc252edb`.
- **Method:** one brief (sha256 `74eee858ffbf59e27ce7bead1e52efa374a2203fff8c2e317b54b6b96f73a5bf`), shared with astra. Round 2, blind to the other round-2 review; it read the round-1 artifacts. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the CLI's running narration before it is omitted).
- **Verdict:** LAND.
- **Disposition:** nothing to take. The last round.

---

The round-1 fixes hold, and the new frame math puts the hidden switch on the accessory's middle. No findings.

`applyGeometry` writes each `frame` op straight into `UIView.frame`, so the numbers are parent-relative (`PresenterIOS.swift:1030`). The list's children live in its scroll. Section 2 and column 4 are framed at that scroll's origin, so row 12's `y` and control 13's `x` are scroll coordinates. `at` is the accessory's window-middle converted into that same scroll. The control box is 63×52 with its top on the row's top, and `ControlHost` centers the `UISwitch` in the content box (the control's style is empty, so that box is the full bounds). Half of 63 and 52 is 31.5 and 26, so the switch center lands on `at` and converts back to `middle`. The test checks that converted rect.

That hit assertion fails if the hidden switch, or any other view, takes the point. The authored switch is a different instance, and the predicate only accepts the accessory or a descendant of it. While the scroll is hidden, `ScrollView.hitTest` returns nil (`ScrollViewIOS.swift:24`), and the test also asserts `scroll.isHidden`. Showing the scroll, moving the authored switch into a visible view above the accessory, or covering the middle with anything else fails the test.

The pitfall now says a coordinate tap resolves no element, the "looks broken" line is a sentence, and `axe touch -x <x> -y <y> --down --up` matches AXe's `touch` command, which requires both coordinates. Both review dispositions match that test: distinct switches, overlap at the accessory's middle, hidden scroll, hit stays on the accessory.

Verdict: LAND
