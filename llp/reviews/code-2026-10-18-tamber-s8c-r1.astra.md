# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `c0cc97afe` (C1 only).
- **Method:** one brief (sha256 `d189f2921e3b0a852bff2a131e845fd3b0dd8e4772fa485b977fc185884bb4d3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed on the pre-landing branch (the reader's column, theme2's best-effort rule, split rows, unmeasured frames), folded into the landed C1.

---

- **BLOCKING — [scripts/measure-controls.mjs:144](/tmp/rv-rl-c1/scripts/measure-controls.mjs:144):** `near[i] < moved` compares against the authored/plain displacement, not the default’s distance from the target. Reproduced: `[150,110,130]` is closer to `#e0218a` than default `[128,128,128]` (17.94 versus 28.29), yet returns `none`; `[0,0,64]` is farther (43.29), yet returns `best-effort`. The required `near[i] > 3` guard is also missing.

- **BLOCKING — [scripts/measure-controls.mjs:98](/tmp/rv-rl-c1/scripts/measure-controls.mjs:98):** Android runs the headless Linux carrier, whose `run_config()` selects the neutral look, rather than D9’s Android reader. This produces incorrect `none` cells: the reader paints `Field` in `material.rs:177` and preserves authored accent on disabled prominent buttons in `button.rs:487,557`. Measure the reader and regenerate its column.

- **SHOULD-FIX — [kernel/tables/schema.json:5800](/tmp/rv-rl-c1/kernel/tables/schema.json:5800):** `progress` conflates determinate bars with indeterminate spinners. Without `value`, iOS, web and Linux use spinner ink rather than `accent-color`, contradicting their `exact` cells. Represent both variants so C2 can warn correctly.

- **SHOULD-FIX — [scripts/fixtures/control-roles/app.contract:27](/tmp/rv-rl-c1/scripts/fixtures/control-roles/app.contract:27):** The approved off/disabled/focused measurements are absent, apart from one disabled button. `capture()` takes only the initial state. Add those states and sample their actual painted regions; inset node rectangles omit exterior focus rings and popup content.

- **SHOULD-FIX — [scripts/measure-controls.mjs:112](/tmp/rv-rl-c1/scripts/measure-controls.mjs:112):** Empty or invalid samples silently become `none`. A two-point-high magenta frame yields zero samples and `ΔE = Infinity`; an offscreen frame yields undefined RGB values and `NaN`. Both reproduced. Validate bounds and sample sufficiency, and report measurement failure separately from unsupported colour.

The two committed tests pass. Host builds and captures were not rerun under the read-only constraint.
