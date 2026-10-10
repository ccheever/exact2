---
name: 20261008-pr-list-title-clip
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-pr-list-title-clip'
pr_url: https://github.com/ccheever/exact2/pull/289
verified_commit: 74e953039
---

# A Pull Requests row keeps the start of an edited title

## Outcome

Found by #261 ([pr-writing-and-metadata](20261005-pr-writing-and-metadata.md), image 18): after a
pull request's title was edited, its Pull Requests row read "#159 cribe the vowel count…" instead of
the title's start. The row must truncate at the end, as the reference's does.

## Scope and exclusions

Included: the cause, the row's fix as the reference has it, the same fix on the other pull request
buttons whose truncating text inherits the button's centre, a regression test that fails on the base,
one live drive on the real-GitHub lane with before/after images. Excluded: framework edits (the macOS
host's half is X57, reported, not changed), buttons outside the pull request surfaces (listed in X57).

## Cause

Two halves, both reproduced:

- **The clone.** `PrRowButton` (`pages-prs.contract`) is a `button`, and a button's text is centred
  (the UA sheet's `text-align: center`, LLP 1001 §1). `layout <title>` on the base build:
  `text_align: center`, `source: inherited`, `from:` the row button. The reference's row button says
  `text-left` (`PULL_REQUEST_ROW_CLASS`, `apps/web/src/components/pullRequest/PullRequestListRow.tsx:28`);
  the clone's port left it out.
- **The macOS host (X57).** A line wider than its box should be start-aligned whatever `text-align`
  says (CSS Text 3 §7.1; Chrome 151 does it under `center`). The host places it at
  `CTLineGetPenOffsetForFlush` of the full line, which is negative for a centred over-wide line, and
  draws the ellipsized line there: the start is cut, the "…" comes early, a gap follows it. So the
  inherited centre was invisible while a title fit its column and cut its start once it did not.
  Editing was incidental: #149 ("Link the FAQ from the usage notes", 33 characters) read
  "#149 k the FAQ from the us…" ("Lin" and half the "k" cut) on its first read in the same column, no edit. One-file repro, macOS
  beside web: [X57](../../issues/closed/20261008-x57-overflowing-centred-line.md).

Alignment it was; not a scroll offset (the title is a paragraph, which never gets a scroll view,
`NodeViewMac.swift`) and not the truncation itself (the ellipsis is made at the box's width).

## What was built

- `PrRowButton`: `text-align="left"` on the row button, as `PULL_REQUEST_ROW_CLASS` says `text-left`;
  the title, author, repository, labels and age inherit it.
- `PrdConversationGroup`'s toggle (`pages-pr-timeline.contract`): `text-align="left"`, as the
  reference's `CollapsibleTrigger` says `text-left` (`PullRequestTimelineTab.tsx:323`); its authors
  line truncates.
- `PrdCopy` (`pages-pr-detail.contract`, the copyable head branch and `gh pr checkout` command):
  `text-align="left"` on the button, as `PullRequestCopyableCode.tsx:35` says `text-left` (found by the
  independent review; its text is `line-clamp=1`).
- `PaFreshnessMark`'s base name (`pages-pr-actions.contract`) and `PrSubTrigger`'s value (the filter
  submenu rows, `pages-prs.contract`): `text-align="left"` on the text. The reference has no
  `text-left` there (a Base UI menu row is no button; the freshness trigger), but Chrome start-aligns an
  overflow and a fitting text is as wide as itself, so this is the reference's rendering until X57.
- Test (`pages-prs.test.ts`): "every single-line text that can overflow inside a pull request button is
  start-aligned, as PULL_REQUEST_ROW_CLASS says text-left" walks each `pages-pr*.contract` text with an
  ellipsis, `line-clamp=1` or `nowrap` + `overflow="hidden"` up to its button; the nearest `text-align`
  on the way (the button's own included) must be left (18 found). On the base contracts it fails with 8:
  `pages-pr-actions.contract:284`, `pages-pr-detail.contract:373`, `pages-pr-timeline.contract:230`,
  `pages-prs.contract:388`, `:515`, `:533`, `:534`, `:538`.
- X57 (issue file, `issues/README.md`, `EXACT2-GAPS.md`). `issues/README.md` also loses the
  `<<<<<<<`/`>>>>>>>` markers a merge left in it on the base (both sections kept).

## Acceptance and reproduction

Lane (not committed): the primary real-GitHub lane server (`daehyeonmun2021`) on 127.0.0.1:16300 with
the sandbox project (`target/github-lane`, this worktree; `lane.mjs`'s `start('primary', 16300)`,
since its CLI takes 16500-16799 only); the app in agent mode at 1280×840 with an isolated `HOME` and
`--storage`, paired by a single-use token (never printed). Before: the base build (`t3-code-evidence-base`
at `07dcef1ab`, copied out of the lock). After: this branch's build. Same drive
([drive.mjs](https://raw.githubusercontent.com/ccheever/exact2/8ce115ba5423e90e3e59bd93c49f37ee03a8fe58/pr-list-title-clip/drive.mjs.txt)):
open Pull Requests, search `#159`, open it (the list becomes the column beside the panel); rename #159
to "Describe the vowel counter (edited)" with the lane gh (`PATCH pulls/159`), press the list's Refresh;
then `#149` in the same column; finally restore the title and read it back.

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Cause found | pass | base `layout`: the title's `text_align` `center`, inherited from `pr-row-159`; branch: `left`, inherited from the row ([before](https://raw.githubusercontent.com/ccheever/exact2/4356a37977b0821ede3b63f5d388c5abdd676631/pr-list-title-clip/record-before.txt), [after](https://raw.githubusercontent.com/ccheever/exact2/7743b3792cece4728bcf787f910fd0ed355abb39/pr-list-title-clip/record-after.txt)); one-file app [03](https://raw.githubusercontent.com/ccheever/exact2/0d0833be4ef61ea1abf087307568f953cc7cdee7/pr-list-title-clip/03-x57-one-file-app.png); Chrome 151 [04](https://raw.githubusercontent.com/ccheever/exact2/d36af12402ccb5430fd2fd323214d57a81375cf6/pr-list-title-clip/04-chrome-151-same-css.png) | — |
| Edited title truncates at the end (real GitHub) | pass | [01](https://raw.githubusercontent.com/ccheever/exact2/9023d66207e11eeb61bfeca7886d106f3c1dc58f/pr-list-title-clip/01-edited-title-row.png): before "#159 cribe the vowel count…", after "#159 Describe the vowel count…"; GitHub read back "Describe the vowel counter (edited)" before the Refresh | — |
| Long title on its first read | pass | [02](https://raw.githubusercontent.com/ccheever/exact2/ad52d754fd147b514fb0226c8f24ac7d5efaf381/pr-list-title-clip/02-long-title-row.png): #149 before "k the FAQ from the us…" (half a "k"), after "Link the FAQ from the us…" | — |
| A title that fits is unchanged | pass | [00](https://raw.githubusercontent.com/ccheever/exact2/da137e40f521847dbde8c76b689b6d592122b3a7/pr-list-title-clip/00-unedited-row.png) (control) | — |
| Matches the reference | pass | `text-left` on `PULL_REQUEST_ROW_CLASS`, the timeline trigger and `PullRequestCopyableCode`, read at `1e2ecbd975`; the freshness mark and the submenu value as Chrome renders them | — |
| Regression test fails on the base | pass | the `pages-prs.test.ts` test above: base 8 offenders, branch 0 of 18 | — |
| Copy buttons unchanged where they fit | pass | 00 (`gh pr checkout 159`, `docs/vowel-count` in both) | — |
| Sandbox restored | pass | both records end "restore title: HTTP 200; read back \"Describe the vowel counter\"" | — |
| Visual oracle and trace | not run | — | user decision 2026-10-06: the desktop oracle and trace tools are not built; before/after pairs instead |
| Other clone buttons with an inheriting truncating text | not changed | six ellipsis sites and about 60 `line-clamp=1` texts outside the pull request surfaces, listed in X57 | out of this task's scope; X57 (framework) makes them right without edits |

Real input: none needed (a rendering bug; agent screenshots draw the same paragraph layers).

Seen on the way, not in this task: at the retry's start (its storage kept from the first after session) the
panel's title read "Describe the vowel counter (edited)" while GitHub and the list row said "Describe the
vowel counter": the kept detail snapshot of #159 was shown and not yet read again ~5 s after opening
(image 00's after half).

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| before 1–2 (agent, base) | `07dcef1ab` | 1: an element screenshot is refused ("view has no world"), crops come from window shots since; 2: the list was 964 pt wide with no panel open, nothing overflowed | — | — |
| before 3 (agent, base) | `07dcef1ab` | reproduced: #159 after the rename and #149 on its first read lose their start | record-before | — |
| one-file app (macOS, web) | framework of `07dcef1ab` | macOS cuts the start of a centred over-wide line (button, row, plain line); the web host does not; Chrome 151 start-aligns | 03, 04 | X57 |
| after (agent, branch; the one live session) | this branch before the review | every row above passes | — | — |
| independent review | the staged diff | no blocking findings. Taken: `PrdCopy` (the reference says `text-left`), the submenu value, the test widened to `line-clamp=1` and clipped `nowrap` texts with the nearest `text-align` deciding, X57's other host sites (region rasters, text flow, iOS) and its `line-clamp=1` count. Noted: under the clone's `align-items="flex-start"` column the timeline's authors line may be as wide as its text and never show an ellipsis (predates this task) | — | — |
| after retry (agent, final build; the one retry) | this branch with the review's changes | every row above passes again; the copy buttons read as before | [record-after](https://raw.githubusercontent.com/ccheever/exact2/7743b3792cece4728bcf787f910fd0ed355abb39/pr-list-title-clip/record-after.txt), 00–02 | — |
| checks | this branch | `bun test examples/t3-code` 3037 pass / 1 skip / 0 fail (base 3036 + 1); strict tsc clean; contract build 3844 slots, 46 resources; `cargo test -p t3-code-macos --lib` 11 pass; caps within; five checks: build exit 0, test 3,383 passed / 0 failed / 33 ignored (94 binaries), clippy and fmt clean, boot allowed paths only; verify runner passed, `source_unchanged: true` | `target/pr-list-title-clip/verify` (not committed) | — |

## Progress

2026-10-08: implemented, reviewed (no blocking findings; the review's additions taken), verified
(`74e953039`; runner attempt 2 passed and the committed tree matches it) and opened as draft PR #289.

## Next action

Coordinator: review and merge the draft PR. X57 waits for the user's approval to publish.
