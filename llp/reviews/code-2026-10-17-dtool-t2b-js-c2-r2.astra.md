# Code review: LLP 1122 T2b, the JS target, step 2 (the inspect table and row keys), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `a6855fe7d`.
- **Method:** one brief (sha256 `1df2a917e0e11f486e868f9c6f28dc35c7834ab4e3d985ec914e73ff5cf47f2e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (5 medium, 1 low).
- **Disposition:** taken in part.
  - 1: a literal length relative to the font or root (`em`, `rem`, and the like) or to the display (a hairline) is flagged (`u`). The page reads it from the element, whose font and display are the instance's, so a dynamic font-size beside it is right.
  - 3: a platform text style's fallback is recorded even where an expression binds the row: a cleared expression row falls back to it. The style's value in each tuple is in the table too (`s: [site, value]`), so the page doesn't read the CSS.
  - 2, 4, 5: limits, for §10. A control start under an ancestor whose row only an expression sets is not recorded. A family token named by an expression isn't recovered (the page sees only its CSS). A row root a host replaces (the markup editor's textarea) loses its key.
  - 6: as before. The release check is a subsequence check, and the runtime's parity is step 3's test, against the wasm page.

---

Five Medium findings and one Low; no High findings.

1. **Medium — Literal lengths still freeze dynamic font context.** [inspect.rs:126](/tmp/rv-dt-js2r2/host/web-js/src/inspect.rs:126)  
   With `for size in [20, 32] key=size` containing `text font-size=size padding-left="1em"`, the table records padding `16` for both instances; the runner resolves `20` and `32`. The new static-tree resolution fixes fully static ancestry only. This exceeds the stated pinned-root limitation. **Fix:** retain relative-unit metadata and resolve against the selected instance’s current font, root size and display scale.

2. **Medium — Control resets still disappear beneath dynamic ancestors.** [inspect.rs:106](/tmp/rv-dt-js2r2/host/web-js/src/inspect.rs:106)  
   Change the new fixture’s parent to `text-transform=transform`, with state initially `"uppercase"`. That ancestor has no static mask, so the button’s initial-valued `none` reset is omitted again. Walking the table’s ancestors then finds the dynamic uppercase row, although `node` reports the button and title as initial. **Fix:** encode the actual reset boundary independently of whether an ancestor currently has a static declaration.

3. **Medium — Potentially dynamic rows lose their fallback sources.** [inspect.rs:92](/tmp/rv-dt-js2r2/host/web-js/src/inspect.rs:92)  
   The existing `conformance/text-style.contract` contains a **static** `font="-exact-headline"` with `font-weight=(big ? 300 : none)`. When cleared, the runner reports the weight’s source as `style`. Because `dynamic` bypasses this branch, the table records only the potential dynamic row and omits its style fallback. Conversely, a static bare text style can receive an unconditional `s` entry despite a dynamic ancestor supplying the weight. **Fix:** record fallback candidates separately from potential own rows, then resolve precedence using current instance state. The acknowledged dynamically chosen style limitation does not cover these cases.

4. **Medium — Property inversion cannot recover inline token provenance.** [inspect.rs:204](/tmp/rv-dt-js2r2/host/web-js/src/inspect.rs:204)  
   For `state family = "family.mono"` and `font-family=family`, the runner reports that token and its original text. `modes::properties` excludes family/type tokens, while the runtime maps the value directly to CSS such as `ui-monospace`. The table supplies neither `k`/`x` nor an invertible property reference. Two tokens with identical CSS are indistinguishable. This fails even for a single token, beyond the disposition’s mixed-text limitation. **Fix:** retain evaluated token text and attribution before CSS mapping in development-only metadata.

5. **Medium — Row lookup loses a replaced single-element root.** [rt.js:1217](/tmp/rv-dt-js2r2/host/web-js/rt.js:1217)  
   An `each` whose row is a Markdown textarea records that textarea as both endpoints. The editor subsequently replaces it ([markup-editor.js:456](/tmp/rv-dt-js2r2/host/web/markup-editor.js:456)); `mde` updates `Views`, but the row still references the detached textarea. Inspecting the replacement returns no row key. **Fix:** resolve single-element roots through stable view identity, or update the development registry when the host replaces an element.

6. **Low — Tests still do not establish the claimed parity.** [inspect.test.mjs:17](/tmp/rv-dt-js2r2/host/web-js/inspect.test.mjs:17), [inspect_tests.rs:73](/tmp/rv-dt-js2r2/host/web-js/src/inspect_tests.rs:73)  
   The release subsequence assertion still accepts an empty runtime. Rust comparisons omit instance keys, skip dynamic values, and accept boundary-implied rows after checking only the boundary flag. No test executes registry lifecycle or replacement behavior. **Fix:** assert release equality against a baseline and compare reconstructed reports through state changes, clears, mode changes, adoption, replacement and disposal.

The `vm::is_literal` fix correctly handles literal `none`; the eight implied boundary rows now match the runner. The `onEnd` cleanup fixes the reported disposal leak, and disconnected regions are correctly skipped without deletion. Type names, region-chain encoding and tuple numbering look consistent. The `project` extraction preserves operation and projection order.

I verified **in memory** that stripped `rt.js` exactly equals `HEAD~1`: **117,314 bytes**. Both production and plain non-dev paths strip the additions and exclude the side table. Complete artifact equality remains unverified; plain builds also retain existing timestamp/revision metadata.

Other limits need stating: table values remain build snapshots during live token batches; nested lazy regions acquire registry entries only upon adoption. Registry overhead is a record per region, a field per row and linear lookup scans; no timing evidence establishes its cost. Builds and tests were not run under the read-only constraint.

**Verdict: Request changes—the release isolation looks sound, but the table and row registry still cannot support reliable row-for-row `node` parity.**
