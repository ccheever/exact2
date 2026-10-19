# Code review: the hold replay by call-site chain (LLP 1122 D8, stage 8 E), round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `755a462401`.
- **Method:** one brief (sha256 `3bcc8f4eb0348f73e0a609e7d6ceba80484f846f0f583b9ecb9037e053c2b579`), shared by both reviewers, blind to the other review; commits d6f71f7e25 (before the rebase onto stage 9 E). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high).
- **Disposition:** documented in `748afd4ce9`, per theme2's 18:53 (a) (a reorder may be documented as the known limit): the protocol now says the correspondence is by places alone (the map carries no other identity of a call or element), so a reorder that keeps its spacing, a call or element moved onto another's old place (line or column), or a copy added or removed while one keeps its line can carry the hold to the other copy, the host letting it go (`gone`) where the copies' instance paths differ. The rule is not made stricter: no rule over places alone separates these from the edits it should follow (a line written above, between or inside), and a stricter one clears holds on those. Not taken: a persisted malformed-map replay test through `tamberServer` (the exception path is two lines; the channel test drives replay).

Two wrong re-holds block sign-off. Both are cases the protocol's named limits do not cover. The round 2 fixes that were taken otherwise do what they say.

**1. A paste that keeps a different column is re-paired by line rank.** `scripts/tamber-inspect.mjs:346` and the return at `:363`.

`oneEdit` records only `y.line - x.line`. A zero delta means "this call stayed," and a constant after that means "everything below shifted by one splice." Column is part of the chain key (`at` at `:322`) and is never read here.

Hold the second of two `PostButton` calls:

- `home.contract:4:1` — `PostButton()`
- `home.contract:9:20` — `PostButton(icon: "x")`, the one held

Cut the second and paste it at line 4, leaving it at column 20. The first call shifts to line 5, column 1. The new chains are `4:20` and `5:1`.

Rank pairs `4:1 → 4:20` (delta 0) and `9:20 → 5:1` (delta -4). That is the pattern "the first stayed, the second moved up by 4," so `holds` accepts it and the hold returns the site at `5:1` — the call that only shifted. `hold-cleared` is not sent (`scripts/tamber-builds.mjs:204`). The call that was held is sitting at `4:20`.

A real deletion of the four lines between them is a different map, `4:1` and `5:20`, and that one is corresponded correctly. The protocol's exception ("landing on another's old line, takes that line's," `docs/tamber-protocol.md:990`) would put this hold on line 4, where the pasted call is. The function puts it on line 5.

The same acceptance moves an innocent call. Three calls at `4:1`, `10:1`, and `16:20`, hold on the middle. Paste the third onto line 4, keeping column 20, and the others shift to `5:1` and `11:1`. The middle hold returns `5:1`, the first call. The middle call is at `11:1`.

**2. Two calls on one line exchange columns and each hold follows the column.** `scripts/tamber-inspect.mjs:336`.

`PostButton()` at `4:1` and `PostButton(icon: "x")` at `4:20`. Hold the first. Swap them on the line so each sits at the other's column. Both chain keys still exist, the counts match, and the early return treats a surviving key as the same call. The hold comes back as the site now at column 1, which is the call that was at column 20.

Line numbers did not change, so this is not "two calls swapped onto each other's lines" (`docs/tamber-protocol.md:990`). It is a call that moved past another. That sentence says the hold clears.

The host keeps a hold only when one view has that site and that instance (`host/linux/src/presenter/held.rs:252`). Distinct call sites carry distinct instance paths, so a mismatched pair is reported `gone` and the hold ends there. With an empty instance path — the path the server test sends — the other site is the one view and the wrong copy stays held.

What round 2 asked for, and what holds:

- Chains of different depths no longer throw. `place` (`:343`) turns a missing link into `''`, and shape is checked before `oneEdit` indexes the new chain. The two-depth shift in `scripts/tamber-inspect.test.mjs:289` returns each copy by rank.
- A throw in correspondence clears the hold. `scripts/tamber-server.mjs:215` returns null, and `replayHold` sends `hold: null` plus `hold-cleared` (`scripts/tamber-builds.mjs:205`).
- An unmoved caller in another file no longer reorders the copies. `cmp` (`:373`) orders by file, then line, then column. Home's calls at 4 and 9 shifting to 9 and 14, with Footer unmoved at line 10 in its own file, re-holds the second Home button at line 14, not at the reused line 9 (`scripts/tamber-inspect.test.mjs:294`).
- A paste that does not land on the other call's line clears. `(4, 9) → (3, 5)` is null (`:286`). Cutting one call and pasting it strictly above the other, with a gap, is fixed.
- The map writer is unchanged and matches the rule. `contract/cli/src/map.rs:197` writes the use, then each parent, with file, line, and column (`:52`). There is no outlet id, so two slot fills with the same chain stay one copy. That is the limit the protocol already names.

The limits that are named match the code. A same-column landing, `(4, 9) → (4, 5)`, still re-holds by rank; the doc calls that out. Two calls exchanged onto each other's lines are taken by those lines. A copy added or removed keeps only a chain key that still exists. Slot outlets stay ordered by node order. The decision not to add a `tamberServer` case is the one in the disposition: the rule's cases are in the pure test, and `scripts/tamber-server.test.mjs:1438` still drives replay for one node at a unique line with an empty chain.

Smaller: the new test pins `(3, 5)`, the paste that clears, and does not pin the same-column landing the doc still allows, nor either case above. The protocol sentence at `:990` is doing a lot of work in one line; "takes that line's" reads as the hold moving to the line that was landed on, while the same-column rank pairing also moves the pasted call's hold onto the call that only shifted.
