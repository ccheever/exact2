# Code review: the title view's avatar at its authored size (c62beb1e5), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `c62beb1e5`.
- **Method:** one brief (sha256 `d2905b58ba6cb51d0c48a8a66fcda4ff9f52cb7698ee549fa278f4592225af8f`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentence is omitted).
- **Verdict:** LAND.
- **Disposition (r2):** No findings to take. The r2 change (faceBoxSize, no ancestry) goes to round 2.

---

The eight-ancestor walk is an arena lookup (`slot_of` is a map get) and only runs once both width and height are already points. Create and update already build the prop map and the style JSON, so those lookups are not a measurable cost. It sees the final kernel tree: a header created in the same commit as its children is already linked when `create` runs, and a later `when` that mounts a header creates the box under that header. A class or a viewport `when` that changes the points emits `SetStyle` or `ClearStyle` on that view, which touches it, so `update` rewrites `headerBoxSize`. When the size is no longer both points, or no header sits within eight ancestors, the key is absent from the new map and the existing prop diff clears it (`paragraph.rs:336-349`). A live view is not reparented into or out of a header without being recreated.

`headerBoxSize` is written only into the Apple host mirror and the batch sent to Swift. The agent tree is built from kernel props (`runner/src/agent.rs`), and the web and Linux hosts have their own projection. An extra key on a Mac `NodeView` is unused.

`BadgeFace.size` is stored, so synthesized equality changes when only the size changes. `source` includes it (`NavigationBarIOS.swift:164`), and that string is what `HeaderTitle`, bar-item identity, and `projectedSource` compare. `richTitle` also calls `update` on every prepare, which assigns the image and the constants again.

The dark-mode constraints are required and square, so they beat the image view's intrinsic size when the dark variant reports a 1× point size. A missing avatar sets `isHidden` and then the constants to 0 before any layout; a hidden arranged subview drops out of the stack, and a nil image has no intrinsic size to fight the zero constants. Bar items still go through `BadgeFace(button)` with `authored` left false, so they stay at 36 and never use these constraints.

The Rust test locks a single `"40x40"` on the boot batch, with the outside 40×40 box present so a second copy would fail. The iOS test checks the fixture's 32 pt image and the avatar bounds after a dark trait on the real window.

No findings.

Verdict: LAND
