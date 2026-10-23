# Code review: stage 13 B, the pen importer, round 3's last re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `efaa47f5ca`.
- **Method:** one brief (sha256 `e628bf6654d5fb769c8afb8de6265d9942c5aaffd69b20e77d56edaa3f7f044f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [landing.rs:243](tamber/src/import/landing.rs:243): Fold charges count tokens, but `laid` copies their full values. A sub-1-MiB file containing a 500-KiB family plus thousands of numeric candidates can cause gigabytes of copying during narrowing, outside the queued checker internals. Charge copied bytes per probe.
- [landing.rs:416](tamber/src/import/landing.rs:416): Exhaustion stops folding but leaves candidate enumeration running. Every remaining candidate clones the entire skip mask; subsequent diagnostics repeat this before the sentinel at line 478. This leaves quadratic work after exhaustion. Propagate failure immediately.
- [export_dials.rs:347](tamber/src/export_dials.rs:347): The prior marks finding remains. Each prior reference rescans and lowercases the imported value. Long accepted RGB literals therefore multiply attacker-controlled bytes by reference count after the import budget ends.

**Should-fix**

- [import_pen.rs:969](tamber/tests/it/import_pen.rs:969): The family fixture requires over 140 million resolution units against a roughly 50-million-unit budget, so it never reaches probes. Results remain discarded at line 1046; the expression fixture likewise does not assert its intended refusal. Assert the targeted outcomes with fixtures that reach those phases.

**Nits:** None.

The number/type guards, lookup replacements, diagnostic truncation, and restored candidate narrowing otherwise look correct. No additional decision regression found.

Source inspection only: both permitted test commands failed before execution because Cargo could not write temporary files. No files changed.
