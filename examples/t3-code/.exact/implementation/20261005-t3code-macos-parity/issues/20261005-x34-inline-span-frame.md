---
name: 20261005-x34-inline-span-frame
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap (unconfirmed)
blocks: [20261005-pr-links-previews-and-routing]
upstream_url: https://github.com/ccheever/exact2/issues/133
reproduced_on: null
---

# X34: Hover and frame of an inline link or span inside rendered Markdown text

## Summary
In T3 Code every web link in rendered Markdown is a hover target. A link to a pull request of a
known project (an authored link, a `#N` reference or a commit hash) opens a preview card after
350 ms. Every other web link shows its URL in a tooltip. Both anchor to the link's own box, even
when the link wraps over two lines. In exact2 a `text` node holds inline runs, and a run is not a
node: it cannot take `hover=` or report a frame. The clone lays out link-bearing prose as one
`text` element per word (for code boxes and the favicon). A one-word link can be wrapped in a hover
box. A link of several words has several elements and no single anchor. exact2 needs hover events and a frame
(the union of the line boxes) for an inline link or span.

## Why this issue arose

### The T3 Code behavior
- **Autolinks.** In pull request prose, `#N` and 40-character hashes become links
  (`apps/web/src/components/pullRequest/pullRequestMarkdown.logic.ts:157-192`
  `remarkPullRequestAutolinks`; the hash shows its first 7 characters). Authored `link` and
  `linkReference` nodes are never autolinked (`AUTOLINK_IGNORED_TYPES`, `:154`).
- **Card.** `ChatMarkdown.tsx:3043-3260` computes a preview target from the link URL with
  `resolvePullRequestPreviewTarget` (`lib/openPullRequestLink.ts:98`: the pull request feature is on
  and a project matches the repository) and wraps the link in `PullRequestLinkPreview`
  (`:3206-3228`). The card opens after 350 ms and closes 120 ms after leave
  (`pullRequest/PullRequestLinkPreview.tsx:110-114`), popup `align="center"`, `w-80`
  (`:118`; offset and side from `ui/preview-card.tsx:9-40`). It shows repository and `#N`, the state
  with its glyph, the title, and "<author> · opened <relative time>" (`:119-147`). While detail
  loads there is no card. If the read fails and no `fallback` was passed (`ChatMarkdown` passes
  none), the link shows a tooltip with the original URL (`showCard` and `showUrlTooltip`, `:88-89`).
  Also used by `contextChipParts.tsx:123`.
- **Click.** A `#N` reference (`confirmBeforeOpen`) first reads `pullRequests.preview`, then
  opens the pull request in the panel, or falls back to the system browser (`:64-84`).
- **Every other web link.** A tooltip with the URL (`ChatMarkdown.tsx:3231-3235`) and a favicon
  before the label (`MarkdownExternalLinkContent`, `:2011`, used at `:3195`). A link without a web
  host gets neither (`:3203`).
- **A17 (`32b77f4fa3`).** A real scroll closes the card or tooltip; the tooltip also cancels the
  card's delayed hover open (`PullRequestLinkPreview.tsx:103-108`). Test: `chat/MessagesTimeline.test.tsx:307`
  mounts the preview in a scroll-dismiss area. Autolink tests: `pullRequestMarkdown.logic.test.ts`
  (16 `it`/`test` blocks).

### What exact2 does today
- Not in `EXACT2-GAPS.md` as its own item (found while writing the plan's tickets, 2026-10-05).
  Related, from its X22 detail: "`frame()` now reads viewport space (`e73605835`), but works only in
  actions and is not reactive (`docs/contract-for-humans.md:1150-1156`)" (written from framework
  source at `c1522fdac`, checked against `main` `d2cb661eb`). Whether a `text` run can carry
  `hover=` or have a frame is **to confirm at `issue-open`**.
- Bundled library (`20261005-platforms-v3`): not covered: unknown. Not reproduced.

