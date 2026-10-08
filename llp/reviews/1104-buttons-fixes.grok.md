# Review: LLP 1104 step 3 fix delta (4e18bc622..f9c24e8f2), 2026-10-07 (grok)

- **Family:** xAI — `grok -m grok-4.7 --reasoning-effort xhigh --permission-mode plan --no-subagents --output-format streaming-json`, one session, at `f9c24e8f2` (`stopReason: end_turn`).
- **Method:** the same delta brief (sha256 `e163d1d7c56679b667181723d2813587638aeb6ecbc45cd8915c6d254044ae3d`) behind the fixed preamble of tool rules (whole prompt sha256 `48662e56a0d7fb6edf732ddf0272656598d4c3aaca49c07d67735bc7e51318d6`); blind to the other review. The orchestrator (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** the new finding (disabled glass accent) went to BF4k; the partial Grok 5 (native menu subtitles) to BF4a.

---

I'll audit the button-fix delta on my own: first the two earlier reports, then the diff from `4e18bc622` to `f9c24e8f2`.The earlier reports name twenty findings across kernel, Apple, web, and Linux. I'll read the delta against each one.The remaining open items are the disabled-glass accent rule, the Linux size constants, and the WebKit allowance. I'll read those before writing the verdict.**Verdict:** LAND WITH FIXES. The earlier blocking failures (environment mismatch, per-layout measurement, button em base, custom projections, web wrapping and clamp) are fixed and tested. One D16 regression remains: a disabled `glass` or `clear-glass` button drops an authored accent. The new `known-webkit.json` entry is bounded to `synthetic-native-buttons` / `layout.open-dialog.y` / `delta [0-6]\.\d\d px` (0.00–6.99 px) and justified — WebKit push buttons run about a pixel shorter, and the new fixture buttons sit in an absolute column, so they do not move `open-dialog`. `step: "*"` also hides any sub-7 px y shift of that one node.

**Earlier findings:**

- Astra 1 — fixed; `host/apple/src/control_text_tests.rs` `resolved_button_geometry_has_one_measurement_and_presentation_payload` exercises `env()` padding, percent and calc, and equal measure and present payloads.
- Astra 2 — partly fixed; `PopoverTouch` calls `lightDismiss`. `testProductionTouchesLightDismissAndHideANativeInvokersPopover` drives `touchesBegan` on a fake touch (outside closes, invoker excluded, hide-only) and does not deliver a `UIEvent` or assert the outside control's press.
- Astra 3 — fixed; `Written.traits` is `FieldChromeCache.Traits`, which now stores appearance and contrast, and trait registration requests a resync. `testAuthoredColoursFollowALightToDarkSwitch` exercises the light-to-dark rewrite, not contrast alone.
- Astra 4 — fixed; `host/web-js/src/rows.rs` `a_native_buttons_bound_display_preserves_the_semantic_grid` asserts the emitted `none`/`grid` mapper.
- Astra 5 — fixed; kernel `control_size_selects_the_font_for_em_and_invalidates_descendants`, the Apple resolved-em payload test, and iOS `testResolvedEmFontIsNotScaledTwiceAtAccessibilitySize`.
- Astra 6 — fixed; kernel `two_text_custom_face_keeps_the_projected_name` joins both texts once `fits` is false.
- Astra 7 — fixed at the bookkeeping layer; `NativeButtonsTVOSTests` checks `focusReturn`, the guide, the scroller, and test-id replacement, not `UIFocusSystem`.
- Astra 8 — fixed for filled and plain; the Linux test samples an opaque red pixel, and `host/web/tests/document.test.mjs` checks the authored disabled fill and the plain accent color. Glass and clear-glass regress (below).
- Astra 9 — fixed; kernel `hosts_receive_resolved_padding` and the Apple geometry test assert points, and `testNonuniformResolvedCornersReportAStandIn` covers a non-uniform radius.
- Astra 10 — fixed; Linux `empty_invoker_targets_still_run_the_press_handler`.
- Grok 1 — fixed; `document.test.mjs` asserts computed `white-space: normal`, `text-align: center`, and that an authored `end` wins inside a nowrap, end-aligned parent.
- Grok 2 — fixed; `document.test.mjs` asserts nowrap is `block` with ellipsis, and the clamp box is about two lines.
- Grok 3 — fixed; `host/web/src/button_css.rs` asserts the column fallback string is `1em` (JS copy at `host/web-js/src/style.rs:505`). That check is the emitted string, not a laid-out column.
- Grok 4 — fixed; same tests as Astra 5.
- Grok 5 — partly fixed; `fits` is false, so custom menus, tabs, and segments keep the joined face (same kernel test as Astra 6). Native menu titles still return `face.shown` and drop the subtitle (`MenusIOS.swift:761`, `MenusMac.swift:515`); no test covers that name.
- Grok 6 — partly fixed; same as Astra 2.
- Grok 7 — fixed; kernel `unrelated_layout_does_not_remeasure_native_buttons` and `face_edits_remeasure_then_the_next_layout_is_free`.
- Grok 8 — fixed; `testAbsentControlSizeDoesNotOverwriteThePlatformControl`.
- Grok 9 — fixed; the assertion is text width + 18 by text height + 14, matching the painter's unauthored medium title (schema default 16 px, which `typography` keeps) and padding 9 / 7.
- Grok 10 — fixed; same as Astra 9.

**New findings:**

**Disabled glass and clear-glass drop an authored accent.** Material. `host/web/index.html:86`. The disabled background is `var(--exact-button-accent-fill, color-mix(in srgb, AccentColor 45%, transparent))`. `glass` and `clear-glass` never set `--exact-button-accent-fill` (the button rule resets it at line 66; lines 81–82 do not set it), so an authored `--exact-accent` is ignored and the wash is 45% system `AccentColor`. The previous rule mixed `var(--exact-accent, AccentColor)` at 45%. Filled, bordered-prominent, and prominent-glass set the variable and keep the authored accent; `document.test.mjs` covers filled and plain only. Fix: make that fallback `color-mix(in srgb, var(--exact-accent, AccentColor) 45%, transparent)`, and add a disabled glass case to `document.test.mjs`.