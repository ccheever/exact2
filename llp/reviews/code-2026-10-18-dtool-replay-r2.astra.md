# Code review: the hold replay by call-site chain (LLP 1122 D8, stage 8 E), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `eb6a84bb6b`.
- **Method:** one brief (sha256 `949202ffaee96713d480cfe735d98633e63750ef67651f6bbe7a9998259c0160`), shared by both reviewers, blind to the other review; commits 710b805698. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high).
- **Disposition:** taken in `d6f71f7e25`: chains of different depths are compared without reading past the shorter, and a throw in correspondence clears the hold with the diagnostic rather than leaving it; copies are ordered by file, then line, so an unmoved caller in another file does not reorder them; a pairing (all by rank, or the unmoved kept and the rest by rank among themselves) holds only where each file's calls moved as one edit there moves them (above it by none, below by one count of lines), so a call cut and pasted above another, or a reused line beside a shift, is not taken; the whole pairing is checked for shared calls. The protocol doc names what is still taken by lines (two calls swapped onto each other's lines; a moved call landing on another's old line; copies added or removed; slot outlets by order), per theme2's 18:53 (a). Not taken: a server-level test through `tamberServer` (the rule's cases are in the pure test, and the channel test drives replay).

**Request changes: two P1 correctness bugs block sign-off.** Line references are to this checkout.

1. **[P1] An unrelated caller can cause the wrong copy to be re-held.** [scripts/tamber-inspect.mjs:338](/tmp/rv-dt-t4r4/scripts/tamber-inspect.mjs:338)  
   Put two `PostButton()` calls in `Home` at lines 4 and 9, and another in `Footer` at line 10, with both components called by `App`. Insert five lines above Home’s calls during a plan-changing rebuild; Footer stays unchanged.

   Copies are sorted together by line number, so their order changes from Home/Home/Footer to Home/Footer/Home. The shape check rejects rank matching, and the fallback treats Home’s reused line 9 as unchanged. **Holding the second Home button replays onto the first**, without `hold-cleared`. I reproduced this through `tamberServer`. No calls were added, removed, or reordered, so the documented limits do not cover it. Round 1’s reused-coordinate issue remains incompletely fixed.

2. **[P1] Mixed call-chain depths throw instead of replaying or clearing.** [scripts/tamber-inspect.mjs:335](/tmp/rv-dt-t4r4/scripts/tamber-inspect.mjs:335)  
   Use `PostButton` directly in `App` and inside `Wrapper`, then shift App’s two calls down one line. Their chains have lengths one and two. The ancestry comparison indexes the shorter chain at the longer chain’s depth and calls `at(undefined)`, throwing `TypeError`.

   Through `tamberServer`, this sends neither a replay nor a clear diagnostic and retains the old hold and `held` report. The build’s `planDigest` has already advanced before replay ([tamber-builds.mjs:370](/tmp/rv-dt-t4r4/scripts/tamber-builds.mjs:370)), so another status with that digest skips replay.

The other taken round 1 points check out: button/label pairs survive an unchanged call beside an added or removed sibling; the reported nested-regrouping example clears correctly; and the listing’s `held` wording now agrees with the frame reset.

The not-taken cases are accurately recorded in [the protocol](/tmp/rv-dt-t4r4/docs/tamber-protocol.md:984): duplicate slot outlets use ordinal position, and changed copy counts permit coordinate reuse. The map’s lack of outlet identity explains the ordinal choice, but these remain possible wrong-copy replays. Smaller documentation point: explicitly stating that reordering calls or outlets can transfer the hold would make that consequence clearer.

The two targeted existing tests passed, with 60 assertions. Both failures above were reproduced using in-memory maps and the server with a fake compiler process. The standard server test was blocked by its temporary-directory setup under read-only permissions. No files changed; no tree-modifying git commands ran.
