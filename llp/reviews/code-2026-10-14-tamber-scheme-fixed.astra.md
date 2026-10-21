# Code review: LLP 1121 stage 2: scheme=fixed, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `f81cd3f48c1c2b4fa2d31f2f911e53992dbb59c1ba0d3346bb393e0538e755cd`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, two low; Grok: four medium, two low.
- **Disposition:** fixed in round 1 except an empty scheme= (a value error), named

---

No high-severity findings. I found four medium and two low issues:

1. **Medium — Fixed marks are lost through role aliases.** [warnings.rs:953](/tmp/rv-sf/contract/lower/src/warnings.rs:953) compares the matched spelling against declaration names. With `label = #fff scheme=fixed` and `ink = -exact-label`, a row naming `ink` still warns: `-exact-label` does not match `label` in `fixed`. **Fix:** resolve every spelling to a canonical declaration identity before checking the mark.

2. **Medium — Role expressions can become completely silent.** [warnings.rs:1120](/tmp/rv-sf/contract/lower/src/warnings.rs:1120) replaces expression traversal with whole-value alias traversal. For `base = #ffffff` and `Canvas = color-mix(in srgb, base 50%, #000000)`, an unstyled text view previously warned at `Canvas`; now neither declaration nor row warns. **Fix:** preserve reference traversal inside role expressions, respecting fixed marks, or classify their checked folded values.

3. **Medium — Equivalent colours can incorrectly reject `scheme=fixed`.** [check.rs:179](/tmp/rv-sf/tamber/src/check.rs:179) compares `Half` representations. `light-dark(#ffffff, color(srgb 1 1 1)) scheme=fixed` is rejected because its sides are `Legacy` and `Wide`, although both are white. **Fix:** use the existing colour-equivalence helper, `same`, and test equivalent colours written in different forms.

4. **Medium — The summary change breaks an existing test.** [warnings.rs:1259](/tmp/rv-sf/contract/lower/src/warnings.rs:1259) now emits “these 25 colours,” while [warnings.rs:480](/tmp/rv-sf/contract/cli/tests/it/warnings.rs:480) still asserts “these 25 literal colours.” That assertion would fail. **Fix:** preserve the old summary for literal-only sites, consistent with “literals unchanged,” and test token-only and mixed summaries separately.

5. **Low — Role declarations lose the new repair message during deduplication.** [warnings.rs:1127](/tmp/rv-sf/contract/lower/src/warnings.rs:1127) creates role-replacement sites with `token: false`. For an unmarked `Canvas = #ffffff`, this earlier site survives deduplication and discards the later token site, so the warning omits both `light-dark()` and `scheme=fixed` fixes. **Fix:** classify declaration sites as tokens, including implicit role replacements and inline themes.

6. **Low — A second invalid scheme value is ignored.** [check.rs:164](/tmp/rv-sf/tamber/src/check.rs:164) checks only the first occurrence. `ink = #ffffff scheme=fixed scheme=dark` therefore succeeds despite the invalid word. **Fix:** reject duplicate `scheme` metadata or validate every occurrence; test both orders.

The new tests miss these cases. Additional useful coverage includes quoted/embedded-`=` metadata, `when` branches, `?:`/function/template references, and mixed warning counts.

`bun scripts/caps.mjs` and `git diff --check HEAD^ HEAD` passed. Rust tests were not run because this worktree has no build artifacts and building would violate the read-only constraint. No files were changed.
