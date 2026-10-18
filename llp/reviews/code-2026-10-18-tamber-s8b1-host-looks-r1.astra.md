# Code review: host looks on the runner and the web (LLP 1121 stage 8 B1), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `2707b0ec8`, the runner's `SetLooks` and the looks on both web targets.
- **Method:** one brief (sha256 `4971e102a7bec1a64b4918e00803f15c884e153f23f78ec19a9a1527938e86b0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: rules surviving a reload, disabled boxes, sentinel collisions with look literals, D7's rules winning, written-only rows passing validate; should-fix: Space, the ease set too late, hover/focus press scales, `unset`, a held press's scale after a live edit).
- **Disposition:** in the round-1 commit, with tests. 1: the looks sheet goes with the last plan's theme at a boot. 2: a box's `disabled` (any value but "false") takes none; the Chrome test disables as the hosts do. 3: look declarations join the sentinel scan. 4: the rules are scoped under `#exact-root`, over the shell's pressed paint; the same paint said twice is also refused at lowering (theme2's 05:28 ruling (3), `lower-state-press-paint`). 5: validate checks set and cleared rows (kernel test). 6: an `:active` look is a declared press for Space too (Chrome test). 7: `data-exact-easing` is set before the press turns (Chrome test reads the colour mid-ease); it now eases under Reduce Motion too (theme2's 05:28 ruling (2)). 9: `unset` crosses written with no value on both paths (runner and JS target tests). Asked theme2: 8, admit `-exact-press-scale` in `:active` only. Declined: 10, a held press's scale after a live edit is the base press scale's behaviour too (pre-existing), not B's.

---

**RIGHT WITH CHANGES**

1. **BLOCKING — Old look rules survive wasm reloads.** [host/web/glue.js:615](/tmp/rv-s8b1-r1/host/web/glue.js:615) retains `#exact-looks` and its map across `bootNow()`. Reloading after removing a look leaves its rules matching reused view IDs, including unrelated nodes. Clear the sheet when accepting a new plan, alongside `#exact-theme`. Test a reload that removes all looks.

2. **BLOCKING — Disabled boxes still receive looks.** [host/web/src/host/looks.rs:83](/tmp/rv-s8b1-r1/host/web/src/host/looks.rs:83) excludes `[disabled="true"]`, but both targets emit `disabled=""`. A disabled `div` matches neither exclusion and still takes hover, focus and active overlays. Use `[disabled]` consistently in the rules and press tracking. The browser test masks this by manually setting `"true"`.

3. **BLOCKING — Sentinel substitution can corrupt literal look colours.** [host/web-js/src/modes.rs:228](/tmp/rv-s8b1-r1/host/web-js/src/modes.rs:228) substitutes sentinels inside looks, while `written()` excludes those declarations from collision detection. With one varying colour token, a literal `#e10000` look can become that token’s `var()` and incorrectly share its interned rule group. Include look declarations in the collision scan; test literal and token looks across a mode change.

4. **BLOCKING — D7 pressed paint overrides the new state look.** [host/web/src/host/looks.rs:108](/tmp/rv-s8b1-r1/host/web/src/host/looks.rs:108) emits attribute-only selectors. D7’s important rules at [index.html:305](/tmp/rv-s8b1-r1/host/web/index.html:305) include `#exact-root`, so they win. For example, D7 press opacity can multiply an active look’s absolute opacity instead of showing it. Make state overlays win overlapping properties while preserving D7 on properties the look omits.

5. **BLOCKING — `SetLooks` validation overlooks written-only rows.** [kernel/src/txn.rs:347](/tmp/rv-s8b1-r1/kernel/src/txn.rs:347) checks only `patch.mask`. A patch with an empty mask and `written={Width}` passes decoding and application, then produces `width:unset!important` on hover. Validate `mask.union(written)` against `look_rows()` and test the wire and direct-op paths.

6. **SHOULD-FIX — Space does not activate an active-only button look.** [host/web/input-glue.js:336](/tmp/rv-s8b1-r1/host/web/input-glue.js:336) adds look recognition only to pointer handling. The keyboard gate still requires `declaresPress()` or native button styling. A custom button with only an active background receives neither its overlay nor the suppressed default dim. Include active looks in the shared feedback predicate and test Space down/up.

7. **SHOULD-FIX — Easing is installed after the state change can already be computed.** [host/web/input-glue.js:299](/tmp/rv-s8b1-r1/host/web/input-glue.js:299) sets `data-exact-easing` after toggling `data-pressed`, running D7 paint, and resolving a variable press scale with `getComputedStyle()`. Those reads can commit the destination colour before its transition exists. Establish transition policy before changing state; explicitly suppress overlay transitions under Reduce Motion and agent mode, where merely omitting the attribute leaves authored transitions active. Test intermediate values—the current test finishes animations before checking colours.

8. **SHOULD-FIX — Hover and focus press scales are discarded.** [host/web/src/host/looks.rs:70](/tmp/rv-s8b1-r1/host/web/src/host/looks.rs:70) removes `PressScale` from every look, but `with_press()` preserves only the active state’s value. Accepted hover/focus scale declarations therefore do nothing. Compose the effective press scale through the same state precedence as other rows.

9. **SHOULD-FIX — Valid `unset` look values diverge between targets.** [host/web-js/src/style.rs:207](/tmp/rv-s8b1-r1/host/web-js/src/style.rs:207) sends `"unset"` directly to the value parser; [runner/src/instance/looks.rs:42](/tmp/rv-s8b1-r1/runner/src/instance/looks.rs:42) does likewise and silently drops failures. An active `opacity="unset"` fails the JS build and leaves the base opacity unchanged in wasm. Handle `bridge::unsets()` as static rows do, preserving the written-only clear.

10. **SHOULD-FIX — A held press keeps an obsolete scale after live updates.** [host/web/input-glue.js:285](/tmp/rv-s8b1-r1/host/web/input-glue.js:285) captures the scale as a numeric animation endpoint. A live token batch or mode switch updates CSS, but a stationary press remains at the previous scale until another pointer event. Retarget held feedback when its effective scale changes; test an update while pressed without pointer movement.

Cargo and browser tests were blocked by filesystem/listen restrictions. In-memory probes exercised the input and sheet-lifecycle paths; the workspace remains unchanged.
