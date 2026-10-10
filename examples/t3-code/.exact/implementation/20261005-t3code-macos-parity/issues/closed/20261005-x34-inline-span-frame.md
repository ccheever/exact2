---
name: 20261005-x34-inline-span-frame
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap (unconfirmed)
blocks: [20261005-pr-links-previews-and-routing]
upstream_url: https://github.com/ccheever/exact2/issues/133
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/272
---

# X34: Hover and frame of an inline link or span inside rendered Markdown text

Moved to main `issues/20261009-apple-inline-run-frame-layout.md` (2026-10-09); tracked there.

## Summary
In T3 Code every web link in rendered Markdown is a hover target. A link to a pull request of a
known project (an authored link, a `#N` reference or a commit hash) opens a preview card after
350 ms. Every other web link shows its URL in a tooltip. Both anchor to the link's own box, even
when the link wraps over two lines. In exact2 a `text` node holds inline runs, and a run is not a
node: when the clone was planned it could not take `hover=` or report a frame.

## Why it arose

### The T3 Code behavior
- **Autolinks.** In pull request prose, `#N` and 40-character hashes become links
  (`apps/web/src/components/pullRequest/pullRequestMarkdown.logic.ts:157-192`
  `remarkPullRequestAutolinks`). Authored `link` and `linkReference` nodes are never autolinked
  (`AUTOLINK_IGNORED_TYPES`, `:154`).
- **Card.** `ChatMarkdown.tsx:3043-3260` computes a preview target from the link URL with
  `resolvePullRequestPreviewTarget` (`lib/openPullRequestLink.ts:98`) and wraps the link in
  `PullRequestLinkPreview` (`:3206-3228`). The card opens after 350 ms and closes 120 ms after leave
  (`pullRequest/PullRequestLinkPreview.tsx:110-114`), popup `align="center"`, `w-80`. If the read fails,
  the link shows a tooltip with the original URL (`:88-89`).
- **Every other web link.** A tooltip with the URL (`ChatMarkdown.tsx:3231-3235`) and a favicon
  before the label (`MarkdownExternalLinkContent`, `:2011`).
- **A17 (`32b77f4fa3`).** A real scroll closes the card or tooltip (`PullRequestLinkPreview.tsx:103-108`).
  Tests: `chat/MessagesTimeline.test.tsx:307`, `pullRequestMarkdown.logic.test.ts` (16 blocks).

### Where the clone hit it
Code reading (mc-orch tree, 2026-10-05): `macos/src/markdown.rs:332` `flow_tokens` lays out any paragraph,
item or quote that holds a code span, a link or a line break, one word per token, each keeping its run's
`href`; prose without these stays one `text` node (`ChatRuns`, `markdown.contract:313-328`). So a one-word link
such as `#101` could be wrapped in a hover box, but a link of several words has several elements and no single
anchor. `20261005-pr-links-previews-and-routing` carried the hover rows.

## Clone workaround
[pr-links-previews-and-routing](../../tasks/closed/20261005-pr-links-previews-and-routing.md) (#311) built the card
without an inline-run frame: `PrLinkRun` sends `hoverTipAtFrame` with the link chip's own box
(`frame(pr-link-<id>)`), and the window's hover layer draws `PrLinkPreviewCard` (350 ms open, 120 ms close),
placed with its `hoverFlip`. Declared difference: a multi-word or wrapped link anchors to its chip's box, not to
the union of its line boxes. Once main gives an inline run's `frame()`, the card anchors to the run.

## Evidence and history
- Filed as [#133](https://github.com/ccheever/exact2/issues/133); closed after main #178 fixed the macOS agent's
  hover on inline runs (enter, leave, hit test at a point). `20261007-adopt-main-fixes-input`: no workaround to
  remove; the inline link's hover became buildable and drivable by the agent.
- Re-checked 2026-10-07 on main `cff90b364` ([adopt-main-fixes-r3](../../tasks/closed/20261007-adopt-main-fixes-r3.md)):
  `frame()` of an inline run and inline runs in agent `layout` still missing.
- The rest filed as [#272](https://github.com/ccheever/exact2/issues/272) on 2026-10-08, reproduced on main
  `0365ad1a4`: macOS `frame("link")` gives `0,0 0x0 unavailable=true` and `layout` lists no inline runs; the web
  gives `24,24 228.34375×36`. Agent hover (#178) still passes.
