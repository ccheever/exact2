---
name: 20261005-pr-header-actions-and-stacks
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-header-actions-and-stacks
pr_url: https://github.com/ccheever/exact2/pull/262
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
other hosts' action sets (capability flags drive the menu; only GitHub runs live, on the sandbox).
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
| merged task PR | [20261007-real-github-lane](closed/20261007-real-github-lane.md) | pending | Sandbox seeded; probe rows for `runAction` and the profiles confirmed; stacks and workflow approval as its probe records them | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| scheduling preference | [20261005-main-fix-adoption](closed/20261005-main-fix-adoption.md) | pending | Merged first (popover/tooltip Contract) | pending |

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
| Primary control per state | Sandbox (lane config dir): `conflict`, `failing`, `draft`, `open-clean`, `merged`, `closed` (numbers in `sandbox.json`) | `bun scripts/agent.mjs macos --json "tap pr-row-<n>" tree` per PR | Resolve conflicts / Auto-merge (m) / Ready for review / Merge label / Merged badge / Closed badge; menu items as the reference's matrix | macOS 1280×840 | tree JSON, shots |
| Each action's effect | Primary account on its own and on the second account's pull requests (seeded, or new ones as the probe makes them) | Merge (3 methods), auto-merge on/off, ready, draft, close, reopen, update-branch (both), revert, approve-workflows (each where `sandbox.json` and the probe record GitHub allows it for this account) | `logs/gh-calls.tsv` argv as the verb table; GitHub read back with the lane gh shows the change; success toast text; Cancel in a dialog sends nothing | macOS | gh log + read-back |
| Failures | Unit tests with injected failures (as `pr-profiles-injection.test.ts`); live where GitHub refuses on its own (merge of a conflicting pull request, update of a branch that is not behind) | Repeat | Title "Could not …", description = host sentence else the hint; buttons re-enabled; list row's optimistic note rolled back | macOS | shots |
| Permissions | Live: the second account (write) on the primary's pull requests and on its own; unit tests for `reader`, `triage`, `contributor-author` (personal repositories have no read or triage collaborators) | Open the same PRs | reader: no action items; contributor-author: close/reopen/ready/draft but no merge; update-branch only when `viewerCanUpdateBranch` | macOS | tree JSON |
| Out-of-date branch | `behindBy` 3, mergeable | Focus the base-branch mark; press "Update with rebase" | Popover "This branch is out-of-date with main by 3 commits." / "Changes can be cleanly merged."; `pr update-branch N --rebase` logged | macOS | shot, log |
| Stack | 3-layer stack, selected layer 2 | Open "2/3"; Merge stack; fail once with stale heads | Layers top-down with check mark and "↳ main"; dialog "Merge 2 pull requests?"; toasts as the reference | macOS | shots, log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab/arrows/Return in the More menu and freshness popover; Escape closes each dialog and popover; focus returns to the trigger | Focus visible and ordered; Escape sends nothing; no scale/fade when reduced | macOS | `tree --ax`, state |
| Visual and protocol parity | Oracle on the same fixture | Pairs at 1280×840 and 840×620, light and dark, for each state and dialog; `target/t3-ui-parity/trace-diff.mjs pr-actions` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; `pullRequests.runAction` payloads equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "pull request merge method", "pull request primary control", "pull request action menu", "what to say when an action fails", "how the branch stands against its base", "which actions need the host read again after they run", "single-PR merge compatibility during stack discovery", "stacked pull request classification", "saved stack navigation", "pull request list overrides", "pull request list override settlement" | Pass | macOS | log |
| Gates | `git add -A` | Clone checks (`bun test`, strict `tsc`, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries); `bun scripts/caps.mjs`; the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.*`, new `pages-pr-actions.*`, `pages-pr-stack.*`, `r6-pr-logic.ts`, `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, the real-GitHub lane (sandbox, shared lane config dirs, two accounts; `tools/github-lane`), reference oracle. Any attended or normal-launch run uses a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Results

