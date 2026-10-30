# Code review: the Mac window bar's read set, the projected-node identity set (follow-up to round 3), quick review (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6da2c9ef9b`.
- **Method:** a quick review of one follow-up commit, by Grok alone; its brief was not kept. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

HEAD is behaviour-identical and correct. `currentInputs` still treats the bar's own hiding as the `besideParts` nodes plus `owner` when `projected` is set, and `NodeView` is a final `NSView` subclass, so `ObjectIdentifier` equality is the same pointer test as `===`. The set is built once from that same `mine` array, which is not mutated afterward, and those views stay alive for the pass, so the same instances are excluded from the hidden gate. The commit changes only that membership test in `ToolbarReadsMac.swift`.

VERDICT: APPROVE
