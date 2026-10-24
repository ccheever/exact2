# Code review: the checker sweep (after the checker bound), round 3 of 3, 2026-10-24 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `03aba06dd3`.
- **Method:** one brief (sha256 `be0b41770b50ad20cb3251ec3e82e332a6495c0ee32ad812aaf8e1901a3594c6`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Four **SHOULD** findings remain:

- **SHOULD — Alternating imports still cause quadratic application.** [tamber/src/edit.rs:507](tamber/src/edit.rs:507). An import alternating existing tokens and new tokens produces singleton set/add runs, so neither batch applies. Each operation parses the whole design in `added()` and again in `step()`. A 6,000-operation example fits both request limits and causes 12,000 full-design parses in the shared server, outside the importer watchdog. Batch mixed edits or bound/isolate their application.

- **SHOULD — DTCG preprocessing still has unbounded pairwise scans.** [tamber/src/import/dtcg.rs:571](tamber/src/import/dtcg.rs:571). Group construction searches every preceding group before the new cap runs. Twenty thousand singleton groups fit in a 748,967-byte request and require 199,990,000 comparisons; each group passes the 64-member cap. [Set ranking also rescans every set for each sort key](tamber/src/import/dtcg.rs:541). Neither loop charges the work budget. Index groups and precompute ranks.

- **SHOULD — Re-import pin marking remains quadratic outside isolation.** [scripts/tamber-import-review.mjs:29](scripts/tamber-import-review.mjs:29). Every pin scans every operation. Changing many alias-backed tokens to literals generates this shape. An instrumented call to the actual `marked()` function with 8,000 pins visited operations 64 million times; its input fits the planner’s output limit. This blocks the JavaScript server thread after planning. Index operation positions once.

- **SHOULD — Padded invalid values still produce oversized diagnostics.** [tamber/src/edit.rs:301](tamber/src/edit.rs:301). The size check examines `v.trim()`, but the refusal prints raw `v`. A value containing 900,000 spaces followed by `x=1` passes the size bound as three bytes, then echoes roughly 900 KB. The request fits the remote body limit. Format a trimmed, bounded diagnostic.

In-memory Bun probes passed the body-limit and re-hold ownership/target checks, with the Rust child stubbed. I found no additional semantic or state/disk mismatch by inspection in the batches, combined restores, scheme/retention walks, `same_conds`, or history.

Rust tests could not run in this read-only checkout without build artifacts. No files changed.