### Where the clone hits it
Observed by code reading (mc-orch tree, 2026-10-05). `macos/src/markdown.rs:332` `flow_tokens`
lays out any paragraph, item or quote that holds a code span, a link or a line break, one word per
token. Each word keeps its run's `href`. The favicon precedes the first word (`chat_run`,
`markdown.rs:109-113`; the `link-start` branch at `markdown.contract:358-362` draws a globe box and
then `text run.text href=…`). Prose without these stays one `text` node (`ChatRuns`,
`markdown.contract:313-328`). In `markdown.contract` only chips and buttons have `hover=`
(lines 478-692); no web link has a tooltip or a card. File links and context chips are boxes
wrapped in `CodeTip` or `Tip` (`:415-436`). So a one-word link such as `#101` can be wrapped in
a hover box and anchored to it, as chips are. A link of several words has several elements. If the
one-word workaround is built, a multi-word or wrapped link anchors to one word, not to the link's
union rectangle with align center, and a wrapper per word changes line breaking (to confirm).

## Why it must be resolved
Parity means the card opens on every PR link and every web link shows its URL, as in the
reference. The one-word workaround covers `#N` references and hashes, which are always one word.
It does not cover `[text](pull request URL)`, which the reference also turns into a card. Nothing
in the plan counts such links as rare. `20261005-pr-links-previews-and-routing` carries the row
and a spike at `prepare`. Other tickets that render ordinary links may need the URL tooltip
and are checked at their `prepare`. Keeping the workaround costs a per-word wrapper in
`FlowRuns`, shared hovered-link state, and a card position that each oracle pair must explain.

## Requested support
The web way: an inline `a` or `span` is an element with `pointerenter`/`pointerleave` and
`getClientRects()` (the union of its line boxes). A popover anchors to it with CSS `anchor-name`.
Offered for macOS first:
- **A (preferred).** Inline children of a `text` node: `span` and `link` with `hover=`, `id` and
  `testId`, a readable `frame()` that returns the union rectangle and the list of line
  rectangles, and `anchor-name` on an inline span as the target of `position-anchor` (see X17).
- **B.** An inline-level box (`display: inline`) around runs, with hover and frame. The host
  reports the union rectangle. The box must wrap like text.
- **C (app side, partial).** Keep the per-word wrapper and publish the hovered word's frame. This
  is exact for one-word links only.
Trade-off: A also serves mentions and any later inline target on every host. C is today's answer.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: a paragraph that wraps over
two lines and holds a three-word link, with a hover handler and a popover anchored to the link.
Expected (Chrome): hovering any word opens one popover centered on the union rectangle, and
leaving closes it. Actual: no hover target for a run, and nothing to anchor to. Clone scenario: a
pull request description with `Fixes #101`, a 40-character hash and
`[see the earlier fix](https://github.com/…/pull/12)`. Hover each for 350 ms. Expected
(reference): a card for `#101` and the pull request link. Actual: to confirm. Real pointer hover
needs an attended session (X8) unless the agent has a hover op (to confirm).

## Acceptance for the fix
- Minimal app: hover on the first, middle and last word of a wrapped link gives the same anchor
  rectangle (union of the line boxes) as Chrome within 1 pt. A card opens after the app's delay and
  closes on leave.
- Agent: `layout <link id>` returns the rectangle list, and `state` shows the hovered id.
- Clone: the hover rows of `20261005-pr-links-previews-and-routing` for `#N`, a hash and a
  multi-word pull request link pass in an attended session with the oracle pair.

## App adoption after resolution
Replace the per-word hover wrapper (if built) with inline spans in `markdown.contract` and anchor
the card to the span. Add URL tooltips for ordinary web links. Unblock the held rows in
`20261005-pr-links-previews-and-routing` and update its Issue assessment row. `issue-close`
verifies those rows.

## Status and next action

Filed as #133 and closed. Main #178 fixed the macOS agent's hover on inline runs (enter, leave, hit test at a point). task `20261007-adopt-main-fixes-input`: the clone has no inline-link hover card and no workaround to remove. Unblocked for `20261005-pr-links-previews-and-routing`: an inline link's hover can be built and driven by the agent on macOS. Still missing on main: `frame()` of an inline run and inline runs in agent `layout`, so the card has no rectangle to anchor to.

Re-checked 2026-10-07 on main `cff90b364` (task [20261007-adopt-main-fixes-r3](../tasks/20261007-adopt-main-fixes-r3.md)): `frame()` of an inline run and inline runs in agent `layout` are still missing; nothing to adopt.
