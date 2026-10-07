---
name: 20261005-pr-conversation-and-refresh
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-conversation-and-refresh
pr_url: https://github.com/ccheever/exact2/pull/247
verified_commit: null
---

# Pull request conversation, loading states and live refresh

## Outcome

The pull request detail panel (Pull Requests page and the thread's right panel) shows the
whole conversation as the reference does and keeps it current. Summary and Timeline include
review comments and line-comment threads; a failed activity read shows "Could not load pull
request activity" with Retry instead of "No comments yet."; ghosts show while detail or
activity load; the truncated-conversation notice, newest/oldest toggle, 10-comment window, bot
group, resolved/dismissed group, reviewer verdict rings and stale verdicts, timeline verdict
rows and counts are present; and the panel re-reads when the server announces a change, on
arrival, on an interval, and shows the last known detail while the new read is out.

## Scope and exclusions

Included: **C5** (honest activity read: `reviewThreads`, review-comment kind, `commentCount`,
`commentsTruncated`, `reviewThreadsTruncated`, error + Retry); **C7** (detail, timeline,
conversation, people ghosts; cached detail snapshot); **C13** (`pullRequests.subscribeRefreshes`,
live-refresh policy, activity re-read when `updatedAt` moves, checks cadence 45 s/60 s);
the Summary and Timeline model gaps listed in Implementation notes; completing the shared
`readableFailure` port (later tickets use it).
Excluded: reactions on remarks and every write (`20261005-pr-writing-and-metadata`); actions and
stacks (`20261005-pr-header-actions-and-stacks`); quick actions and hand-off buttons
(`20261005-pr-handoffs-and-quick-actions`); the Code tab and commit links into it
(`20261005-pr-code-tab`; commit rows stay plain here); thread links and previews
(`20261005-pr-links-previews-and-routing`). Already done and reused: list ghost (`pages-prs.contract:537`), whole-detail
error view `PrUnavailable` (`pages-pr-detail.contract:211`), not-found handling (`pages-pr-detail.ts:161-171`).

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (reference at T3 Code `1e2ecbd975`;
`W/` = `apps/web/src/components/pullRequest/`, `P` = `W/PullRequestDetailPanel.tsx`):
`P:760-835,2698-2740`; `W/PullRequestSummaryTab.tsx:136-330,455-1056`; `W/PullRequestTimelineTab.tsx:284-628`;
`W/PullRequestGhosts.tsx`; `W/PullRequestActivityUnavailableState.tsx`; `W/pullRequestDetail.logic.ts`;
`W/pullRequestPresentation.tsx`; `apps/web/src/hooks/useLiveRefresh.ts`; `packages/client-runtime/src/state/pullRequests.ts:222-300`;
server `apps/server/src/pullRequest/PullRequestService.ts:3160-3290`. Clone paths are repo-relative
`examples/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (resources read a
snapshot; loading/failed states; late old replies), components (child state dies with the
instance), layout-and-interaction (keep tab stacks mounted, hide with display; bound scrollers),
design (every state; keep content during refresh), accessibility, testing-and-debugging.
Unknown in the library: app-local Swift transport streams, hooks, data-module timers; the basis is
the clone's own evidence on the pinned main (`20261005-clone-on-exact2-main`).
Consumer framework revision and toolchain: the pin from that ticket; pinned Bun and Hermes.
Observed today (clone): `.catch(() => null)` on activity (`pages-pr-detail.ts:90`) shows "No
comments yet." (`pages-pr-detail.contract:455`); review comments are dropped (`:129`); the panel
removes the inactive tab (`when tab == …`, contract:174-177) so its state resets; the timeline empty text
is "Nothing has happened…" (`contract:489`), the reference says "No activity yet."; no `pullRequests.subscribeRefreshes`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle shots, trace diff) | pending |
| merged task PR | [20261007-real-github-lane](closed/20261007-real-github-lane.md) | pending | Sandbox seeded (`second-review` conversation); probe rows for `activity`, `threadComments` and `subscribeRefreshes` decoded; injection by unit tests | pending |
| scheduling preference | [20261005-main-fix-adoption](closed/20261005-main-fix-adoption.md) | pending | Merged first if popover/tooltip Contract is edited | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and the draft records in
`../issues/` (not reproduced on the pinned `main`, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X19](../issues/20261005-x19-data-source-timers.md) | Timers in data sources (live-refresh interval, activity debounce) | macOS | nonblocking (workaround: time passed as an argument through the existing `wallTime` resource, as `pages-prs.ts`/`r6-pr-actions.ts` do) | Keep; no new timer API |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Sending on a stream | Swift `T3Transport` already owns streams | nonblocking (workaround: `restAccess(native).call({op:'subscribe',…})`, `shell-vcs.ts:97`) | Reuse |
| [X30](../issues/20261005-x30-ts-announce-readback-picker.md) | A stream event waking a TS read | Clone wakes reads by bumping `client.revision` (`client.ts:143`) | nonblocking (same workaround) | Reuse |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resources in child components; `app.contract` is 1,327 lines | cap 1,500 | nonblocking until the cap | No new root resource: extend `prDetail` (`app.contract:100`); new views in new files |
| [X32](../issues/20261005-x32-sticky-positioning-in-lists.md) | Sticky Summary section headings inside a scroll (`sticky top-0`, `PullRequestSummaryTab.tsx:331`) | Not established in the library; check `bun exact.mjs contract vocab --json position` at `prepare` | unknown | If unsupported, that one row (headings stay at the top while a section scrolls) is held for a user decision; no matching workaround |

## Implementation notes

- **Model in TS, render in Contract.** Extend `pullRequestDetail`/`presentDetail` (`pages-pr-detail.ts:74,112`) and `PrDetailView` (`pages-pr-detail.contract:74`). Port, with names and tests,
  from `W/pullRequestDetail.logic.ts`: `shouldRefreshPullRequestActivity` (163), `mergePullRequestThreadComments` (170),
  `orderPullRequestComments` (432), `pullRequestReviewOutcome` (447), `newestPullRequestCommitAt` (477),
  `isPullRequestVerdictStale` (502), `latestPullRequestReviewOutcomes` (529), `groupPullRequestTimelineConversations` (590),
  `visibleBody` (618), `buildPullRequestTimeline` (631), `readPullRequestDetailSnapshot`/`writePullRequestDetailSnapshot`/`resolveDisplayedPullRequestDetail` (1331-1395; storage moves from `localStorage` to the app's versioned `t3-code.json`, noted in the header);
  from `W/pullRequestPresentation.tsx`: outcome and check labels (`pullRequestReviewOutcomeLabel`, `…StaleLabel`, `pullRequestCheckStatusLabel`);
  from `hooks/useLiveRefresh.ts`: `shouldLiveRefresh`, `shouldRefreshOnArrival`, `shouldRefreshOnInterval` and the three constants (10 s, 5 min, 6 min).
  Complete `readableFailure(failure: unknown, hint)` in `r6-pr-logic.ts:156` (operation-prefix strip, tool-noise patterns, 320-char cap; reference `pullRequestDetail.logic.ts:1196-1240`).
- **Summary.** Keep Description/Checks/Comments sections; add the comment window (`COMMENT_PAGE = 10`; "Show N older comment(s) (M hidden)", "Show only 10 recent comments"), bot group ("N bot comment(s)", "Show N older bot comment(s)"),
  resolved/dismissed group with collapsed cards (labels "Resolved"/"Review dismissed"), location line `path:line` + "Outdated", review-state badge, the truncated notice ("This conversation is longer than this page reads in one go…"), the order toggle (`aria-label` "Show oldest comments first"/"Show newest comments first"), check rows that open details in the system browser, and "Check details are out of date." + Refresh.
  Reviewers show verdict outcome (ring colour is never the only cue: add the `sr-only` text and the tooltip `"<name> — <outcome>"`).
- **Timeline.** Events from `buildPullRequestTimeline`; consecutive plain comments fold into "N comments" groups; verdict rows stand alone; counts (comments, commits, approvals) and the order toggle sit in the tab bar with "—"/"…" while error/loading. Commit rows are plain until `20261005-pr-code-tab`.
- **Loading and error.** Ghosts replace the centred "Loading pull request…" (`contract:155-157`); activity failure uses compact (Summary) and full (Timeline) "Could not load pull request activity" + Retry (`aria-label` "Retry"). Keep both tabs mounted and toggle `display` (library: tab stacks stay mounted) so order, window and open groups survive tab switches; key child state by PR so another PR starts fresh.
- **Refresh.** One `subscribe` (`pullRequests.subscribeRefreshes`, payload `{}`; pattern `shell-vcs.ts:85-110`) while any PR surface is open; an event bumps a PR epoch that `prDetail`/`prList` take as arguments. Interval/arrival use the ported pure functions with `now`. Explicit Refresh keeps `pullRequests.invalidate{reference}` (`pages-pr-detail.ts:88`). Open decision: the "last interaction" fact for the 6-minute idle rule (reuse `20261005-client-activity-reporting` if it has merged, else drop the idle rule and record it).
- Motion: the reference rotates section chevrons (`transition-transform`); with reduced motion the state still changes at once.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Conversation complete | Sandbox `second-review` seen by the primary account (admin reviewer): comments from both accounts, a dismissed and a standing request-changes review, an approval on `primary-review`, threads (1 resolved, 1 outdated, 1 of 12 comments); the task seeds the extra comments the counts need (the seed is idempotent); bot comments by unit test (no bot account) | `bun scripts/agent.mjs macos --json tree "tap pr-row-103" state` then scroll Summary | 10 recent comments, "Show 4 older comments (4 hidden)", "4 bot comments", resolved/dismissed group, location line with "Outdated", reviewer outcomes | macOS, 1280×840 | tree JSON + shots |
| Visual parity | Same, reference desktop oracle on the same fixture | `target/t3-ui-parity/electron-oracle.mjs` pairs for Summary, Timeline, Checks open | Pairs at 1280×840 and 840×620, light and dark; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | pair table |
| Activity error | Unit test: an injected failure of `pullRequests.activity` | Open PR; press Retry | Error text and Retry (not "No comments yet."); Retry recovers; one extra `pullRequests.activity` in the trace | macOS | tree, trace |
| Loading | Unit test: a delayed `pullRequests.activity` reply (2000 ms) | Open PR; `clock +500`; `clock settle` | Detail ghost, then conversation/timeline ghosts, then content with no layout jump | macOS, both sizes | shots, `layout` |
| Truncated notice | Unit test: an activity with `reviewThreadsTruncated` | Open PR | Notice text equals the reference's; counts from the read | macOS | shot + oracle pair |
| Timeline | Same PR | Toggle order; read counts; stale verdict after a later commit | `aria-label`s switch; verdict rows standalone; stale text "before the latest commits" in the accessibility tree | macOS | `tree --ax` |
| Server-announced refresh | Panel open; post a comment on GitHub with the second account's lane gh, the probe sends `pullRequests.invalidate{}` | Wait `clock +2000` | New comment appears with no Refresh press; `logs/gh-calls.tsv` shows one detail and one activity read; trace shows one `subscribeRefreshes`, closed when the panel closes | macOS | log, trace |
| Cached detail | Seeded snapshot; unit test with a delayed detail reply | Relaunch (named storage), open the PR | Cached title and stats show first, then update | macOS | shots |
| Trace parity | Scenario `pr-conversation` on clone and reference | `target/t3-ui-parity/trace-diff.mjs pr-conversation` | Read multisets equal (detail, activity, checks, summary, stack, linkedThreads) except allowed, reasoned differences | macOS | diff report |
| Ported tests | `bun test examples/t3-code` | Original names kept: "pull request activity refresh", "review thread comment pages", "ordering comments", "review verdicts", "pull request timeline", "cached pull request detail"; `useLiveRefresh` pure cases ("waits five minutes between automatic host reads", "does not read again for every window tabbed through", "reads once for a window hidden an hour, not once per interval it missed", …) | Pass; React-hook-only cases classified n/a-ui in the header | macOS | log |
| Keyboard and a11y | — | Tab through section headers, order toggle, "Show older…", Retry; `Space`/`Return` | Focus ring visible; `aria-expanded` flips; icon buttons named | macOS | `tree --ax` |
| Reduced motion | `prefer prefers-reduced-motion reduce` | Toggle a section | State changes at once, no rotation | macOS | state + shot |
| `(attended session)` | Real trackpad; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Hover timestamps and verdict faces; wheel-scroll, collapse a section mid-scroll | Tooltips show; heading stays under the pointer | macOS | recorded steps |
| Gates | `git add -A` | Clone checks (`bun test`, strict `tsc`, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries); `bun scripts/caps.mjs`; the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.ts`, `pages-pr-detail.contract`,
new `pages-pr-summary.*`, `pages-pr-timeline.*`, `pages-pr-logic.ts` (ported modules + tests),
`r6-pr-logic.ts` (`readableFailure`), `app.contract` (arguments only), `AGENT-HANDOFF.md` (matrix row).
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, the real-GitHub lane (sandbox, shared lane config dirs, two accounts; `tools/github-lane`), reference oracle; no credentials beyond the lane logins. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Results

Built on `feat(example)/t3-code` at `d82fb6a47` (records #244 merged in). Prerequisites: real-github-lane (#233) and
hot-file-split are merged; desktop-oracle-and-trace is not built (user decision 2026-10-06), so its rows are "not run".
X32 (sticky headings) checked at prepare: `position: sticky` is in the vocabulary (`contract vocab`), and each Summary
section is its own box so its heading sticks only while the section is in view.

**What was built.**
- Reads (`pages-pr-detail.ts`): the detail and the activity are separate reads. The panel shows the detail ghost
  (seeded by the list row, or the detail kept in `t3-code.json`), then the conversation and timeline ghosts, then the
  content; a failed activity read is "Could not load pull request activity" with Retry (compact under Comments, full in
  the Timeline) instead of "No comments yet."; a failed refresh keeps the last conversation. An answer that has a new state
  to show returns it at once and wakes the resource (`r10Wake` topic `t3.pr`, `R10Connect.swift`), so the runner lets that
  reply land and asks again for the read that follows (LLP 1016.002 D4); every read is awaited by the answer that sent it.
- Refresh (`pages-pr-refresh.ts`): one `pullRequests.subscribeRefreshes` stream (payload `{}`) while any pull request
  surface reads (the detail panel, the Pull Requests list); each announcement reads the detail, the activity and the list
  again while the last state stays on screen. The server sends its current epoch on subscribe only when it is above 0, so a
  first value cannot be told from a change: a read in flight absorbs it (readers note the epoch when their read completes).
  `useLiveRefresh`: an arrival (the panel reopened on a pull request, the window shown or focused again) and the 5-minute
  interval read the detail, under the 10 s minimum and the 6-minute idle rule; the last interaction is the activity
  reporter's (`activityLastInteraction`, decision taken below). The activity reads again when `updatedAt` moves
  (`shouldRefreshPullRequestActivity`). Explicit Refresh keeps `pullRequests.invalidate{reference}` first. The checks
  cadence (45 s while runs are pending or none are reported, else 60 s) is the thread row's (`r6-pr-actions.ts readDetail`,
  test "the checks alone refresh every 45 s while runs are pending"); the panel's checks come with its detail, as the
  reference's do.
- Summary (`pages-pr-summary.ts`, `pages-pr-summary.contract`): reviewers with the verdict each last gave (a ring; the
  words in the accessible name; the tooltip "<name> — <outcome>"; stale verdicts "… earlier changes"), labels, sticky
  Description/Checks/Comments sections with turning chevrons (no rotation under reduced motion), the 10-comment window
  ("Show N older comment(s) (M hidden)", "Show only 10 recent comments"), the bot group ("N bot comment(s)", its own
  window), the resolved-or-dismissed group with collapsed cards ("Resolved", "Review dismissed", a one-line preview), the
  location line `path:line` with "Outdated", the verdict badge or review state, the truncated notice, the order toggle
  ("Show oldest comments first"/"Show newest comments first"), check rows that open their details in the system browser,
  and "Check details are out of date." with Refresh when a newer list rollup disagrees.
- Timeline (`pages-pr-timeline.ts`, `pages-pr-timeline.contract`): `buildPullRequestTimeline` events newest first,
  consecutive plain remarks folded into "N comments" sections (faces dimmed while closed), verdict rows standing alone
  (a stale verdict keeps its word without its colour; "Approved, before the latest commits" for a screen reader),
  lifecycle rows, plain commit rows (until 20261005-pr-code-tab), "No activity yet."; the tab bar's comment, commit and
  approval counts ("—" when the read failed, "…" while it runs; no approval count from a truncated conversation) and the
  order toggle ("Show oldest activity first"/"Show newest activity first").
- The reviewer's tooltip paints over the Labels row: the Summary's top column and its Reviewers row are raised
  (`position="relative" z-index=1`), since z-index orders siblings (found in the agent stand-in shot, image 17).
- Section collapse keeps the pressed heading in place (sectionCollapseAnchorScrollTop): a heading pinned over a section
  scrolled past the top moves the body's scroll to the section's natural top (`frame()` reads in the toggle action, the
  body's `scrollTop` binding; added after the retry while preparing the real-input session).
- Both tabs stay mounted (hidden with `display`), keyed by the pull request, so the order, the window and the open
  groups survive a visit to the other tab and start fresh for another pull request.
- Ghosts (`pages-pr-ghost.contract`): PullRequestDetailGhost (seeded or bars, the tab bar inert, the pulse off under
  reduced motion), PullRequestConversationGhost, PullRequestTimelineGhost. PullRequestPeopleGhost belongs to the reviewer
  picker, which the clone does not have yet (20261005-pr-writing-and-metadata).
- Cached detail: `readPullRequestDetailSnapshot`/`writePullRequestDetailSnapshot`/`resolveDisplayedPullRequestDetail`
  over `prDetailSnapshots` in `t3-code.json` (the newest 24), adopted at load (`client.ts`).
- Ported (`pages-pr-logic.ts`): the pullRequestDetail.logic and useLiveRefresh functions named in the notes;
  `readableFailure(failure: unknown, hint)` completed in `r6-pr-logic.ts`.
- Plan size: the panel body is drawn once per surface instead of once per scheme (`light-dark()` resolves on the host,
  LLP 1034 D3; the compiler refuses scheme-chosen colours): 70,474 nodes, 13.4 MB plan (base 59,420 nodes, 11.5 MB).
- Lane: `lane.mjs start --port` accepts 16500-16799 (task lanes' own hundreds); the seed gains `stale-approval` (#144:
  approved by the second account, then a commit dated a minute after the approval) and `--only <keys>` (seeds just those
  scenarios and merges their numbers into `sandbox.json`). Twice `--only stale-approval`: the second run created nothing.

**Acceptance.**

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Conversation complete | pass (live, #115; bots by unit test) | [03](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/03-summary-top.png), [04](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/04-summary-conversation.png), [record](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/live-drive-record.txt); unit "splits the conversation…", "a line remark names its place…" | The seeded conversation has 15 active remarks, so the live label is "Show 5 older comments (5 hidden)" (the row's "4" assumed 14). "Outdated" sits on a remark among the five older ones: opened live (image summary-older) but read back only by the unit test |
| Visual parity | not run | — | user decision 2026-10-06: the desktop oracle is not built |
| Activity error | pass (unit) | "a failed activity read says … with Retry, and Retry reads it once more" (one extra `pullRequests.activity`, no extra detail read) | real GitHub cannot inject a failure |
| Loading | pass (unit + live) | unit "loading: the detail ghost, then the conversation and timeline ghosts, then the content" (2000 ms activity delay); live: ghost on the press (tree), [01](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/01-loading-detail-ghost.png), [02](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/02-loading-conversation-ghost.png); title, tab bar, Reviewers, Labels and Description sit at the same points in the ghost and the content (y 150, 330, 399, 449, 506 at 1280×840) | `layout` numbers not taken (screenshots only) |
| Truncated notice | pass (live + unit) | live #115 (`commentsTruncated`): "This conversation is longer than this page reads in one go. The most recent 17 are here; open it on the host to read the rest." | — |
| Timeline | pass (live) | [06](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/06-timeline.png), [07](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/07-timeline-oldest-open.png), [08](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/08-stale-verdict.png), [09](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/09-approvals-count.png), [ax](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/ax-excerpts.txt): "Approved, before the latest commits" | — |
| Server-announced refresh | pass (live) | [10](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/10-live-refresh.png); the second account's comment on #116, then `pullRequests.invalidate {}`: server trace +1 detail, +1 activity, +1 list; `gh` calls: detail GraphQL, `pr view 116` and the review-thread GraphQL (activity), the list GraphQL; one `subscribeRefreshes` per session (79.4 s, ended with the session; the list stayed open after the panel closed). Base: nothing read, no comment | — |
| Cached detail | pass (unit; agent relaunch with the kept file); normal relaunch deferred to the real-input batch — screen locked (user away) | unit "a relaunch shows the kept detail first…"; agent relaunch given the previous session's `t3-code.json` (X50: agent mode keeps module data per process), the lane gh slowed 8 s for detail reads: 57 ms after the press the panel read back #115's kept title and header with the conversation ghost, and the first `pullRequests.detail` reached the server 43 ms later ([15](https://raw.githubusercontent.com/ccheever/exact2/b9cf470c96bd6bb1769cacc19dab3d0010b6ed97/pr-conversation-and-refresh/15-agent-relaunch-cached.png), [record](https://raw.githubusercontent.com/ccheever/exact2/f09126670abd8e9327a38d384f5c7c58a7dbe3b3/pr-conversation-and-refresh/agent-standins-record.txt)) | the normal-launch relaunch: real-input batch (steps below) |
| Trace parity | not run | server trace RPC counts per session in the record | user decision 2026-10-06: the trace tools are not built |
| Ported tests | pass | `pages-pr-logic.test.ts` (52): "pull request activity refresh", "review thread comment pages", "ordering comments", "review verdicts", "pull request timeline", "cached pull request detail", "what to say when an action fails", shouldLiveRefresh/shouldRefreshOnArrival/shouldRefreshOnInterval, "waits five minutes between automatic host reads"; React-hook-only "live refresh cadence" timers and "keeps an idle view paused after %s until input" are n/a-ui (header) | — |
| Keyboard and a11y | partial; real keys deferred to the real-input batch — screen locked (user away) | agent keys: Tab from the Comments heading went to the order toggle, then each comment's time link in turn ([record](https://raw.githubusercontent.com/ccheever/exact2/f09126670abd8e9327a38d384f5c7c58a7dbe3b3/pr-conversation-and-refresh/agent-standins-record.txt)); `aria-expanded` on section, group and card toggles (driven); Retry and the order toggles named; reviewer faces named "<login>, <outcome>" | real Tab, Space/Return and the visible focus ring: real-input batch (steps below) |
| Reduced motion | pass (agent `prefer`); live deferred to the real-input batch — screen locked (user away) | [16](https://raw.githubusercontent.com/ccheever/exact2/b9cf470c96bd6bb1769cacc19dab3d0010b6ed97/pr-conversation-and-refresh/16-agent-reduced-motion.png): with no preference the Checks chevron is mid-turn at +50 ms; with `reduce` it has turned at +0 ms. The app reads only the system setting (`DisplayPreferences.reducedMotion` = `NSWorkspace.accessibilityDisplayShouldReduceMotion`; T3 Code has no in-app override; Exact's `prefer` is agent-only) | a live check needs System Settings › Reduce motion, which the agent must not change: real-input batch, with the user's permission (steps below) |
| (attended session) | partial (agent pointer); real pointer deferred to the real-input batch — screen locked (user away) | [13](https://raw.githubusercontent.com/ccheever/exact2/b9cf470c96bd6bb1769cacc19dab3d0010b6ed97/pr-conversation-and-refresh/13-agent-tooltips.png): an agent hover shows "daehyeonmun2021 — Changes requested" on the reviewer face and "Approved earlier changes" on #144's stale verdict, but the Labels row painted over the reviewer's tooltip (z-index orders siblings); fixed by raising the Summary's top column and Reviewers row, [17](https://raw.githubusercontent.com/ccheever/exact2/f09126670abd8e9327a38d384f5c7c58a7dbe3b3/pr-conversation-and-refresh/17-reviewer-tooltip-paint-order.png) before/after; [14](https://raw.githubusercontent.com/ccheever/exact2/b9cf470c96bd6bb1769cacc19dab3d0010b6ed97/pr-conversation-and-refresh/14-agent-collapse-mid-scroll.png): an agent wheel pins the Comments heading at the top of the scroll box (y 52); pressed, it collapses and, the page now shorter than the window, the box returns to the top (the heading at y 433), as the reference does with nothing below | real trackpad hover and wheel: real-input batch (steps below) |
| Gates | see the PR | numbers in the PR body | — |

**Live drive** (agent mode, 1280×840 then 840×620, the branch's development build paired with the primary lane server on
127.0.0.1:16720; [record](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/live-drive-record.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/drive.mjs.txt)). Attempt 1 found two bugs, fixed before
the retry: the refresh stream took its first value as the starting epoch, but this server sends none when its epoch is 0,
so the first real change was swallowed; and all three Summary headings were sticky in one box, stacking at the top (a tap
on Checks hit Comments). The retry passed every scripted step; its comment on #116 was deleted again (as was attempt 1's).
The same steps on the base build (`da4f4512f`) give the before images.

**Decisions.** The 6-minute idle rule reads the activity reporter's last interaction (`activityLastInteraction`,
`T3ActivityReporter.lastInteractionAt`); client-activity-reporting has merged, so the rule is kept, and it stands aside where
the window's time is not an instant (agent mode without an epoch). Provisional, user decision pending: none.

## Real-input batch steps

Deferred rows (screen locked while the user is away, 2026-10-08): the cached details after a normal relaunch, real Tab and
the focus ring, reduced motion live, real-pointer tooltips, collapse mid-scroll. One session for all of them:

1. Worktree `t3-code-pr-conversation-and-refresh` at the PR head. `export PATH="$HOME/.bun-1.4.2/bin:$PATH"
   EXACT_APP_DIR="$PWD/examples/t3-code"`; `bun examples/t3-code/stage-runtime.mjs` if `server-runtime/*.tar.gz` is missing;
   `bun host/apple/build.mjs t3-code-macos --bundle`.
2. Lane server: `cd examples/t3-code/tools/github-lane`, `export T3_GITHUB_LANE_SHARED=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-ui-parity/github-lane`,
   `bun lane.mjs start primary --port 16720`, `bun lane.mjs project primary`, `bun lane.mjs pair primary` (writes the
   single-use URL to `target/github-lane/servers/primary/pairing-url`, 0600; never print it).
3. Lane copy: copy `target/clients/d3bd09d90e4cd670f6dbeb6f/com.exact.t3code.macos/macos/T3 Code (Exact).app` to
   `target/github-lane/realinput/T3 Code (Lane PRC).app`; with PlistBuddy set `CFBundleIdentifier` to
   `com.exact.t3code.laneprc` and `CFBundleName`/`CFBundleDisplayName` to `T3 Code (Lane PRC)`; `codesign --force --deep
   --sign <the Apple Development identity build.mjs signs with>`. Take the shared real-input lock (owner
   "pr-conversation-and-refresh: real-input batch"). Launch by path:
   `CFFIXED_USER_HOME=$PWD/target/github-lane/realinput/home "<copy>/Contents/MacOS/T3 Code (Exact)" &`; record the pid;
   the window id from `orca computer list-windows --app pid:<pid> --json`.
4. Pair: the welcome wizard's Pairing link field by `set-value --value-stdin` from the `pairing-url` file, click Pair, then
   Continue through the wizard. Open Pull Requests (sidebar), put `#115` in the search with `set-value`, click the row.
5. **Collapse mid-scroll** (#115 Summary): `orca computer scroll` down over the panel until the Comments heading is pinned at
   the panel's top with comments under it; click that heading. Read back: before, the heading at the scroll box's top;
   after, collapsed. With #115 the collapsed page is shorter than the window, so the box returns to the top (the
   reference does the same); the anchor case needs a section with content after it at least a window tall.
6. **Real-pointer tooltips**: click the reviewer face beside Reviewers (not a link): after 1 s, "daehyeonmun2021 — Changes
   requested". Click a location line (`src/validate.js:11`, not a link): the path's tooltip. Open #144 (search `#144`),
   Timeline, click the grey "approved" word: "Approved earlier changes". Do not click a comment's time: it is a link to
   GitHub and opens the browser.
7. **Tab and the focus ring**: click the Summary tab, then `press-key Tab` repeatedly with a window screenshot after each:
   a visible ring moves through the Timeline tab, the Description, Checks and Comments headings, "Newest first", "Show 5
   older comments", the comment time links. Space on a heading flips it (collapses the section); Space on "Newest first"
   flips the label to "Oldest first". Do not press Return on a time link.
8. **Reduced motion** (only with the user's permission, since it is a system setting the app reads with no in-app
   override): turn System Settings › Accessibility › Display › Reduce motion on, click the Checks heading (the chevron
   turns at once), turn it off again. Without permission the row stays verified by agent `prefer` (image 16).
9. **Cached details after a normal relaunch**: check that `target/github-lane/realinput/home/Library/Application Support/exact/com.exact.t3code.laneprc/data/t3-code.json`
   has `prDetailSnapshots` with the `#115` key. Quit the copy (`osascript -e 'tell application id "com.exact.t3code.laneprc"
   to quit'`), wait 20 s (the server's detail cache), slow the lane gh by adding
   `case " $* " in *" pr view "*|*" -F number="*) sleep 8;; esac` before the token check in
   `target/github-lane/bin/primary/gh`, and launch the copy the same way (it reconnects from Keychain). Pull Requests,
   `#115`, click the row and screenshot within 1 s: the kept title and header with the conversation ghost. Read back the
   first `ws.rpc.pullRequests.detail` `startTimeUnixNano` after the click in
   `target/github-lane/servers/primary/t3home/userdata/logs/server.trace.ndjson`, and `target/github-lane/logs/gh-calls.tsv`:
   both later than the screenshot. After 10 s the live detail. Restore the wrapper with `bun lane.mjs setup`.
10. Cleanup: quit the copy; `bun lane.mjs stop primary`; delete the copy's Keychain item
    (`security delete-generic-password -s com.exact.t3code.macos.access-token -a "$(printf 'http://127.0.0.1:16720\n%s' "$(cat target/github-lane/servers/primary/t3home/userdata/environment-id)")"`);
    remove `target/github-lane/realinput`; release the lock.

## Progress

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented on `feat(example)/t3-code-pr-conversation-and-refresh`; unit tests, one live drive and one retry
on the real-GitHub lane; draft PR #247. Later: the coordinator approved one normal-launch real-input session, but the
screen was locked (user away); the rows ran in agent mode and the real-input versions wait for the batch (steps above).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (live drive) | WIP `0d583f1b3` | loading, Summary, Timeline, stale verdict pass; the server-announced refresh read nothing (stream baseline bug); Checks unreachable (stacked sticky headings) | [record](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/live-drive-record.txt) (attempt 1) | fixed in attempt 2 |
| 2 (retry + relaunch) | `e991c4b84` | every scripted step passes; the refresh read once and showed the comment; the relaunch showed the list-seeded ghost, not the kept detail (agent-mode data per process) | images 01–12, [record](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/live-drive-record.txt), [replay](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/relaunch-replay.txt), [tests](https://raw.githubusercontent.com/ccheever/exact2/79ccaff3bb25bfb4b936b8d1a6f0faa7e9941b5b/pr-conversation-and-refresh/tests-before-after.txt) | X50 for a live relaunch; real-input rows |

| 3 (agent stand-ins, screen locked) | `e991c4b84` + the collapse anchor; the tooltip fix | agent hover tooltips, agent wheel collapse, agent Tab order, `prefer` reduced motion, agent relaunch given the kept file: pass. The hover shot showed the Labels row over the reviewer's tooltip: fixed (Reviewers row raised) and re-driven, image 17. Attempt 1 of this drive stopped at an unsupported key name (`Return`, fixed to `Enter`); a stepped clock replaced the film (empty frames) | images 13–17, [record](https://raw.githubusercontent.com/ccheever/exact2/f09126670abd8e9327a38d384f5c7c58a7dbe3b3/pr-conversation-and-refresh/agent-standins-record.txt) | real-input batch (steps above) |

## Next action

Review. Owed: the real-input batch (steps above: normal relaunch, real Tab and focus ring, reduced motion with the user's
permission, real-pointer tooltips, collapse mid-scroll); the oracle and trace rows wait on the user decision.
