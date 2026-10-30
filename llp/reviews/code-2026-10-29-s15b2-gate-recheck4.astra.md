# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck4, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `77ba409ed14057f0553d9656ed1182a0ee17e0970ed235e1987eaaee6569bb6c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium: P2).
- **Disposition:** the text scan replaced by the compiler's parse in 90f26b3e4c.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g5-brief.txt. At g5, Grok was CLEAN. Astra's P2: a binding declaring its own `slot` and rendering `children` beside its PersonRow places its caller's nodes, whose chains and walks never name the binding, so both map checks pass.
This commit adds declaresSlot(text, component). It finds the declaration by `^component\s+<name>` followed by whitespace, end of line or a comment, so `B-old` isn't `B`. The declaration ends at the next top-level declaration keyword, not at a column-0 `)`. Any line that is the word `slot` alone (an optional comment allowed) is the slot declaration. consumerPixels fails a binding that declares one, or that isn't declared in its named file. S15B2_BINDINGS names the files.
Check this closes the forwarding hole: a component's children can only come from its own slot. Check the declaration finder against legal Contract (keywords, comments, a column-0 `)` inside a call), and whether `slot` can appear alone on a line inside a declaration in any other meaning. Is anything else left under the node-level scope?
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [scripts/stage12-named.mjs:202](/tmp/iosfix-rv-g6/scripts/stage12-named.mjs:202): Forwarding hole remains through a false declaration boundary.** `state` is a legal contextual identifier. With a prop named `state`, this compiler-accepted expression ends the scan early:
  ```contract
    derive label = (
  state
  )
    slot
  ```
  `declaresSlot` returns `false`, missing the actual slot. Rendering `children` beside PersonRow therefore still forwards caller nodes that neither map check catches. Declaration boundaries must account for bracket depth and contextual keywords.

- **P2 — [scripts/stage12-named.mjs:205](/tmp/iosfix-rv-g6/scripts/stage12-named.mjs:205): A standalone `slot` can be a value, falsely rejecting valid bindings.** A component with a prop named `slot` can legally write:
  ```contract
    provide
      slot
  ```
  Likewise, `slot` can occupy its own line inside a parenthesized argument. The compiler parser accepts both without declaring a slot, but this scan returns `true`, causing `consumerPixels` to reject the binding. Match the component’s actual slot section.

VERDICT: CHANGES
