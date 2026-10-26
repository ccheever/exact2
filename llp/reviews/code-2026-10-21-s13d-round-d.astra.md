# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), D1/D2 narrow re-check, D3 round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `01e51b5271`.
- **Method:** one brief (sha256 `d55d85bb8abf8a1c52d683a982ec2cd7ead207f7036752b6554b52162a568da4`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**Still blocked: one blocker and two majors.**

1. **Blocker — D3:** [contract/lower/src/theme/scan.rs:278](contract/lower/src/theme/scan.rs:278) walks original children, including styles that grouped-list lowering removes.

   In an otherwise valid app:
   ```text
   list appearance="auto" height=300
     section
       svg width=20 height=20 -exact-row-look="platform" padding-left="gap"
   ```
   Changing `gap` from `8px` to `#444444` makes the checker reject `padding-left` with `tamber-kind`. However, `nodes.rs:195` applies `grouped::sections`, whose `row_look::yielded` removes that padding before lowering the SVG. The resulting build accepts it. SVG rows were previously excluded, so D3 exposes this case.

   **Fix:** share the grouped-row transformation when collecting rows, or conservatively omit decorations that yield to the platform. Add a regression asserting both the transaction and subsequent build succeed.

2. **Major — D3:** [contract/lower/src/theme/scan.rs:156](contract/lower/src/theme/scan.rs:156) checks SVG eligibility but misses other `prepare_element` exits before styles.

   For `svg "oops" width=20 height=20` containing `rect x="pos" width=10 height=10`, changing `pos` from a length to a colour produces a checker refusal at `x`. The build instead returns `lower-positional`, “`svg` takes no positional argument,” at `"oops"` (`nodes.rs:446`), and never lowers the rectangle. A malformed `font-family` similarly stops at `nodes.rs:478`, while non-font rows and descendants remain retained.

   **Fix:** share the remaining pre-style eligibility checks, or conservatively leave unchecked contexts to the build; propagate refusal to descendants. Cover positional arguments and malformed families in regression tests.

3. **Major — D2:** [contract/lower/src/theme/scan.rs:254](contract/lower/src/theme/scan.rs:254) bypasses the new identifier guard for choices whose arms are all literals.

   With an action `bump`, `color=((bump == bump) ? "mute" : "#000000")` enters `Some(false)`, so `typed()` never examines its condition. Removing `mute` produces `tamber-unknown-token` at the literal. Equality passes type inference, but lowering compiles the condition first and returns `lower-action-as-value` at `bump`. The id, message and location differ.

   **Fix:** apply the identifier-loadability guard before splitting literal choices as well as before retaining computed rows. Add a regression confirming the write is left to the build.

The requested D1 stale-file fixes look correct. I found no behavior change in the extracted SVG check itself.

These findings are source-traced; tests were not run because the session is read-only and this checkout has no build artifacts. No files were changed.
