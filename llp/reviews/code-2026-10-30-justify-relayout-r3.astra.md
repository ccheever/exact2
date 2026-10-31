# Code review: web hosts follow a block parent's justify-items (progress), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `/tmp/dt3-rv-c3`.
- **Method:** one brief (sha256 `0025eb78fc2fbacc3f83d9def532a4b6926e8b0b21136a7ba47c3cd7abda7af2`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web-js/src/rows.rs:536](/tmp/dt3-rv-c3/host/web-js/src/rows.rs:536): Server normalization remains incomplete.** With bound `alignment = "normal/**/"`, the server DOM preserves that string and this mapper returns `auto`, not `initial`. CSS parses it as `normal` ([comment parsing](https://drafts.csswg.org/css-syntax-3/#consume-comments)), so the emitted CSS lets a 20px spinner stretch across its 240px block parent until adoption. Escaped spellings and invalid strings also reproduce the classification error. Whitespace normalization alone is insufficient; classification needs parsed, admitted values. The emit test’s `.trim()` assertion misses this behavior.

No other findings. The wasm parent-update path reaches `refold` and recomputes progress CSS. The 30 canonical alignment values classify correctly; the inheritance reset and absolute-position exemption are sound. Canvas remains unchanged.

Read-only mapper probes and the existing grid test passed. Rust/browser suites were not rerun: this checkout has no build artifacts. No files changed.

**Verdict: CHANGES REQUIRED — one remaining MAJOR server-rendering regression.**
