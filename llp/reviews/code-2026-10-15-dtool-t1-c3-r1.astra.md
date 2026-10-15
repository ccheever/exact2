# Code review: LLP 1122 T1 commit 3 (the dials panel and the token grid), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `6b334829e`.
- **Method:** one brief (sha256 `15658f15d66173394479df3451f8eaa8e8b5a167a2754788b1c01eb810c54a25`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 6 medium, 4 low).
- **Disposition:** taken, on top of commit 4 (1eab66090), which had already wired the controls (the read-only finding is moot there): each design read is its own snapshot, by number, and contrast and a cell's branch name the read they are of; contrast is text composited over an opaque background, a translucent background said so; metadata that does not fit a dial's kind falls back to its own control, said in its help, a step must be a positive number (none: one fine enough for the value), a priority 1, 2 or 3; a select's value matched by meaning; columns labelled `axis=value`; roles from the IR's `role`; a residual's "fallback" its own badge; a failed design read has Try Again and is read again after a failed poll; the gate compares every control's range, step, value and order; the stub refuses a file it has no design for.

---

The main blockers are stale contrast results and incorrect ratios for translucent colours.

1. **High — Contrast does not follow the completed design load.** [app.contract:114](/tmp/rv-dt-3/apps/design-tool/app.contract:114), [app.ts:196](/tmp/rv-dt-3/apps/design-tool/app.ts:196)  
   A change to `since` starts both resources together. `loadContrast` immediately reads the previous cache while `loadDesign` awaits HTTP; completing the design does not rerun contrast. Changing files does not invalidate contrast either. Thus an edit can update the swatches while leaving the old ratios indefinitely. **Fix:** calculate contrast from the completed design snapshot supplied as a resource argument, including its file identity.

2. **High — Contrast ignores alpha.** [app.ts:225](/tmp/rv-dt-3/apps/design-tool/app.ts:225)  
   `#00000000` against opaque white reports **21:1**, although transparent foreground over white produces **1:1**. This also affects the fixture’s translucent `text.system` fallback. The opaque sRGB luminance formula is correct, but “stored values” does not make discarding alpha valid. **Fix:** establish foreground/background order and composite before calculating luminance; report an unresolved backdrop when transparency prevents a meaningful ratio.

3. **Medium — Overlapping design loads corrupt shared caches.** [app.ts:160](/tmp/rv-dt-3/apps/design-tool/app.ts:160), [app.ts:166](/tmp/rv-dt-3/apps/design-tool/app.ts:166)  
   `columns` changes before awaiting resolves, and `stored` changes afterward. When switching files quickly, a late load can overwrite the current file’s cache even though the resource runtime discards its obsolete reply. Another interleaving builds the current file’s cells using the previous file’s columns. **Fix:** keep columns and values local to each load and return one immutable snapshot; eliminate shared mutable state between these resources.

4. **Medium — The default step can misrepresent a valid dial.** [app.ts:103](/tmp/rv-dt-3/apps/design-tool/app.ts:103)  
   Every number without explicit metadata receives `step=0.01`. A valid dial with value `0.001` and range `0..0.005` consequently has no nonzero permitted slider step; the browser sanitizes its displayed value while the adjacent label still says `0.001`. **Fix:** use `step="any"` when omitted, or a default that preserves the declared range and value.

5. **Medium — Invalid metadata creates unusable controls.** [app.ts:98](/tmp/rv-dt-3/apps/design-tool/app.ts:98)  
   Tamber preserves editor metadata without validating it. This mapping accepts `control=stepper` for a colour, producing a numeric input whose hexadecimal value becomes blank. It also accepts zero/negative steps and partially numeric strings through `parseFloat`. **Fix:** validate the kind/control combinations, positive steps and integer priorities; surface invalid metadata as data or apply an explicit, consistent fallback.

6. **Medium — Select options cannot reliably match the stored value.** [app.ts:104](/tmp/rv-dt-3/apps/design-tool/app.ts:104), [app.contract:267](/tmp/rv-dt-3/apps/design-tool/app.contract:267)  
   Options remain raw strings while dial values are folded. With D15-style options such as `brand.red|brand.blue`, the select’s value is a CSS literal and matches neither option. Even literal `#1083fe` does not match the server’s `#1083feff`. **Fix:** resolve and normalize option values while preserving their labels, and explicitly handle an empty list or unmatched current value.

7. **Medium — A transient design failure has no recovery path.** [app.contract:113](/tmp/rv-dt-3/apps/design-tool/app.contract:113), [app.ts:165](/tmp/rv-dt-3/apps/design-tool/app.ts:165)  
   If one resolve request times out after connection succeeds, `noDesign` becomes the settled result. Successful polls with unchanged sequence/epoch never retry it, and the error display offers no retry action. **Fix:** provide a design retry and refresh failed design reads during reconnection or bounded retry handling; invalidate dependent contrast on failure.

8. **Medium — The tests do not establish the claimed metadata gate.** [design-tool.test.mjs:129](/tmp/rv-dt-3/scripts/design-tool.test.mjs:129), [tamber-stub.ts:42](/tmp/rv-dt-3/apps/design-tool/stub/tamber-stub.ts:42)  
   The comparison omits ranges, steps, values, help and options; sorting removes ordering assertions. Screen checks inspect test IDs rather than actual control properties. Hard-coding every slider’s maximum or step would still pass. The stub also returns unchanged design data after events and ignores the requested file, masking refresh bugs. **Fix:** compare rendered control properties and order, include select/default/invalid-metadata cases, and exercise changed values, file switches and delayed replies. The existing real-server comparison meaningfully checks canned IR and resolves, but only for the static fixture.

9. **Low — The “read-only” controls remain editable.** [app.contract:261](/tmp/rv-dt-3/apps/design-tool/app.contract:261)  
   Fields, sliders and selects have neither handlers nor `readonly`/`disabled`. A person can move a slider or change its text locally while the stored value, swatch and grid remain unchanged. **Fix:** make text fields read-only and disable sliders/selects until mutation handlers land.

10. **Low — Distinct tuples can have identical labels.** [app.ts:136](/tmp/rv-dt-3/apps/design-tool/app.ts:136)  
    Axis names and default values disappear from labels. With two axes both offering `normal|compact`, two different columns become “Light · compact”; their contrast rows are indistinguishable too. **Fix:** include `axis=value` in column and ratio labels.

11. **Low — Uppercase token names are incorrectly grouped as roles.** [app.ts:185](/tmp/rv-dt-3/apps/design-tool/app.ts:185)  
    A legal token such as `Brand.accent` enters “roles” solely because it starts uppercase. The IR already supplies the authoritative `role` field, which this transformation drops. **Fix:** retain that field and use it to identify role overrides.

12. **Low — Residual labels hide their fallback qualification.** [app.contract:224](/tmp/rv-dt-3/apps/design-tool/app.contract:224)  
    The qualification appears after the residual expression inside a 96-pixel, single-line cell. Long role names—and especially platform-colour expressions—clip away “fallback”. **Fix:** display a separate visible fallback badge and make the complete expression available without truncation.

Tuple enumeration matches `modes_of` and the IR’s axis order; pruning redundant light columns is consistent with `under scheme=dark`. The added sRGB fields use the existing alpha-preserving clip, and token dial dependencies match `explain`. The length-fixture omission follows current Tamber restrictions. No files were changed and no builds or tests were run.

**Verdict: Request changes before landing.**
