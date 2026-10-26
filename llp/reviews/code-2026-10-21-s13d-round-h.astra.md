# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `0c1dd8f1a3`.
- **Method:** one brief (sha256 `5bd2ed6e475c7dc13059a38682c9183c7ea2fe8c3c7db5d122b8d362f497a77d`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two blockers remain in `0c1dd8f1a3`:

1. **P2 — [contract/lower/src/theme/scan.rs:323](contract/lower/src/theme/scan.rs:323): shared spans suppress unrelated expanded nodes.** Component expansion preserves declaration spans. A slot component whose root is `column -exact-row-look="platform" padding-left="gap"`, used as a section row with another instance inside it, therefore suppresses both instances’ padding in this scan. Lowering yields only the outer instance’s look; the inner padding remains. Changing `gap` from `8px` to a colour escapes the row check but breaks the build. **Fix:** track individual expanded-node paths, with rebuilt `delete=` content mapped to its original node. Add a nested-component regression that requires the compiler’s diagnostic.

2. **P2 — [contract/lower/src/theme/scan.rs:319](contract/lower/src/theme/scan.rs:319): nested grouped rows discard inherited selections.** Consider a section’s handwritten swipe naming a marked descendant inside `list appearance="auto" → section → column → row id="content"`. Outer swipe lowering yields that row’s look. At the inner section’s `column`, the scanner replaces the inherited set with the column’s span, so it checks the content’s padding again. A token change can consequently be refused even though the build accepts it. **Fix:** preserve ancestor selections when adding nested row selections, using the distinct identities from item 1; add an acceptance-parity regression.

Otherwise, control flow through `over()`, header/footer exclusion, and rebuilt delete content retaining its span align with lowering.

Validation: source review and an in-memory traversal trace. Rust integration tests were not run; no files were changed.
