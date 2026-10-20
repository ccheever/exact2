# Code review: Type and family tokens, and Expose converted to them (LLP 1121 stage 6 C), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the change and Expose's conversion.
- **Method:** one brief (sha256 `de5a5868116498e6917c7b11b76c9c1c1d77c380ba5936edd81845a7f61c0283`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a P1: undotted token names interned as runtime families, Expose's fonts regressing natively; P2s on JS bindings, run-time names and the inherited-axis rule, `oblique`, undotted aliases, stack identity, capitals).
- **Disposition:** in the first-round commit.

---

Blocking issue found: Expose’s native fonts regress.

- **P1 — Bare token names bypass resolution.** [runner/src/bridge.rs:619](/tmp/rv-s6c-1/runner/src/bridge.rs:619): `NodeInst` calls `RuntimeFonts::intern` before reaching this branch. Expose’s `textFace`, `liltFace`, and `glyphFace` become installed font names instead of their bundled stacks. Native/wasm typography changes despite identical JS stylesheets.

- **P2 — JS refuses only some unsupported bindings.** [host/web-js/src/measures.rs:135](/tmp/rv-s6c-1/host/web-js/src/measures.rs:135): release builds without axes or scheme boundaries never call this guard. `font=(large ? "type.big" : "type.small")` therefore emits token names as CSS values. State/data references also evade `can_be`, including in development builds.

- **P2 — Runtime names bypass the inherited-axis rule.** [contract/lower/src/type_tokens.rs:154](/tmp/rv-s6c-1/contract/lower/src/type_tokens.rs:154): `column font-family=chosen`, where `chosen` contains a varying `"family.face"`, passes because only literal leaves are checked. The kernel accepts it, and text beneath a mode pin inherits the parent’s stack without re-selection.

- **P2 — Accepted `oblique` cannot reach the kernel.** [tamber/src/typeface.rs:93](/tmp/rv-s6c-1/tamber/src/typeface.rs:93): `oblique 16px` folds successfully, but `FontStyle` admits only `normal` and `italic`. Native rendering drops the style, JS projection fails, and a live `italic` → `oblique` edit leaves the old style unchanged.

- **P2 — Undotted family aliases become font names.** [tamber/src/typeface.rs:240](/tmp/rv-s6c-1/tamber/src/typeface.rs:240): with `textFace = "Inter"`, `family.copy = textFace` resolves to an installed font called `textFace`. The same happens inside a type token. The alias also loses its dependency on `textFace`.

- **P2 — Stack matching loses family identity.** [runner/src/bridge.rs:417](/tmp/rv-s6c-1/runner/src/bridge.rs:417): quoted `"serif"` collides with generic `serif`; `"A,B"` collides with the two-member `A, B` stack. Conversely, `-apple-system` finds no stack because lowering canonicalizes it to `system-ui`, while this lookup does not.

- **P2 — Uppercase text styles silently lose all longhands.** [kernel/src/style/theme/typeface.rs:30](/tmp/rv-s6c-1/kernel/src/style/theme/typeface.rs:30): Tamber accepts `type.body = -EXACT-BODY/1.4`, but this case-sensitive parser rejects it. Lowering treats that failure as an empty longhand set and emits only `fontShorthand`.

Rust tests and app launches were unavailable in the read-only sandbox. Expose’s literal substitutions matched in memory; worktree and index remain unchanged.
