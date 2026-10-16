# Code review: LLP 1121 stage 3, commit 9, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `f332cf30c452532061de7867b4cc076c9bc8e5915880d335777b3feba2c23bea`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one medium; Grok: none.
- **Disposition:** applied without re-review

---

The three round-1 fixes hold on inspection of `git diff f7e758f54~1..HEAD` (I did not run the script or the Rust tests).

**Mapped-name collisions.** In `tamber/src/import/astryx.rs`, a colour token name is recorded only when both sides parse (`171`), and a later variable that would produce that same name is reported and skipped (`162–165`). `--color-data-blue-5` and `--color-data-blue-s5` both become `astryx.data.blue.s5` because a trailing numeric step is its own segment and `tamber_name` prefixes a digit with `s`. The new test expects source 2, mapped 1, reported 1. The pinned fixture has no colour-name collisions (150 distinct colour tokens), so the 258 = 150 + 108 gate is unchanged. Pending text styles still share one name on purpose (size, weight, and leading of one style); they are reported, not written.

**Exactly one exported theme.** `scripts/astryx-tokens.mjs` `37–39` keeps an export only when it has `tokens`, and throws unless there is one. `@astryxdesign/theme-neutral@0.7.0` exports `neutralTheme`, `neutralPalettes`, and `neutralIconRegistry`; only `neutralTheme` has `tokens`. `resolveThemeTokens(undefined)` is what core documents as defaults only, and that fallback is gone.

**Exact versions.** Line 18 accepts only `name@1.2.3` (optional prerelease). Lines 29–31 read `@astryxdesign/core` from the installed theme’s peer dependencies, or its dependencies, and require that same exact form. The published `0.7.0` peer is `"@astryxdesign/core": "0.7.0"`. The script installs that version and writes `core` into the JSON (`40`). The fixture diff is that one field; the resolved values are unchanged.
