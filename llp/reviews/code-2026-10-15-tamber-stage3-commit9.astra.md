# Code review: LLP 1121 stage 3, commit 9, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `72f5881aceca10a510eb28435dda4ed93eff9a02205788a329bca5b1fbb69c97`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, one low; Grok: none.
- **Disposition:** fixed in round 1

---

Reviewed `f7e758f54`. **No high-severity findings.**

- **Medium — mapped-name collisions go unreported.** [tamber/src/import/astryx.rs:158](/tmp/rv-s3c9/tamber/src/import/astryx.rs:158)  
  `--color-data-blue-5` and `--color-data-blue-s5` both become `astryx.data.blue.s5`. Both count as mapped. Against an empty design, this produces duplicate `add-token` operations and acceptance fails; against an existing token, one source can overwrite the other. Track destination names, report collisions before incrementing `mapped`, and test both cases.

- **Medium — a missing theme export silently imports core defaults.** [scripts/astryx-tokens.mjs:26](/tmp/rv-s3c9/scripts/astryx-tokens.mjs:26)  
  When discovery finds nothing and there is no default export, `theme` is `undefined`. Astryx explicitly resolves that to its defaults. A reproduction returned 258 tokens successfully, with default accent `#0064E0`, while retaining the requested package’s source label. Reject missing or ambiguous theme exports before resolving, with a regression test.

- **Low — version pinning is neither enforced nor independent.** [scripts/astryx-tokens.mjs:14](/tmp/rv-s3c9/scripts/astryx-tokens.mjs:14)  
  `--theme …@latest` or a range passes through to both installs, making regeneration mutable. The script also assumes every theme’s version equals its compatible core version. The default `0.7.0` pair is correct. Require exact versions and select core through an explicit pin or validated theme dependency; record both versions in the output.

The fixture reproduces exactly using the cached `0.7.0` packages. The commit’s probe accurately accounts for all **108 reported token names**: 6 duration tiers, 3 nominal durations, 42 text-style parts, 3 shadows, 5 rings, 40 lengths, 4 weights, 3 families, 1 easing and 1 line style. Together with 150 colours, that gives 258 paired token names.

No additional defect found in status mappings, the separate body-background offer, alpha handling or equal-side colours. Caps and formatting checks passed. Rust tests were not run; resolver checks used cached packages and in-memory I/O stubs. No files changed or packages installed.
