# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), final, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `7a88718ef` (quick final check (Astra only)).
- **Method:** one brief (sha256 `3701a73b83d5d57d24925acccca29d7f3d1543140f14c30ab4c422e9fcd7b380`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 9ce32cadc (C2), folded.

---

- **SHOULD-FIX** — [contract/lower/src/control_roles.rs:68](/tmp/rv-rl-c5/contract/lower/src/control_roles.rs:68): `956e143cf` combines mutually exclusive class branches. A disabled button with `class=(flag ? Primary : Plain)`, where Primary sets `buttonStyle="filled"` / `accent-color="auto"` and Plain sets `buttonStyle="plain"` / an authored accent, now earns the iOS disabled-prominent warning. Accent selection and control detection independently combine branches, although neither branch has an authored accent on a prominent button. Keep these decisions correlated per branch.

No BLOCKING issues found. Both measurement tests pass; the recomputed schema digest matches its pin. Rust lint tests were not run because this read-only worktree has no build artifacts.
