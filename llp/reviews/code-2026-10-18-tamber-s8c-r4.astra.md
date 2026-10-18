# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r4, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `182f4d61c..7451b70bd` (C1 round 4, C2 round 3).
- **Method:** one brief (sha256 `5432ee8fd9cfc972d8bd04db0b0b43b6833fe05aea79cbd1ab0a55c0897288b5`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 0357e064a (C1), 956e143cf and 7a88718ef (C2), folded.

---

- **SHOULD-FIX — [control_roles.rs:216](/tmp/rv-rl-c4/contract/lower/src/control_roles.rs:216):** Conditional classes are concatenated, then `rfind` considers only the last branch’s row. `class=(flag ? Primary : Plain)`, with `filled` versus `plain`, misses the iOS disabled-button warning. Likewise, `Brand` versus `Auto` can suppress the Mac switch warning. Preserve alternatives separately from node-over-class precedence.

- **SHOULD-FIX — [control_roles.rs:176](/tmp/rv-rl-c4/contract/lower/src/control_roles.rs:176):** `sure` retains a class’s colour even when the node overrides it with a possibly cleared value. With `class=Red color=(flag ? "#ff0000" : none)`, the web’s `FieldText` warning disappears although the class row is replaced and one branch supplies no colour. Class rows containing `none` are also counted as guaranteed.

- **SHOULD-FIX — [control_roles.rs:225](/tmp/rv-rl-c4/contract/lower/src/control_roles.rs:225):** Any `appearance="none"` alternative exempts the entire control. A checkbox switch with `appearance=(flag ? "auto" : "none")` and an authored accent receives no Mac warning, despite using `NSSwitch` in the `auto` branch. Exempt only controls that are always custom.

- **SHOULD-FIX — [schema.json:5918](/tmp/rv-rl-c4/kernel/tables/schema.json:5918):** Radio and range lack disabled rows, so disabled instances inherit `exact` fidelity. The Linux painter applies `MARK_DISABLED_ALPHA` to both, altering their authored colour. The fixture measures only enabled instances. Add disabled measurements and corresponding rows, including Windows’s inferred cells.

Both measurement tests passed; the recomputed schema digest matches the pin. Rust integration tests and native measurements were not rerun.
