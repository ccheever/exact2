# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck3, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `c4a3b380a29890845986444144405d05c594d576f442076ff358f9b456df9323`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium: P2).
- **Disposition:** taken in f278ade57d.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g4-brief.txt (the latest, g4, has the node-level scope from theme2's terms). At g4, Grok was CLEAN. Astra had one P2: content forwarded into PersonRow's slot through a `slot` of the binding's own (the caller's nodes, whose chains don't name the binding) passed both checks.
This commit adds to bindingShape: every node whose walk's first step is that PersonRow call's slot must be one of the binding's own direct FollowButton nodes. Anything else rendering in the slot fails ("its PersonRow's slot holds ..."), and a test reproduces the forwarded case.
Check the fix against the map's walk semantics (contract/cli/src/map.rs). Can a FollowButton's own inner nodes, or PersonRow's own slot marker or wrapper, have a walk that starts at that slot and so fail the real copy wrongly? (On the copy's real map both bindings pass.) Is there any remaining way under the node-level scope?
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [scripts/stage12-named.mjs:188](/tmp/iosfix-rv-g5/scripts/stage12-named.mjs:188): Forwarding beside PersonRow still bypasses both checks.** A binding can declare `slot` and render `PersonRow(...)` followed by `children` at the same indentation. Caller-supplied text then renders beside the row. Inlining preserves the caller’s instance, so its chain excludes the binding; its walk also never enters PersonRow’s slot. Both checks pass despite the extra node. Confirmed with an in-memory map matching these semantics.

The suspected false positives do not occur: FollowButton’s own inner nodes qualify as `follows`; PersonRow’s wrappers skip their already-seen instance, and `children` is replaced during inlining rather than becoming a marker node. Both real-map bindings and the three focused tests pass.

VERDICT: CHANGES
