# Code review: the hold replay by call-site chain (LLP 1122 D8, stage 8 E), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `175aecb882`.
- **Method:** one brief (sha256 `1ed0774ffd27ab3069124106ca7aa37fae75b2141988c0be8e6328e6bce5efb9`), shared by both reviewers, blind to the other review; commits d058feff01. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 1 medium).
- **Disposition:** taken in `710b805698`: correspondence by copies, one copy a call-site chain whose sites (a `button "…"` and its label) are kept in order; the same calls in both plans take the same chain; as many copies whose calls moved take the same rank only where each pair is of one shape and the copies share calls alike, so a regrouped nest is not ranked onto another; otherwise the copy whose calls did not move, so a call added or removed beside a held one keeps it; calls moved onto each other's lines go by rank. The listing doc now says `held` is its frame's. Not taken, recorded in the protocol doc: a slot filled at two outlets is one copy, its outlets told apart by their order; with copies added or removed, a line another call now has is taken as unmoved.

Three correctness issues block sign-off.

1. **[P1] Reused coordinates can re-hold the wrong call.** [tamber-inspect.mjs:315](/tmp/rv-dt-t4r3/scripts/tamber-inspect.mjs:315)  
   Two calls at lines 4 and 9 move to 9 and 14 during a plan-changing rebuild, with the component definition unchanged. Both old sites resolve to the first new call: the second takes the “exact” match at line 9 before rank is considered. I reproduced the wrong hold frame through `buildChannel`, with no diagnostic. With `instance: []`, the host’s single-match check accepts it. Exact matches must be consistent with the correspondence of the other copies.

2. **[P1] Equal total counts do not preserve nested-call identity.** [tamber-inspect.mjs:319](/tmp/rv-dt-t4r3/scripts/tamber-inspect.mjs:319)  
   Start with two `Wrapper()` calls, each containing one `PostButton()`. Remove the second wrapper call and add a second button call inside `Wrapper`. Both maps still contain two buttons with the same chain’s files and component names. Rank transfers a hold from the deleted wrapper’s button to the newly added button in the surviving wrapper. I reproduced this with unique chains and no coordinate collision. Correspondence must respect ancestry; equal leaf counts alone cannot establish it.

3. **[P2] Duplicate full chains fall through to an arbitrary rank.** [tamber-inspect.mjs:321](/tmp/rv-dt-t4r3/scripts/tamber-inspect.mjs:321)  
   A slot rendered at two `children` outlets can produce identical node locations and call chains—the inliner restores the fill’s instance at each outlet. When both maps contain two such copies, `exact.length === 2` falls through, the comparator ties, and array order chooses a site. Reordering the outlets can re-hold the wrong copy. This ambiguity must return `null` and emit `hold-cleared`; the current implementation replays without that diagnostic.

The `held` reset looks correct. The design tool handles explicit `null` as awaiting confirmation; the agent CLI calls the host directly and does not depend on this retained report. Checks also confirmed preservation of keyed-list instance paths and separation of calls from different files.

Both targeted Bun tests pass (52 assertions). Additional in-memory matcher/channel checks reproduced the failures above. No files were changed; no tree-modifying git commands were run.
