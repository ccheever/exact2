---
name: 20261005-pr-handoffs-and-quick-actions
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Pull request hand-offs, folding header, Shift quick actions and row popovers

## Outcome

From a pull request a person can hand work to the agent as the reference does: the Check out
menu (separate worktree or this repository), "Ask a question", "Explain this PR", "Fix findings",
and a per-finding "Fix" on Summary comments and failing checks. The panel header folds to one row
while Summary or Timeline scrolls. On the Pull Requests page, holding Shift alone shows quick
actions on each row, right-click on a number offers "Copy link" and "Open on GitHub", and rows show
a checks popover and an n/m stack popover.

## Scope and exclusions

Included: hand-off menu items and `startAsk`/`startHandoff` for the panel, `build*Handoff` functions
and the draft plumbing they use; the Check out menu; per-finding Fix; the folding header (**200 ms**
grid-row transition with scroll compensation); **A6** Shift quick actions; row context menu, checks
popover and stack popover (n/m).
Excluded: the primary control, More menu actions, dialogs, failure hints and the panel's stack menu
(`20261005-pr-header-actions-and-stacks`); comments, reviews, edits (`20261005-pr-writing-and-metadata`);
the Code tab's "Fix in a thread"/"Add to agent" buttons, which call this ticket's functions
(`20261005-pr-code-tab`); the "Act on" environment radio (`20261005-pr-links-previews-and-routing`).
Reuse (done): `startHandoff`, `resolveConflictsPrompt`, `fixChecksPrompt`, `handoffPrompt`, `preparePayload` (`r6-pr-actions.ts`,
`r6-pr-logic.ts`), `ensureDraftThreadId`/`withHandoffThread` (`r7-handoff-thread.ts`), `git.preparePullRequestThread`, checks helpers (`checksList`,
`describeChecks`), `T3ContextMenu.swift`, the ⌘ jump-hint flag monitor (`modules/apple/T3Sidebar.swift` `flags`, `jumpModifiers`),
and the list-override and action-runner code that `20261005-pr-header-actions-and-stacks` adds.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` = `apps/web/src/components/pullRequest/`,
`P` = `W/PullRequestDetailPanel.tsx`): `P:1035-1480,1999-2022,2317-2335,2656-2680`; `W/usePullRequestActions.ts:197-407`;
`W/pullRequestDetail.logic.ts` hand-off builders; `W/PullRequestSpeedActions.tsx`; `W/PullRequestRow.tsx:251`; `apps/web/src/routes/_chat.pull-requests.tsx:347-349,957-968,1832`;
`W/PullRequestStackPopover.tsx`, `PullRequestChecksPopover.tsx:133-214`, `pullRequestLinkContextMenu.ts`; `apps/web/src/shortcutModifierState.ts`.
Clone paths `examples/t3-code/<file>`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (single pending hand-off; late old replies), layout-and-interaction (virtualized list; popover and menu structure; keyboard focus), design (every state; disabled with a reason), accessibility (named icon buttons, `aria-expanded`), motion (fold; reduced motion), testing-and-debugging. Unknown in the library: Swift key monitors, context menus, dialog and popover focus handling; the clone's own runtime evidence on the pinned main is the basis.
Observed today: the list row is a single button (`pages-prs.contract` `PrRowButton`), no right-click or popovers on rows, modifier state exists only for ⌘ (`T3Sidebar.swift`), the panel has no hand-off items, no Check out menu and does not fold; `startHandoff` serves only the thread card's Resolve and Fix.
Reference rules to keep: one hand-off at a time whatever the surface; the thread is opened before the checkout so the setup script runs; a task never overwrites text the reader typed (`handoffPrompt` replaces only the last hand-off's sentence); with an active composer ("attach target") the task is written there instead of opening a thread; `speedMode` = Shift alone and not while an editable control has focus.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged (Summary model, refresh) | pending |
| merged task PR | [20261005-pr-header-actions-and-stacks](20261005-pr-header-actions-and-stacks.md) | pending | Merged (action runner, list overrides, `pullRequests.stack` read, Resolve conflicts button) | pending |
| merged task PR | [20261007-real-github-lane](20261007-real-github-lane.md) | pending | Sandbox seeded; probe rows for list search, `listStats` and `stack` decoded | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| scheduling preference | [20261005-main-fix-adoption](closed/20261005-main-fix-adoption.md) | pending | Merged first (popover/tooltip Contract) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and the draft records in `../issues/` (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | "Shift held alone" fact for quick actions | Only the ⌘ jump-hint monitor exists | nonblocking (workaround: Swift `NSEvent` flags monitor like `T3Sidebar.swift`, status field on `t3.status`) | Add `shiftHeld` with its AppKit test; reset on resign-active and while a text input has focus |
| [X26](../issues/20261005-x26-app-menu-control.md) | Menu at the pointer for the number's right-click | `T3ContextMenu.swift` | nonblocking (workaround exists) | Reuse |
| [X17](../issues/20261005-x17-popover-position-try.md) | Popover flips near window edges (checks, stack) | Fixed placement | nonblocking (workaround: fixed placement; declared) | Declare in `EXACT2-GAPS.md` |
| [X23](../issues/20261005-x23-scroll-restore-offsets.md) (sub-case X23d: nested scroll offset read and same-frame write), related [X22](../issues/20261005-x22-reactive-layout-facts.md) | Scroll offset that drives the header fold, and a `scrollTop` write in the same frame as the fold | Answered by main #210 (#138 X23d, merged in [adopt-main-fixes-r4](closed/20261007-adopt-main-fixes-r4.md)): macOS anchors a plain `scroll` box as Chrome does, so a fold above the port no longer moves the content | none (was blocking for the fold row) | Build the fold row; measure against the reference whether its `compensationRef` write is still needed on top of anchoring |
| [X13](../issues/20261005-x13-hover-keys-during-pan.md) | Hover reveals during a pan | Documented clone limit | nonblocking (workaround: partial, r12) | Declare |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` near its cap | 1,327/1,500 lines | nonblocking | New files, no new root resource |
| [X21](../issues/20261005-x21-two-way-websocket.md) | RPC send | Swift transport | nonblocking | Reuse `client.rpc` |

