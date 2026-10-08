---
name: 20261008-x54-focus-within-subtree
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-pr-writing-and-metadata]
upstream_url: https://github.com/ccheever/exact2/issues/283
reproduced_on: 0365ad1a4 (main)
---

# X54: an ancestor hearing the focus enter its subtree (`focusin`, `:focus-within`)

## Summary

T3 Code expands a long pull request comment when the keyboard focus moves into it
(`onFocusCapture` on the collapsed frame), and reveals a remark's pencil while anything in the
remark has the focus (`group-focus-within`). Contract carries `focus` and `blur` only on the element
that takes the focus; there is no `focusin`/`focusout` (which bubble) and no `:focus-within` state,
so a box cannot learn that a link inside its markdown took the focus.

## Why this issue arose

### The T3 Code behavior
- `apps/web/src/components/pullRequest/PullRequestCommentBody.tsx` (1e2ecbd975): the collapsed frame
  (`max-h-[240px] overflow-hidden`) has `onFocusCapture={() => setExpanded(true)}`, so tabbing to a
  link hidden below the fold opens the comment instead of focusing something out of sight.
- `PullRequestEditButton.tsx`: the pencil is `opacity-0 … group-focus-within:opacity-100`, so it shows
  while any control of its remark has the focus, not only while the pencil itself does.

### What exact2 does today
- `bun scripts/exact.mjs contract vocab focusin` / `focusout`: "not a tag or an attribute";
  `vocab focus-within`: not a built-in attribute.
- `docs/contract-grammar.md` (Events): `focus` and `blur` carry no payload and fire on the element
  that takes the focus; an element with a `focus` handler becomes focusable itself (a Tab stop), so a
  wrapper cannot listen without joining the Tab order.

### Where the clone hits it
`pages-pr-edit.contract` `PrdWords` (pr-writing-and-metadata): a remark taller than 240pt is held
behind "Show full comment"; a link inside the held part can take the Tab focus while it is clipped.
`PrdEditButton` shows on the remark's hover or its own focus only. What differs for the user: a
keyboard reader must press "Show full comment" before tabbing into a long remark's links, and the
pencil appears when it is itself focused rather than when the remark is.

## Why it must be resolved

Keyboard parity: the reference never moves the focus into clipped content. Nonblocking: "Show full
comment" is a Tab stop before the body, and the pencil is a Tab stop that shows itself when focused.

## Requested support

The web's: `focusin` and `focusout` events that bubble (with the target's identity or a boolean
"inside" as the payload), heard without making the listener focusable; or a `:focus-within` fact a
binding can read on any element.

## How to reproduce

On the pinned framework: `bun scripts/exact.mjs contract vocab focusin` (refused). A one-file app
`column focus=opened` around a `link` makes the column a Tab stop of its own (the doc's rule), and a
Tab onto the link does not run `opened`.

## Acceptance for the fix
- A `focusin` (or `:focus-within`) on a column runs when a descendant link takes the focus by Tab,
  on macOS and the web, and the column is not itself a Tab stop.
- The clone: `PrdWords` expands on it; `PrdEditButton` shows while its remark has the focus.

## Status and next action
Draft; reproduced with `contract vocab` on the feature branch's framework; no upstream match by
title; not published (the brief: report framework problems with a repro, file nothing).

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/283 (#283, [Feature] `focusin`/`focusout` (or `:focus-within`): an ancestor hears the focus enter its subtree). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) with a minimal public-API app before filing. `contract vocab focusin`/`focusout`/`focus-within`: not a tag or an attribute. A `column focus=open` around a `link` is a stop of its own (3 → 5 → 7 → 9) and Tab onto the link leaves `opened 1`, on macOS and the web. Searched: focusin, focus-within: no duplicate.

Next: issue-close once #283 lands: `PrdWords` expands on it; `PrdEditButton` shows while its remark has the focus.
