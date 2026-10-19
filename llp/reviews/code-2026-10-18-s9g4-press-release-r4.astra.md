# Code review: LLP 1121 stage 9 G4, an ancestor's press release and a descendant's press, round 4, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change and rounds 1-3.
- **Method:** one brief (sha256 `de602afe4019d98450689613bcaf7877440bb84fadd7023e12abefa543b69c7d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: a written colour stopping the lender walk before the press; a down node's new written colour).
- **Disposition:** fixed in round 4's commit.

---

1. **blocking — [PressPaint.swift:269](/tmp/rv-g4-4/host/apple/Sources/ExactKit/PressPaint.swift:269).** `liveLender()` rejects a written base colour before checking the ancestor’s active press. For `A(color:white, press:yellow) > B(press:blue) > label`, press A, press B, release B: B and its label become white although A remains yellow. This also affects a release-held A. **Fix:** check for an active press colour before the written-colour boundary. Add this case with `written: ["text_color"]` on A; current release fixtures omit it.

2. **should-fix, pre-existing — [PressPaint.swift:410](/tmp/rv-g4-4/host/apple/Sources/ExactKit/PressPaint.swift:410).** With A lending yellow to a background-only pressed B and its label, give B an authored red colour. B becomes red, but its label keeps A’s yellow—even after receiving inherited red from the engine. Descendant cleanup only runs in the `!down` branch. **Fix:** clear the lender’s ink below a newly written colour boundary while down too, preserving descendants’ own press paint.

The other round-3 fixes look consistent, including recycled-view ownership cleanup. The excluded removal of an own `press_color` can remain separate.

Static review only; no edits, builds, or tests run.
