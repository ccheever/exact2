# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), r1, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `ebdc1988e5`.
- **Method:** one brief (sha256 `9df692febcddbf92c2ae680b16098577628a628fe699009516bd251f75ddba60`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Round 1 review of the branch: diff origin/main..HEAD, one commit. Read-only: no checkout, restore, stash, reset or edits.
This is a follow-up to stage 15 B batch 2c (LLP 1121, LLP 1139): @exact/cards EmptyState gains Bluesky's per-call variants. theme2 approved it at 22:28Z.
Bluesky's reference is at ~/.tuft/projects/reference/social-app. Its EmptyState is src/view/com/util/EmptyState.tsx, its icon sizes are src/components/icons/common.tsx, and its calls are ChatList.tsx, Inbox.tsx, ActivityList.tsx, Bookmarks.tsx, Lists.tsx, ProfileLists.tsx and DraftsListDialog.tsx.
The problem: the landed package drew every icon at 64 (Bluesky's 4xl, which only the chats pass). Bluesky's default is 3xl, 48 in a 64 box. Its calls also vary the icon and message colour (textStyle, iconColor) and the button colour (Bookmarks' secondary).
The change:
- `iconSize: enum<"medium","small","large">` = 48/32/64, medium first so the base cell is the default, the icon in a 64 box either way;
- `tone: enum<"high","medium","label">`: the message contrast_high / contrast_medium / the text colour, and the icon the text colour for label, else contrast_low;
- `buttonTone: enum<"primary","secondary">`;
- new params over new cards.empty-* tokens (the secondary button reuses FollowButton's following colours, Bluesky's secondary contrast_50/contrast_700);
- library-gate cells for each other value (63 cells);
- s13c-restyle READS rewritten for the new node paths (the icon is now box 0 holding image 0/0) and every cell;
- the batch2_exports consumer;
- the LLP 1139 note.
Check:
- the values against Bluesky's source: sizes, which colour per tone, the secondary button's colours, the icon box;
- the platform-role token choices (-exact-label, -exact-secondary-label, -exact-tertiary-label) against the existing cards tokens' conventions;
- the READS paths and cell lists: does every token and param land on exactly the listed nodes and cells, with none missing or extra? A wrong path (box vs image) matters;
- the library-gate cell rule (first values, then each other value alone);
- breaking changes for existing consumers (the props are new and required);
- the docs.
Checked: the batch2_exports consumer passes, library-gate.test and stage12-gate.test pass, the web build passes. The s13c restyle on terminal and web-js is running.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll review the branch diff read-only against Bluesky's EmptyState and the landed token, path, and cell rules. First I'm loading the review instructions and the one-commit diff.The branch is one commit ahead of `origin/main`, and the diff is much larger than the EmptyState follow-up. I'll load the review persona and confirm the tree is clean before collecting that diff.The commit itself is seven files. `origin/main..HEAD` as a two-dot diff also reverses newer main commits, so I'm reviewing the merge-base range — the one commit that would actually land.No findings.

The review is commit `ebdc1988` (the one commit on `HEAD` that `origin/main` does not contain): the seven EmptyState files. A two-dot `git diff origin/main HEAD` also reverses commits that landed on `origin/main` after this branch’s parent; those are not this change.

Checked against Bluesky’s `EmptyState.tsx`, `icons/common.tsx` (`2xl`/`3xl`/`4xl` = 32/48/64), and the seven call sites, plus ALF 0.1.15 (`text` = `contrast_1000`, `text_contrast_high` = `contrast_900`, `text_contrast_medium` = `contrast_700`, `text_contrast_low` = `contrast_400`):

- `iconSize` is `medium`, `small`, `large` (48/32/64). Medium is the first enum value, so the base cell is Bluesky’s 3xl default. The icon sits in `cards.empty-icon-box` (64) either way.
- `tone` high keeps the message on `cards.empty-message` (`-exact-label`, the existing contrast_high role). Medium uses `-exact-secondary-label`, the same step as `cards.meta`. Label uses `-exact-label` for the message and the icon; every other tone keeps the icon on `-exact-tertiary-label` (contrast_low). High and label share that primary role on purpose: the package already maps contrast_high there, and the two params can still be restyled apart.
- `buttonTone` secondary copies FollowButton’s following colours, which are Bluesky’s solid secondary (`bg_contrast_50` `#eff2f6`/`#222e3f`, text `contrast_700` `#405168`/`#abb8c9`). Primary is unchanged.
- The restyle paths match the tree. With an icon, node `0` is the box and `0/0` is the image (same shape as FollowButton’s icon box). Title, message, and button stay `1`/`2`/`3` on the six `es-*` cells; the message-only and button-only samples stay `0` and `1`/`1/0`. Each new token and param is listed on the cells that actually use it, and not on the others.
- The gate cells are the first values, then each other value alone: small, large, medium tone, label, secondary. 58 + 5 = 63. `iconSize`, `tone`, and `buttonTone` have no defaults, so they are required. The in-repo `@exact/cards` callers (library-gate and the batch 2 consumer) pass them. `exact:content`’s `EmptyState` is a different component. The LLP 1139 note matches the sizes, tones, and button.

The s13c restyle on terminal and web-js was still running, so those computed reads are not confirmed here. The paths were checked against the contract tree.

VERDICT: CLEAN
