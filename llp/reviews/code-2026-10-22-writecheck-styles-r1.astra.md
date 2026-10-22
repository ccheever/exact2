# Code review: The write check judges a style's rows where an element applies it (stage 13 D QUEUE), round 1, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `4f214a52b9`.
- **Method:** one brief (sha256 `1c48f9b05e117a57d996482695c0dab4a2fe0d6b12ea925622d7f6d34ea60442`), blind to the other reviewer; 3 rounds (theme2's 02:08 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **[P2] Component instances collide in the per-element key** — [scan.rs:488](contract/lower/src/theme/scan.rs:488). Inlining preserves the template element’s span across instances. Two instances applying the same style therefore share `(row, element)`. For a shared `font-weight=700` row, one instance’s already-invalid family can suppress another instance’s newly-invalid family. Use a unique expanded-node identity; retention builds are unmapped, so adding `instance` alone is insufficient. Test two instances with different font contexts.

- **[P2] Deduplication still permits several refusals for one style row** — [rows.rs:578](contract/lower/src/theme/rows.rs:578). A shared `font-weight="weight"` applied with normal and italic fonts, both lacking bold faces, passes at `weight=400`. Changing it to `700` produces different messages (“bold” versus “italic bold”), so both survive this comparison. Deduplicate qualifying **style** failures by authored row, selecting a diagnostic in build order; preserve separate own-row diagnostics.

- **[P2] Button appearance validation is missing from class eligibility** — [scan.rs:438](contract/lower/src/theme/scan.rs:438). With `button appearance="auto" class=Pad` and `Pad` containing `padding="gap"`, changing `gap` from `8px` to a colour produces a token-kind refusal here. Lowering instead stops in `button_appearance` with `lower-button-style-attr` because native buttons reject padding. This occurs before the background failure verdict, violating the explicitly required button exclusion and diagnostic parity. Share button preparation or exclude those elements’ class rows whole.

- **[P3] The `for` test cannot detect a missing exclusion** — [tamber_rows_style.rs:134](contract/cli/tests/it/tamber_rows_style.rs:134). The outer column already fails at the same style row; mistakenly checking the inner row produces an identical diagnostic that deduplication hides. `same_words` also checks only the first diagnostic. Add a case whose only application is inside the `for` and assert acceptance; cover `match` separately.

No files changed. Targeted tests could not start because the read-only sandbox denied Cargo’s target-directory creation.
