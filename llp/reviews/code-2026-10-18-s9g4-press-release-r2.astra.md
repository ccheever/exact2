# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `2ece4991b75b560d626e98edb6a286cd4e29d2ccf87d96ecc9919a33efbb7e3b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: a middle press over a pressed descendant's own colour; a middle node's own colour wiping a pressed grandchild; a release under a live lender).
- **Disposition:** fixed in round 2's commit.

---

Two blocking issues remain.

1. **Blocking — [PressPaint.swift:216](/tmp/rv-g4-2/host/apple/Sources/ExactKit/PressPaint.swift:216): `inkBelow` can overwrite another node’s own press colour.** In an A → B → C tree with the same inherited base colour, let A lend yellow, then C show its own navy press. Press B while those releases are held. C still records A as lender, and its saved base matches B’s inherited colour, so the new condition lets B overwrite C’s navy despite C remaining down. **Fix:** treat a descendant whose own displayed press supplies `text_color` as an inheritance boundary, including `releaseHeld`. Add this three-level press-order test.

2. **Blocking — [PressPaint.swift:287](/tmp/rv-g4-2/host/apple/Sources/ExactKit/PressPaint.swift:287): authored-colour cleanup bypasses the new selective restoration.** Let A lend ink through B to C, then press C with its own green background. If B gains an explicit `text_color`, `pressPainted` calls `endInk`, which still invokes unrestricted `endPressStandIn()` on C because C retains A as lender. C loses its green background while still pressed or held active. Making B’s explicit colour equal its previous inherited base avoids a descendant style update masking the failure. **Fix:** route this cleanup through the same ownership-aware, `pressPaint.down`-aware logic as `endInked`; test the same-colour authored boundary.

3. **Should-fix — [PressPaint.swift:247](/tmp/rv-g4-2/host/apple/Sources/ExactKit/PressPaint.swift:247): the reverse release order still loses a live lender’s colour when the descendant supplies its own press colour.** A lends yellow; B presses navy and takes its labels; B releases while A remains down or `releaseHeld`. Because B’s `pressOverlay` contains `text_color`, this branch restores engine values and clears ownership on B and its labels, although A’s yellow should show again. The new reverse-order test covers only a pressed background. **Fix:** restore the live lender’s colour and ownership after B’s own colour ends, including affected labels. Extend the test with B’s own `press_color`, then verify A’s later release and `dropPressPaint` clean everything up.

Read-only source review; no builds or tests run.