Built on `feat(example)/t3-code` (prepared at `bdca4216a`; merged forward to `757d9517a`, then `e91fcfc65`). Prerequisites:
pr-conversation-and-refresh (#247) and real-github-lane (#233) are merged; desktop-oracle-and-trace is not built (user decision
2026-10-06), so its rows are "not run". GitHub stacks are available to the primary account (real-github-lane's probe).

**What was built.**
- Ported (`r6-pr-logic.ts`, `pages-pr-stack.ts`, `pages-pr-actions.ts`; tests in `pages-pr-actions.test.ts` under the
  reference's names): `PULL_REQUEST_MERGE_METHOD_LABELS`, `resolvePullRequestMergeMethod`, `allowsSinglePullRequestMerge`,
  `resolvePullRequestPrimaryControl`, `pullRequestActionMenuHasGroup`, `isStackedPullRequestBase`, `resolveBaseFreshness`,
  `pullRequestActionNeedsHostRefresh`; `savedPullRequestStack`, `pullRequestStackView`; `pullRequestOverrideAfterAction`,
  `applyPullRequestOverrides`, `settlePullRequestOverrides`; the panel's success, failure and hint tables for all ten actions
  and the rebase hint (`P:181-243`); `actionPayload` with `mergeMethod`, `updateMethod`, `stackNumber`, `expectedStackHeads`.
- One runner (`performAction`): one action at a time, marked pending and drawn while GitHub works (the panel's resource is
  woken, `t3.pr`), the reference's success toast, a failure toast "Could not …" with `readableFailure(failure, hint)` (the
  rebase hint for "Update with rebase"), then the detail, activity and stack read again — around the server's cache
  (`pullRequests.invalidate`) after update-branch and approve-workflows. The list is told "sent"/"done"/"failed": a
  state-only action (close, reopen, ready, draft) is written onto the row as it is sent and taken back if refused; a merge
  only once done; anything else re-reads the list. A whole-list answer settles the overrides it agrees with (60 s trust).
- The header (`pages-pr-actions.contract` PrdHeaderActions): the stack controls, the armed badge beside a primary control
  that is not the badge, the primary control per `resolvePullRequestPrimaryControl` (Merge with the method's label /
  "Merging...", "Auto-merge (method)" / "Enabling...", Ready for review, Resolve conflicts → the existing conflicts
  hand-off, the info badge, the Merged/Closed outline badge), each with its tooltip and `aria-label`; labels go under a
  30rem panel (`@max-[30rem]/pr-header:hidden`), the tooltip keeping the words.
- The More menu (PrdActionsMenu, replacing PrdMore): "More pull request actions"/"Refreshing pull request" with the turning
  refresh glyph while the host is read again (still under reduced motion); Refresh; the open pull request's group
  (Convert to draft / Ready for review where the slot does not hold it, Merge now, Enable/Disable auto-merge, the
  merge-method radios when more than one is allowed and the branch can merge, with the reference's separators); Open on
  GitHub, Copy link, Copy PR number; Close (destructive, confirmed), Reopen, Revert changes (confirmed). The merge method
  follows `resolvePullRequestMergeMethod`: the choice for this pull request, then the project's Default merge method,
  then the last method chosen on this device (kept in `t3-code.json`); an armed method becomes the choice.
- The five confirmations over the window (PrActionDialog): "Merge pull request?" / "This merges #N using <method>." /
  <method label>; "Enable auto-merge?"; "Revert these changes?" / "Create revert PR"; "Approve workflows to run?" /
  "Approve and run"; "Close pull request?" / "Close" (destructive). Cancel is Escape; focus starts on Cancel and Tab keeps
  to the two buttons (X53's stopgap). Cancel sends nothing.
- The out-of-date base (PrdBaseBranch): amber base name with the warning mark; pointing at it shows the card at once,
  pressing it (or Return) opens it as a popover; "This branch is out-of-date with <base> by N commit(s)." / "Changes can
  be cleanly merged." / Update branch, Update with rebase (each only where the host offers it and the viewer may take it).
  A stacked base wears the layers glyph and the tooltip "Stacked on <base>" (`vcs.listRefs`, limit 2).
- "Approve workflows to run" (warning outline, `play` glyph) in the Summary tab bar where checks would be, when the host
  reports runs awaiting approval and the checks there are current.
- The stack (`pages-pr-stack.*`): `pullRequests.stack` once the detail says `capabilities.stacks` (with the environment's
  `threadPullRequests`), read with the activity; the saved membership from thread links while it is out; "Retry stack
  lookup" when there is none and the lookup failed; the trigger "i/n" (`aria-label` "Stack N, layer i of n", tooltip "View
  stack #N, layer i of n · notice", a warning glyph when stale); the menu: "Stack #N" with "May be stale"/"Refreshing…",
  "Retry stack refresh", the layers top-down (state glyph, title over "#n · branch · State", a check on this one;
  pressing one opens it: the page's selection, or a pull request surface beside a thread), "↳ base", "Merge stack (k)",
  "Rebase stack", "Every layer being merged must be open and ready for review."; the separate "Merge stack" button with
  its tooltip; the Dialog "Merge k pull requests?" / "Rebase n pull requests?" with the layers, Cancel and "Merge stack" /
  "Rebase stack" / "Working…"; Merge sends `stackNumber`, the merge method and the heads of the layers it merges; Rebase
  targets the top with every unmerged head; toasts "Stack merge request completed" (+ "GitHub merged the stack or added it
  to its merge queue."), "Stack rebased", "Stack operation did not complete" with the error's name and words. The single
  Merge waits while a stack exists or its discovery is out (`allowsSinglePullRequestMerge`).
- Lane: `seed.mjs` gains `actionScenarios()` (eleven pull requests whose state a drive changes, and a three-layer stack),
  seeded only by name (`--only act-…`), each run opening the next generation of any a drive used up.

**Acceptance.**

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Primary control per state | pass (live + unit) | live #132 Merge, #111 "Auto-merge (merge)", #107 Ready for review, #110 Resolve conflicts, #109 Merged, #108 Closed ([01](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/01-open-clean.png), [02](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/02-failing-auto-merge.png), [03](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/03-draft-ready.png), [04](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/04-conflict-resolve.png), [05](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/05-merged-badge.png), [06](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/06-closed-badge.png); header text in the record); #148 armed "Auto-merge (rebase and merge)" badge; unit "the header per state" (7 tests) | — |
| Each action's effect | pass (live) except approve-workflows (unit) | `logs/gh-calls.tsv` argv and lane-gh read-back per action ([record](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/live-drive-record.txt)): `pr merge 145 --merge` (merged, 2 parents), `--squash` #146 (1 parent), `--rebase` #147; `pr merge 148 --auto --rebase` (autoMergeRequest REBASE) then `--disable-auto` (null); `pr ready 149 --undo` (draft), `pr ready 149`, `pr close 149` (closed), `pr reopen 149` (open); `pr update-branch 150` and `pr update-branch 151 --rebase` (behind_by 21 → 0); revert of #152 opened #160 (closed again); each success toast as the reference's; Cancel on the Merge dialog and Escape on the Close dialog sent nothing (no write in the log, #132 still open) | approve-workflows: GitHub asks approval only for an outside or first-time contributor's fork run; both lane accounts are collaborators (real-github-lane W28) — unit "every action sends the verb table's payload" |
| Failures | pass (live + unit) | live: Merge now on #148 while `ci/build` is pending → "Could not merge this pull request" + the merge hint (GitHub's refusal reached the server as "GitHub CLI command failed."), the controls back; stack with a stale head → "Stack operation did not complete" · "PullRequestOperationError: Pull request operation runAction failed: The stack changed. Refresh it before trying again."; unit "a refusal: …" (host sentence, the hint, the rebase hint, the auto-merge hint) and the list rollback | update of a branch that is not behind: the mark is offered only for a behind branch, so the UI cannot send it; unit covers the hint |
| Permissions | pass (live write collaborator (second account: #132 and its own #115 show the write menu; it merged #161, read back merged by `daehyeon-mun`) + unit) | unit "permissions" (reader: no action; read-only author: close/reopen/ready/draft, no merge; update-branch only with GitHub's `viewerCanUpdateBranch`) | — |
| Out-of-date branch | pass (live) | #150 "This branch is out-of-date with main by 17 commits." / "Changes can be cleanly merged." / Update branch, Update with rebase ([20](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/20-freshness-popover.png), [21](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/21-freshness-hover.png)); Update with rebase on #151 logged `pr update-branch 151 --rebase`, read back behind_by 0 | the row's "by 3" fixture: real GitHub said 17 and 21 (main moved); unit covers "by 3 commits" and "1 commit" |
| Stack | pass (live) (+ retry: stack #164 from layer 2, Merge stack → "Stack merge request completed" · "GitHub merged the stack or added it to its merge queue.", #162 and #163 read back merged) | #154 layer 2 of stack #156: "2/3", the menu top-down with the check on #154 and "↳ main", "Merge 2 pull requests?" ([22](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/22-stack-menu.png), [23](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/23-stack-merge-dialog.png)); a commit pushed out of band to #153 → "The stack changed. Refresh it before trying again." ([24](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/24-stack-stale-failed.png)); Merge stack then merged #153 and #154 on GitHub (read back) though the server answered "GitHub CLI command failed." after the merge-async poll (finding below); a layer pressed opens it (#154 from #155's menu) | — |
| Keyboard focus, Escape, reduced motion | partial (agent) | an agent Escape closes the Close dialog and nothing is sent (no write in the log, #132 still open); the dialogs' focus start (Cancel, `autofocus`) and Tab cycle (`key` handlers, X53's stopgap) are in the build, not yet driven; `prefer prefers-reduced-motion reduce`: the refresh glyph does not turn ([27](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/27-reduced-motion.png)) | real Tab/arrows/Return and the visible ring: real-input batch (steps below) — screen locked (user away) |
| Visual and protocol parity | not run | before/after pairs against the base build instead (12 pairs, 1280×840 and 840×620, light and dark) | user decision 2026-10-06: the desktop oracle and trace tools are not built |
| Ported tests | pass | `pages-pr-actions.test.ts`: "pull request merge method", "pull request action menu", "pull request primary control", "stacked pull request classification", "how the branch stands against its base", "which actions need the host read again after they run", "single-PR merge compatibility during stack discovery", "saved stack navigation", "pull request list overrides", "pull request list override settlement"; "what to say when an action fails" in `pages-pr-logic.test.ts` | — |
| Gates | see the PR | numbers in the PR body | — |

**Live drives** (agent mode, the branch's development build; [record](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/live-drive-record.txt), scripts
[drive](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/drive.mjs.txt) and [retry](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/drive2.mjs.txt)). Pull requests seeded for this task: #145–#155 (stack
#156), then #161 and #162–#163 (stack #164) for the retry; neutral names, `bun seed.mjs --only act-…`. Before: the same
read-only screens on the base build (`757d9517a`, the evidence worktree). After: one session as the primary account
(owner) ran every action above. Retry (the one allowed): the fixes below, as the second account (write collaborator) on
its own lane server.

The first session found three things, fixed before the retry: the base branch's hover card kept its state after the
pointer left another pull request's mark (the card was drawn on later pull requests), the tab bar painted over the card
(z-index orders siblings: the header is now raised), and the header kept its labels under a 30rem panel (the reference
hides them). The retry showed the card over the tab bar ([21](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/21-freshness-hover.png)) and the icon-only header at
840×620 ([11](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/11-narrow-open-clean.png)), and caught "Merging..." the moment Merge was confirmed
([17](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/17-merging.png)).

**Findings (not clone bugs).**
- Rebase stack twice ended in GitHub/server refusals, shown as the reference shows them: with an out-of-band commit on
  the bottom layer, the server rebased two layers and then read the stack as changed ("The stack changed at PR #153 after
  2 layers. Earlier updates remain on GitHub."); as the second account, "Stack rebase stopped at PR #162 after 0 layers."
  The request was the reference's (`update-branch`, `updateMethod: rebase`, the top layer, every unmerged head); the success
  toast "Stack rebased" is covered by the unit test.
- Merge stack on #154 merged #153 and #154 on GitHub (read back), but the server's second poll of `merge-async` failed
  ("GitHub CLI command failed."), so the toast was "Stack operation did not complete". The retry's Merge stack on #164
  completed with the success toast. T3 Code server behaviour (`githubStackActions.ts` polling), not the clone's.
- The second account's first `pullRequests.stack` read failed once (7 s, "GitHub CLI command failed."); the panel showed
  "Retry stack lookup" and no single Merge until a later read answered ([26](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/26-retry-stack-lookup.png)), as
  `allowsSinglePullRequestMerge` says.
- A burst of answers (a media preference change together with Refresh, at the end of the first session) was refused by
  the native executor ("native executor admission limit reached": 16 ordered requests, `host/apple/src/executor_core.rs`
  `COUNTS`); the Pull Requests list then said "Could not load pull requests" until its Retry. The panel's stack and
  default-branch reads now follow the activity one at a time instead of beside it. Framework capacity: on main `0365ad1a4` a
  resource refused at admission is asked again, but a request a source makes inside its answer gets the refusal as a
  failed `fetch` (filed 2026-10-08 as [#286](https://github.com/ccheever/exact2/issues/286)).

**Decisions.** The merge method a device remembers (`useUiStateStore pullRequestMergeMethod`) is kept in `t3-code.json`
(`pages.mergeMethod`). The ghost's action slot (Check out, and Resolve conflicts before the detail lands) is left with the
Check out menu (`20261005-pr-handoffs-and-quick-actions`). Provisional, user decision pending: none.

## Real-input batch steps

Deferred rows (screen locked while the user is away, 2026-10-08): real Tab, arrow keys and Return in the More
menu and the freshness popover with the visible focus ring, the real pointer resting on the base branch's mark,
and reduced motion from System Settings. Everything else ran in agent mode. One session:

1. Worktree `t3-code-pr-header-actions-and-stacks` at the PR head. `export PATH="$HOME/.bun-1.4.2/bin:$PATH"
   EXACT_APP_DIR="$PWD/examples/t3-code"`; `bun examples/t3-code/stage-runtime.mjs` if `server-runtime/*.tar.gz` is
   missing; `bun host/apple/build.mjs t3-code-macos --bundle`.
2. Lane server: `cd examples/t3-code/tools/github-lane`, `export T3_GITHUB_LANE_SHARED=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-ui-parity/github-lane`,
   `bun lane.mjs start primary --port 16720`, `bun lane.mjs project primary`, `bun lane.mjs pair primary` (writes the
   single-use URL to `target/github-lane/servers/primary/pairing-url`, 0600; never print it). Fresh pull requests for the
   writes, if needed: `bun seed.mjs --only act-lifecycle,act-update-merge` (each run opens the next generation).
3. Lane copy: copy `target/clients/<hash>/com.exact.t3code.macos/macos/T3 Code (Exact).app` to
   `target/github-lane/realinput/T3 Code (Lane PHA).app`; with PlistBuddy set `CFBundleIdentifier` to
   `com.exact.t3code.lanepha` and `CFBundleName`/`CFBundleDisplayName` to `T3 Code (Lane PHA)`; `codesign --force --deep
   --sign <the Apple Development identity build.mjs signs with>`. Take the shared real-input lock (owner
   "pr-header-actions-and-stacks: real-input batch"). Launch by path with
   `CFFIXED_USER_HOME=$PWD/target/github-lane/realinput/home`; record the pid; the window id from
   `orca computer list-windows --app pid:<pid> --json`.
4. Pair: the welcome wizard's Pairing link field by `set-value --value-stdin` from the `pairing-url` file, click Pair,
   Continue through the wizard. Open Pull Requests, Filters › State › All, put `#132` in the search with `set-value`,
   click the row.
5. **More menu by keyboard** (#132): click the panel's title (no action), then `press-key Tab` until the ring is on
   the "…" (More) button (a screenshot after each Tab); Return opens the menu; Down/Up move the highlight across
   Refresh, Convert to draft, Enable auto-merge, Merge, Squash and merge, Rebase and merge, Open on GitHub, Copy link,
   Copy PR number, Close pull request; Escape closes it and the ring is back on More. Read back: the menu closes and
   nothing was sent (`target/github-lane/logs/gh-calls.tsv` has no write line).
6. **Freshness popover by keyboard and pointer** (`#150` or the newest `docs/install-steps` pull request, behind main):
   Tab to the amber base branch mark, Return opens "This branch is out-of-date with main by N commits." with Update
   branch and Update with rebase; Tab moves onto Update branch, then Update with rebase; Escape closes it (nothing
   sent). Then move the real pointer onto the mark: the card shows at once and stays while the pointer moves into it;
   click Update with rebase: the toast "Branch updated with the base branch", and `gh-calls.tsv` shows
   `pr update-branch <n> --rebase`; read back `behind_by 0` with the lane gh compare.
7. **Dialog keys** (#132): More › Close pull request; the ring starts on Cancel; Tab moves to Close and back to Cancel;
   Escape closes the dialog; `gh-calls.tsv` has no `pr close` line.
8. **Reduced motion** (only with the user's permission, a system setting the app reads with no in-app override): turn
   System Settings › Accessibility › Display › Reduce motion on, More › Refresh: the trigger shows the refresh glyph
   without turning; turn it off again.
9. Cleanup: quit the copy; `bun lane.mjs stop primary`; delete the copy's Keychain item
   (`security delete-generic-password -s com.exact.t3code.macos.access-token -a "$(printf 'http://127.0.0.1:16720\n%s' "$(cat target/github-lane/servers/primary/t3home/userdata/environment-id)")"`);
   remove `target/github-lane/realinput`; release the lock.

## Progress

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented on `feat(example)/t3-code-pr-header-actions-and-stacks`; unit tests; one live session and one
retry on the real-GitHub lane (agent mode; the screen is locked, so the real-input rows wait for the batch); draft PR #262.

2026-10-08 (real-input batch, records PR): freshness by keyboard, update-branch write and reduced motion pass; four clone bugs (keyboard menu focus, hover card, refresh storm/stale freshness after update, no ring in the Close dialog). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (live, owner) | `bf7c19d41` with the dialog focus stopgap (committed in `39ee7aa29`) | every action passed on GitHub; Merge stack merged but the server reported a failure; Rebase stack refused by GitHub/server; three UI faults: the hover card's state survived another pull request, the tab bar painted over it, labels under 30rem | [record](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/live-drive-record.txt) | fixed in attempt 2 |
| 2 (retry, write collaborator) | `39ee7aa29` before its sequential stack reads | hover card over the tab bar, icon-only header at 840×620, "Merging...", the second account's merge, a stack layer opened, Merge stack completed; Rebase stack refused again | [record](https://raw.githubusercontent.com/ccheever/exact2/01f53038a5eef8c759b81e2ce842d556b194fe9d/pr-header-actions-and-stacks/live-drive-record.txt) (retry) | Rebase success live: GitHub/server refusals (findings) |

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| 5. More menu by keyboard | FAIL (clone bug): the menu opened from the keyboard keeps the focus on "…"; ↓/↑ move nothing; Escape closes, nothing sent | [pha-stab3-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/01-pha-stab3-small.png), [pha-menu-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/02-pha-menu-small.png), [pha-menu-zoom2](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/03-pha-menu-zoom2.png) |
| 6a. Freshness popover by keyboard | PASS | [pha-fresh-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/04-pha-fresh-small.png) |
| 6b. Freshness card by pointer | FAIL (clone bug): the hover card closes when the pointer moves into it | [pha-hover](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/05-pha-hover.png) |
| 6c. Update with rebase (pressed popover) | Write PASS (`pr update-branch 150 --rebase`, behind_by 8 → 0); FAIL (clone bug): no success toast, panel kept "out-of-date by 8", then ~200 invalidate/detail/list RPCs in 47 s | [pha-15-now](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/06-pha-15-now.png), [pha-16-later-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/07-pha-16-later-crop.png) |
| 7. Close dialog keys | Escape / nothing sent PASS; FAIL (clone bug): no visible focus ring on Cancel/Close | [pha-close-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/08-pha-close-small.png), [pha-close-zoom](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/09-pha-close-zoom.png) |
| 8. Reduced motion (System Settings) | PASS: the refresh glyph does not turn | [pha-on-strip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/10-pha-on-strip.png) |

Full record: [pr-header-actions-and-stacks.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/pr-header-actions-and-stacks/pr-header-actions-and-stacks.txt). Reduced-motion details: [reduced-motion.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/reduced-motion/reduced-motion.txt).

## Next action

Review the draft PR. The real-input batch (steps above) closes the keyboard and reduced-motion rows. Approve workflows
needs an outside or first-time contributor's fork run with a workflow, which the lane cannot produce (account blocker).
