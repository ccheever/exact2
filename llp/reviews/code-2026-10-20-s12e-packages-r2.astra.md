# Code review: Stage 12 E: @exact/cards, @exact/actions and apps/library-gate (LLP 1139 §5 E), round 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `086a3086af`.
- **Method:** one brief (sha256 `994835612436bff863a06c3a227f9a3afd7084b7fdcfee882bc4ef613f768151`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** cards PASS, actions PASS; library-gate should-fix: a trailing comment overwrites an argument, a commented-out marker passes, placements in an uncalled component count
- **Disposition:** taken in 9a1fe59dcf: comments set aside before parsing, the marker as the slot's first child with the glyph under it, one root component; markers are pressable boxes so a tap reveals them

Verdicts: **@exact/cards PASS; @exact/actions PASS; library-gate SHOULD-FIX.** No blocking findings or nits.

Three should-fix findings in the fixture test:

- **[scripts/library-gate.test.mjs:33](/tmp/iosfix-rv-pk2/scripts/library-gate.test.mjs:33): Trailing comments can still falsify variant coverage.** Changing `ar-liked` to `liked=false` and appending `// (case, liked=true)` leaves the parsed coverage unchanged: the greedy regex consumes the comment, whose assignment overwrites the real argument. Exclude comments before parsing, respecting quoted strings.

- **[scripts/library-gate.test.mjs:72](/tmp/iosfix-rv-pk2/scripts/library-gate.test.mjs:72): A commented-out marker passes.** Commenting out the marker box and leaving `text glyph` directly in the slot satisfies both assertions. Check an actual marker node with the glyph beneath it; independent text searches don’t establish that relationship.

- **[scripts/library-gate.test.mjs:29](/tmp/iosfix-rv-pk2/scripts/library-gate.test.mjs:29): Placements aren’t restricted to the rendered view.** Moving the gallery into an uncalled component still yields all 17 placements and passes the control-flow check. Restrict the fixture to one root component or verify reachable placements.

The package fixes and async scheduling match the stated decisions. The current fixture’s cells and markers are present.

Reproduced the parser issues entirely in memory. The permitted Bun run had one pass; the compiler-backed test failed at `mkdtemp` with sandbox `EPERM`, before compilation. No files changed.
