# Code review: stage 13 B, the pen importer, round 3's re-check 5 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `842c78fabb`.
- **Method:** one brief (sha256 `ebd46e68df3aa74d49b3b8e9792a198cb45f2b0bc3a7b76d8417295c7dfb6875`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [pen.rs:744](tamber/src/import/dtcg/pen.rs:744), [dtcg.rs:971](tamber/src/import/dtcg.rs:971), [dtcg.rs:989](tamber/src/import/dtcg.rs:989): **Prefix amplification remains.** A permitted prefix of 128 four-byte characters, one literal colour, 16,999 aliases to it, and six binary theme axes fit in a **918,721-byte request**. Across 64 tuples, results and memo entries retain **1.05 GiB of prefixed-name bytes alone**, while spending approximately 20 million of the 50.3-million budget. The isolate limit intervenes before budget refusal. Charge these copies before allocation.

**Should-fix:** None.  
**Nits:** None.

By inspection, `At.ids` fixes both descendant-ID cases, and colour-walk charging plus the final spend propagates exhaustion to an import refusal.

The allowed Cargo test could not start: the read-only sandbox denied temporary-file creation. No files changed.
