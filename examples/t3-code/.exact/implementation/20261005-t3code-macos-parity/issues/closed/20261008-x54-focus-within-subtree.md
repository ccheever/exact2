---
name: 20261008-x54-focus-within-subtree
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-pr-writing-and-metadata]
upstream_url: https://github.com/ccheever/exact2/issues/283
reproduced_on: 0365ad1a4 (main)
---

# X54: an ancestor hearing the focus enter its subtree (`focusin`, `:focus-within`)

Moved to main `issues/20261009-bubbling-focus-events.md` (2026-10-09); tracked there.

## Summary

T3 Code expands a long pull request comment when the keyboard focus moves into it
(`onFocusCapture` on the collapsed frame), and reveals a remark's pencil while anything in the
remark has the focus (`group-focus-within`). Contract carries `focus` and `blur` only on the element
that takes the focus; there is no `focusin`/`focusout` (which bubble) and no `:focus-within` state,
so a box cannot learn that a link inside its markdown took the focus.

## Why it arose

### The T3 Code behavior
- `apps/web/src/components/pullRequest/PullRequestCommentBody.tsx` (1e2ecbd975): the collapsed frame
  (`max-h-[240px] overflow-hidden`) has `onFocusCapture={() => setExpanded(true)}`, so tabbing to a
  link hidden below the fold opens the comment instead of focusing something out of sight.
- `PullRequestEditButton.tsx`: the pencil is `opacity-0 … group-focus-within:opacity-100`, so it shows
  while any control of its remark has the focus, not only while the pencil itself does.

### Where the clone hits it
`pages-pr-edit.contract` `PrdWords` ([pr-writing-and-metadata](../../tasks/closed/20261005-pr-writing-and-metadata.md)):
a remark taller than 240pt is held behind "Show full comment"; a link inside the held part can take the
Tab focus while it is clipped. `PrdEditButton` shows on the remark's hover or its own focus only.

## Clone workaround

Nonblocking: "Show full comment" is a Tab stop before the body, and the pencil is a Tab stop that shows
itself when focused. What differs for the user: a keyboard reader must press "Show full comment" before
tabbing into a long remark's links, and the pencil appears when it is itself focused rather than when
the remark is. On adoption, `PrdWords` expands on `focusin`, and `PrdEditButton`'s pencil hides only
when `relatedTarget` is outside its remark (the design the upstream decision of 2026-10-08 named).

## Evidence and history

- Draft reproduced with `contract vocab` on the feature branch's framework (`focusin`/`focusout`: "not a
  tag or an attribute"; `focus-within`: not a built-in attribute).
- Filed as [#283](https://github.com/ccheever/exact2/issues/283) ([Feature] `focusin`/`focusout` (or
  `:focus-within`): an ancestor hears the focus enter its subtree) on 2026-10-08, reproduced on main
  `0365ad1a4` with a minimal public-API app: a `column focus=open` around a `link` is a stop of its own
  (3 → 5 → 7 → 9) and Tab onto the link leaves `opened 1`, on macOS and the web. No duplicate found.
