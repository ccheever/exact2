# Code review: stage 13 B, the pen importer, round 3's re-check 6 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `26fc6751f9`.
- **Method:** one brief (sha256 `7230295ccd4c4a972ba59d164c7bd06ff7cb1781ccb460ac032559785951cb4f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [dtcg.rs:986](tamber/src/import/dtcg.rs:986): **Error-string amplification remains.** `Err` is charged zero bytes but cloned into each tuple’s memo. An 884,026-byte request with 18,000 short aliases reaching a self-cycle named by 128 four-byte characters, six binary axes and a scheme retains **1.16 GiB of error strings alone**, well below the work budget. Charge retained errors before cloning.
- [pen.rs:269](tamber/src/import/dtcg/pen.rs:269): **Ignored descendant values still influence kinds.** `descendants: {"type":"gradient","node":{"width":"$n"}}` reads the map’s scalar `type` before skipping it, propagating gradient context into `node`. A numeric `n = 8` becomes unitless `8` instead of `8px`. An ID map must not supply an object type.

**Should-fix:** None.  
**Nits:** None.

Successful colour-name copies are now charged. The original scalar-descendant case is fixed. The [1.17 clause](docs/tamber-protocol.md:162), server constant and assertions correctly cover `pen`, `notes` and the prefix bound.

Findings are by inspection and byte accounting. The permitted Cargo test could not start: the read-only sandbox denied temporary-file creation. No files changed.
