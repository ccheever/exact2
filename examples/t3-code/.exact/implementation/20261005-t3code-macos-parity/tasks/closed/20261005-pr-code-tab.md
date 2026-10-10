---
name: 20261005-pr-code-tab
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-code-tab
pr_url: https://github.com/ccheever/exact2/pull/308
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

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` = `apps/web/src/components/pullRequest/`):
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
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged (threads, activity, refresh) | pending |
| merged task PR | [20261005-pr-writing-and-metadata](20261005-pr-writing-and-metadata.md) | pending | Merged (composer, review store, editor, reactions) | pending |
| merged task PR | [20261005-diff-review-engine](20261005-diff-review-engine.md) | pending | Merged (annotation rows, tree, lazy rows, line-comment cards) | pending |
| merged task PR | [20261005-pr-handoffs-and-quick-actions](20261005-pr-handoffs-and-quick-actions.md) | pending | Merged (`buildFixFindingHandoff`, `buildAddSelectionToAgentHandoff`, the hand-off runner) | pending |
| merged task PR | [20261007-real-github-lane](20261007-real-github-lane.md) | pending | Sandbox `many-files` and `second-review` threads; probe rows for diff slices, `diffFileContents`, `filesViewed`, `setFilesViewed` and the thread writes confirmed | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../../issues/closed/20261005-x21-two-way-websocket.md) | Authenticated POST with a body from a data module | Swift transport is GET-only for TS | nonblocking (workaround: new allow-listed POST-JSON op in `T3Transport.swift`) | Implement; keep the bearer in Swift |
| [X19](../../issues/closed/20261005-x19-data-source-timers.md) | 400 ms burst gathering of Viewed ticks | Reference `FLUSH_DELAY_MS = 400` (`usePullRequestFilesViewed.ts:27`) | nonblocking (workaround: flush on the next tick of the existing `wallTime` argument or a Contract task) | Test with `now` |
| [X23](../../issues/closed/20261005-x23-scroll-restore-offsets.md) | Reveal from the tree/timeline commit, keep scroll per tab | `scrollIntoView` on main | nonblocking | Reuse |
| [X13](../../issues/closed/20261005-x13-hover-keys-during-pan.md) / [X24](../../issues/closed/20261005-x24-still-pointer-rehover.md) | Gutter "+" and hover reveals | `t3-rehover` hook | nonblocking | Reuse Update 2026-10-07 (adopt-main-fixes-shell): X24 fixed on main #174; the `t3-rehover` hook no longer exists, the host does it. |
| [X9](../../issues/closed/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327/1,500 | nonblocking | No new root resource |
| [X32](../../issues/closed/20261005-x32-sticky-positioning-in-lists.md) = [#131](https://github.com/ccheever/exact2/issues/131) | Sticky file headers in the Code tab's list | Charlie deferred #131 on 2026-10-08 "until a list-lifetime design is selected" (grouped containment is one option) | nonblocking (headers scroll as ordinary rows, as the thread diff panel draws them) | Wait for #131; if it lands as grouped containment, `diffItems` becomes per-file groups |

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
| Tab and slices | Sandbox `many-files` (310 files: GitHub answers 406 to `pr diff`, the files API pages 100 at a time); this task seeds the 14 commits, the rename and the binary file it also needs | `bun scripts/agent.mjs macos --json "tap pr-row-<n>" "tap pull-request-tab-code" tree` | Tab present (absent when `capabilities.diff` is false, unit test); first slice then "Loading more files..." then rest; counts "310 files"; withheld-file icon | macOS 1280×840 | tree JSON, trace of `/api/pull-requests/diff` |
| Slice failure | Unit test: an injected failure of the second slice | Scroll to the end; Retry | Message + Retry; recovers | macOS | shot, log |
| Commit scope | Same | Pick a commit; "Show more (4 left)"; return to All commits | Diff changes; gutter disabled with the icon text; `…/commits/<sha>` logged | macOS | shots |
| Viewed marks | Primary account on `second-review` | Tick 3 files quickly; push a new commit with the second account's lane gh; refresh | One `markFileAsViewed` document with aliases `f0…f2` after the flush; counts "3 / 310" on `many-files`; file folds; pushed file shows "Changed"; failure reverts the overlay | macOS | log, shots |
| Line comments to review | Same | Draft on an added, deleted and context line (split view) `(attended session)` for the drag; "Add to review"; open composer; Submit review | Pending cards and badge; `POST …/pulls/N/reviews` body has three `comments` with correct `position`, `oldPath` for the rename; store cleared | macOS; attended part on a lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | log, shots |
| Add to agent | Active thread composer | Draft; "Add to agent" | Chip + prompt from `buildAddSelectionToAgentHandoff`; toast "Asked in a thread" or "Added to the composer" per context | macOS | draft state |
| Thread cards | Threads: open on added line, resolved on deleted line, outdated, 12 comments | Reply, Resolve, Unresolve, edit own comment, Load more, react, Fix in a thread | `addPullRequestReviewThreadReply`, `resolveReviewThread`/`unresolveReviewThread`, `updatePullRequestReviewComment` bodies logged; texts as listed; `reader` hiding Resolve by unit test | macOS | log, shots |
| Orphans | Thread on a line outside the hunks | Open Code | Under "Conversations not on the current diff", count and file groups | macOS | shot |
| Timeline link | Timeline with commits | Click a commit row | Code tab opens scoped to that commit | macOS | tree JSON |
| Visual and trace | Oracle on the same fixture | Pairs at 1280×840 and 840×620, light and dark: toolbar, slices, viewed, draft, pending, thread open/resolved; `target/t3-ui-parity/trace-diff.mjs pr-code` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; diff request bodies and `runAction`-free read multisets equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "isLineInFileDiff", "isFileDiffCollapsed", "toggleFileDiffFoldForViewed", "isFileViewed", "countViewedFiles", "settleFileViewedOverlay", "toFileViewedBatch", "revertFileViewedOverlay", `orderDiffFiles` cases, "review thread comment pages", "keeps concurrent diff file reads on different hosts separate", "routes %s viewed marks to their storage environment" (GitHub case) | Pass; React-hook cases classified n/a-ui | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab through toolbar, scope menu, headers, checkbox, draft card and thread buttons; Escape closes the scope menu and cancels a draft or reply | Focus visible and returned to the trigger; icon buttons named; nothing is sent on Escape; chevron changes without rotation | macOS | `tree --ax`, state |
| Gates | `git add -A` | Clone checks (incl. the Swift transport test for the POST op); `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.*`, new `pages-pr-code.*`, `pages-pr-threads.*`, `pages-pr-viewed.ts`, `modules/apple/T3Transport.swift` (POST-JSON op) with `macos/tests/transport`, `diff.ts`/`diff.contract` (shared rows), `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, the real-GitHub lane (sandbox, shared lane config dirs, two accounts; `tools/github-lane`), reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

