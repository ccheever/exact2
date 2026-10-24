# Code review: the design checker's bound (rt1's PRIORITY finding), round 3, narrow re-check 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `aa097f3aa0`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at aa097f3aa0 (before the rebase).

**BLOCKER — [tamber/src/import/reach.rs:127](tamber/src/import/reach.rs:127): role spellings multiply graph storage.** Each distinct spelling gets a node that expands the same resolved token’s value again. Roots include words inside quoted family strings.

A fixture containing 40,000 case variants of `secondary-background` across quoted family values, plus a 4,090-byte platform-colour value containing repeated asset-name words, demonstrates this. The design is 849,515 bytes and 212 tokens, with every value within bounds. An in-memory reproduction counted **81,162,029 edges**: forward and reverse `usize` storage alone requires **1.21 GiB**, exceeding the importer’s 1 GiB limit. This work is uncharged. Share outgoing dependencies by resolved definition while preserving exact-name precedence.

No other BLOCKER / SHOULD / NIT found. Differential models matched the old behavior across 10,000 reachability fixtures and 20,000 profile fixtures, including ordering, role spellings, first-wins and cycles.

Rust tests were not rerun: the checkout is read-only and has no build artifacts. No files changed.
