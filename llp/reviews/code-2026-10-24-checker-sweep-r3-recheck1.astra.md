# Code review: the checker sweep (after the checker bound), round 3, narrow re-check 1, 2026-10-24 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `5fffcf8baf`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed before the rebase onto main.

Two **SHOULD** findings remain. No BLOCKER or NIT.

- **SHOULD — Held imports still reach quadratic fallback.** [tamber/src/edit.rs:541](tamber/src/edit.rs:541). Hold 6,000 set operations, then remove their last target before accepting. [Recompute](scripts/tamber-server.mjs:1059) re-marks those operations and holds them against the **current** digest; the marking probe succeeds despite the missing target. Batching then repeatedly fails and steps one operation, reparsing the design and cloning the remaining segment. The request carries no `ops`, so the cap does not bound this path. Reject missing targets before fallback or batch the successful prefix.

- **SHOULD — `mixed()` does not always preserve stepping’s text.** [tamber/src/edit/batch.rs:147](tamber/src/edit/batch.rs:147). Start with `t0` having no `when`, and `t1` having a six-space-indented `when`. Interleave: set a new `when` on `t0`, add a token with `whens`, set `t1`, add another token with `whens`. Stepping creates `t0`’s four-space `when`, which subsequent additions inherit through `Doc::indents`. `mixed()` adds first, giving those additions six-space indentation, and returns success. Both layouts check. Preserve this formatting dependency or decline; the new property test never adds tokens with `whens`.

The cap/docs, DTCG indexing, pin indexing, and trimmed diagnostic findings are closed. All 22 relevant prebuilt Rust tests and the Bun probes passed. No files changed.
