# Code review: LLP 1121 stage 2, commit 7 (CLI verbs, layout's tokens, no-tells, docs), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `359165893a7ddc97582e0cd25771e2836deb0a9a4a423034d04c3a7df781360c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: five medium, three low; Grok: three medium, two low.
- **Disposition:** all fixed in the round-1 commit

---

Reviewed `bfe4ca5bf` against its actual parent, `42abcd182`. **No high-severity findings.**

1. **Medium — Documented option ordering fails.** [contract/cli/src/tamber.rs:25](/tmp/rv-s2c7r1/contract/cli/src/tamber.rs:25)  
   `contract tamber eval --mode dark-style=dim apps/theme-fixture` treats `--mode` as the input path and exits 2. The advertised `ir -o file <app>` ordering fails similarly. Parse options before selecting the positional input, and test both option placements.

2. **Medium — `layout` can name tokens the row never read.** [runner/src/runner/mode.rs:200](/tmp/rv-s2c7r1/runner/src/runner/mode.rs:200)  
   `card` and `surface173.card` both hash to bit 31. When both are retained, a row reading only `surface173.card` reports both names because `text.contains("card")` succeeds. The bloom is suitable for conservative refresh, but not exact provenance. Record or recover exact resolved token identities; add this collision case.

3. **Medium — Role aliases disappear from `token.names`.** [runner/src/runner/mode.rs:199](/tmp/rv-s2c7r1/runner/src/runner/mode.rs:199)  
   A token declared as `secondary-label`, read through `-exact-secondary-label`, produces an empty names array: the parser records the alias’s hash, while this loop checks the declaration’s hash. Likewise, `CanvasText` read as `canvastext` fails the case-sensitive substring check. Resolve role aliases and casing back to the declared token, with tests for both forms.

4. **Medium — Valid Tamber declarations cause no-tells false positives.** [scripts/no-tells.mjs:259](/tmp/rv-s2c7r1/scripts/no-tells.mjs:259)  
   The scanner recognizes exactly two spaces and an untyped declaration. Valid `surface.card: color = #fff`, or declarations indented four spaces, yield no token names. Consequently, `rgb(from surface.card r g b / 0.5)` is incorrectly flagged. Match the parser’s indentation and optional-kind grammar, and cover both cases.

5. **Medium — Lowercase system roles are falsely flagged.** [scripts/no-tells.mjs:158](/tmp/rv-s2c7r1/scripts/no-tells.mjs:158)  
   `rgb(from CanvasText r g b / 0.5)` passes, while the equivalent `canvastext` spelling fails. Role resolution is case-insensitive; this also affects currently foldable expressions when the design overrides `CanvasText`. Use a case-insensitive lookup of actual schema roles and test equivalent spellings.

6. **Low — Direct Tamber help starts Cargo.** [scripts/exact.mjs:887](/tmp/rv-s2c7r1/scripts/exact.mjs:887)  
   `bun scripts/exact.mjs tamber --help` invokes `contract()`, potentially building the compiler before printing help. Mirror the existing `contract` help interception. Add Tamber to the direct-script help test; the new app-wrapper test does not exercise this branch.

7. **Low — The “Always Black” preview follows light mode.** [docs/recipes/tamber.md:37](/tmp/rv-s2c7r1/docs/recipes/tamber.md:37)  
   A mode pin does not change `color-scheme`. Under light appearance, this preview uses the light colours despite its label. Add `color-scheme="dark"` to the preview, as RFC D5 specifies.

8. **Low — The introductory guide implies size tokens already work.** [docs/start-here.md:15](/tmp/rv-s2c7r1/docs/start-here.md:15)  
   The new “colour or size … token” instruction exceeds the shipped implementation: a length token is rejected with `tamber-stage` until stage 6. Qualify the line to distinguish currently supported colour tokens from future size tokens.

The four no-tells tests, recipe lint, and caps check passed. In-memory probes reproduced the lint and name-matching failures. I found no JSON-escaping, tuple-decoding, or newly admitted literal-colour bypass defect. Rust integration tests were not run because they create files. No files were edited.
