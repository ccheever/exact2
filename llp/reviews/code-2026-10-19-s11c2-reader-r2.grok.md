# Code review: Stage 11 C2: the Android reader held to the Material column, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `365e628374`.
- **Method:** one brief (sha256 `63efeca56f5fe246d304765705c27690c28c8cd28956d26ca5b91d6ec6fba216`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking
- **Disposition:** none needed

Nothing blocking. No findings.

The two commits together drop `apps/scale-fixture` and measure the reader from the plan scale-check compiles. The fractional edge, the horizontal guard, and `--reader --app --plan` all do what §10 says.

1. **The edge formula is right for both partial pixels, including a two-pixel ramp.** `first + (1 - at(first)) - at(first - 1)` in `pageTop` (`scripts/scale-check.mjs`) is one expression for both shapes. When the run’s first pixel is the partial one, the pixel before is empty, so only `(1 - at(first))` moves the edge. When the pixel before is the partial one (coverage under one half, so it is not in the run) and the first run pixel is full, only that subtraction moves it. A ramp that spans both pixels needs both terms: for a two-pixel linear ramp, and for a tent filter of radius one, they meet at the true edge. They do not double-count. Out of range, `ink` is 0, so a missing pixel before the image contributes nothing. The test at y = 70.4 covers the first shape (the pixel before is empty). The other shape is the same expression with the full pixel’s uninked term at 0.

2. **The horizontal guard’s 1px slack holds for a box at x = 0.** It refuses when the fill’s left edge and `box.x * scale` differ by more than one device pixel. Antialiasing error on this formula is a fraction of a pixel, so a true side bar still trips it. This plan’s top fill is `r-sm`, which sits at x = 0, and that is the sensitive case: the difference is the inset itself. With no pixel before, the phantom coverage is about an eighth of a pixel on a tent filter, under the slack, so a page that starts on the image edge is not refused. One pixel of inset on a phone-width screenshot does not move a corner by the 1pt radius tolerance.

3. **The reader runs the pushed plan on unsaid’s data, and the scale plan’s capabilities are already in that APK.** `drive` launches `android --reader --app <app> --plan <the compiled plan>`. `readerDeploy` copies that file to `/data/local/tmp`, passes it as `x_plan`, and the activity sets `EXACT_PLAN` before the library boots. `Config::from_env` reads those bytes in place of the baked plan. There is no Rust module beside this plan, so the APK’s data source stays `hello_data::Hello`. The scale contract asks nothing of data, so Hello is never called. Native Android parses every kernel grammar without the wasm link-by-use gate. The only APK piece gated by `uses.txt` is Compose for a date or time control, which this plan does not have. A plan the file cannot be read as still falls back to the baked app, and that boot would miss these test ids and fail the run.

4. **Pushing the plan to unsaid stays with the ruling’s intent.** §10’s ruling is that the Material column is held by the scale’s own boxes, not E’s fixture. The bytes on the reader are that conform plan, the same plan the other hosts run, and the canvas host still calls `show_material_look`. unsaid is the carrier: Hello, and an APK that already links the host. The fixture app was the round-1 way to bake those bytes. Removing it, and the symlinks, does not change which boxes are measured.

§10’s reader row (4/8/12, 16 and 20, 8/20) is the Material column with the design’s pill clamp. I did not rebuild or drive the emulator, so the 20/20 figure is the code’s account of that run, not a measurement taken here. The unit test does not build a two-pixel ramp or an edge whose partial pixel is the one before the run.
