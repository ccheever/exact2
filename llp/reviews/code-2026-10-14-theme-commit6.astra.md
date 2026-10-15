# Code review: LLP 1121 stage 1, commit 6 (theme1's minors, no-tells, the Expose and reader ports), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `74ebaf63fe0e623734c18985a98c027fd1db67a2134e1cc78b8ef415d5b4e0e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in c0910653b: the JS bound-colour reset, the scheme rule's precedence (one rule at an id and a class), no-tells depth, the LLP text; also the element lint

---

- **MAJOR — Bound authored colours lose to the theme.** [css.rs:399](/tmp/rv-c6/host/web/src/css.rs:399) clears `--exact-scheme-color` only during CSS generation. The JS target’s bound `color` writes no corresponding reset. With `column color=ink` containing a dark-scheme card, the card uses the theme’s CanvasText instead of inheriting `ink`; wasm preserves the authored colour. Add conformance coverage for bound colours and their updates.

- **MAJOR — Scheme changes override native field colours, inconsistently across targets.** [css.rs:406](/tmp/rv-c6/host/web/src/css.rs:406) adds `color` even to an input with no authored colour. Its generated class outranks the neutral stylesheet’s field colour. A bound scheme on JS instead uses the zero-specificity rule in [roles.rs:364](/tmp/rv-c6/kernel/src/style/roles.rs:364), which loses to that stylesheet; wasm emits the stronger inline declaration. Thus literal and bound versions of the same input differ. Preserve control colour precedence and test both forms.

- **MINOR — no-tells mistakes continued expressions for theme declarations.** [no-tells.mjs:92](/tmp/rv-c6/scripts/no-tells.mjs:92) recognizes a column-zero `theme` without tracking expression depth. A parenthesized `color=(…theme…)` spanning lines can trigger it, blanking the closing parenthesis and subsequent indented nodes. An in-memory reproduction reports two findings before this commit and zero afterward. Add a regression alongside the real-theme exclusion test.

- **MINOR — Reader documentation still describes the deleted API.** [LLP 1034:176](/tmp/rv-c6/llp/1034-scheme-aware-colour.rfc.md:176) and [LLP 1033:511](/tmp/rv-c6/llp/1033-markdown-viewer.plan.md:511) still describe `markdown-parse::theme`, the record, and provide/inject. Mark those descriptions superseded by the root theme.

Palette expansion matched every ported executable line; the reader values match the deleted table, and no new names collide with roles or reserved keywords. The generation check adds no new traversal on unchanged updates.

Shelf’s four Bun tests and `git diff --check` passed. Expose’s no-tells run found 210 type findings and zero colour findings. Cargo was blocked by read-only filesystem permissions; rendering remained unverified, and the Mac appearance probe was inconclusive. No files changed.

VERDICT: fix (MAJOR host/web/src/css.rs:399, host/web/src/css.rs:406; MINOR scripts/no-tells.mjs:92, llp/1034-scheme-aware-colour.rfc.md:176)
