# Code review: a column flex item's main size measured at its fit-content width (Taffy patch 28; numbered 27 when reviewed, renumbered on landing after a multicol patch took 27), round 2, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x26`.
- **Method:** one brief (sha256 `a83149d3f54dc8976c1c9ad9551b69fc55a68bd8de5031dc62cc3666c1ef31a6`), shared with astra. Round 2, blind to the other review. Reviewed the staged diff in a worktree at 39955bf5d. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r3):** Astra 1 taken, with a correction from Chrome: the probe now uses the final width's own helper (`common::fit_content_width`) and condition (adding its compressible-replaced exclusion), so the two cannot disagree. The counterexample measured in Chrome 154 through the web host gives a 100-wide, one-line row: min-content wins, as CSS Sizing 3's max(min-content, min(max-content, available)) says, and as round 1's probe had it; the shared helper's order (max-content winning) was the outlier. The helper is corrected to CSS's order, for patch 25's grid, column-flex and absolute widths alike; the Chrome case "intrinsic sizes inverted by a negative margin" is added (it fails before on both the old helper and the round-1 probe); all kernel browser cases (incl. browser_position's 28) still pass. Grok: READY, no findings.

---

READY

No findings.

Round 1 is resolved in the staged diff. The fit-content width is clamped with `maybe_clamp(min, max)` and then floored with `.max(padding + border)` at `flexbox.rs:1092`, which matches `determine_hypothetical_cross_size` (`flexbox.rs:2041`) and the `max-width: 0` case. The branch requires a resolved cross size of `None`, `size_style.cross(dir).is_auto()`, no aspect ratio, and at least one child (`flexbox.rs:1068`). Aspect-ratio items and a wrapping column in a row stay out of scope, as recorded in patch 27.

`known_for_probe` (`flexbox.rs:1081`) computes that size once into `fit_content_known` and returns the same `Size` to step E (`flexbox.rs:1201`) and the automatic-minimum probe (`flexbox.rs:1246`). If the available cross size is not definite, both calls return `provisional_cross_removed` and nothing is cached. The closure only reads `child.node` (Rust 2021 field capture), so the later writes to `child.flex_basis` are disjoint. Items that never reach those probes do not measure.

The width measures and the later height measure do not share a cache entry. Patch 11’s key includes the known width, and a definite available width is stored as `(-value).to_bits()`, so a height previously measured at available width 200 is not reused as the height at known width 200. `size_is_valid_for` also refuses to answer a vertical request from a horizontal entry. `determine_container_main_size` may remeasure under a max-content height, but on a column it keeps `max(remeasured, flex_basis)` (`flexbox.rs:1699`), so the container does not collapse below the patched base size.

The two new tests cover the nested wrapping row (the case that was one line tall before), the padding floor, centering, the bubble shape, and wrapped text for flex and block under flex-start, center, and stretch. The 290px comment matches the 16px monospace (29 × 10).
