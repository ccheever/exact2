# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck1, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `80b681920e45d807367a1f96d60d895e7e85f6d662b00a8cfa465e5d28c6dc59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 high, 1 medium, 1 low).
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

I'll re-check only HEAD on the s15b2 gate branch, read-only, against the round 1 and 2 briefs and the new shape and noise checks.The commit replaces the textual parser and changes how named-box noise is counted. I'll read the shape check, the walk contract, and the noise test next.[P1] Treat a hyphen as part of a component name — `scripts/stage12-named.mjs:186`

`^component ${component}(?![\w])` ends the name at anything other than `[A-Za-z0-9_]`. A Contract name continues through a hyphen followed by a letter, so `component PersonResultRow-legacy` is one identifier (`contract/syntax/src/lexer.rs:271`) and this pattern matches it as `PersonResultRow`. `declares` in `scripts/stage12-gate.mjs:310` already uses `[^A-Za-z0-9_-]`. `findIndex` keeps the first hit, so an earlier component with that longer name supplies the view whose words are checked. That view can be free of `if`, `else`, `for`, `match`, and `when` while the real binding is `for x in people key=x.id` around its one `PersonRow(...)`. A `for` is one call site, so the map check accepts it and the gate reports the binding ok. Probed: `component B-extra` above `component B` with a `for` returns `{ok: true}`.

[P2] Count `if` and `match` when `(` follows immediately — `scripts/stage12-named.mjs:195`

The patterns are `(^|\s)if\s` and `(^|\s)(else|for|match|when)(\s|$)`. The lexer ends an identifier before `(`, and `node_if` / `node_match` then parse an expression, so `if(p.on)` and `match(p.user)` compile. A slot written `if(p.on)` / `if(p.off)` / `FollowButton(...)` is two conditions and is reported as one PersonRow, its FollowButton, and zero `if`s. `match(p.user)` with the FollowButton under `case some` and an empty `case none` (a case arm may be empty) is reported ok. `for` and `else` still require a following name or a newline, so those are refused. `format` and `elsewhere` are not counted. Strings are blanked before `//`, so `name="http://else"` and `` label=`http://x` `` do not invent a keyword, and a real `else` on the next line is still refused.

[P3] End the word scan with the view block — `scripts/stage12-named.mjs:188`

`end` is the next column-0 line, and the words are everything after `view` up to that line. An `action` or `derive` written after the view sits in that range, so its `else` fails a view that is the one PersonRow and the one `if`. The same action placed above `view` is outside the range and ignored.

The noise change matches the test. A differing pixel inside a named rect is counted before the floor (`scripts/stage12-named.mjs:52`); outside, the floor is unchanged, including when there are no names. The new case’s fringe pixel `(2,3)` lies in `[2,2,4,4]`, differs by 2 on an edge, and becomes the name’s box `[2,3,3,4]`; `(2,7)` is outside that rect and stays out of `differ`. `bun test` on `stage12-named.test.mjs` and `stage12-gate.test.mjs`: 37 pass, 2 skip, 0 fail. The added release runs are on `origin/main`: `0952139ef4` (FollowButton’s primary stays the brand literal) and `d6d5fc3ec4` (EmptyState follow-up). `scripts/stage12-gate.mjs` is 1478 lines.

A second PersonRow call site, a FollowButton whose walk does not start in that PersonRow slot, and a spaced `else` are all refused.

VERDICT: CHANGES
