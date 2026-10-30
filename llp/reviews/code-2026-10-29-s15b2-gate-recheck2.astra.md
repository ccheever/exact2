# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck2, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `dd9aaecf38f23701ab9d9c78ddedf3117e8c421d972b4c68f0394e9a7ca05a74`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium: P2).
- **Disposition:** taken in 626bcc56af.

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

- **P2 — [scripts/stage12-named.mjs:179](/tmp/iosfix-rv-g4/scripts/stage12-named.mjs:179): Forwarded slot content bypasses both checks.** Declare `slot` on `PersonResultRow`, add `children` beneath its existing package PersonRow call, and supply an app-owned `text` at the binding’s caller. The rendered text retains the caller’s `component` and lexical `chain`; neither mentions the binding. Consequently, `bindingsCheck` skips it, and `bindingShape` inspects only the directly instantiated package nodes. Both pass while PersonRow’s slot contains unauthorized app UI. The compiler’s existing slot-provenance tests establish this behavior; an in-memory map reproduction returned `ok: true` from both checks. Forwarded slot contents must also be checked.

VERDICT: CHANGES
