# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `8e0029eee..cfcef760e` (C1 round 3, C2 round 2).
- **Method:** one brief (sha256 `c9df51f7861670e417900b21a8f9770ba6070d614062091bc29ac7af97499861`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 182f4d61c (C1, with theme2's 08:42 ruling) and 9035e2cf9 (C2), folded.

---

- **BLOCKING — [schema.json:5926](/tmp/rv-rl-c3/kernel/tables/schema.json:5926):** Android `FieldText` is marked `exact`, but disabled fields use `GrayText` ([hosted.rs:392](/tmp/rv-rl-c3/host/linux/src/presenter/hosted.rs:392)). Without disabled field/textarea rows, the lint silently misses these unreachable overrides. Measure and record those states.

- **BLOCKING — [schema.json:5921](/tmp/rv-rl-c3/kernel/tables/schema.json:5921):** The web spinner’s explanation incorrectly blames browser behavior. Exact draws it with `currentColor` in [index.html:266](/tmp/rv-rl-c3/host/web/index.html:266). This is an Exact omission and needs the required explanation and QUEUE entry.

- **SHOULD-FIX — [measure-controls.mjs:206](/tmp/rv-rl-c3/scripts/measure-controls.mjs:206):** One capture per build cannot distinguish tinting from animation. An in-memory reproduction moving four unchanged default-colour pixels returned `best-effort`, ΔE00 4.22; stationary pixels returned `none`. Capture the plain build twice and mask unstable pixels.

- **SHOULD-FIX — [control_roles.rs:156](/tmp/rv-rl-c3/contract/lower/src/control_roles.rs:156):** Class rows remain even when the node overrides them. A class’s `appearance="none"` suppresses a switch warning despite its own `appearance="auto"`; a class’s prominent style triggers warnings despite its own `buttonStyle="plain"`. Apply lowering’s class precedence before classification.

- **SHOULD-FIX — [control_roles.rs:153](/tmp/rv-rl-c3/contract/lower/src/control_roles.rs:153):** Field exemptions misclassify certainty both ways: conditional classes that *both* supply `color` still warn for `FieldText`, while `color=(flag ? "red" : none)` suppresses the warning even when the row clears. Determine whether an effective override exists in every branch.

- **SHOULD-FIX — [control_roles.rs:65](/tmp/rv-rl-c3/contract/lower/src/control_roles.rs:65):** Any `accent-color` attribute counts as an override. A macOS switch with `accent-color="auto"` and no theme override therefore warns about an authored colour that does not exist. Handle `auto` and literal `none` explicitly, preserving inheritance correctly.

- **SHOULD-FIX — [control_roles.rs:225](/tmp/rv-rl-c3/contract/lower/src/control_roles.rs:225):** `progress start=0 end=120000` is classified as a spinner. Lowering and the kernel recognize it as a timed progress bar, so an authored accent incorrectly warns on iOS/web. Include `start` in bar detection.

- **SHOULD-FIX — [control_roles.rs:101](/tmp/rv-rl-c3/contract/lower/src/control_roles.rs:101):** The claimed terminal exclusion is missing: `CONTROL_HOSTS` includes `"terminal"`, so this guard admits it and emits warnings from its `none` cells whenever it appears among shipped targets.

Both measurement tests passed; the digest recomputes to `0x24a0_0cfe_5916_60dc`. Mac disabled-cell corrections and thread-local reset paths are present. Rust/host tests were not rerun under the read-only constraint. No files changed.
