# Code review: the hold replay by call-site chain (LLP 1122 D8, stage 8 E), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `eb6a84bb6b`.
- **Method:** one brief (sha256 `949202ffaee96713d480cfe735d98633e63750ef67651f6bbe7a9998259c0160`), shared by both reviewers, blind to the other review; commits 710b805698. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high).
- **Disposition:** taken in `d6f71f7e25`: chains of different depths are compared without reading past the shorter, and a throw in correspondence clears the hold with the diagnostic rather than leaving it; copies are ordered by file, then line, so an unmoved caller in another file does not reorder them; a pairing (all by rank, or the unmoved kept and the rest by rank among themselves) holds only where each file's calls moved as one edit there moves them (above it by none, below by one count of lines), so a call cut and pasted above another, or a reused line beside a shift, is not taken; the whole pairing is checked for shared calls. The protocol doc names what is still taken by lines (two calls swapped onto each other's lines; a moved call landing on another's old line; copies added or removed; slot outlets by order), per theme2's 18:53 (a). Not taken: a server-level test through `tamberServer` (the rule's cases are in the pure test, and the channel test drives replay).

Two correctness bugs block sign-off. Both are in `correspondingSite`. The round 1 cases that were taken do what they say, and the two limits that were not taken are the ones the protocol names.

**[P1] A component used at two depths throws, and the hold stays on the old site — `scripts/tamber-inspect.mjs:335`**

`alike` walks every copy's chain to the length of the longer one. A direct call is one link and a nested call is two (`contract/cli/src/map.rs:198` writes the use, then its parent). `at` then reads `file` on a missing link.

`PostButton` at `home.contract:4`, and the same button inside `Card` (`card.contract:3`) called from `home.contract:8`. Insert one line at the top of `home.contract`. The direct call is now line 5 and the card call is line 9. Counts still match and the shapes still line up, so rank runs and throws `TypeError: undefined is not an object (evaluating 'x.file')`.

The status handler has already stored the new plan digest (`scripts/tamber-builds.mjs:370`) before `replayHold` calls `correspond` (`:372`, `:204`). The exception leaves the hold at the old site. No new frame is sent and there is no `hold-cleared`. `serveReport` turns the throw into a 500 (`:573`). A repeat of that same status is 204 and does not try again. I ran this through `buildChannel`.

**[P1] Pasting one call above the other re-holds the call that did not move — `scripts/tamber-inspect.mjs:336`**

Rank runs for any equal count whose sharing pattern matches, including when the calls change order. Source order is not the same calls once one has moved past the other.

Two `PostButton()` calls, at lines 4 and 9. The hold is the second (site 2). Cut it and paste it above the first. Each call is one line, so the map is lines 3 and 5: line 3 is the call that was held, line 5 is the old line 4 shifted by the insert. `alike` is true (same shape, still two different lines). Rank returns site 2. The rebuild is sent `hold` on the other copy, with the same instance path, and `hold-cleared` is not sent. I ran this through `buildChannel`: status 204, frame `{site: 2, ...}`, no clear.

The same pairing happens for two cards that share the inner call. Old chains `[wrap:3, home:4]` and `[wrap:3, home:9]`, hold on the second. After the paste the chains are `[wrap:3, home:4]` and `[wrap:3, home:5]`. The hold on site 1 is returned as site 1, which is the card that only shifted.

`(4, 9) → (9, 14)` and `(4, 9) → (4, 10)` stay right under this rule. Those are the cases the tests lock. A cross is not.

Round 1, taken and right where the tests stand:

- A line landing on the other call's old line goes by rank: `(4, 9) → (9, 14)` is site 1 then site 2 (`scripts/tamber-inspect.test.mjs:268`).
- A regrouped nest is not ranked across: the deleted wrapper's button is null, the one that stayed is itself (`:279`).
- A `button "…"` and its label are one copy. Adding or removing the other call keeps the one that did not move (`:272`).
- The listing's `held` is that frame's report, null until the build answers (`docs/tamber-protocol.md:177` and `:991`).

Not taken, and named. A slot filled at two outlets is one copy, the outlets told apart by order, so swapping the outlets follows that order onto the other outlet. With a copy added or removed, a line some other call now has is taken as the call that did not move (`docs/tamber-protocol.md:985`). Both match the code: identical chains are one copy (`scripts/tamber-inspect.mjs:323`), and the last branch is an exact chain (`:338`).

Smaller: the server test still replays one node at a unique line with an empty chain (`scripts/tamber-server.test.mjs:1440`), and the channel test stubs `correspond`. Neither of the cases above is in `scripts/tamber-inspect.test.mjs`. The eight tests in that file pass (115 assertions).
