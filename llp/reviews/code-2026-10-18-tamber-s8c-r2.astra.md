# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `1a61196df..a6d4db4a5 (as reviewed: ebd43a6a3, before the rebase)` (C1 round 2, C2 round 1).
- **Method:** one brief (sha256 `650d423336ffc484ac2aa5d8f195d4dfd1c125cb8fccf33f53b5b8e52c5b0cc0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 8e0029eee (C1) and cfcef760e (C2), folded into the landed commits.

---

- **BLOCKING — C1:** [measure-controls.mjs:193](/tmp/rv-rl-c2/scripts/measure-controls.mjs:193) adds hue/chroma restrictions beyond theme2’s rule. Reproduced: 20 pixels of `#857b81` against plain `#808080`, target `#e0218a`, return `none`, despite ΔE00 improving from **28.29 to 25.23**. This must be best-effort under the stated ruling.

- **SHOULD-FIX — C1:** [measure-controls.mjs:164](/tmp/rv-rl-c2/scripts/measure-controls.mjs:164) checks pixel counts, not frame geometry or coordinates. Equal-count frames with different dimensions or positions pass. A shifted-coordinate probe returned `exact`, contradicting the documented `unmeasured` result for differing frames.

- **SHOULD-FIX — C2:** [controls.rs:69](/tmp/rv-rl-c2/contract/lower/src/warnings/controls.rs:69) passes raw attributes into control detection. A valid `style Primary` containing `buttonStyle="bordered-prominent"` therefore escapes the disabled-button warning. Conditional `buttonStyle` expressions are also missed.

- **SHOULD-FIX — C2:** [controls.rs:146](/tmp/rv-rl-c2/contract/lower/src/warnings/controls.rs:146) ignores supported conditional classes. An ancestor’s `class=(flag ? A : B)` accent produces no warning; a field whose two classes both supply `color` incorrectly receives a `FieldText` warning. Reuse the compiler’s class resolution.

- **SHOULD-FIX — C2:** [controls.rs:178](/tmp/rv-rl-c2/contract/lower/src/warnings/controls.rs:178) matches input types case-sensitively. `type="CHECKBOX" switch` and `type="TEXT"` compile normally through existing canonicalization but bypass this lint.

- **SHOULD-FIX — C2:** [controls.rs:97](/tmp/rv-rl-c2/contract/lower/src/warnings/controls.rs:97) silently skips `tvos`: shipped targets retain that name, while `CONTROL_HOSTS` has only `ios`. A tvOS spinner’s unreachable accent consequently produces no warning.

- **SHOULD-FIX — C2:** [theme.rs:212](/tmp/rv-rl-c2/contract/lower/src/theme.rs:212) clears compile state too late. Font/sound lowering can return before `declare_theme`, yet `lower_all` still runs warnings. A subsequent failed compile can inherit the previous app’s targets, role overrides, and diagnostic spans.

- **SHOULD-FIX — C2:** [controls.rs:69](/tmp/rv-rl-c2/contract/lower/src/warnings/controls.rs:69) ignores `appearance="none"`. A custom switch painting `background-color="AccentColor"` still gets “NSSwitch draws the system accent,” although the Mac host removes the native switch and the authored background can honour that accent.

Read-only verification: measurement tests **2/2 passed**, table/document fidelities agree, and the schema digest matches its pin. Rust tests and device measurements were not run because they write files. No files changed.
