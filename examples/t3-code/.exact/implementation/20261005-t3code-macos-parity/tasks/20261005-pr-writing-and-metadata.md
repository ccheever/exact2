---
name: 20261005-pr-writing-and-metadata
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-writing-and-metadata
pr_url: PR_URL_PENDING
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
host actions (`20261005-pr-header-actions-and-stacks`); Forgejo's required-summary rule is ported, other hosts are not run live.
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
| merged task PR | [20261007-real-github-lane](closed/20261007-real-github-lane.md) | pending | Probe rows for every write confirmed by GitHub read-back; the second account for reviewer requests | pending |
| scheduling preference | [20261005-pr-header-actions-and-stacks](20261005-pr-header-actions-and-stacks.md), [20261005-main-fix-adoption](closed/20261005-main-fix-adoption.md) | pending | Close/Reopen with comment calls the same `pr-act-action` path that `20261005-pr-header-actions-and-stacks` upgrades; merge that one first if both are open | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over `EXACT2-GAPS.md` and `../issues/` drafts (not reproduced, not searched upstream). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | ⌘↵ and Escape in a textarea; composition guard (`isComposing`/229) | macOS Korean 2-Set | nonblocking (workaround: `R9Input`/`T3KeyRecorder` pattern) | Prove in an attended session |
| [X16](../issues/closed/20261005-x16-smart-substitutions-off.md) | Exact bytes typed in a textarea | `t3-plain-text` hook | nonblocking (workaround exists) | Use the hook on every editor 2026-10-07: adopted (#111, main #160): `autocorrect="off"` on every textarea; the `t3-plain-text` hook is gone (adopt-main-fixes-input). |
| [X17](../issues/20261005-x17-popover-position-try.md) | Composer popover opens above, end-aligned; picker popups flip | fixed placement | nonblocking (workaround: fixed placement) | Declare near-edge difference |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Preview needs a root resource argument (`app.contract:101`) | +3 lines | nonblocking | Keep the addition minimal |
| [X21](../issues/20261005-x21-two-way-websocket.md) | RPC send | Swift transport | nonblocking | Reuse `client.rpc` |
| [X16](../issues/closed/20261005-x16-smart-substitutions-off.md) | Exact typed bytes in the PR title, description, comment and review textareas | X16 | nonblocking (workaround: the `t3-plain-text` hook on these textareas) | remove the hook when X16 is adopted 2026-10-07: adopted (#111, main #160): `autocorrect="off"` on every textarea; the `t3-plain-text` hook is gone (adopt-main-fixes-input). |

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
| Comment and close-with-comment | Sandbox, the primary account on a second-account pull request | Type, ⌘↵ via `type`/`key`; then "Close with comment" | Logged `pr comment N --body-file -` body equals typed text; then `pr close N`; Summary shows the comment; failure of close keeps the comment and toasts | macOS, 1280×840 | `logs/gh-calls.tsv`, GitHub read-back, tree JSON |
| Review verdicts | The primary account on `second-review` (reviewer) and on its own pull request (author) | Submit Comment/Approve/Request changes with a summary | GitHub shows the review's state and summary (read back with the lane gh); toasts as listed; author sees Comment only; empty Comment disabled | macOS | log, shots |
| Pending store | Unit plus popover | Add and discard through the store API (the Code tab fills it later) | Badge "Review (n)", discard clears; summary kept across close/reopen and PR switch | macOS | tests, shots |
| Title/description/comment edit | Own and others' comments | Edit each; unchanged title; Escape; failure by unit test with an injected failure | Mutation variables logged; no call when unchanged; editor and text retained on failure; pencil absent on others' comments and on reviews | macOS | log |
| Preview and collapse | 40-line comment | Preview toggle; "Show full comment" | Same render as the saved body; collapse at 240 pt | macOS | shots |
| Reviewers | Live: the primary asks the second account and the second asks the primary; `reader` by unit test | Pick, unpick, search | `POST`/`DELETE …/requested_reviewers` bodies; toasts; reader's button disabled with the tooltip | macOS | log |
| Labels | Live with the primary account; `triage` and `reader` by unit test | Apply, remove (name with space and slash) | `POST …/labels`, `DELETE …/labels/<encoded>`; no success toast; failure title "Could not put … on" | macOS | log |
| Reactions | Comment with two reactions | Add, remove, fail once | Optimistic change, `addReaction`/`removeReaction` content enum, rollback on failure; tooltip "You, A and 2 others reacted with … emoji" | macOS | log, shots |
| Visual and trace | Oracle, same fixture | Pairs for composer, editors, pickers, pills at 1280×840 and 840×620, light and dark; `target/t3-ui-parity/trace-diff.mjs pr-writes` | Every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link; RPC payloads equal | macOS | pair table, diff |
| Ported tests | `bun test` | Original names: "canEditPullRequestChangeRequest", "canEditPullRequestComment", "reaction presentation", "reaction tooltip", "reaction tooltip, with a reaction in flight", "pending reactions", the review-store cases, "updates cached labels after successful edits without rereading the host", "updates reviewer requests and enriched reviewers without rereading the host", "refreshes pull request activity after a comment is updated" | Pass | macOS | log |
| Keyboard focus, Escape, reduced motion | `prefer prefers-reduced-motion reduce` | Tab, arrows and Return through the composer popover, editors and pickers; Escape in each (popover closes, editor cancels unless saving, picker closes) | Focus visible and ordered; focus returns to the trigger; icon buttons named; pencil and popover appear without fade or scale when reduced | macOS | `tree --ax`, state |
| `(attended session)` | Real keyboard, hardware hover; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Korean 2-Set typing and Enter mid-composition in the comment box; hover reveals pencil/reaction tooltips; ⌘↵ | No submit during composition; bytes exact; reveals work | macOS | recorded steps |
| Gates | `git add -A` | Clone checks; `bun scripts/caps.mjs`; five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `examples/t3-code/pages-pr-detail.ts`, new `pages-pr-writes.ts` (+ tests), `pages-pr-compose.contract`, `pages-pr-edit.contract`, `pages-pr-meta.contract`, `app.contract` (Preview argument), `AGENT-HANDOFF.md`.
Required environment: macOS 26.6.2, Xcode 27.0, Bun 1.4.2, the real-GitHub lane (sandbox, shared lane config dirs, two accounts; `tools/github-lane`), reference oracle. Attended and normal-launch rows use a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (see `20261005-embedded-server-runtime`).

## Results

Built on `feat(example)/t3-code` (prepared at `bdca4216a`, merged up to `e91fcfc65`). Prerequisites: real-github-lane (#233),
pr-conversation-and-refresh (#247) and hot-file-split are merged; desktop-oracle-and-trace is not built (user decision
2026-10-06), so its rows are "not run". pr-header-actions-and-stacks runs in parallel: Close/Reopen with comment calls
`pullRequests.runAction` from `pages-pr-writes.ts` with the reference's strings; reconcile with its upgraded action path
when it merges. `app.contract` is untouched (the Preview argument was not needed: the preview is one more document of
the existing `prMarkdown` resource).

**What was built.**
- Composer (`pages-pr-compose.contract`, `pages-pr-writes.ts`): the round trigger at the panel's bottom right
  (`aria-label` "Comment on pull request" / "Review pull request" / "Review pull request, N comments pending", the
  count badge), the popover "Pull request composer" above it, end-aligned, with the "Composer mode" toggle (Comment /
  Review (n)) or the single mode's title, "Discard pending line comments" and "Close composer". Comment: "Leave a
  comment", ⌘↵, Comment ("Posting..."), Close with comment ("Closing...") / Reopen with comment ("Reopening...") where
  the viewer may; the box is locked while posting, cleared and the popover closed when the comment lands (the popover
  is rebuilt when the write's serial arrives); "Could not post the comment" keeps the words and the popover; a refused
  close or reopen keeps the posted comment and toasts the action's failure with the host's reason. Review: "Summarize
  your review (optional)" (or "(required to request changes)" on Forgejo), the verdict `select` from the viewer's
  allowed set (an author sees Comment only), Submit review ("Submitting...") disabled per `canSubmit`, the summary and
  the pending line comments sent in one `pullRequests.submitReview`, the toasts "Review submitted" / "Pull request
  approved" / "Changes requested" / "The review could not be submitted" (the draft kept). The composer is absent where
  neither a comment nor a verdict is allowed. Both forms stay mounted across the toggle.
- The review store (`pages-pr-writes-logic.ts` `PullRequestReviewStore`, per client, keyed by `pullRequestReviewKey`,
  not persisted): line comments (the Code tab will add them), the summary, `removeComments` of the submitted snapshot
  only, `clearSummary` only of the exact words sent. The comment box's text is kept beside it. The boxes send their
  words when they lose the focus (`chatlocal:prw-draft-*`, beside the writes' one-at-a-time route: attempt 1 found a
  blur-caused draft write holding back the Submit press behind it), and a late draft never brings back words already sent.
- Editing (`pages-pr-edit.contract`): the pencil (`PrdEditButton`, "Edit title" / "Edit description" / "Edit comment",
  revealed on hover or its own focus, no fade under reduced motion); the title as a one-line field (Enter saves,
  Escape cancels, an unchanged or empty title sends nothing, trimmed; "The title could not be saved" with the host's
  reason keeps the field); the description and the reader's own remarks in the Write/Preview editor ("Markdown editor
  mode", "Nothing to preview.", ⌘↵ saves, Escape cancels unless saving, Cancel, Save / "Saving..."; a description may be
  emptied, a remark may not; "Could not save the description" / "Could not save the comment" keep the editor). Preview
  sends the draft once (`preview`) and renders it through the pull request's own markdown resource as one more
  document (`pr-preview:<editor>`). One markdown slot per site shows the saved words or the preview
  (`PrdWords`), so the plan grew by 1,317 nodes, not 25,000 (the first cut, with a separate editor, made 95,638 nodes
  and 18.1 MB). `canEdit` is now `canEditPullRequestChangeRequest` (`edit.changeRequest === true` and the author or
  someone who may merge), the planning correction. Pencils on remarks follow `canEditPullRequestComment` (the
  viewer's own issue or review comment; never a review's summary).
- Long remarks (`PrdWords`, PullRequestCommentBody): a Summary remark taller than 240 pt is held behind a fade with
  "Show full comment" / "Show less" (`aria-expanded`, `aria-controls`; "Show less" brings the frame back into view).
- Reactions (`pages-pr-meta.contract` `PrdReactionBar`): the pills (`aria-pressed`, `aria-label` "<name>, <count>",
  the tooltip "You, A and 2 others reacted with … emoji"), and where `capabilities.reactions` is true the "Add a
  reaction" picker of the eight (closing on a pick); read-only pills otherwise; on Summary cards, collapsed finished
  remarks, Timeline conversation cards and verdict rows. A press shows at once (the overlay keyed by the host's
  signature, `applyPendingPullRequestReactions`), is forgotten when the host's counts land, and is rolled back with
  "The reaction could not be saved" when refused.
- Pickers (`PrdCandidatePicker`): "Request a review" beside the reviewer faces (where the host can request and list
  candidates) and "Change labels" beside the labels (where the host can change labels); disabled with the reason
  ("Asking someone to review needs write access on this repository", "Changing labels needs triage access on this
  repository") where the viewer may not. Candidates are read when the menu opens and kept a minute; the people ghost
  while they load; "<errorLabel> <host's words>" on a failure (a later open reads again); empty, no-match and truncated
  notes; search narrows only what arrived; the arrows move the highlight and Enter picks; the menu stays open on a
  pick and every row locks while one change is out. Reviewer toasts "Review requested from <login>", "Review request to
  <login> taken back", "Could not ask <login> for a review" / "Could not take back the review request to <login>";
  labels toast only their failure ("Could not put <label> on", "Could not take <label> off"). Both patch the panel's
  detail (and the activity's reviewers, keeping one who already reviewed) and the candidate list in place, as the
  reference's cache patches do, without reading the host again; a reviewer missing from the candidates makes a read.
- Found and fixed on the way (pages-prs.ts): one Refresh press sent `pullRequests.invalidate {}` 141 times in 3 s,
  with ~50 detail and list reads (each invalidate announced a change that asked the list again while its read was out,
  and that read invalidated again); now one invalidate per press (test "one Refresh press invalidates once…", failing
  before: 3 invalidates for three overlapping asks).
- The old, never-wired `prCandidates` and the `comment`/`title`/`label`/`request-review` ops of `prCommand` are gone
  (their replacements are `pages-pr-writes.ts`'s).
- Lane: `seed.mjs` gains `writes-second` (#158, `feature/vowel-count`, the second account's) and `writes-primary` (#159,
  `docs/vowel-count`, the primary's) and the label `area/docs and help`.

**Acceptance.**

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Comment and close-with-comment | pass (live + unit) | [10](https://raw.githubusercontent.com/ccheever/exact2/f05464e90aaa6c6f121aca841bb360bf4e9a7669/pr-writing-and-metadata/10-composer-comment.png), [16](https://raw.githubusercontent.com/ccheever/exact2/29020050bc9b32f616e608d83b9a29e130573c0a/pr-writing-and-metadata/16-close-reopen.png); [record](https://raw.githubusercontent.com/ccheever/exact2/a73ad8f647cd5f63a98a3e78d8a33823d1c26cbf/pr-writing-and-metadata/record-attempt2.txt): the typed bytes (a backtick, an em dash, quotes, a blank line) read back equal from GitHub; gh `pr comment 158 --repo … --body-file -`, then `pr close 158` / `pr reopen 158`; toasts "Pull request closed"/"reopened". Unit: "a failed comment keeps the words and the popover…", "close with comment … a refused close keeps the comment and explains" | — |
| Review verdicts | pass (live + unit) | [15](https://raw.githubusercontent.com/ccheever/exact2/85fb7ce3eb5e1b945bb827cbd4394984e43f5f90/pr-writing-and-metadata/15-reviews.png), [17](https://raw.githubusercontent.com/ccheever/exact2/9707fd9d419e558926b97794073d0a8b3bc11c6d/pr-writing-and-metadata/17-author-review.png), [read-back](https://raw.githubusercontent.com/ccheever/exact2/0b2f16dcc4598df5a0ea91a3cccff2ead4241910/pr-writing-and-metadata/readback-after.txt): COMMENTED, APPROVED, CHANGES_REQUESTED by the primary on #158 with their summaries (`POST …/pulls/158/reviews`); the author on #159 is offered Comment only, Submit disabled while the summary is empty. Unit: the three toasts, "an empty Comment is not sent, an empty approval is", "a failed review keeps the summary and the line comments" | — |
| Pending store | pass (unit; live in part) | "pull request review drafts" (4 ported cases), "the count, the line comments sent with the review (without their ids), discard, and the summary across pull requests"; live: a comment typed and left on #158 was still in the box after #159, #115, #159 and back ([22](https://raw.githubusercontent.com/ccheever/exact2/f86519cef203d98c455c2aae04071a8b08ce86a4/pr-writing-and-metadata/22-dark-and-narrow.png), "840 composer") | the badge and "Review (n)" with real pending line comments need the Code tab (20261005-pr-code-tab fills the store) |
| Title/description/comment edit | pass (live + unit) | [03](https://raw.githubusercontent.com/ccheever/exact2/013575d6b903101c0223bf4e105e7127769c2bc5/pr-writing-and-metadata/03-title-pencil.png), [18](https://raw.githubusercontent.com/ccheever/exact2/30895da2485ff9d97751b299e0a2c2efd661d4ba/pr-writing-and-metadata/18-title-edit.png), [19](https://raw.githubusercontent.com/ccheever/exact2/6ea267c8c2c7dbd2e079cb2a9f68441b82635698/pr-writing-and-metadata/19-description-edit.png), [11](https://raw.githubusercontent.com/ccheever/exact2/7c181381503fd18337180471c8db67688423d4e1/pr-writing-and-metadata/11-comment-edit.png); record: an unchanged title sent no `pullRequests.update` (server trace 0); the edited title and description read back from GitHub, then restored; the comment edit read back; Escape closed the editor with 0 `updateComment`; no pencil on the second account's remark. Unit: "the title: trimmed, sent once; unchanged or empty sends nothing; a failure keeps the editor…", "the description: sent verbatim, empty allowed…", "a comment: the pencil on the reader's own remarks only…" | — |
| Preview and collapse | pass (live) | [11](https://raw.githubusercontent.com/ccheever/exact2/7c181381503fd18337180471c8db67688423d4e1/pr-writing-and-metadata/11-comment-edit.png) and [19](https://raw.githubusercontent.com/ccheever/exact2/6ea267c8c2c7dbd2e079cb2a9f68441b82635698/pr-writing-and-metadata/19-description-edit.png) (Preview renders through the pull request markdown), [12](https://raw.githubusercontent.com/ccheever/exact2/9fcc00e79010d81f975318bd7294c023fc54a55e/pr-writing-and-metadata/12-collapse.png): the 40-line remark held at 240 pt (`layout` h 240), "Show full comment" → "Show less" | focus inside the held body does not expand it: X54 (no `focusin`), "Show full comment" is a Tab stop instead |
| Reviewers | pass (live + unit) | [20](https://raw.githubusercontent.com/ccheever/exact2/52e551ae3ec3725b974463aea1bc960cceb7feaa/pr-writing-and-metadata/20-reviewers.png): the primary asked daehyeon-mun on #159 and took it back (`api --method POST/DELETE … pulls/159/requested_reviewers --input -`, read back `["daehyeon-mun"]` then `[]`); the second account asked the primary on #158 through its own lane server (16761) and took it back (read back `["daehyeonmun2021"]` then `[]`). Unit: "updates reviewer requests and enriched reviewers without rereading the host", the reader's disabled button and reason | the second account's side ran through its server's RPCs, not a second app session (one live session, user rule 3) |
| Labels | pass (live + unit) | [14](https://raw.githubusercontent.com/ccheever/exact2/c9075d1c376f1e5fd15d3e95f620c3fbe61158d7/pr-writing-and-metadata/14-labels.png); attempt 1: "area/docs and help" applied (`api --method POST … issues/158/labels --input -`) and taken off (`api --method DELETE … issues/158/labels/area%2Fdocs%20and%20help`), read back each time, the menu stayed open, no success toast; attempt 2: the host's label read took 30.4 s and the menu showed "The labels could not be read. The server did not confirm the request…". Unit: "updates cached labels after successful edits without rereading the host", the failure titles, triage/reader | — |
| Reactions | pass (live + unit) | [04](https://raw.githubusercontent.com/ccheever/exact2/0c790b58c618cb07bd34f6b36e743b0d06f029bf/pr-writing-and-metadata/04-reactions-summary.png), [05](https://raw.githubusercontent.com/ccheever/exact2/adbcb2bf7275d0453a7bdfadca51becad825fb99/pr-writing-and-metadata/05-reactions-timeline.png), [13](https://raw.githubusercontent.com/ccheever/exact2/2316889cefda6ba7e42002a95140050ac38dacb4/pr-writing-and-metadata/13-reaction-picker.png): on #115's comment with two reactions the primary joined the heart (read back `heart daehyeonmun2021`), the tooltip read "You and daehyeon-mun reacted with heart emoji", left it (read back as seeded); on its own comment heart from the picker, then off. Unit: the ported "reaction presentation/tooltip/…/pending reactions", "a press shows at once…", "a refused press is rolled back and says so" | — |
| Visual and trace | not run | — | user decision 2026-10-06: the desktop oracle and trace tools are not built; before/after pairs from the feature head instead (images 01–07) |
| Ported tests | pass | `pages-pr-writes-logic.test.ts` (32): "canEditPullRequestChangeRequest", "canEditPullRequestComment", "reaction presentation", "reaction tooltip", "reaction tooltip, with a reaction in flight", "pending reactions", "pull request review drafts"; `pages-pr-writes.test.ts` (26) with "updates cached labels after successful edits without rereading the host", "updates reviewer requests and enriched reviewers without rereading the host", "refreshes pull request activity after a comment is updated" | — |
| Keyboard focus, Escape, reduced motion | pass (agent); real keys deferred to the real-input batch — screen locked (user away) | record: the composer opens with the focus in its box; Escape closes it and the focus returns to the trigger; the label menu opens with the focus in its search; Enter picks the highlighted row (unit and attempt 1); Escape cancels the editors; [21](https://raw.githubusercontent.com/ccheever/exact2/f3ccb50cd4ee12e2ae958abf00828473964c96b4/pr-writing-and-metadata/21-reduced-motion.png): the pencil mid-fade at +40 ms by default, whole at +0 ms with `prefers-reduced-motion: reduce` (the popovers have no fade or scale) | real Tab order and the focus ring: real-input batch (steps below) |
| (attended session) | deferred to the real-input batch — screen locked (user away) | — | Korean 2-Set typing and Enter mid-composition, hardware hover, real ⌘↵: steps below |
| Gates | see the PR | numbers in the PR body | — |

**Live drives** (agent mode, 1280×840 then 840×620, the branch's development build paired with the primary lane server on
127.0.0.1:16760; the second account's server on 16761). Attempt 1 ([record](https://raw.githubusercontent.com/ccheever/exact2/385c6fb17c332f4e2c2e4d0618d65ec6272259f7/pr-writing-and-metadata/record-attempt1.txt),
[script](https://raw.githubusercontent.com/ccheever/exact2/71be665838b65d1fd017bd45d2254d00cba0f325/pr-writing-and-metadata/drive-attempt1.mjs.txt)) found two bugs, fixed before the retry: a box's draft, saved when it lost the
focus, went through the writes' one-at-a-time route, so the blur a press on Submit review caused held that press back
(no review was sent); and one Refresh press made 141 `pullRequests.invalidate` (the list re-invalidated while its read was
out). Attempt 2 ([record](https://raw.githubusercontent.com/ccheever/exact2/a73ad8f647cd5f63a98a3e78d8a33823d1c26cbf/pr-writing-and-metadata/record-attempt2.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/f919f63f936aa834431e553fc14b16111776435d/pr-writing-and-metadata/drive-attempt2.mjs.txt)) passed the composer,
the edits, the collapse, the reactions, the reviews, close and reopen, the reviewers both ways and the reduced-motion
check; GitHub answered slowly (an activity read 18 s, a label read 30.4 s, `#158` missing from a search right after it
was reopened), which failed its label step (attempt 1 had passed it) and the scripted draft round trip (the draft was
seen kept later in the same session). The before images come from the feature head (`757d9517a`) on the same lane
([record](https://raw.githubusercontent.com/ccheever/exact2/d53677cf2e4c1938e8667642e57ade7fbab4b9b2/pr-writing-and-metadata/record-before.txt)). Cleanup: every comment the drives made was deleted, reactions, labels, reviewer
requests, the title and the description are as seeded ([read-back](https://raw.githubusercontent.com/ccheever/exact2/0b2f16dcc4598df5a0ea91a3cccff2ead4241910/pr-writing-and-metadata/readback-after.txt)); the three submitted
reviews on #158 stay (GitHub keeps them).

**Decisions.** Provisional, user decision pending: none. The comment box's draft outlives a visit to another pull
request (the task's implementation note; the reference keys the composer by the pull request and loses it).

## Real-input batch steps

Deferred rows (screen locked while the user is away, 2026-10-08): Korean 2-Set typing and Enter mid-composition in the
comment box, real ⌘↵, hardware hover over the pencils and reaction pills, real Tab and the focus ring through the
composer, editors and pickers. One session:

1. Worktree `t3-code-pr-writing-and-metadata` at the PR head. `export PATH="$HOME/.bun-1.4.2/bin:$PATH"
   EXACT_APP_DIR="$PWD/examples/t3-code"`; `bun examples/t3-code/stage-runtime.mjs` if `server-runtime/*.tar.gz` is missing;
   `bun examples/t3-code/terminal-host/build.mjs`; `bun host/apple/build.mjs t3-code-macos --bundle`.
2. Lane: `cd examples/t3-code/tools/github-lane`, `export T3_GITHUB_LANE_SHARED=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-ui-parity/github-lane`,
   `bun seed.mjs --only writes-second,writes-primary`, `bun lane.mjs start primary --port 16760`, `bun lane.mjs project primary`,
   `bun lane.mjs pair primary` (the single-use URL goes to `target/github-lane/servers/primary/pairing-url`, 0600; never print it).
3. Copy `target/clients/<id>/com.exact.t3code.macos/macos/T3 Code (Exact).app` to `target/github-lane/realinput/T3 Code (Lane PRW).app`;
   PlistBuddy: `CFBundleIdentifier` `com.exact.t3code.laneprw`, `CFBundleName`/`CFBundleDisplayName` `T3 Code (Lane PRW)`;
   `codesign --force --deep --sign <the Apple Development identity build.mjs signs with>`. Take the shared real-input lock
   (owner "pr-writing-and-metadata: real-input batch"). Launch by path with `CFFIXED_USER_HOME=$PWD/target/github-lane/realinput/home`;
   record the pid and the window id (`orca computer list-windows --app pid:<pid> --json`).
4. Pair: the welcome wizard's Pairing link field by `set-value --value-stdin` from the `pairing-url` file, Pair, Continue.
   Pull Requests, `#158` in the search (`set-value`), click the row.
5. **Korean 2-Set in the comment box**: click the round button at the panel's bottom right; with the Mac's Korean 2-Set
   source type `gksrmf` (한글) and press Return while the last syllable is still composing: no comment is posted (read
   back `gh api repos/daehyeonmun2021/playground/issues/158/comments` with the lane gh: unchanged); finish the syllable,
   ⌘↵: one comment whose body is exactly `한글` (read back). Delete it with the lane gh.
6. **Hover**: move the real pointer over the description, over the primary's own comment and over a reaction pill on
   #115's "Thanks for this…" (search `#115`): the pencils appear on hover (no fade with System Settings › Reduce motion,
   only with the user's permission), the pill's tooltip "daehyeon-mun reacted with heart emoji" after about 0.6 s.
7. **Tab and the focus ring**: in the composer, Tab moves Comment/Review → close → the box → Close with comment →
   Comment, with a visible ring; Escape closes it and the ring is on the round button. Open "Change labels": the
   search has the focus; ↓/↑ move the highlight, Return applies (read back, then press the row again to take it off);
   Escape closes the menu with the focus on its button. Pencil → the editor's Write/Preview, Cancel, Save in order.
8. Cleanup: quit the copy; `bun lane.mjs stop primary`; delete the copy's Keychain item
   (`security delete-generic-password -s com.exact.t3code.macos.access-token -a "$(printf 'http://127.0.0.1:16760\n%s' "$(cat target/github-lane/servers/primary/t3home/userdata/environment-id)")"`);
   remove `target/github-lane/realinput`; release the lock.

## Progress

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented on `feat(example)/t3-code-pr-writing-and-metadata`; unit tests, one live drive and one retry on the
real-GitHub lane; draft PR (front matter). The real-input rows wait for the batch (steps above): the screen was locked.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (live drive) | WIP `adf9b95a5` | comment (exact bytes), labels (apply/remove, the encoded DELETE), reactions on #115, title, description, reviewers: pass; reviews: none sent (a blur-caused draft write held Submit back); comment edit and own reactions: the comment appeared after an 18 s host read, after the steps ran; a Refresh press made 141 invalidates | [record](https://raw.githubusercontent.com/ccheever/exact2/385c6fb17c332f4e2c2e4d0618d65ec6272259f7/pr-writing-and-metadata/record-attempt1.txt), images 14 (search, applied) | fixed in attempt 2 |
| 2 (retry) | `5354b70d0` before the reaction picker's opaque fill | composer, edit, preview, collapse, reactions, three verdicts, close/reopen, reviewers both ways, reduced motion: pass; labels: the host's read took 30.4 s (the menu showed the failure); the scripted draft round trip: `#158` missing from a search right after its reopen (the draft was seen kept later) | images 01–22, [record](https://raw.githubusercontent.com/ccheever/exact2/a73ad8f647cd5f63a98a3e78d8a33823d1c26cbf/pr-writing-and-metadata/record-attempt2.txt), [read-back](https://raw.githubusercontent.com/ccheever/exact2/0b2f16dcc4598df5a0ea91a3cccff2ead4241910/pr-writing-and-metadata/readback-after.txt) | real-input batch (steps above) |

## Next action

Review. Owed: the real-input batch (steps above); the oracle and trace rows wait on the user decision; reconcile Close/Reopen
with comment with pr-header-actions-and-stacks' action path when it merges.
