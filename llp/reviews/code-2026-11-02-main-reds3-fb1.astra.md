# Code review: the form-control smoke's compile, and the docs lookup's driver and safe-area sections (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `8ba1d7a2a5`.
- **Method:** one brief (sha256 `01bf7beff4097b21bcf0791148c9e60c864919770280e665aec89718fe78492a`); a quick review under the main-red permission. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** the MINOR (no timeout on the cargo subprocess) is left as is: the HTTP iframe fixture's compile beside it has none either.

No BLOCKER or MAJOR findings.

- **MINOR — [scripts/smoke-inspect.mjs:23](/Users/admin/x2scratch/fb1-rv-reds/scripts/smoke-inspect.mjs:23):** The Cargo subprocess has no timeout, so standalone smoke runs can wait indefinitely on a stalled build or Cargo lock. The 600-second setting is a [per-test timeout](https://bun.sh/docs/test/runtime-behavior#per-test-timeout); it does not bound standalone execution. This matches the existing HTTP fixture and is non-blocking hardening.

The changes otherwise look correct and minimal:

- [TOPICS](/Users/admin/x2scratch/fb1-rv-reds/scripts/exact.mjs:1158) matches complete normalized queries, so neither override captures unrelated queries. `Declarations` is unique across GUIDES **and recipes**, at `docs/contract-grammar.md:92`. The new operations heading is also unique.
- All 41 existing documentation lookup assertions passed using the isolated lookup implementation in memory. The heading additionally improves `prefer` and `driver operations` routing.
- `cargo run` selects the package’s sole binary with the same profile and compiler arguments. Absolute fixture/output paths preserve compilation behavior despite explicitly setting the working directory. An existing binary gains Cargo’s freshness checks and build/toolchain requirements.
- The back-button expectation accurately follows b4e8417508’s recipe wording.

No builds or filesystem-mutating tests were run.

VERDICT: APPROVE
