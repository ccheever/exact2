# Code review: Bindings of lengths, numbers and fonts, and the DTCG export of every kind (LLP 1121 stage 6 G), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~3..HEAD, the export, the bindings, the sample host and `prefer bold-text`, and the first round.
- **Method:** one brief (sha256 `ce340d2773c7d4e692f16169797a31528095b92dbb8e35c9f33287ee2cccd1a2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P1s: untouched tuples changed across an axis under scheme=dark, a reprinted type size losing its text style; P2s on a leading placeholder, a currentcolor colour, a module's system designs).
- **Disposition:** applied without re-review (the last round), in the second-round commit.

---

Blocking issues remain.

- **P1 — Re-import changes untouched tuples.** [export_kinds.rs:495](/tmp/rv-s6g-2/tamber/src/export_kinds.rs:495): With `dark-style=black|dim under scheme=dark` and `radius=12px`, editing only dim’s light/dark values to `16px` proposes changing the default. Filtering out the dark-only axis broadens the edit; skipping unchanged tuples during validation lets it change black too.

- **P1 — Reprinting still destroys a typography size’s source.** [export_kinds.rs:316](/tmp/rv-s6g-2/tamber/src/export_kinds.rs:316): Export `-exact-body/1.35 InterVariable`, reprint numbers through JSON, then change only its weight. Re-import proposes `600 16px/1.35 InterVariable`, losing Dynamic Type because this comparison still distinguishes `16` from `16.0`. Reproduced.

- **P2 — Explicit leading masks incoming line-height edits.** [export_kinds.rs:319](/tmp/rv-s6g-2/tamber/src/export_kinds.rs:319): For `14px/18px ui-monospace`, changing exported `lineHeight` to `2` while retaining extensions still proposes `18px` leading. The canonical check validates that reconstruction, and reports the edit as mapped. Reproduced.

- **P2 — A currentcolor shadow cannot accept a colour edit.** [export_kinds.rs:345](/tmp/rv-s6g-2/tamber/src/export_kinds.rs:345): Changing the exported colour of `1px 1px currentcolor` to red retains the `currentcolor` extension, so re-import emits `1px 1px 0px` and silently discards red. Reproduced.

- **P2 — Native-module fonts lose system-family variants.** [ExactTamber.swift:108](/tmp/rv-s6g-2/host/apple/modules/ExactTamber.swift:108): A `ui-monospace` token resolves to `.AppleSystemUIFontMonospaced-Regular`, but reconstruction uses plain `systemFont`. AppKit confirmed the result loses fixed pitch.

All 19 existing export tests passed. Worktree and index unchanged.
