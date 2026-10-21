# Code review: Linux: the authored accent, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `7b28b544300cf0518664ac2c063b894f2778674e10b880e5a02cf16c5a740cd3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one medium; Grok: none.
- **Disposition:** Astra's finding named in QUEUE (item 7), not fixed

---

The round-2 fixes in `be9a762de` are in place, and there is nothing else to report.

There are none at high, medium, or low severity. The gaps still open are the ones already named in QUEUE, "Linux's authored accent, what the painting scope does not reach," items (1)–(6), and the older disabled-button entry.

What `be9a762de` changed, checked against the three commits (`29468db5d`, `e3b4023ee`, `be9a762de`; `origin/main` has moved, and it does not edit these files past the merge-base):

- Only an `accent-color` of `AccentColor` keeps the outer accent (`kernel/src/style/roles.rs:433`). `LinkText`, `-exact-link`, and `-exact-symbol` are stored and resolve with the scope cleared, matching `host/web/src/css.rs`.
- A run's shadow and stroke resolve under that run's scope (`host/linux/src/paint/text_shadow.rs:46`), and the guard lives through both resolves.
- A content region's scope covers the node's box, image tint, and SVG capture (`host/linux/src/paint/region.rs:255`).
- The window background resolves under the root's accent (`host/linux/src/presenter/window.rs:205`).
- `color_targets` has no scope again (`kernel/src/motion.rs` is unchanged against the merge-base), which is QUEUE item (6).
- The region capture calls `accent_of(node)` with no extra borrow, and `host/linux/tests/it/accent.rs` asserts the `kept` pixel is `[0, 200, 0, 255]` on that box's own background.

Other resolve sites (column rules, borders, live gradients, caret, inline backgrounds, kept-row image tint, bar and field chrome) run under the scope that belongs to that node, or they bake a concrete inherited accent. The same keyword-only miss as QUEUE item (5) shows up on the select menu, and the 3D path still resolves every colour with the parent's scheme; neither is a new accent defect.
