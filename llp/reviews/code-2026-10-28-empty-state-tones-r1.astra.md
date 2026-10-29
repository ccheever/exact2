# Code review: @exact/cards EmptyState follow-up (x2/empty-state-tones), r1, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `ebdc1988e5`.
- **Method:** one brief (sha256 `9df692febcddbf92c2ae680b16098577628a628fe699009516bd251f75ddba60`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 low: P3).
- **Disposition:** taken in 491a982552 (the agents doc names the three enums).

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

- **P3 — [docs/contract-for-agents.md:4104](/tmp/iosfix-rv-es1/docs/contract-for-agents.md:4104): Update EmptyState’s documented signature.** It still lists the old eight props, omitting required `iconSize`, `tone`, and `buttonTone`. Calls following it now fail compilation. Document their enum values and required status; the first values establish the gallery baseline, not defaults for omitted arguments.

No implementation findings. The read-only audit matched all **266 node/row/cell reads**, all 31 tokens and 10 params, and the six variant cells plus two samples. Sizes, colours, platform roles, and updated consumers check out. Build/restyle runs were not repeated.

**VERDICT: CHANGES**
