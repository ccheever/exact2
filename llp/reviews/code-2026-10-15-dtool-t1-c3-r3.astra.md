# Code review: LLP 1122 T1 commit 3 (the dials panel and the token grid), round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `540fe0e2a` (commit `73a5d1c9a` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `f667a29dd5013e68f7dfc82285aa036ba0010627d4b426a013264acb4f3e32e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** all taken (in the commit with commit 4's round-2 fixes): a select's option is a literal by its dial's kind (a colour's hex, CSS name or colour function call; a number's plain number; a length's `0` or `Npx`); names, `transparent` and `rgb()` with percentages or decimals fold as the server folds them, other colour functions compare as written; `resolve` and `explain` now carry the digests they read (the server, tamber_serve.rs) and a design read is read again unless every resolve read describe's revision; the gate types into each slider past its end and between its steps and checks the value the page keeps to its input's own min, max and step (the agent's tree reports no input attributes, so they are exercised, not read). Round 3 was the last: other colour functions are still compared as written, not folded.

---

1. **Medium — The select filter rejects valid literals and admits invalid ones.** [app.ts:142](/tmp/rv-dt-3r3/apps/design-tool/app.ts:142)  
   A dot anywhere makes a colour option “not a literal.” Thus `options="rgba(255,0,0,0.5)|#fff"` loses its translucent-red choice; decimal `oklch()` options are also removed. Conversely, `numberOf()` strips `px`, so a number dial offers `0.2px`, which Tamber refuses. **Fix:** validate options against the dial kind using Tamber’s literal rules, rather than punctuation and a shared number/length parser. Cover both cases in tests.

2. **Medium — Matching before/after digests still permits mixed revisions.** [app.ts:278](/tmp/rv-dt-3r3/apps/design-tool/app.ts:278)  
   Each `resolve` independently rereads the files. If revision A is black text on white, revision B reverses both colours, and the file returns to A before the final `hello`, the digest check passes. Resolving text during A and background during B produces black-on-black and **1:1**, although both revisions have **21:1**. Unchanged axes also pass `placed`. A quick disk edit/revert can escape the watcher’s debounced notification. **Fix:** return each resolve’s snapshot digests and compare them with `describe`, or resolve all tokens from one server snapshot. Add a deterministic edit/revert interleaving test.

3. **Medium — The metadata gate still checks captions instead of control properties.** [design-tool.test.mjs:156](/tmp/rv-dt-3r3/scripts/design-tool.test.mjs:156)  
   Control type comes from a test-ID suffix; value and bounds come from neighbouring text; options are checked only by their labels. Changing the actual slider to `max=1 step=1` still satisfies these assertions while making the scientific-notation fixture unusable. The describe comparison checks only names and inferred control types. **Fix:** inspect the rendered inputs’ actual types, bounds, steps, values and accessible names, plus option values; exercise increments and endpoints where inspection is unavailable.

4. **Low — Named-colour option matching remains unfixed.** [app.ts:111](/tmp/rv-dt-3r3/apps/design-tool/app.ts:111)  
   `folded()` handles hex and a restricted RGB syntax but leaves named colours unchanged. For `ink: color = red control=select options="red|blue"`, Tamber supplies `#ff0000ff`; the menu consequently prepends and selects that hexadecimal value alongside `red`. Percentage RGB and HSL spellings have the same mismatch. **Fix:** compare canonical values produced by Tamber’s colour evaluator while preserving declared option labels.

Read-only inspection; no files changed and no builds or tests run.

**Verdict: Request changes — 3 Medium and 1 Low findings remain.**
