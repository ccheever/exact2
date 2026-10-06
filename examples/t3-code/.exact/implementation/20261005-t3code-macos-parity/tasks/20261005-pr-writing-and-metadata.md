---
name: 20261005-pr-writing-and-metadata
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

# Pull request comments, reviews, edits, reactions, reviewers and labels

## Outcome

From the pull request panel a person can comment, close or reopen with a comment, submit a
review (Comment / Approve / Request changes) with a summary, rewrite the title, the
description and their own comments in a Write/Preview editor, react to remarks, ask people for
a review, and put labels on or take them off, with the reference's controls, wording, limits,
disabled reasons and failure behavior. The review store that holds pending line comments
(count badge, discard) exists here so `20261005-pr-code-tab` can fill it.

## Scope and exclusions

Included: **C1** (floating composer, Comment and Review modes), **C2** (title, description,
comment editing; long-comment collapse), **C3** (reviewer and label pickers), **C6** (reaction
pills and picker on remarks in Summary and Timeline).
Excluded: line comments, thread Reply/Resolve/Edit and reactions inside thread cards (`20261005-pr-code-tab`);
reactions on the PR description (the reference reads them, `P:700`, but renders no bar);
host actions (`20261005-pr-header-actions-and-stacks`); Forgejo's required-summary rule is ported, other hosts are not fixture-run.
Reuse (done): `prCommand` ops `comment`, `title`, `label`/`unlabel`, `request-review`/`remove-review` and `prCandidates`
(`pages-pr-detail.ts:189-245`, never wired to UI), the `pageslocal:pr-act-*` route (`app.contract:134-136`, one write at a time), `canEdit/canReview/canLabel`
fields (`pages-pr-detail.contract:113-121`, unused), the PR Markdown renderer for Preview.
Corrections needed: `canEdit` is `edit.changeRequest !== false` (`pages-pr-detail.ts:149`); the reference requires `=== true` and viewer = author or merge permission.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`; `W/` =
`apps/web/src/components/pullRequest/`, `P` = `W/PullRequestDetailPanel.tsx`): `W/PullRequestComposer.tsx`, `PullRequestCommentForm.tsx`,
`PullRequestReviewForm.tsx`, `pullRequestReviewStore.ts`, `PullRequestMarkdownEditor.tsx`, `PullRequestEditButton.tsx`, `PullRequestCommentBody.tsx`,
`pullRequestEditing.logic.ts`, `PullRequestReactions.tsx`, `pullRequestReactions.logic.ts`, `PullRequestReviewerPicker.tsx`, `PullRequestLabelPicker.tsx`,
`PullRequestCandidatePicker.tsx`, `P:981-1026,2332-2392,2755-2780`, `PullRequestSummaryTab.tsx:136-190,578-700,714-860`, `PullRequestTimelineTab.tsx:177-290`;
client cache patches `packages/client-runtime/src/state/pullRequests.ts:259-328`; server rules `PullRequestService.ts:2027-2194,2349-2460`
(empty comment, host capability, viewer permission refusals); contracts `pullRequest.ts:1094-1255`. Clone paths `examples/t3-code/<file>`, lines from the mc-orch tree, 2026-10-05.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (keep draft text in state that input always updates; commit or reset it separately; one mutation per action path; late replies), components (child state disappears with the instance: keep drafts in a longer-lived owner), layout-and-interaction (native `select`, `textarea`, popover structure), accessibility, design (all states), motion (hover-revealed pencil; reduced motion), testing-and-debugging. Unknown in the library: IME/composition in a textarea, popover focus return; the clone's runtime evidence on the pinned main is the basis.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Reference wording to keep: placeholders "Leave a comment", "Summarize your review (optional)", "Describe this pull request"; buttons "Comment", "Close with comment", "Reopen with comment", "Submit review", "Save", "Cancel"; toasts "Review submitted", "Pull request approved", "Changes requested", "Could not post the comment", "The review could not be submitted", "The title could not be saved", "Could not save the description", "Could not save the comment", "The reaction could not be saved", "Review requested from <login>", "Review request to <login> taken back", "Could not put <label> on", "Could not take <label> off".

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-pr-conversation-and-refresh](20261005-pr-conversation-and-refresh.md) | pending | Merged (conversation model, refresh, `readableFailure`) | pending |
| merged task PR | [20261005-fake-github-fixture](20261005-fake-github-fixture.md) | pending | Write verbs and profiles served | pending |
| scheduling preference | [20261005-pr-header-actions-and-stacks](20261005-pr-header-actions-and-stacks.md), [20261005-main-fix-adoption](20261005-main-fix-adoption.md) | pending | Close/Reopen with comment calls the same `pr-act-action` path that `20261005-pr-header-actions-and-stacks` upgrades; merge that one first if both are open | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | ⌘↵ and Escape in a textarea; composition guard (`isComposing`/229) | macOS Korean 2-Set | nonblocking (workaround: `R9Input`/`T3KeyRecorder` pattern) | Prove in an attended session |
| [X16](../issues/20261005-x16-smart-substitutions-off.md) | Exact bytes typed in a textarea | `t3-plain-text` hook | nonblocking (workaround exists) | Use the hook on every editor |
| [X17](../issues/20261005-x17-popover-position-try.md) | Composer popover opens above, end-aligned; picker popups flip | fixed placement | nonblocking (workaround: fixed placement) | Declare near-edge difference |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Preview needs a root resource argument (`app.contract:101`) | +3 lines | nonblocking | Keep the addition minimal |
| [X21](../issues/20261005-x21-two-way-websocket.md) | RPC send | Swift transport | nonblocking | Reuse `client.rpc` |
| [X16](../issues/20261005-x16-smart-substitutions-off.md) | Exact typed bytes in the PR title, description, comment and review textareas | X16 | nonblocking (workaround: the `t3-plain-text` hook on these textareas) | remove the hook when X16 is adopted |

## Implementation notes

- **Port** (names, tests, header changes): `pullRequestEditing.logic.ts` (`canEditPullRequestChangeRequest`, `canEditPullRequestComment`, `sameLogin`), `pullRequestReactions.logic.ts`
  (`PULL_REQUEST_REACTION_ORDER`, `pullRequestReactionEmoji/Name/Tooltip`, `applyPendingPullRequestReactions`), `pullRequestReviewStore.ts` (as a plain module keyed by `pullRequestReviewKey`; not persisted),
  the cache-patching semantics of `requestReviewers`/`setLabels`/`updateComment` (`pullRequests.ts:259-328`) onto the clone's detail cache. `presentDetail` also carries `viewer`, `capabilities.edit/reactions/labels/reviewers`, `viewerPermissions`, comment reactions.
- **Drafts.** Comment text, review summary and pending comments live in a TS draft store keyed by PR (they survive closing the popover, switching tabs and PRs in the session), mirrored into child text state that `input` always updates; Preview sends the text once through `prAct("preview", …)` so the PR Markdown resource renders it. No draft is persisted.
- **Composer.** Floating icon button (`aria-label` "Comment on pull request" / "Review pull request" / "Review pull request, N comments pending") with count badge; popover `aria-label` "Pull request composer"; mode toggle `aria-label` "Composer mode"; "Close composer", "Discard pending line comments" buttons. Hidden when neither comment nor any verdict is allowed. Submit rules: `canSubmit` (Approve may be empty; others need a summary or a pending comment; request-changes needs a summary where the host requires one). Verdict labels from the viewer's allowed set only (an author sees Comment only). Close/Reopen with comment: post, then the existing action; if the action fails the comment stays and the failure toast explains.
- **Edit.** Pencil buttons (`aria-label` "Edit title"/"Edit description"/"Edit comment") appear on hover or keyboard focus (always when no hover); title uses a one-line field (Enter saves, Escape cancels, unchanged sends nothing, ≤1024, trimmed); description/comment use the Write/Preview editor (`aria-label` "Markdown editor mode"; "Nothing to preview."; ⌘↵ saves, Escape cancels unless saving; description may be empty; comment may not; body ≤65,536). Failures keep the editor and text. Comments taller than 240 pt collapse with "Show full comment"/"Show less" (`aria-expanded`); focus inside expands.
- **Pickers.** Candidates are read when the popup opens (`pullRequests.reviewerCandidates`/`labelCandidates`); the popup stays open after a pick; states: ghost rows, error "<label> <error>", empty, no match, truncated note, disabled button with tooltip ("Asking someone to review needs write access on this repository", "Changing labels needs triage access on this repository"). Shown only per `capabilities.reviewers.request && listCandidates` and `capabilities.labels`.
- **Reactions.** Eight pills, picker `aria-label` "Add a reaction", pills `aria-pressed` and `aria-label` "<name>, <count>", optimistic overlay cleared when the host's signature changes, rollback + toast on failure; read-only pills when `capabilities.reactions` is not true.
- Keep new views in new contract files; do not grow `app.contract` beyond the Preview argument.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Comment and close-with-comment | Fake gh, profile `admin-reviewer`, open PR | Type, ⌘↵ via `type`/`key`; then "Close with comment" | Logged `pr comment N --body-file -` body equals typed text; then `pr close N`; Summary shows the comment; failure of close keeps the comment and toasts | macOS, 1280×840 | `calls.ndjson`, tree JSON |
| Review verdicts | Profiles `admin-reviewer`, `admin-author` | Submit Comment/Approve/Request changes with a summary | `POST …/pulls/N/reviews` body has the event and summary; toasts as listed; author sees Comment only; empty Comment disabled | macOS | log, shots |
| Pending store | Unit plus popover | Add and discard through the store API (the Code tab fills it later) | Badge "Review (n)", discard clears; summary kept across close/reopen and PR switch | macOS | tests, shots |
| Title/description/comment edit | Own and others' comments | Edit each; unchanged title; Escape; failure with `failNext` | Mutation variables logged; no call when unchanged; editor and text retained on failure; pencil absent on others' comments and on reviews | macOS | log |
| Preview and collapse | 40-line comment | Preview toggle; "Show full comment" | Same render as the saved body; collapse at 240 pt | macOS | shots |
| Reviewers | Profiles `writer`, `reader` | Pick, unpick, search | `POST`/`DELETE …/requested_reviewers` bodies; toasts; reader's button disabled with the tooltip | macOS | log |
| Labels | Profiles `triage`, `reader` | Apply, remove (name with space and slash) | `POST …/labels`, `DELETE …/labels/<encoded>`; no success toast; failure title "Could not put … on" | macOS | log |
| Reactions | Comment with two reactions | Add, remove, fail once | Optimistic change, `addReaction`/`removeReaction` content enum, rollback on failure; tooltip "You, A and 2 others reacted with … emoji" | macOS | log, shots |
| Visual and trace | Oracle, same fixture | Pairs for composer, editors, pickers, pills at 1280×840 and 840×620, light and dark; `target/t3-ui-parity/trace-diff.mjs pr-writes` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; RPC payloads equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "canEditPullRequestChangeRequest", "canEditPullRequestComment", "reaction presentation", "reaction tooltip", "reaction tooltip, with a reaction in flight", "pending reactions", the review-store cases, "updates cached labels after successful edits without rereading the host", "updates reviewer requests and enriched reviewers without rereading the host", "refreshes pull request activity after a comment is updated" | Pass | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab, arrows and Return through the composer popover, editors and pickers; Escape in each (popover closes, editor cancels unless saving, picker closes) | Focus visible and ordered; focus returns to the trigger; icon buttons named; pencil and popover appear without fade or scale when reduced | macOS | `tree --ax`, state |
| `(attended session)` | Real keyboard, hardware hover; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Korean 2-Set typing and Enter mid-composition in the comment box; hover reveals pencil/reaction tooltips; ⌘↵ | No submit during composition; bytes exact; reveals work | macOS | recorded steps |
| Gates | `git add -A` | Clone checks; `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.ts`, new `pages-pr-writes.ts` (+ tests), `pages-pr-compose.contract`, `pages-pr-edit.contract`, `pages-pr-meta.contract`, `app.contract` (Preview argument), `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, fake gh, reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Progress

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-pr-conversation-and-refresh` and `20261005-fake-github-fixture` merge.