2026-10-08 (records sync): merged into `feat(example)/t3-code` as #308 (`0e2901aec`); the record moved to `tasks/closed/`. Its real-input rows are in `STATUS.md` "Next real-input batch".

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08 (draft PR #308, base `ec32c8c37`, merged `fa46ad5d0`): built end to end.

- **Files.** `pages-pr-code.ts` (state, reads, view, presses), `pages-pr-code-logic.ts` (the ported logic), `pages-pr-code-rows.ts` (the list), `pages-pr-viewed.ts` (the ticks' store), `pages-pr-threads.ts` (the cards' model and writes), `pages-pr-code.contract`, `pages-pr-threads.contract`, `diff-rows.contract` (the thread diff panel's `DiffRow`/`DiffTool`, moved: the panel is reachable from `shapes.contract`, which reaches the PR panel), `modules/apple/T3Transport+PullRequests.swift` (`prDiff`), `macos/tests/transport/pull-requests.swift`; edits in `pages-pr-detail.*`, `pages-pr-timeline.*`, `pages-pr-handoffs.ts` (`prSelectionHandoff`), `pages-sources.ts`, `timeline-presentation.ts` (`chatlocal:pr-code-*`), `pages-prefs.ts` (`prFileTree`), `diff.ts` (exports), `r7-handoff.contract` (its own `prdTone`), `T3Transport.swift` (`http` internal, a 60 s session, the area entry), `app.contract` (only `prTab` on the `prDetail` line: **0 net root lines**, 1478).
- **Root budget (coordinator).** No root task: the ticks' 400 ms wait is the panel resource's native sleep and the write is detached (`composer-replies.ts` `startDetached`), so X19's stand-in is the "wait inside" one, with no root line.
- **Seed.** `tools/github-lane/seed.mjs` `codeScenarios()` (`--only code-tab`, #168), a `history` field for multi-commit scenarios, `codeReviewConversation`; `lane.mjs` ports 16300-16799.
- **Fixed from the drives:** the Timeline commit row's empty id; the scope menu's glass and headline; the scope trigger at 840; the Refresh storm (the detail's invalidate flag, same fix as #306).
- **Decisions.** Provisional, user decision pending: none. Kept deviations: headers not sticky (X32 = #131, deferred by Charlie on 2026-10-08 until a list-lifetime design is selected; if #131 lands as grouped containment, `diffItems` becomes per-file groups); the scope menu's glass on an opaque colour (X11); split-view annotations stack under the row; tooltips through the existing `Tip` until the shared hover layer (#307, fix-hover-cards) lands (a TODO hook on the commit headline).
- **Merge order (coordinator, 2026-10-08).** fix-keyboard-focus adds `KeyMenu` with an `id` per row to every menu: the scope menu's rows already carry stable ids (= their testIds: `pull-request-code-scope`, `pull-request-code-scope-<short oid>`, `pull-request-code-scope-more`; "All commits" takes `id=testId` from that PR's `PrMenuItem`). Expect a conflict in `diff.contract` (this branch moved `DiffRow`/`DiffTool` out, that one adds its `KeyMenu` uses beside them): keep both. After #307: move the `Tip`s inside the diff list (the "Changed" tick, the conversations-off-the-diff paths, a comment's author and age) and the file tree onto the hover layer.

## Live sessions 3 and 4 (the extra session, 2026-10-08)

One `agent.mjs` drive (3) and its retry (4), the session the coordinator allowed; then the real-input session (5). Rows:
- **Pass (drive 3, 08:42Z):** Resolve then Unresolve the open thread (GraphQL `isResolved` true, then false; the card folds
  to "Resolved · 1 comment"); 👍 on the second account's comment (+1 by daehyeonmun2021) and taken back (0); split-view
  "Add to review" on added 5, deleted 5, deleted 4 of docs/usage.md and added 3 of src/strings.js, then the composer's
  Review mode and Submit review: one `submitReview`, one COMMENTED review with the 4 comments (bodies match), placed back
  as threads on their lines, pending 0, badge empty. Re-checks of the three fixes: the Timeline's commit row opens Code
  scoped ("Diff scope: Search the catalog", 1 file); 840×620 keeps the scope trigger (36×24); the scope menu is opaque with
  its headlines cut by an ellipsis, Escape closes it.
- **Pass (drive 4, 08:50Z):** Load more comments (the second account added 11 replies first: the card shows 10 of 12,
  pressed: 12, one `threadComments` call, the button gone; replies deleted after); Escape on a draft (closed, nothing
  pending, no `submitReview`); Add to agent (a new thread's composer holds the request with the "#168" and "usage.md L11"
  chips; no toast was read).
- **Not run:** the review comments' line/side read back from `pulls/comments/<id>` (drive 3 read `reviews/<id>/comments`,
  which has no line or side; drive 4 stopped when the tree reveal of src/strings.js did not move the list — see below); the
  opened conversations-off-the-diff list under reduced motion (drive 4 asked for the rename commit, which sits behind
  "Show more (5 left)": a script error); Fix in a thread (the driver's clock refused a step: "the clock cannot go
  backwards", not the app); #115 (its search never listed `pr-row-115` in drive 3, so drive 4 used #168).
- **To check first next time:** in drive 4, after Load more grew the open thread's card from 10 to 12 comments, the tree
  reveal of `src/strings.js` (a few files from the list's end) left the list where it was. Drive 3 revealed the same file
  without the grown card. If it reproduces, the list keeps a stale extent after a row grows (a list re-measure bug).

## Real-input batch steps

Session 5 (2026-10-08, lock 08:55:34Z–09:09:52Z, [record](https://raw.githubusercontent.com/ccheever/exact2/fa5314e640befd51d192ce61fe76877d6e578f6d/pr-code-tab/real-input-record.txt)): lane copy
`T3 Code (Lane PCT).app` (`com.exact.t3code.lanepct`, re-signed, `CFFIXED_USER_HOME` under `target/github-lane/realinput`,
no `T3_LOCAL_HOME`/`T3_LOCAL_PORT`, so no embedded server), paired to 127.0.0.1:16310. **Pass:** a real click opens the scope
menu and real Escape closes it with the ring on the trigger; real Return on the trigger opens it, Escape closes it; real Tab
from the trigger: whitespace, Expand all, Stacked, Split, wrap, tree, the conversations-off-the-diff row, a file's chevron,
its name, its Viewed box, the next file, the ring on each; no GitHub write. (A file chevron's ring is clipped by the list's
left edge.) The screen locked at 09:08:50Z, so the rest waits for the next batch (listed in `STATUS.md`):
1. Build `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`; copy the `.app` to
   `target/github-lane/realinput/T3 Code (Lane PCT).app`; PlistBuddy `CFBundleIdentifier` `com.exact.t3code.lanepct`,
   `CFBundleName`/`CFBundleDisplayName` "T3 Code (Lane PCT)"; `codesign --force --deep --sign <development identity>`.
   Lane: `bun lane.mjs start primary --port 16310`. Take the lock. Launch the binary with
   `CFFIXED_USER_HOME=$PWD/target/github-lane/realinput/home` (record the pid). Pair: click the field by coordinates,
   ⌘A, `paste-text` a fresh `http://127.0.0.1:16310/pair#token=<lane.pairingToken('primary')>`; Pair, Continue,
   Continue, Do not import projects. Pull Requests (real click), #168, Code; tree › "Load more files" ×3 (310 files).
2. Drag in the gutter from docs/usage.md new line 3 to 5 (`orca computer drag`): the lines paint, the draft says
   "Comment on lines docs/usage.md:3-5"; `paste-text` a body, ⌘↵ adds it (pending card, badge 1); a second draft, Escape:
   nothing pending, no write in `gh-calls.tsv`. Discard the pending card.
3. A thread's Reply: `paste-text`, ⌘↵ sends (read back with the lane gh, then delete it); a second Reply, Escape: nothing sent.
4. System Settings › Accessibility › Display › Reduce motion on (with the user's permission); scope "Rename the text
   helpers" (after "Show more (5 left)"); open "Conversations not on the current diff": the chevron swaps without turning,
   the list shows both threads; Reduce motion off, confirm it reads off.
5. Fix in a thread on a card (real click): the shared hand-off opens its thread.
6. Submit review read back by line/side: one "Add to review" on added 3 of src/strings.js (the rename), Submit review,
   `gh api repos/daehyeonmun2021/playground/pulls/comments/<id>` shows `path` src/strings.js, `line` 3, `side` RIGHT;
   delete it.
7. Drive 4's reveal: Load more on the open thread (after seeding 11 replies by the second account), then the tree's
   src/strings.js: the list must bring it into view (delete the replies after).
Cleanup: delete what was posted; quit the copy (its pid); delete its Keychain item (`com.exact.t3code.macos.access-token`,
account `http://127.0.0.1:16310` + environment id); remove the copy and its home; restore the pasteboard;
`bun lane.mjs stop primary`; release the lock.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (drive 1, 07:46Z) | `e2236599b` build | slices 4/4 (310 files), tree, threads, reply + edit read back, split drafts, scope menu, commit scope, dark: pass; Timeline row, scope menu look, 840 trigger: **fail → fixed** | [record](https://raw.githubusercontent.com/ccheever/exact2/0ed67de567c17c37e007b3dd179bae8550323bdc/pr-code-tab/record-attempt1.txt) | drive script: hidden duplicate testIds, `tap … into` |
| 2 (drive 2, 08:13Z) | `295b7cac9` build | 3 ticks → one write, VIEWED; push → "Changed"; untick → one write; real slice-4 failure → message + Retry; Refresh storm **found → fixed** | [record](https://raw.githubusercontent.com/ccheever/exact2/b5eafd9801a9897690f81514f8a0606685d18372/pr-code-tab/record-attempt2.txt), [summary](https://raw.githubusercontent.com/ccheever/exact2/a8bec068a6c0aba30b04c7bcb2421fa86888175f/pr-code-tab/live-summary.txt) | rows after slice 4 not run (session budget) |
| 3 (drive 3, 08:42Z) | `e8845c17d` build | Resolve/Unresolve, 👍 and back, 4 drafts → Submit review (4 comments on GitHub), Timeline row, 840, scope menu: pass; draft Escape unclear; #115 never listed | [record](https://raw.githubusercontent.com/ccheever/exact2/c32c819c1997cec70630591f65275fcb49d0a481/pr-code-tab/record-attempt3.txt), images 14–19 | #115 search; reviews/<id>/comments has no line/side |
| 4 (drive 4, 08:50Z; retry of 3) | same build | Load more (10 → 12), draft Escape, Add to agent: pass; line/side read back, off-diff list, Fix: not run (reveal after a grown card, a script error, the driver's clock) | [record](https://raw.githubusercontent.com/ccheever/exact2/833fb18ccc01a2176ff436bbed181aae98f0b922/pr-code-tab/record-attempt4.txt), images 20–22 | session budget used |
| 5 (real input, 08:55–09:09Z) | build of `e8845c17d` + the scope rows' ids | real click/Return/Escape on the scope menu, real Tab through toolbar and file header: pass | [record](https://raw.githubusercontent.com/ccheever/exact2/fa5314e640befd51d192ce61fe76877d6e578f6d/pr-code-tab/real-input-record.txt), images 23–24 | the screen locked: drag, Reply keys, reduced motion, Fix → next real-input batch |
| gates | `39bed7633` | bun 3219/1 skip/0 fail; tsc clean; contract build 4471 slots; Swift transport 62/0; `t3-code-macos --lib` 13; five checks green (cargo test 3521/0/34) | PR #308 body | — |
| gates (session 4) | `10d4773b5` (merged #290 `84a52dde0`) | bun 3227/1 skip/0 fail; contract build 4478 slots; caps within; `app.contract` 1478; `--bundle` build of the scope-id change passed; five checks not rerun (only `.contract` ids and records since `39bed7633`, plus #290's own checked change) | PR #308 body | — |

## Next action

Coordinator: the extra session is used (drives 3 and 4, the real-input session cut short by the screen lock). What is left
is in "Real-input batch steps" 2–7, listed in `STATUS.md` "Next real-input batch". Arrow keys in the scope menu: moved to
fix-keyboard-focus (batch bug 13). Sticky file headers: blocked on #131 (X32). After #307: the `Tip`s onto the hover layer.
Then flip PR #308 to ready.
