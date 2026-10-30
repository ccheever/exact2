# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck3, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `c4a3b380a29890845986444144405d05c594d576f442076ff358f9b456df9323`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g4-brief.txt (the latest, g4, has the node-level scope from theme2's terms). At g4, Grok was CLEAN. Astra had one P2: content forwarded into PersonRow's slot through a `slot` of the binding's own (the caller's nodes, whose chains don't name the binding) passed both checks.
This commit adds to bindingShape: every node whose walk's first step is that PersonRow call's slot must be one of the binding's own direct FollowButton nodes. Anything else rendering in the slot fails ("its PersonRow's slot holds ..."), and a test reproduces the forwarded case.
Check the fix against the map's walk semantics (contract/cli/src/map.rs). Can a FollowButton's own inner nodes, or PersonRow's own slot marker or wrapper, have a walk that starts at that slot and so fail the real copy wrongly? (On the copy's real map both bindings pass.) Is there any remaining way under the node-level scope?
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD on this branch, read-only, against the g4 brief and the map walk rules. First I'll load the earlier briefs and the commit diff.HEAD is a small gate change on the PersonRow slot. I'll read the diff, the walk rules in `map.rs`, and the earlier briefs so the check is judged against those semantics.The new slot check is small. I'll confirm it against `steps()` in `map.rs` and the real PersonRow and FollowButton trees, including whether inner nodes would be treated as strangers.No findings.

The new check does not reject the real copy. A FollowButton's own inner nodes do start their walk at that PersonRow call, and they stay inside `follows`. PersonRow's slot columns and the `children` marker do not start a walk there.

`steps` seeds `seen` with the node's own instance (`contract/cli/src/map.rs:137-140`), then emits one step per new copy on the way up. A non-ancestor is `(host, host, slot: true)` at the host's own call (`map.rs:157-160`). FollowButton's button, icon box, image, and label are all that one FollowButton instance (`packages/actions/follow-button.contract:60-100`), so those nodes are skipped and the first step is the PersonRow call, `slot: true`. They are also `follows` (`scripts/stage12-named.mjs:179-180`): `chain[0]` is the binding's FollowButton call, and every element of an exported view carries that library identity (`contract/lower/src/anatomy.rs:165-172`, `573-574`). `strangers` subtracts `follows` (`stage12-named.mjs:188-189`), so the inner nodes pass. That is why both bindings pass on the copy's map.

The columns around `children` (`packages/cards/person-row.contract:123-127`) are the PersonRow instance, which is already seeded, so the walk never records PersonRow as a step of those nodes. The first step is the lexical caller with `slot` false, or the walk is omitted when it equals the chain (`map.rs:165`). `children` is replaced by the fill before lowering (`contract/syntax/src/inline.rs:11-15`, `contract/lower/src/lib.rs:1042-1046`) and is not a map node.

Nothing else can render in that slot and still pass. The fill is parented under that column. A component with no elements of its own adds no plan node, so the walk skips it and the next real element still takes this PersonRow call as its first slot step (the same shape as `panel-fill` in `contract/cli/tests/it/source_map.rs:1216-1222`). That node is in `inSlot`. It is allowed only when it is one of the binding's direct FollowButton nodes. A caller's forwarded node (the case at `scripts/stage12-named.test.mjs:125`), another package component, or a wrapper of the binding's own is a stranger. Content nested in some other host is reached through that host's elements, which themselves start at this slot. FollowButton has no child component and no slot, so its interior is only those direct nodes. `if`, `for`, and `match` still add no node, so they do not cover a fill.

VERDICT: CLEAN
