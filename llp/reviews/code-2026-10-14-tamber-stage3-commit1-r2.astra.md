# Code review: LLP 1121 stage 3, commit 1, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `716a0ea8af3aacadfaafff611468e5c91012d9a797150c23dfd20f0c288bc9b8`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one medium, one low (main moved); Grok: one high (main moved), one medium.
- **Disposition:** fixed in round 2; the RFC text was main having moved

---

No high-severity findings.

- **Medium — [tamber/src/fmt.rs:203](/tmp/rv-s3c1b/tamber/src/fmt.rs:203): Single-quoted literals can still change internally.** `font.family = 'A x=y'` becomes `font.family = 'A  x=y'`. HEAD teaches comment stripping about single quotes, but `lex::words` still recognizes only double quotes, so it treats `x=y'` as metadata and inserts spacing inside the string. Reparsing accepts both versions. **Fix:** recognize both quote delimiters in the word scanner and add an exact literal-preservation regression.

- **Low — [llp/1121-tamber.rfc.md:1236](/tmp/rv-s3c1b/llp/1121-tamber.rfc.md:1236): The diff removes shipped `scheme=fixed` documentation.** It removes token metadata from the grammar, deletes its explanation and follow-up section, and changes its status back to pending. The implementation and new fixture still use it. **Fix:** restore those portions from `origin/main`.

The other listed round-1 fixes look correct. I found no additional idempotence, IR-preservation, or CLI defects.

Validation: 14 existing Tamber tests passed using prebuilt binaries from a same-HEAD checkout with matching sources; caps and `cargo fmt --all -- --check` passed. CLI write tests were not run under the read-only constraint. No files changed.
