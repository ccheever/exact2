# Code review: LLP 1122 T2b, the JS target, step 3 (inspect on the JS page), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `b1c22c4b5`.
- **Method:** one brief (sha256 `895dd9525b2f5ebb495b487f8ffc636ab468817b881a623d4b151720ac9ffe26`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 7 medium, 1 low).
- **Disposition:** taken in part (the last round).
  - A live edit of a token that doesn't vary by axis is seen: the table now records each token's build-time value and slices, and the page compares both. (Step 2's table, a one-line change, folded there.)
  - A written clear that inherits (`inherit`, `unset`) bypasses a control's start and a text style, as the runner keeps it written.
  - A colour scheme's `normal` is a clear. A font family's `initial` is its own row.
  - Escape before the module loads records the exit, so the server's replay is answered as the exit. A guest frame that takes focus meanwhile is blurred.
  - A boundary's implied row is `dynamic` when the site binds that row by an expression, as `node` says.
  - A report is read again if a live batch moves the design while it reads, and is made under the facts it read under.
  - A later click or refresh supersedes an earlier one still reading.
  - Limits, for §10: an expression row's ownership is read from what the page wrote, so an animation's `none` (no declaration) and a tint's `unset` (mapped to a value) disagree with `node`. The fixture test compares both pages' identities, not an expected one.

---

1. **High — Constant-token edits still report stale values.** [inspect-dev.js:120](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:120)  
   The “unchanged” check compares only `slices`. Tokens that do not vary by axis have empty slices, even when their `value` changes. Edit the fixture’s `accent.fill`: the SVG stroke changes, but inspection returns its build-time value under the new design digest. The server accepts it as current. **Fix:** store and compare both `value` and `slices`, then resolve changed values. This leaves the earlier live-edit fix incomplete.

2. **Medium — Explicit clears incorrectly retain control resets.** [inspect-dev.js:189](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:189)  
   Under static `text-transform="uppercase"`, give a button a dynamic transform currently equal to `"inherit"`. `setNow` drops its own row, then `entry.i` supplies initial `none`. The runner preserves the declaration’s `written` bit, bypasses the control start, and inherits uppercase. This exceeds the declared limitation concerning *dynamic ancestors*. **Fix:** distinguish absent bindings from explicit clears when resolving initial and style fallbacks, including through ancestors.

3. **Medium — DOM declaration presence still does not establish row ownership.** [inspect-dev.js:142](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:142)  
   A dynamic animation whose value is the string `"none"` loses its declaration through `style_writes`; inspection omits the row, although the runner retains an own dynamic animation row. Conversely, dynamic tint `"unset"` becomes a concrete custom-property value through `TINT_MAP`, so inspection invents an own row the runner cleared. These failures occur with correctly mapped property names. **Fix:** retain development-only binding ownership before CSS translation, rather than infer it from translated declarations.

4. **Medium — The early gate still permits guest-frame keyboard input.** [dev.mjs:468](/tmp/rv-dt-js3r3/host/web-js/dev.mjs:468)  
   Reload while inspecting and delay the module response. If app startup or the guest programmatically focuses an iframe, the early gate suppresses focus/blur events without moving focus back. The cover and `pointer-events` shield prevent pointer access, but subsequent keys—including Escape—stay in the guest document. **Fix:** implement focus rejection and guest-focus recovery synchronously alongside the early pointer gate.

5. **Medium — Early Escape is undone by inspect replay.** [dev.mjs:468](/tmp/rv-dt-js3r3/host/web-js/dev.mjs:468)  
   Press Escape while the inspector import is pending. `letGo()` removes the held flag but records neither the exited sequence nor a null selection. When the module loads, a queued/replayed `inspect {on:true, seq}` activates inspection again. **Fix:** persist the exited sequence and replay its null-selection report, matching `setInspect(false, true)`.

6. **Medium — Cleared dynamic bindings give implied rows the wrong source.** [inspect-dev.js:185](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:185)  
   Put a token colour above a mode boundary whose dynamic colour binding currently clears. The boundary correctly receives an implied token row, but this branch always labels it `authored`. The runner labels it `dynamic`: its source calculation tests the site’s dynamic binding whenever the resulting row is own, including an implied row. **Fix:** preserve that binding classification when emitting the implied row.

7. **Medium — An asynchronous snapshot can be stamped with a newer design.** [inspect-dev.js:174](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:174), [inspect-dev.js:236](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:236)  
   While a selection awaits the D1 design snapshot, another live batch applies D2. A delayed D1 response can supply D1 token values, while `env.facts()` labels the report D2. The server’s currency check cannot detect this. **Fix:** bind the report to captured plan/design/revision facts and retry if they change during asynchronous reads.

8. **Medium — Earlier clicks can overwrite later selections.** [inspect-dev.js:249](/tmp/rv-dt-js3r3/host/web-js/inspect-dev.js:249)  
   Two clicks before the first table fetch completes start independent report requests. If the second finishes first, the first can subsequently replace and persist its selection. The guard checks only the inspect command sequence; click tokens do not enforce ordering. **Fix:** reject superseded selection requests with a monotonic request identifier, including refresh requests.

9. **Low — The fixture still does not prove its claimed node coverage.** [tamber-server.test.mjs:675](/tmp/rv-dt-js3r3/scripts/tamber-server.test.mjs:675)  
   Comparing both pages’ `testId` values allows both to select the same unintended descendant. A button-centre click can select its text child, so button reset behavior remains untested. **Fix:** assert each expected selected identity, normalize `from` through site/instance identity, and exercise the cases above.

The plan check, empty-pin handling, installed focus gate, preservation of `computed` during trimming, and selection-sequence persistence look correct. The named SVG, property-map, style-fallback, outline-order and page-local-ID limitations are real; they still preclude universal parity.

Release exclusion looks correct by source inspection. The module endpoint is Host-gated, not authenticated to an individual page, and grants no cross-origin CORS access. The release test checks source strings rather than built artifacts. I did not run tests because they build and write files; the worktree is unchanged.

**Verdict: Request changes — live reports remain incorrect, and input isolation is incomplete during module loading.**
