---
name: 20261005-pr-conversation-and-refresh
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
`examples/macos/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/macos/t3-code/tools/` with the same relative paths, decision U23).
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
| merged task PR | [20261005-fake-github-fixture](20261005-fake-github-fixture.md) | pending | Activity, thread, injection verbs served | pending |
| scheduling preference | [20261005-main-fix-adoption](20261005-main-fix-adoption.md) | pending | Merged first if popover/tooltip Contract is edited | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

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
| Conversation complete | Fake gh, profile `admin-reviewer`: PR 103 with 14 comments (4 bots), APPROVED + CHANGES_REQUESTED reviews, 3 threads (1 resolved, 1 outdated), 1 dismissed review | `bun scripts/agent.mjs macos --json tree "tap pr-row-103" state` then scroll Summary | 10 recent comments, "Show 4 older comments (4 hidden)", "4 bot comments", resolved/dismissed group, location line with "Outdated", reviewer outcomes | macOS, 1280×840 | tree JSON + shots |
| Visual parity | Same, reference desktop oracle on the same fixture | `target/t3-ui-parity/electron-oracle.mjs` pairs for Summary, Timeline, Checks open | Pairs at 1280×840 and 840×620, light and dark; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | pair table |
| Activity error | `failNext` on `pr view --json author,comments,…` | Open PR; press Retry | Error text and Retry (not "No comments yet."); Retry recovers; one extra `pullRequests.activity` in the trace | macOS | tree, trace |
| Loading | `delay` 2000 ms on activity | Open PR; `clock +500`; `clock settle` | Detail ghost, then conversation/timeline ghosts, then content with no layout jump | macOS, both sizes | shots, `layout` |
| Truncated notice | `failNext` on the `reviewThreads(` query | Open PR | Notice text equals the reference's; counts from the read | macOS | shot + oracle pair |
| Timeline | Same PR | Toggle order; read counts; stale verdict after a later commit | `aria-label`s switch; verdict rows standalone; stale text "before the latest commits" in the accessibility tree | macOS | `tree --ax` |
| Server-announced refresh | Panel open; edit fake state (new comment), probe sends `pullRequests.invalidate{}` | Wait `clock +2000` | New comment appears with no Refresh press; `calls.ndjson` shows one detail and one activity read; trace shows one `subscribeRefreshes`, closed when the panel closes | macOS | log, trace |
| Cached detail | Seeded snapshot; `delay` on detail | Relaunch (named storage), open the PR | Cached title and stats show first, then update | macOS | shots |
| Trace parity | Scenario `pr-conversation` on clone and reference | `target/t3-ui-parity/trace-diff.mjs pr-conversation` | Read multisets equal (detail, activity, checks, summary, stack, linkedThreads) except allowed, reasoned differences | macOS | diff report |
| Ported tests | `bun test examples/macos/t3-code` | Original names kept: "pull request activity refresh", "review thread comment pages", "ordering comments", "review verdicts", "pull request timeline", "cached pull request detail"; `useLiveRefresh` pure cases ("waits five minutes between automatic host reads", "does not read again for every window tabbed through", "reads once for a window hidden an hour, not once per interval it missed", …) | Pass; React-hook-only cases classified n/a-ui in the header | macOS | log |
| Keyboard and a11y | — | Tab through section headers, order toggle, "Show older…", Retry; `Space`/`Return` | Focus ring visible; `aria-expanded` flips; icon buttons named | macOS | `tree --ax` |
| Reduced motion | `prefer prefers-reduced-motion reduce` | Toggle a section | State changes at once, no rotation | macOS | state + shot |
| `(attended session)` | Real trackpad; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Hover timestamps and verdict faces; wheel-scroll, collapse a section mid-scroll | Tooltips show; heading stays under the pointer | macOS | recorded steps |
| Gates | `git add -A` | Clone checks (`bun test`, strict `tsc`, contract build, `cargo test -p macos-t3-code-apple --lib`, affected AppKit binaries); `bun scripts/caps.mjs`; the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/macos/t3-code/pages-pr-detail.ts`, `pages-pr-detail.contract`,
new `pages-pr-summary.*`, `pages-pr-timeline.*`, `pages-pr-logic.ts` (ported modules + tests),
`r6-pr-logic.ts` (`readableFailure`), `app.contract` (arguments only), `AGENT-HANDOFF.md` (matrix row).
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, fake gh, reference oracle; no credentials. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-fake-github-fixture` and `20261005-hot-file-split` merge; decide the idle-rule input and the sticky-heading result there.
