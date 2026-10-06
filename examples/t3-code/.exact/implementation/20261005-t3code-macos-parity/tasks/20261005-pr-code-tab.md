---
name: 20261005-pr-code-tab
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Pull request Code tab: diff, viewed marks, line comments and review threads

## Outcome

The pull request panel has the reference's third tab, "Code". A person can read the whole
change or one commit's change, page through large diffs, tick files as viewed, write line
comments into a pending review and send the review from the composer, read and answer the host's
review conversations (reply, resolve, edit, load more, react, "Fix in a thread"), and hand a
selection to the agent. Clicking a commit in the Timeline opens the tab on that commit.

## Scope and exclusions

Included: **C4** and the pull-request flavor of **G2**: Code tab and its toolbar; HTTP diff
slices; commit scope; Viewed marks (also "Changed" after a push); line-comment draft ("Add to
review", "Add to agent"); pending comment cards; thread cards; the list of conversations not
on the diff; Timeline commit rows opening the tab.
Excluded: the composer, review store and `submitReview` (done by `20261005-pr-writing-and-metadata`,
used here); the tree, lazy-file, annotation and comment-card engine (`20261005-diff-review-engine`,
reused); hand-off builders (`20261005-pr-handoffs-and-quick-actions`, called from here); other hosts' diff differences
(Azure DevOps has no patch; capability flags hide the tab).
Reuse (done): `parsePatch`/`diffSnapshot` model and toolbar toggles (`diff.ts`, `diff.contract:181-230`), tab bar (`pages-pr-detail.contract:166-167` gets a third `PrdTab`),
`pullRequests.invalidate` refresh (`pages-pr-detail.ts:88`), the PR draft/hand-off plumbing (`r6-pr-actions.ts:142-189`).

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` = `apps/web/src/components/pullRequest/`):
`W/PullRequestCodeTab.tsx:194-1557` (state 194-330, slices 330-380, positions 640-720, annotations 880-1040, toolbar 1060-1330), `W/PullRequestReviewAnnotation.tsx:57-358`,
`W/pullRequestDiff.logic.ts`, `pullRequestFilesViewed.logic.ts`, `usePullRequestFilesViewed.ts`, `pullRequestFileOrder.logic.ts`, `pullRequestReviewStore.ts`,
`W/PullRequestDetailPanel.tsx:2740-2758` (mount), `apps/web/src/lib/diffFileContents.ts:57-99`, `apps/web/src/reviewCommentContext.ts:220-400`;
client `packages/client-runtime/src/state/pullRequests.ts:163-260,285-330`, `pullRequestDiffHttp.ts`; server `apps/server/src/pullRequest/http.ts` (`POST /api/pull-requests/diff`, 60 s timeout),
`GitHubPullRequestCli.ts:1393-1550,2226-2280,2717-2790` (slices of 100 files; `gh pr diff` first, files API past 300 files or 8 MiB). Contracts `pullRequest.ts:948-1063,1135-1210`.
Clone paths `examples/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (late old replies, single-flight reads, persistence of nothing), layout-and-interaction (virtualized list rules), components (state lifetime; drafts survive tab switches only if kept in a longer-lived owner), design (all states), motion (collapse chevron, reduced motion), accessibility, testing-and-debugging. Unknown in the library: Swift transport changes, hooks.
Observed today: `readHTTP` accepts only GET without a body ("The client HTTP interface is read-only", `modules/apple/T3Transport.swift:490`), so the diff endpoint (POST with a JSON body) needs a narrow, allow-listed POST-JSON op there (large replies already stream through `readChunk`, `protocol.ts:56`); no client call exists for `diffFileContents`, `filesViewed`, `setFilesViewed`, `threadComments`, `replyToThread`, `setThreadResolution`, `updateComment`.
Reference rules to keep: line comments only on the whole change (not under a commit scope; icon `aria-label` "Line comments are written from the whole change"); a thread is placed only if its line is inside a rendered hunk, else it is listed under "Conversations not on the current diff" ("…not on the diff loaded so far" while slices remain); ticks are kept against the change request, not the scope; marking viewed folds the file (`toggleFileDiffFoldForViewed`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged (threads, activity, refresh) | pending |
| merged task PR | [20261005-pr-writing-and-metadata](20261005-pr-writing-and-metadata.md) | pending | Merged (composer, review store, editor, reactions) | pending |
| merged task PR | [20261005-diff-review-engine](20261005-diff-review-engine.md) | pending | Merged (annotation rows, tree, lazy rows, line-comment cards) | pending |
| merged task PR | [20261005-pr-handoffs-and-quick-actions](20261005-pr-handoffs-and-quick-actions.md) | pending | Merged (`buildFixFindingHandoff`, `buildAddSelectionToAgentHandoff`, the hand-off runner) | pending |
| merged task PR | [20261005-fake-github-fixture](20261005-fake-github-fixture.md) | pending | Diff, files, contents, viewed, thread verbs served (also reached through `20261005-pr-conversation-and-refresh`) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Authenticated POST with a body from a data module | Swift transport is GET-only for TS | nonblocking (workaround: new allow-listed POST-JSON op in `T3Transport.swift`) | Implement; keep the bearer in Swift |
| [X19](../issues/20261005-x19-data-source-timers.md) | 400 ms burst gathering of Viewed ticks | Reference `FLUSH_DELAY_MS = 400` (`usePullRequestFilesViewed.ts:27`) | nonblocking (workaround: flush on the next tick of the existing `wallTime` argument or a Contract task) | Test with `now` |
| [X23](../issues/20261005-x23-scroll-restore-offsets.md) | Reveal from the tree/timeline commit, keep scroll per tab | `scrollIntoView` on main | nonblocking | Reuse |
| [X13](../issues/20261005-x13-hover-keys-during-pan.md) / [X24](../issues/20261005-x24-still-pointer-rehover.md) | Gutter "+" and hover reveals | `t3-rehover` hook | nonblocking | Reuse Update 2026-10-07 (adopt-main-fixes-shell): X24 fixed on main #174; the `t3-rehover` hook no longer exists, the host does it. |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327/1,500 | nonblocking | No new root resource |
| [X32](../issues/20261005-x32-sticky-positioning-in-lists.md) | Sticky file headers in the Code tab's list | Same open check as `20261005-diff-review-engine` (`contract vocab --json position`) | unknown | Follow that ticket's result |

## Implementation notes

- **Port** (names, tests, header changes): `pullRequestDiff.logic.ts` (`isLineInFileDiff`, `isFileDiffCollapsed`, `toggleFileDiffFoldForViewed`), `pullRequestFilesViewed.logic.ts` (`toFileViewedStates`, `isStaleViewedState`, `isFileViewed`, `countViewedFiles`, `settleFileViewedOverlay`, `revertFileViewedOverlay`, `toFileViewedBatch`),
  `pullRequestFileOrder.logic.ts` (`orderDiffFiles`), `reviewCommentContext.ts` (`resolveDiffReviewPosition`, shared `buildDiffReviewComment` for "Add to agent"), `pullRequestDetail.logic.ts` (`mergePullRequestThreadComments`, `editPullRequestThreadComment`), the `usePullRequestFilesViewed` rules (queue, flush ≤500 paths per write, one write in flight per PR, rollback with the original overlay), `createPullRequestDiffFileContentsLoader` (`diffFileContents.ts:57-99`).
- **Slices.** `POST /api/pull-requests/diff` with `{…reference, cursor?, commit?}`; each answer is parsed on its own and cached by `cursor`+patch hash; a changed page drops later slices; first page re-read on Refresh (`refreshToken`); "Loading more files..." row and Retry ("The rest of this diff could not be loaded."); `omittedFileStats` fill withheld counts; warning icon `aria-label` "Some of this diff was not shown".
- **Toolbar** `aria-label`s: "Diff scope: <scope>" (menu with "All commits", commit headline + 7-char oid, "Show more (N left)", 10 per page), "Show/Hide whitespace changes" (re-parses, discards the draft), "Expand/Collapse all files", "Diff layout" (Stacked/Split), "Enable/Disable diff line wrapping", "Show/Hide file tree"; counts "N files" (+ when more slices), "<n> / <total> viewed" (or "viewed in T3 Code" with its info icon on hosts that keep ticks in the environment), "Your ticks could not be read", "This count covers only part of the change".
- **Viewed.** Checkbox in each file header (`aria-label` "Viewed", "Changed" with tooltip "This file has been pushed to since you marked it viewed."); the header press folds the file but not when the checkbox is pressed.
- **Draft/pending.** Positions: added `{newLine}`, deleted `{oldLine}`, context `{oldLine,newLine,side}`, `oldPath` for renames; "Add to review" writes the pending store (`20261005-pr-writing-and-metadata`); "Add to agent" (only with an active composer) builds the chip with the hand-off module; pending card `aria-label` "Discard this comment".
- **Thread cards** per `PullRequestReviewAnnotation.tsx`: header "Open · N comments"/"Resolved · N comments" (+ "outdated"), "Fix in a thread" (hand-off module), Resolve/Unresolve (needs `capabilities.review.resolve` and `viewerPermissions.resolve`), comment list with reaction bars and own-comment pencil (`kind: "review-comment"`), "Load more comments" by cursor, Reply (`aria-label` "Reply to this conversation", placeholder "Reply", ⌘↵ sends, Escape cancels). Every action disables the others while pending; failures toast "Reply could not be posted", "The comment could not be saved", "The conversation could not be updated", "More comments could not be loaded" and keep the typed text.
- Keep the Code tab mounted once visited (display toggle) so slices, scroll and drafts survive tab switches; reset on PR change.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Tab and slices | Fake gh: PR with 130 files (`pr diff` answers 406 so the files API pages 100 + 30), 14 commits, rename, binary file | `bun scripts/agent.mjs macos --json "tap pr-row-<n>" "tap pull-request-tab-code" tree` | Tab present (absent when `capabilities.diff` is false, unit test); first slice then "Loading more files..." then rest; counts "130 files"; withheld-file icon | macOS 1280×840 | tree JSON, trace of `/api/pull-requests/diff` |
| Slice failure | `failNext` on `…/files?…page=2` | Scroll to the end; Retry | Message + Retry; recovers | macOS | shot, log |
| Commit scope | Same | Pick a commit; "Show more (4 left)"; return to All commits | Diff changes; gutter disabled with the icon text; `…/commits/<sha>` logged | macOS | shots |
| Viewed marks | Profile `admin-reviewer` | Tick 3 files quickly; push a new commit in the fixture; refresh | One `markFileAsViewed` document with aliases `f0…f2` after the flush; counts "3 / 130"; file folds; pushed file shows "Changed"; failure reverts the overlay | macOS | log, shots |
| Line comments to review | Same | Draft on an added, deleted and context line (split view) `(attended session)` for the drag; "Add to review"; open composer; Submit review | Pending cards and badge; `POST …/pulls/N/reviews` body has three `comments` with correct `position`, `oldPath` for the rename; store cleared | macOS; attended part on a lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | log, shots |
| Add to agent | Active thread composer | Draft; "Add to agent" | Chip + prompt from `buildAddSelectionToAgentHandoff`; toast "Asked in a thread" or "Added to the composer" per context | macOS | draft state |
| Thread cards | Threads: open on added line, resolved on deleted line, outdated, 12 comments | Reply, Resolve, Unresolve, edit own comment, Load more, react, Fix in a thread | `addPullRequestReviewThreadReply`, `resolveReviewThread`/`unresolveReviewThread`, `updatePullRequestReviewComment` bodies logged; texts as listed; permission profile `reader` hides Resolve | macOS | log, shots |
| Orphans | Thread on a line outside the hunks | Open Code | Under "Conversations not on the current diff", count and file groups | macOS | shot |
| Timeline link | Timeline with commits | Click a commit row | Code tab opens scoped to that commit | macOS | tree JSON |
| Visual and trace | Oracle on the same fixture | Pairs at 1280×840 and 840×620, light and dark: toolbar, slices, viewed, draft, pending, thread open/resolved; `target/t3-ui-parity/trace-diff.mjs pr-code` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; diff request bodies and `runAction`-free read multisets equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "isLineInFileDiff", "isFileDiffCollapsed", "toggleFileDiffFoldForViewed", "isFileViewed", "countViewedFiles", "settleFileViewedOverlay", "toFileViewedBatch", "revertFileViewedOverlay", `orderDiffFiles` cases, "review thread comment pages", "keeps concurrent diff file reads on different hosts separate", "routes %s viewed marks to their storage environment" (GitHub case) | Pass; React-hook cases classified n/a-ui | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab through toolbar, scope menu, headers, checkbox, draft card and thread buttons; Escape closes the scope menu and cancels a draft or reply | Focus visible and returned to the trigger; icon buttons named; nothing is sent on Escape; chevron changes without rotation | macOS | `tree --ax`, state |
| Gates | `git add -A` | Clone checks (incl. the Swift transport test for the POST op); `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.*`, new `pages-pr-code.*`, `pages-pr-threads.*`, `pages-pr-viewed.ts`, `modules/apple/T3Transport.swift` (POST-JSON op) with `macos/tests/transport`, `diff.ts`/`diff.contract` (shared rows), `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, fake gh, reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-pr-conversation-and-refresh`, `20261005-pr-writing-and-metadata`, `20261005-diff-review-engine` and `20261005-pr-handoffs-and-quick-actions` merge.
