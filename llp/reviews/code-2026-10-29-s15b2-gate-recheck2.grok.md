# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck2, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `dd9aaecf38f23701ab9d9c78ddedf3117e8c421d972b4c68f0394e9a7ca05a74`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1-brief.txt, /tmp/iosfix-rv-g2-brief.txt, /tmp/iosfix-rv-g3-brief.txt.
The previous re-check broke the source keyword scan in six ways: `if(`/`match(`, a column-0 `)`, a hyphenated component name prefix, nested templates, double spaces, and words after the view. This commit REMOVES the scan. The decision and its reasoning:
- theme2's own terms for option B (18:40): "an 'if' whose only branch is the package FollowButton counts as part of the binding (it's control, not drawing); any other NODE fails the check". Plan (~/x2theme/iosfix-s15b2-gate-plan.md): "checked on after's source map".
- So the check is node-level, on the map, which is formatting-proof:
  - bindingsCheck: no node of the binding's own, none through an app helper, everything it instantiates allowed;
  - bindingShape: exactly one PersonRow call site, at most one FollowButton call site, every FollowButton rendering in that PersonRow call's slot (its walk's first step).
- Control flow (if, for, match) draws no node, so it's out of scope by theme2's terms. A for around the row repeats PersonRow nodes, each a package node.
Judge whether, under those terms, the remaining checks are correct and complete. Is there any way a binding can draw a node that isn't the package's PersonRow, or the package FollowButton in that PersonRow call's slot, and pass? If you think the control-flow scope itself should change, say so as a note for theme2, not a finding.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD on the s15b2 gate branch, read-only, against the earlier briefs and the plan's node-level rules.HEAD drops the source keyword scan and leaves the source-map checks. I'll read that commit against theme2's node-level terms and the plan.The scan is gone. Next I’ll read `bindingsCheck` and `bindingShape` against how the source map records nodes, walks, and slots.The map checks are the whole gate now. I'll trace call chains, slot walks, and what still counts as a node before judging whether a foreign node can pass.No findings.

Under theme2's node rule, a binding cannot draw a node other than the package `PersonRow`, or the package `FollowButton` in that one `PersonRow` call's slot, and still pass. `bindingsCheck` and `bindingShape` cover every node the map attributes to the binding.

`bindingsCheck` (`scripts/stage12-named.mjs:143`) takes every node whose chain names the binding, plus any element of the binding itself. An element written in the binding, including one placed in the slot or under an `if`, `for`, or `match`, has `component` equal to the binding, so it fails as its own node. Control flow does not create an instance (`contract/syntax/src/inline/use_node.rs` pushes instances only for component uses), so it cannot retarget that element onto another name. A call through an app component puts that call in the chain and fails as an app helper. A direct call of anything other than the two allowed components fails as a stray. Nodes inside the package `PersonRow` (its `Avatar`, its badge, its text) stay on `PersonRow`'s chain entry, so they count as that package component.

`bindingShape` (`scripts/stage12-named.mjs:175`) then keeps the shape. Direct `PersonRow` nodes are those with `chain[0].component` equal to the binding and `library` equal to the package id, which is one call span (`file:line:col` of that use). A second use is a second span. Direct `FollowButton` nodes are the same kind of set, capped at one span. Each of those nodes must have a walk whose first step is that same use with `slot: true`. The walk is the visual parent chain (`contract/cli/src/map.rs:128`): the first step is the nearest host that has elements, and `PersonRow`'s own `row` is that host for its slot (`packages/cards/person-row.contract:56`). A `FollowButton` beside the row, or in some other host's slot, has a different first step or no walk, and fails. A missing walk fails closed.

A `for` around the row or around the button repeats nodes from the same span. Each repeated node is still the package component, and each button's walk still starts at that same `PersonRow` use. An `else` or `match` arm that calls either component again is a second span and fails. An arm that draws a box or text fails in `bindingsCheck`. That is the same rule as the `if` theme2 already allowed. The control-flow scope does not need to change.

VERDICT: CLEAN
