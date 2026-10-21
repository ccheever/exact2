# Code review: exact-tamber crate, round 3 (the last), 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `72896cb91ad4c63e2baef74356c706d1fcf538bd983f38904364b0bd9de20658`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 2 MAJOR, 2 MINOR; Grok 2 MINOR).
- **Disposition:** all applied after the last round without re-review, each with a corpus case, and named in the commit message

---

1. **MAJOR — unclamped intermediates now produce inconsistent fallbacks.** [color_fn.rs:297](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:297): `oklch(from red .6 -.1 20)` emits `oklch(0.6 -0.1 20)` with fallback `#0d9298ff`. The kernel clamps that serialized chroma to zero, giving `#808080ff`. Preserve unclamped components for subsequent folds, but make the exported value and fallback agree.

2. **MAJOR — powerless-hue detection remains incomplete.** [color_fn.rs:491](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/color_fn.rs:491): the universal `1e-6` threshold misses CSS’s space-specific thresholds. For example, `lch(from lab(50 .001 0) 50 40 h)` silently uses hue zero: its converted chroma is approximately `.001`, within CSS LCH’s powerless threshold of `.0015`. Under the declared missing-component limitation, this must be refused. [CSS Color 4](https://www.w3.org/TR/css-color-4/#specifying-lab-lch)

3. **MINOR — hidden branches are checked against the wrong tuple.** [check.rs:607](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:607): `unselected()` always uses tuple zero. A hidden `when dark-style=dim density=compact`, covered by more-specific `variant=a/b` branches, is therefore compared against the default density. With default light `#fff` and compact light `#eee`, a valid hidden branch preserving `#eee` incorrectly receives `tamber-under-light`. Check contexts satisfying its conditions.

4. **MINOR — failed probes count as dial influence.** [check.rs:676](/Users/admin/x2p2/apps/exact2-tamber/tamber/src/check.rs:676): with `accent = #1083fe`, this token’s visible sides are constant:
   ```
   light-dark(#000, light-dark(oklch(from accent l c h), #fff))
   ```
   The new powerless-hue refusal makes black/white probes fail. Their `None` results compare unequal to the baseline, marking `accent` influential; probe errors are then discarded. This incorrectly suppresses `tamber-dial-inert` for a dial used only in a discarded side.

The other listed fixes look correct by inspection. `cargo test -p exact-tamber` was blocked by the read-only Cargo lock; existing binaries predate this commit. Findings are source-derived, with the colour calculations checked independently. No files changed.

VERDICT: fix (MAJOR: tamber/src/color_fn.rs:297, tamber/src/color_fn.rs:491; MINOR: tamber/src/check.rs:607, tamber/src/check.rs:676)
