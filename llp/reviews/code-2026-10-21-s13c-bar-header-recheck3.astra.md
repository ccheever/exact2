# Code review: @exact/actions BarItem and HeaderButton, and lower-library-control's button rules (stage 13 C piece 4), third narrow re-check, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `bff4a046c4`.
- **Method:** one brief (sha256 `inline prompt (the narrow re-check's brief was given on the command line)`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (one P2)
- **Disposition:** taken in e0269c60be (a label text's own bindings)

VERDICT: **WRONG**

1. **P2 — Referenced text still needs its own lexical scope** — [contract/lower/src/anatomy/library.rs:860](/tmp/iosfix-rv-bi5/contract/lower/src/anatomy/library.rs:860). Passing `&[]` fixes the reported outer-label case, but incorrectly treats a label’s local binding as the component’s state. With a boolean `selected` prop:
   ```text
   derive fade = (selected ? 1 : 0.5)
   view
     for selected in ["row"] key = selected
       text selected id="name"
       button press=go aria-labelledby="name" opacity=fade
         box width=4 height=4
   ```
   The label always says “row,” while opacity follows the component prop. This previously triggered the diagnostic; the empty bound now suppresses it. Preserve each referenced text’s enclosing bindings and add this negative case beside the tests at [library_rules.rs:755](/tmp/iosfix-rv-bi5/contract/cli/tests/it/library_rules.rs:755).

The empty/whitespace fallback and duplicate-ID fixes are correct. The added fixtures cover the three original examples. `cargo fmt --all -- --check` passes.

Integration tests were not run because their fixtures write files, contrary to the read-only constraint.
