# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck1, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `80b681920e45d807367a1f96d60d895e7e85f6d662b00a8cfa465e5d28c6dc59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (5 medium, 1 low).
- **Disposition:** the source scan removed in 3f17d3664b.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Rounds 1 and 2's briefs are at /tmp/iosfix-rv-g1-brief.txt and /tmp/iosfix-rv-g2-brief.txt. Round 2's findings were:
- Astra P2: bindingShape's indentation parser let extra slot content pass, by moving a `)`.
- Astra P2: legal formatting failed (a trailing comment on the component line, tabs, compact calls).
- Grok P1: `//` inside a string was taken for a comment, so an else or a second if passed.
- Grok P2: the web's edge noise inside a named box was dropped before the name counted it, so a fringe-only change read STALE.
This commit:
- REPLACES the textual parser. bindingShape(map, text, binding) now works on the source map, which is formatting-proof:
  - the binding calls the package PersonRow from exactly one site (distinct chain[0] call sites) and the package FollowButton from at most one;
  - every FollowButton it draws renders in that PersonRow call's slot (its walk's first step is that call, slot: true; see contract/cli/src/map.rs for walks and walk_steps).
- On the source, only the view's words are checked, with strings (double quotes and backticks, with escapes) and then comments blanked: at most one `if`, and no else, for, match or when.
- pixelDiff gives a named box every differing pixel in it, before the noise floor; outside named boxes the floor applies as before.
- The release runs add two landed package commits.
Try to break the new shape check: formatting, strings with //, a component name that prefixes another, a FollowButton written beside the row or in an else, a second PersonRow, keywords inside identifiers (`format`, `elsewhere`), and nested components. Check the noise change and its test.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [stage12-named.mjs:195](/tmp/iosfix-rv-g3/scripts/stage12-named.mjs:195): Compact control flow bypasses the scan.** Legal nested `if(true)` statements count as zero `if`s and pass. Likewise, `match(p)` escapes the forbidden-keyword check. The regex requires whitespace where Contract permits punctuation.

- **P2 — [stage12-named.mjs:188](/tmp/iosfix-rv-g3/scripts/stage12-named.mjs:188): A continuation line can truncate the checked body.** Putting a multiline PersonRow call’s closing `)` at column zero is legal. The scanner treats it as the component’s end, so two subsequent nested `if`s pass unchecked.

- **P2 — [stage12-named.mjs:186](/tmp/iosfix-rv-g3/scripts/stage12-named.mjs:186): Hyphenated prefixes select the wrong component.** When `component B-old` precedes `component B`, checking B scans B-old’s view because `-` satisfies `(?![\w])`. A harmless B-old therefore hides forbidden control flow in B.

- **P2 — [stage12-named.mjs:198](/tmp/iosfix-rv-g3/scripts/stage12-named.mjs:198): The check never establishes what the `if` wraps.** An `if` surrounding the entire PersonRow passes and is reported as wrapping FollowButton. An unconditional slotted FollowButton also passes. Both violate the approved shape: one row, with the optional `if` around FollowButton inside its slot.

- **P2 — [stage12-named.mjs:190](/tmp/iosfix-rv-g3/scripts/stage12-named.mjs:190): Nested templates are not blanked correctly.** A legal nested template containing literal `for else when` is rejected as control flow. The regex stops at the inner opening backtick, exposing string contents.

- **P3 — [stage12-named.mjs:186](/tmp/iosfix-rv-g3/scripts/stage12-named.mjs:186): Legal declaration spacing still fails.** `component  B` parses successfully but returns “declares no component B” because the matcher requires exactly one space.

The noise change and its regression test are correct. The two release additions match the intended commits. Twenty-five selected read-only tests passed; parser-accepted probes reproduced the issues above. No files changed.

VERDICT: CHANGES