## Implementation notes

- **Port** (names, tests; header records changes) from `W/pullRequestDetail.logic.ts`: `buildFixFindingsHandoff`, `buildFixFindingHandoff`, `pullRequestFindingKey`, `buildAskAboutPullRequestHandoff`, `buildExplainPullRequestHandoff`, `buildAddSelectionToAgentHandoff`, `buildPullRequestReferenceContext`, `stripPullRequestHandoffReferences`, `handoffPrompt`, `handoffReviewComments`, `pullRequestHandoffLabels`, `buildResolveConflictsPrompt` (extend `r6-pr-logic.ts`; keep the existing exports); `shortcutModifierState.ts` pure functions (`areShortcutModifierStatesEqual`, `shortcutModifierStateAfterKeyboardEvent`) for the predicate tests.
- **Hand-offs.** Generalize `startHandoff` to the reference's `startAsk` (no checkout) and `startHandoff` (checkout, thread, task) with toasts "Added to the composer", "Asked in a thread", "Preparing the pull request checkout...", "Checkout ready", "Checked out", "Checked out here", "Checked out, but not on the latest commits", "Checked out, but the thread stayed where it was", "Could not open a thread", "Could not prepare the pull request checkout". Menu rows: "Ask a question" (subtitle depends on an active composer), "Explain this PR", "Fix findings"/"Fix findings in this thread"; Check out `aria-label` "Check out" / "Checking out..." with "In a separate worktree" and "In this repository". Per-finding Fix on Summary review comments and failing checks calls `buildFixFindingHandoff`; the Code tab reuses it.
- **Fold.** Two header states (full block with title, people and checkout command; one condensed row); the transition animates grid rows 0fr↔1fr in 200 ms; compensate the scroll offset by the height that leaves, reopen with no compensation at the hard top (offset < 4), close once past the block plus 32. Each tab remembers its own state. With reduced motion the change is immediate.
- **Quick actions (A6).** Group `aria-label` "Quick actions for pull request #N"; Reopen / Close+Ready for review / Close+Merge (GitHub rows, not merged); no confirmation; Merge re-reads the detail with `allowStale:false` and refuses a non-open, draft, unpermitted or stacked PR ("This pull request cannot be merged.", "Open this pull request to merge its stack.", "No merge method is available for this repository."); stays visible while pending; optimistic row override except Merge; buttons `aria-label` "<Label> #N".
- **Row extras.** Right-click on the number: "Copy link", "Open on GitHub" (host-named); checks popover: headline, attention rows then completed behind "Show all"/"Show less", "No checks reported", "Loading checks…"; stack popover `aria-label` "Stack N, layer i of n" with "Loading stack…", "Retry stack refresh", "May be stale", "↳ <base>".

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Hand-offs | The sandbox clone as the project (lane gh), worktree support | `bun scripts/agent.mjs macos --json "tap pr-row-102" "tap pull-request-more"` then Ask, Explain, Fix findings, Check out (both modes) | New draft holds the ported prompt and PR chip, nothing sent; `git worktree list` and branch effects; toasts as listed; a typed prompt survives a second hand-off | macOS 1280×840 | draft state, git output |
| Hand-off failures | Branch already checked out (live); other server failures by unit test with an injected failure | Repeat | Error toast with the server sentence; no draft change; menu usable again | macOS | shots |
| Per-finding Fix | PR with a failing check and a review comment | Press Fix on each | Prompt from `buildFixFindingHandoff`; one hand-off at a time (second press ignored) | macOS | draft state |
| Quick actions | List of open, draft, closed, stacked rows | AppKit test for the flag predicate; `(attended session)` hold Shift alone | Buttons for Shift alone only (not ⌘/⌃/⌥ combos, not while the search field has focus); stacked Merge disabled with tooltip; effects as the header ticket's runner | macOS, real keyboard; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | AppKit binary, recorded steps |
| Row menu and popovers | Row with checks and a stack | `(attended session)` right-click the number; open checks and stack popovers with the agent | "Copy link", "Open on GitHub"; checks headline and "Show all"; stack layers and "↳ main" | macOS; same lane build | shots |
| Header fold | Long Summary | Scroll past the fold; `screenshot fold.apng over 300 every 30`; `prefer prefers-reduced-motion reduce` | Folds to one row, reopens at the top; no slide when reduced; per-tab memory | macOS | apng, state |
| Keyboard focus, Escape, reduced motion | — | Tab/arrows/Return in the hand-off and Check out menus and popovers; Escape closes each; focus returns to the trigger | Focus visible and ordered; no animation when reduced | macOS | `tree --ax` |
| Visual and protocol parity | Oracle on the same fixture | Pairs at 1280×840 and 840×620, light and dark: menus, rows with quick actions, popovers, folded header; `target/t3-ui-parity/trace-diff.mjs pr-handoffs` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; hand-off RPC order equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "fix findings handoff", "findings that cannot be attached", "one finding handed over on its own", "findings that are already on a line", "asking about a change rather than working on it", "a second ask into the same composer", "pull request panel context beside a thread", "tracks bare modifier keydown and keyup events explicitly", "ignores poisoned modifier flags on non-modifier keys", "clears a held modifier when a non-modifier key reports it released" | Pass; React-hook-only cases classified n/a-ui in the header | macOS | log |
| Gates | `git add -A` | Clone checks (`bun test`, strict `tsc`, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries); `bun scripts/caps.mjs`; the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.*`, new `pages-pr-handoffs.*`, `pages-pr-quick.*`, `pages-prs.*` (row), `r6-pr-actions.ts`, `r6-pr-logic.ts`, `modules/apple/T3Sidebar.swift` or a new modifier module with `macos/tests/`, `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, git, the real-GitHub lane (sandbox, shared lane config dirs, two accounts; `tools/github-lane`), reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

2026-10-06: on hold (user decision). Tasks that need a sign-in (GitHub, provider accounts, T3 Connect) do not start until the user lifts the hold.

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

Starts after [20261007-real-github-lane](20261007-real-github-lane.md) merges: `prepare` from `feat(example)/t3-code` on its shared lane login and sandbox (`examples/t3-code/tools/github-lane/README.md`), with a unit-test fallback for injected failures and delays and for the read, triage and read-only-author profiles.
