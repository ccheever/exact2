# Code review: the Android navigation bar as Material 3 1.4's, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort xhigh, unsandboxed (the read-only sandbox cannot start on `expo-build-1000`), a throwaway detached worktree, told to write nothing; it was clean afterwards.
- **Scope:** `d58b8ad8d` (the commit before the schema digest was moved and the findings below were answered).
- **Method:** one brief (sha256 `93a50ad0aef54926b0529ddc33bd751098fdde9a19907a23816bafb4cd8430b2`), blind to the author's reasoning. The author (Claude) is not a reviewer.
- **Answered:** 1 (targets take the gaps' halves) and 2 (the palette's whole state and the bars' Material roles invalidate) fixed; 5 in part (a test of the bar's colours and indicator; a palette told mid-run is process-global, so not tested); 3 and 4 predate the commit and are in `QUEUE.md`.

## Review

1. **[P2] Correctness — narrow destinations lose their minimum touch target.** [look.rs:418](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars/look.rs:418) subtracts gaps, but line 482 uses the resulting width directly for hit testing. Five tabs in a 260 dp bar now have 45.6 dp targets. A touch at `(46, 40)` hits nothing; Compose expands that first target to its [48 dp minimum](https://github.com/androidx/androidx/blob/androidx-main/compose/ui/ui/src/commonMain/kotlin/androidx/compose/ui/platform/ViewConfiguration.kt). Preserve the visual gaps while expanding touch bounds.

2. **[P2] Correctness — the new selected colours do not invalidate cached bars.** [look.rs:430](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars/look.rs:430) reads `Secondary` and `OnSecondaryContainer`, but [bars.rs:314](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars.rs:314) tracks neither. Updating only those palette entries also leaves `Palette::report()` unchanged, so `set_palette` returns early. The selected destination keeps its old colours until another change rebuilds its layout. Palette invalidation must include the newly consumed roles.

3. **[P2] Correctness — RTL still places destinations from the left.** [look.rs:422](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars/look.rs:422) always advances from the physical left inset; neither its inputs nor its caller supplies direction. With three tabs at 412 dp, the first target starts at x=0 even under RTL, where Compose puts it at x=280. This predates HEAD but remains a gap in the claimed match.

4. **[P2] Correctness — the no-palette fallback mixes Material foregrounds with non-Material fills.** [look.rs:453](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars/look.rs:453) still resolves the indicator through `ButtonFace`, and line 490 resolves the background through `-exact-secondary-grouped-background`. Without a reported palette, the light defaults are translucent `#78788029` and white, rather than Material’s `#e8def8` and `#f3edf7`. The new foreground fallbacks therefore do not produce a baseline Material bar. These fill mismatches predate HEAD.

5. **[P3] Tests — changed paint behaviour lacks regression coverage.** [bars_tests.rs:266](/home/ccheever/projects/exact2-review-s2/host/linux/src/presenter/bars_tests.rs:266) checks only three-item geometry. Restoring the old disabled colour would pass this assertion and the navigation oracle’s entirely enabled specimen. Add coverage for disabled colours, selected icon colour, indicator dimensions, and palette updates. No existing assertion was found newly false.

6. **Schema: no findings.** The six rows follow the existing `field-` declaration format, append without shifting existing indices, and remain unset outside Material. The neutral web census excludes them. Generated table data grows; other looks’ values remain unchanged.

7. **Other: no findings.** `nav_height` preserves its previous calculation, and the full-cell label maximum matches the pinned Compose source.

Read-only inspection only; builds and tests were not run because they write files.

READY WITH CHANGES