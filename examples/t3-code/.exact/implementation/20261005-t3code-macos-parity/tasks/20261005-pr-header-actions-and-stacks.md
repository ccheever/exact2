---
name: 20261005-pr-header-actions-and-stacks
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

# Pull request header actions, dialogs and the host stack menu

## Outcome

A person can run every host action the reference offers from the pull request panel header, with
the reference's controls, confirmations, wording and failure handling: the primary control
(Merge with its method, Ready for review, "Auto-merge (method)", Resolve conflicts, Merged/Closed
badge), the More menu (Merge now, Enable/Disable auto-merge, merge-method radios, Convert to
draft, Close, Reopen, Revert changes), the five confirmation dialogs, the out-of-date branch
popover (Update branch / Update with rebase), "Approve workflows to run", and the host stack menu
(n/m, layers, Merge stack, Rebase stack).

## Scope and exclusions

Included: **C14**, **C8** (the panel's stack menu and the `pullRequests.stack` read), the Close
confirmation, unified action failure toasts and hints for all ten actions, and the list-override
phases ("sent"/"done"/"failed") that the panel reports to the Pull Requests page.
Excluded: comments, reviews, edits, reactions, reviewers, labels (`20261005-pr-writing-and-metadata`);
the Check out menu, Ask/Explain/Fix hand-offs, the folding header, Shift quick actions, and the row
context menu, checks and stack popovers (`20261005-pr-handoffs-and-quick-actions`); the Code tab
(`20261005-pr-code-tab`); the "Act on" environment radio (`20261005-pr-links-previews-and-routing`);
other hosts' action sets (capability flags drive the menu; only GitHub is fixture-run).
Reuse (done): Merge/Ready with its dialog on the thread card (`r6-pr-actions.ts` `performAction`, `askMerge`, `confirmMerge`;
`r6-pr.contract:149-154`), `actionPayload`/`readableFailure`/`ACTION_*` (`r6-pr-logic.ts`), the More menu shell
(`pages-pr-detail.contract` `PrdMore`), the Default merge method setting (`source-control-view.ts`), and the existing
`startHandoff('conflicts')` (`r6-pr-actions.ts`) that "Resolve conflicts" calls.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` =
`apps/web/src/components/pullRequest/`, `P` = `W/PullRequestDetailPanel.tsx`): `P:181-243,336-409,920-1000,1380-1480,1744-2180,2231-2248,2419-2435,2782-2848`;
`W/usePullRequestActions.ts:40-170`; `W/PullRequestStackMenu.tsx`, `PullRequestStackLayers.tsx`, `PullRequestStackHeader.tsx`, `pullRequestStackSnapshot.ts`,
`apps/web/src/state/usePullRequestStack.ts`; `apps/web/src/routes/_chat.pull-requests.tsx:957-968,2175-2215` (override phases); server `PullRequestService.ts:1906-2025`,
`githubStackActions.ts:195-418`, `GitHubPullRequestProvider.ts:53-100`. Clone paths: `examples/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (one mutation per action path; pending and failed states), layout-and-interaction (menu and popover structure; keyboard focus), design (every state, disabled with a reason), accessibility (named icon buttons, `aria-expanded`), motion (menu/dialog open, chevrons; reduced motion), testing-and-debugging. Unknown in the library: dialog focus handling and Escape routing; the clone's own runtime evidence on the pinned main is the basis.
Observed today: Close runs at once (`pages-pr-detail.contract` `pr-more-close`), 6 of 10 actions have labels (`pages-pr-detail.ts` `ACTION_DONE`), the failure toast shows the raw message (`prCommand` catch), `canMerge`/`canUpdateBranch` are computed but unused (`PrDetailView`), `app.contract` `prAct(op,value)` already carries any op (encode extra arguments in `value`; keep `app.contract` from growing).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged (refresh events, `readableFailure`, Summary model) | pending |
| merged task PR | [20261005-fake-github-fixture](20261005-fake-github-fixture.md) | pending | Action, stack and permission-profile verbs served | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| scheduling preference | [20261005-main-fix-adoption](20261005-main-fix-adoption.md) | pending | Merged first (popover/tooltip Contract) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and the draft records in `../issues/` (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X17](../issues/20261005-x17-popover-position-try.md) | Menu and popover flips near window edges (More menu, freshness popover, stack menu) | Fixed placement | nonblocking (workaround: fixed placement; the near-edge difference is declared) | Declare in `EXACT2-GAPS.md` |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` near its cap | 1,327/1,500 lines | nonblocking | Child components in new files |
| [X21](../issues/20261005-x21-two-way-websocket.md) | RPC send | Swift transport | nonblocking | Reuse `client.rpc` |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | Escape/Return inside a dialog while a button has focus | Key handlers on dialogs | nonblocking (workaround: `key=` handler with `preventDefault()` on the dialog column) | Prove with the keyboard row |

## Implementation notes

- **Port** (names, tests; header records changes) from `W/pullRequestDetail.logic.ts`: `resolvePullRequestMergeMethod`, `allowsSinglePullRequestMerge`, `resolvePullRequestPrimaryControl`, `pullRequestActionMenuHasGroup`, `isStackedPullRequestBase`, `allowedPullRequestMergeMethods`, `resolveSelectedMergeMethod`, `resolveBaseFreshness`, `pullRequestActionNeedsHostRefresh`, `PULL_REQUEST_MERGE_METHOD_LABELS`; `pullRequestStackSnapshot.ts` (`savedPullRequestStack`, `pullRequestStackView`); from `pullRequestList.logic.ts`: `pullRequestOverrideAfterAction`, `applyPullRequestOverrides` and override settlement; the action label and hint tables (`P:181-243`, `usePullRequestActions.ts:79-124`).
- **Actions.** One runner (`usePullRequestActionRunner` semantics: one pending action; toasts "Pull request merged", "Marked ready for review", "Auto-merge turned on — merges as soon as this is ready, sooner if it already is", …; failure title plus `readableFailure(failure, hint)`; `update-branch` with `updateMethod: "rebase"` uses the rebase hint). Merge and auto-merge send `mergeMethod`; after `update-branch` or `approve-workflows` re-read through `pullRequests.invalidate{reference}`. Close, Revert, Enable auto-merge, Merge and Approve workflows confirm first (titles "Close pull request?", "Revert these changes?", "Enable auto-merge?", "Merge pull request?", "Approve workflows to run?"; buttons Cancel and "Close"/"Create revert PR"/"Enable auto-merge"/<method label>/"Approve and run").
- **Primary control** per `resolvePullRequestPrimaryControl`; tooltips and `aria-label`s follow the reference ("Merging...", "Enabling...", "Resolve conflicts", "Ready for review"); the merge-method radios appear only when more than one method is allowed and the PR is mergeable; the armed badge shows beside Resolve conflicts.
- **Stack.** Read `pullRequests.stack` once the detail says `capabilities.stacks`; "Retry stack lookup" on failure; the header menu `aria-label` "Stack N, layer i of n"; Merge/Rebase send `stackNumber` and `expectedStackHeads`; notices "May be stale"/"Refreshing…"; single-PR Merge hidden while a stack exists or is pending; merge-async polling is server-side.
- Give icon-only controls `aria-label` ("More pull request actions"/"Refreshing pull request", "Collapse pull request panel"). Keep new views in new Contract files.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Primary control per state | Fake gh: 101 conflicting, 102 failing, 103 draft, a clean open PR, a merged PR, a closed PR | `bun scripts/agent.mjs macos --json "tap pr-row-<n>" tree` per PR | Resolve conflicts / Auto-merge (m) / Ready for review / Merge label / Merged badge / Closed badge; menu items as the reference's matrix | macOS 1280×840 | tree JSON, shots |
| Each action's effect | Profile `admin-reviewer` | Merge (3 methods), auto-merge on/off, ready, draft, close, reopen, update-branch (both), revert, approve-workflows | `calls.ndjson` argv equals the fixture ticket's verb table; state changes; success toast text; Cancel in a dialog sends nothing | macOS | log + state diff |
| Failures | `failNext` per action | Repeat | Title "Could not …", description = host sentence else the hint; buttons re-enabled; list row's optimistic note rolled back | macOS | shots |
| Permissions | Profiles `reader`, `contributor-author`, `triage`, `writer` | Open the same PRs | reader: no action items; contributor-author: close/reopen/ready/draft but no merge; update-branch only when `viewerCanUpdateBranch` | macOS | tree JSON |
| Out-of-date branch | `behindBy` 3, mergeable | Focus the base-branch mark; press "Update with rebase" | Popover "This branch is out-of-date with main by 3 commits." / "Changes can be cleanly merged."; `pr update-branch N --rebase` logged | macOS | shot, log |
| Stack | 3-layer stack, selected layer 2 | Open "2/3"; Merge stack; fail once with stale heads | Layers top-down with check mark and "↳ main"; dialog "Merge 2 pull requests?"; toasts as the reference | macOS | shots, log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab/arrows/Return in the More menu and freshness popover; Escape closes each dialog and popover; focus returns to the trigger | Focus visible and ordered; Escape sends nothing; no scale/fade when reduced | macOS | `tree --ax`, state |
| Visual and protocol parity | Oracle on the same fixture | Pairs at 1280×840 and 840×620, light and dark, for each state and dialog; `target/t3-ui-parity/trace-diff.mjs pr-actions` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; `pullRequests.runAction` payloads equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "pull request merge method", "pull request primary control", "pull request action menu", "what to say when an action fails", "how the branch stands against its base", "which actions need the host read again after they run", "single-PR merge compatibility during stack discovery", "stacked pull request classification", "saved stack navigation", "pull request list overrides", "pull request list override settlement" | Pass | macOS | log |
| Gates | `git add -A` | Clone checks (`bun test`, strict `tsc`, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries); `bun scripts/caps.mjs`; the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.*`, new `pages-pr-actions.*`, `pages-pr-stack.*`, `r6-pr-logic.ts`, `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, fake gh, reference oracle. Any attended or normal-launch run uses a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-pr-conversation-and-refresh` and `20261005-hot-file-split` merge.
