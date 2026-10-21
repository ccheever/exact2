# Code review: LLP 1121 stage 11 A1, package designs in the compiler, re-check 1, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `8cc68cb25a`.
- **Method:** one brief (sha256 `a567d862bcd8988200c657117dc73e173a2677abd686adccc1943ec78b43f4f8`), blind to the other reviewer; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (2 BLOCKING, 3 SHOULD-FIX).
- **Disposition:** taken: `pair=` names judged, profile aliases read from the package's text, dials by direct read (then recorded by the fold, r4).

Two **BLOCKING** issues remain.

- **BLOCKING — Dial influence is not a reference list.** [theme/packages.rs:263](contract/lower/src/theme/packages.rs:263) checks `Checked.dials`, which records whether changing a dial changes the token, including transitively. Thus `reading.gap = space.md` is wrongly refused when the app sets `space.md` from a dial; the same affects role overrides and overridden package tokens. Conversely, `reading.n = calc(amount * 0)` escapes refusal if another app token keeps `amount` non-inert. Track resolved **direct dial reads**, independently of influence.

- **BLOCKING — Profile aliases bypass the reference restriction.** [theme/packages.rs:253](contract/lower/src/theme/packages.rs:253) sees dependencies after [theme.rs:652](contract/lower/src/theme.rs:652) replaces profile aliases with `transparent`. With app `brand = color(--profile …)` and package `reading.ink = brand`, `Checked.names` contains no `brand`; the prohibited dependency passes. This also permits aliases across packages. Preserve those alias edges during profile substitution.

- **SHOULD-FIX — `pair=` references escape the allowlist.** [theme/packages.rs:253](contract/lower/src/theme/packages.rs:253) checks `names`, but the checker records metadata references separately in `Checked.pairs`. Package `reading.ink = #000000 pair=appSurface` therefore accepts an app-only token. Apply the ownership rule to resolved pairs too.

- **SHOULD-FIX — Confirmed A2 integration gap.** [tamber_serve.rs:735](contract/cli/src/tamber_serve.rs:735) still folds only the app design and params. An app token aliasing `reading.card` can compile under A1 while server `describe`/`resolve` refuse it as unknown. Proposal folding also lacks packages. This is a concrete compiler/server disagreement for A2 to address.

- **SHOULD-FIX / ruling clarification — Namespace collisions remain conditional.** [sources/packages.rs:123](contract/cli/src/sources/packages.rs:123) skips packages without designs before checking collisions. The quoted ruling says *two linked packages*, without that exception. The narrower interpretation needs theme2’s explicit confirmation.

What checks out on inspection: whole-token overrides remove package `when`s; band bounds stay below params and synthetic lines; token/value/`when` locations are banded consistently; duplicate-message line numbers are decoded. I found no current `SOURCES` ordering bug: lowering materializes spans before subsequent merges, repeated compiles establish fresh bands, and threads are isolated. Case-insensitive role overrides, ordinary param-reference rejection, synthetic-token separation, and icon refusal also look sound.

The driver uses manifest names, deterministic traversal, lexical plus canonical containment, explicit-file missing errors, and discovers relative vendored and `exact.json` packages. Normal missing-design creation is covered by directory watches/Cargo reruns. D7 retention and the app-only scope of params and warning declarations look appropriate.

The **11 tests** cover useful basics but miss the failures above, multiple-band diagnostics, `when` value locations, repeated/threaded compiles, transitive/vendored/native discovery, valid/missing explicit paths, symlinks, and actual watcher rebuilding. The duplicate test asserts its primary line, not the decoded line inside its message.

Read-only review completed. `git diff HEAD^ HEAD --check` passed. Tests were not run: they create files, and this worktree has no build artifacts.
