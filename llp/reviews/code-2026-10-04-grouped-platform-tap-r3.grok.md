# Code review: a real touch aims at what a grouped list draws, round 3 (5da0e24df), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, no subagents, `--cwd` a detached worktree at `5da0e24df`.
- **Method:** one brief (sha256 `5603e9a78a5b04eab1b80f684cf0a54b2dbef00fc2d87cb6f668ebce00696398`), shared with astra. Round 3, the last; blind to the other round-3 review; it read the earlier artifacts. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the CLI's running narration before it is omitted).
- **Verdict:** LAND.
- **Disposition:** nothing to take.

---

No findings.

Round 2's delivery hole is closed. `TouchLog.landing` records `projected` from `GroupedListHost.part`, `aim` returns that for the hit view, and `realTap` rejects the tap when the began entry's row or part differs. A list update that puts another row's cell or control under the finger fails that check, because `configure` writes `cell.row` to the new row id before the touch is delivered.

The four edge cases do not false-refuse:

- A custom row is still aimed by the ordinary path (`list(drawing:)` skips `custom`). Its carried `NodeView` sits in the cell, so `projected` is set, but aim's hit-test view and `touch.view` are the same view. `part` is a pure walk of that view, so the row and part match. A nested `UIControl` is only labeled `switch` or `detail` when the hit is that control or one of its subviews; the node check still tells those controls apart.
- A private subview of the switch or the detail button walks to the same outermost `UIControl` as the control itself (`UISwitch` becomes `switch`, any other control `detail`). The new unit assertion covers the switch's first subview, and the smoke leg's switch, detail, and row taps pass the live comparison.
- A cell reused for another row reports the new `cell.row` on the began entry, so the row no longer matches the aim.
- `part` compiles for tvOS: the `UISwitch` test is inside `#else`, and both `GroupedListIOS.swift` and `TouchIOS.swift` are built for `os(iOS) || os(tvOS)`. On tvOS any control is `detail`, which matches a platform with no `UISwitch`.

The round-2 dispositions match the code. The outside-port wording is right: `obscured` runs first and can return "covers its middle"; otherwise the port check returns the port error. The LLP §6.3 note matches the new `projected: {row, part}` refusal. The deferred race test is stated as deferred, not as covered.

Verdict: LAND
