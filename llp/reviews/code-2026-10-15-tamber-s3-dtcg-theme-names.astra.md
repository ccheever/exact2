# Code review: DTCG: a Tokens Studio theme group with a nameless or repeated theme is refused, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `2baa503579b27c28b5ce0bb6583c818f6d441196f306f512c831216edf4fd8f4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (both).
- **Disposition:** landed as reviewed; Astra's optional test additions not taken

---

Nothing blocking found in HEAD `08e9dea18`; no correctness findings.

- [tamber/src/import/dtcg.rs:550](/tmp/rv-de1/tamber/src/import/dtcg.rs:550): Validation precedes scope selection, covering named and ungrouped groups. Valid light/dark pairs remain the scheme; other valid groups remain proposed axes. Single-theme groups bypass validation and remain `Scope::Base`.
- [tamber/src/import/dtcg.rs:541](/tmp/rv-de1/tamber/src/import/dtcg.rs:541): Missing/non-string names become empty strings, and `trim().is_empty()` also catches whitespace-only names. Exact duplicates trigger the new refusal. Distinct spellings that collide after `ident()` still reach `written` and are rejected by the existing check at [line 1040](/tmp/rv-de1/tamber/src/import/dtcg.rs:1040).
- [tamber/src/import/dtcg.rs:1035](/tmp/rv-de1/tamber/src/import/dtcg.rs:1035): The refusal becomes a `tamber-import-source` diagnostic, preserving the group-specific message. The CLI prints it and exits unsuccessfully.

**P3 — optional test strengthening:** [tamber/tests/import.rs:885](/tmp/rv-de1/tamber/tests/import.rs:885) covers empty names, exact duplicates in the ungrouped group, and successful single/distinct-theme imports. It does not explicitly cover omitted/whitespace-only names, folded collisions, light/dark themes, or the diagnostic ID. The single-theme assertion checks success rather than verifying base output. These are coverage opportunities, not observed defects.

`git show --check HEAD` passed. Tests were not built or run: no existing build artifacts were available, and the review remained read-only.
