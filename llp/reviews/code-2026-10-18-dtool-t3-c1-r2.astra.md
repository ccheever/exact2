# Code review: LLP 1122 T3 step 1, the param rung in the selection join, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ebafe6aac`.
- **Method:** one brief (sha256 `bd246a813ee1e4efb8288a9fc64e02041bdd3f893f5c6596727c73e253cc2f65`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium).
- **Disposition:** all taken in `a90dbbf0c`: a new plan's read keeps the names but offers no alias until it answers, and a failed read (an error or a 504) clears the plan stamp too, so the next selection reads again; the IR name is matched as a whole word of its real shape (`param.f<8 hex>.<Component>.<param>`), so a token that only contains one is left be; the custom property a JS page writes for a param (`var(--x-param-…)`, modes.rs) is named in the text too; a role override aliases by any spelling of its role. The protocol says a row whose one name is a param links it, and that a client that knows no param rung offers no edit. Not taken: an icon param aliases no token (no operation writes an icon).

**I would block this round.** Four correctness issues remain.

1. **[P1] Plan refreshes still offer stale aliases, and a failed refresh becomes sticky.**  
   [tamber-server.mjs:221](/tmp/rv-dt-t3r2/scripts/tamber-server.mjs:221), [369](/tmp/rv-dt-t3r2/scripts/tamber-server.mjs:369), [375](/tmp/rv-dt-t3r2/scripts/tamber-server.mjs:375).  
   Change a package component’s default from `ink` to `accent`, then inspect its rebuilt plan. The new map passes the source checks, but the pending param read retains—and offers—`token: "ink"`. Re-scope can therefore target the wrong token. If that read fails, only `params.key` is cleared; `params.plan` remains the new plan, so subsequent selections never retry. I reproduced both behaviors. Retaining names during refresh must not retain actionable stale aliases, and retry eligibility must survive a same-plan selection.

2. **[P2] The JS dynamic-text leak from round 1 remains.**  
   [tamber-inspect.mjs:111](/tmp/rv-dt-t3r2/scripts/tamber-inspect.mjs:111), [152](/tmp/rv-dt-t3r2/scripts/tamber-inspect.mjs:152).  
   The JS producer reports compound dynamic expressions using custom properties ([inspect-dev.js:109](/tmp/rv-dt-t3r2/host/web-js/inspect-dev.js:109)). For example:
   `color-mix(in oklab, var(--x-param-f12345678-Card-fill) 50%, black)`.  
   `names` becomes the full param name, but this `text` survives unchanged in both `selection` and published `selected`. The dotted-name regex cannot match that representation. This round 1 point was not fixed.

3. **[P2] The regex corrupts valid non-param tokens.**  
   [tamber-inspect.mjs:111](/tmp/rv-dt-t3r2/scripts/tamber-inspect.mjs:111).  
   A legal design token named `ui.param.face.fill` is rewritten to `ui.a component param`; its name disappears and its row becomes permanently “not read yet.” Only names *starting* with `param.` are reserved by the compiler. The regex lacks token boundaries and also accepts shorter hashes and fewer segments than the real generator produces.  
   I found **no missing legal identifier characters**: the parsers’ ASCII letters, digits, underscores and hyphens are covered. The problem is overmatching. The actual generator uses eight hexadecimal digits followed by `Component.param` ([params.rs:34](/tmp/rv-dt-t3r2/contract/lower/src/params.rs:34)).

4. **[P2] Role aliases still miss supported casing.**  
   [tamber-inspect.mjs:120](/tmp/rv-dt-t3r2/scripts/tamber-inspect.mjs:120), [125](/tmp/rv-dt-t3r2/scripts/tamber-inspect.mjs:125).  
   With a declared `label` override, defaults `label` and `-exact-label` get an alias, but `Label` and `-Exact-Label` get `null`. Rust resolves these role spellings case-insensitively ([check.rs:346](/tmp/rv-dt-t3r2/tamber/src/check.rs:346)). Those valid params consequently lose re-scope. Normalize role lookup while preserving ordinary token-name case sensitivity.

The other substantive round 1 fixes check out:

- Published `selected` now uses the scrubbed report. Dotted synthetic expressions are normalized, including their reasons.
- File refreshes empty the map; another file refresh retries a failure. Older concurrent responses are discarded, and a restarted server begins empty.
- Params with `when` lines offer no alias. `modes` correctly remains the copy’s complete tuple.
- Direct inherited/dynamic param rows and moved-file rows retain full-name links. Internal unread flags stay inside the join.
- The `{publish}` callback contract preserves the other existing callers’ behavior: no callback, `null`, and explicit refusal responses all worked.

Smaller points:

- **Tests still miss the important lifecycle transitions.** [tamber-server.test.mjs:65](/tmp/rv-dt-t3r2/scripts/tamber-server.test.mjs:65) keeps the files at `d1`, uses only one plan, and answers immediately. It covers an initial failure followed by success, but not failure after a populated map, pending reads, concurrent responses, or restart. Fixed 5 ms sleeps remain. The expression fixture and assertions covering both events are improvements.
- **The protocol still overpromises.** [tamber-protocol.md:962](/tmp/rv-dt-t3r2/docs/tamber-protocol.md:962) promises full naming and failed-read retries that the paths above violate. Its blanket param-link promise is also broader than the code: a synthetic `color-mix(…param…)` row still gets `link: null`. The corrected tuple and `when` wording matches.
- The old-client “ignores unknown rungs” claim remains false for the pre-T3 UI: an aliased param still offers “Open ink.” The newer UI in this checkout explicitly handles params.
- The review disposition records approximately 118 ms for describe; I did not remeasure it.

Six existing tests that do not write files passed, plus in-memory lifecycle reproductions and 64 dotted-name scrubbing cases. I did not run suites whose setup writes files or creates Git repositories. No files or Git state changed.
