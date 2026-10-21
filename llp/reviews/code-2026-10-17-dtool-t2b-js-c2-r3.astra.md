# Code review: LLP 1122 T2b, the JS target, step 2 (the inspect table and row keys), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `8a74a0cf8`.
- **Method:** one brief (sha256 `0004b89048446fee514d5a5ae37c37d7940f63e5f352a94cb0b167be58116b84`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium, 1 low).
- **Disposition:** taken in part (the last round).
  - The relative flag is the kernel's own parse (`relative::parse`): `em`, `rem` and a hairline in any case or spelling, and nothing else, so `ch`, `lh` and calc text keep the table's value. Tested: the `em` padding is flagged and the plain font size is not.
  - A control start is recorded even where an expression binds the row, so a cleared expression row falls back to the control's initial value.
  - The table carries its plan's digest (`contract::plan_digest`) and each token's build-time slices, for step 3's checks.
  - Limits, for §10: a text style's size (a heading's too) is its ramp at the 16 px root, so a pinned root moves it in `node` and not here. The CSS map is `css_property`, which misses a few rows the emitter writes elsewhere (the press scale, the control size, legacy line clamps, timelines). Tokens an expression names are recovered only through custom properties, so family, type and inline tokens aren't.
  - Not taken: an exact release baseline in the test (the release rt.js was checked equal to main's by hand at each round).

---

No High findings. Four Medium findings and one Low remain.

1. **Medium — Relative-value detection still misses supported values.** [inspect.rs:189](/tmp/rv-dt-js2r3/host/web-js/src/inspect.rs:189)  
   `u` examines the literal’s spelling instead of its parsed units. It misses platform text styles, relative lengths behind tokens, uppercase units and exponent notation. For example, the existing measures fixture’s `line.hair = -exact-hairline` produces `u:false`, leaving the table’s `1` instead of `0.5` on a 2× display. A literal `font-size="-exact-title1"` similarly retains its build-time size after a root-font pin. The round-2 fix is therefore incomplete. **Fix:** derive relative metadata from each tuple’s parsed `StyleProps.relative`, including text styles and token expansion.

2. **Medium — Cleared dynamic rows lack control-default fallbacks.** [inspect.rs:110](/tmp/rv-dt-js2r3/host/web-js/src/inspect.rs:110)  
   The `!dynamic` condition suppresses `i` even when the static kernel identifies a control start. With `button appearance="auto" font-weight=(on ? 700 : none)`, the cleared state reports `initial: 500` in the neutral runner. The table supplies a potential dynamic row but no initial override; its global initial is `400`. This occurs without a dynamic ancestor, so the stated ancestor-reset limitation does not cover it. **Fix:** record control-start fallback candidates independently of potential own bindings, as the new `s` handling does for text styles.

3. **Medium — The CSS lookup table names properties the runtime never writes.** [inspect.rs:58](/tmp/rv-dt-js2r3/host/web-js/src/inspect.rs:58)  
   `css_property` is a spelling helper, not the emitter’s complete mapping. A dynamic `-exact-press-scale` is stored in `--exact-press` by `rows.rs`, but the table points to `press-scale`. Its value is otherwise null, so following this metadata loses both its current value and ownership. Other exceptions include control size, drag timelines and composite transform rows. **Fix:** describe the actual DOM properties/attributes and decoding needed for each emitted row, sharing the emitter’s mappings; explicitly identify rows with no DOM representation.

4. **Medium — The token-provenance limitation extends beyond family tokens.** [inspect.rs:230](/tmp/rv-dt-js2r3/host/web-js/src/inspect.rs:230)  
   `properties()` also excludes type tokens and tokens classified by `inline_tokens`, including statically used font-weight tokens. Suppose `weight.a` and `weight.b` both equal `600` and have literal font-weight uses elsewhere. A dynamic font-weight choosing between them writes the same `600`, while neither appears in `props`; `node` still distinguishes their names and original text. The declared family-token limitation is too narrow. **Fix:** retain evaluated token attribution before CSS mapping in development metadata; otherwise explicitly defer all these inline-token cases.

5. **Low — The tests do not prove the latest fixes or release equality.** [inspect_tests.rs:98](/tmp/rv-dt-js2r3/host/web-js/src/inspect_tests.rs:98), [inspect.test.mjs:17](/tmp/rv-dt-js2r3/host/web-js/inspect.test.mjs:17)  
   The Rust tests never assert `u`, exercise cleared control bindings, change state/modes, or execute row lookup. Instance keys are discarded and boundary-implied values are skipped. The release subsequence assertion still accepts an empty runtime. **Fix:** add the concrete cases above, compare reconstructed reports through changes, exercise registry lifecycle/adoption/replacement, and assert an exact release baseline.

The taken `vm::is_literal` fix, eight boundary-implied row names, and `onEnd` cleanup are correct. The new `s` branch correctly records the static shorthand fallback beside a potentially dynamic longhand. Type names, arm/row chain encoding and tuple numbering agree with the runner. The `project` extraction preserves operations and projection order.

For connected, adopted rows, the registry’s single-element and bracketed matching follows moves correctly. The documented replacement failure remains; nested lazy regions also remain unavailable until adoption. Cost is nonzero: a registry record and cleanup callback per region, a field/write per row, and linear scans during lookup. There is no timing evidence establishing its practical cost.

I verified in memory that stripped `rt.js` exactly equals `HEAD~1`: **117,314 bytes**. Both non-dev paths strip it and exclude the side table. Complete artifact equality was not established; ordinary builds already embed revision/time metadata. Builds and tests were not run because they write files.

**Verdict: Request changes — release isolation looks sound, but the inspect metadata still falls short of row-for-row `node` parity beyond its stated limits.**
