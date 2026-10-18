# Code review: LLP 1122 T2b, the JS target, step 3 (inspect on the JS page), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `faf272b86`.
- **Method:** one brief (sha256 `87d029298cdfcc7ad14f2a3192cdc293a089387b8936d5df36e7a492e004491f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 8 medium, 2 low).
- **Disposition:** taken in most part.
  - The dev client's early gate is complete while the module loads: it installs a cover and a guest-frame shield, stops pointer movement too, and lets Escape go (it clears the kept state). A module that fails to load lets go too. The module removes the stand-ins when it installs.
  - Focus events stop for the inspector's own controls too; only the app's are blurred.
  - A colour's `currentcolor` is a clear, as the runner treats it, and an empty pin sets nothing.
  - After a live edit, a token row keeps the table's value when its tokens' slices are as they were built. A changed token's value is written as `node` writes it (a colour, a number, a px length as the table's kind), else as the computed CSS, said so.
  - The table must be the page's plan's (`plan`); a page whose plan was rebuilt says to select again after it reloads.
  - A kept selection names its inspect command: a new command clears it, and a reload restores it only under the same one.
  - An app with no axes reports `modes: {}`, as `node` does.
  - The test now compares `testId` and `computed` too.
  - Limits, for §10: a style row's fallback against an expression's row above; the CSS map's gaps (step 2's); SVG geometry written as attributes (`attribute_rows`: `x`, `y`, `width`, `height` on svg and its kin, and a radial gradient's `cx`, `cy`, `r`); outline marks and scrolling in document order, not the runner's site order; `from` and `node` are the page's view ids.

---

1. **High — Reload gating still leaves guest frames and pointer movement exposed.** [dev.mjs:468](/tmp/rv-dt-js3r2/host/web-js/dev.mjs:468)  
   The synchronous gate fixes listener ordering, but explicitly passes `pointermove` and installs no cover or frame shield. If the app mounts before the asynchronous inspector loads, pointer movement reaches app listeners, and clicks inside an iframe never traverse the parent’s gate. **Fix:** install the cover, frame shield and complete event gate synchronously when restoring inspect mode.

2. **Medium — Inspector focus still reaches app handlers inside modals.** [inspect-dev.js:307](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:307)  
   `focusing` returns without stopping events targeting the banner. Clicking Close—or returning focus there from a guest frame—therefore delivers `focusin` to an enclosing app dialog’s handlers. This leaves round 1’s focus fix incomplete. **Fix:** stop propagation for every focus event while inspecting; exempt inspector controls only from being blurred.

3. **Medium — The property-map fix still drops supported dynamic rows.** [inspect-dev.js:78](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:78)  
   `css_property` does not describe every emitter write. For example, the existing native-buttons fixture writes dynamic `-exact-control-size` through `--exact-control-font-size`; inspection reads nonexistent `control-size` and omits the own row. Legacy line clamps and animation timelines have similar mismatches. **Fix:** generate readers from the actual emitted writes, including custom properties and attributes.

4. **Medium — Clear detection still disagrees with the runner.** [inspect-dev.js:121](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:121)  
   Outside a boundary, dynamic `color="currentcolor"` is reported as own `dynamic`, although the runner clears that row and reports inheritance. A dynamic empty mode string likewise passes this check and incorrectly produces tuple-plus-one. **Fix:** use the runner’s row-specific clear semantics and binding ownership metadata; check `$xi` for ordinary colour nodes too, and reject empty pins.

5. **Medium — Static text-style metadata overrides current inheritance.** [inspect-dev.js:173](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:173)  
   Put dynamic `font-weight=w`, with `w=300`, above a child with static `font-size="-exact-headline"`. The static table records the headline’s fallback weight, and this branch returns it before examining the dynamic ancestor. The runner reports inherited `300`. **Fix:** resolve style candidates against current ancestor ownership rather than treating `entry.s` as unconditional. The documented dynamically chosen style limitation does not cover this case.

6. **Medium — Control fallback failures extend beyond the stated limit.** [inspect-dev.js:172](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:172)  
   Under a *static* `text-transform="uppercase"` parent, give a button `text-transform=(flag ? "lowercase" : none)`. When cleared, its table entry lacks the control start because `inspect.rs` excludes dynamic rows from `i`. Inspection inherits uppercase; the runner uses the button’s initial `none`. **Fix:** retain control-start candidates even for dynamically bound rows, and distinguish an absent declaration from an explicit inherited clear.

7. **Medium — Live token values are not serialized as row values.** [inspect-dev.js:113](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:113)  
   After a live edit, an opacity token valued `0.25` becomes the JSON string `"0.25"`, whereas `node` returns the number `0.25`. A length token valued `0.5rem` is similarly returned unresolved. Neither is marked `computed`. **Fix:** resolve and serialize by row type and instance context, or use an explicitly marked computed fallback.

8. **Medium — The fetched table is not verified against the page’s plan.** [inspect-dev.js:26](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:26)  
   A rebuild replaces `dist` before the old page completes its checkpoint/reload. A first selection during that interval can fetch the new `dev-inspect.json`, cache it under the old page’s plan, and report another generation’s rows using the old identity. The server’s currency check cannot detect that substitution. **Fix:** bind tables to plan digests and verify the fetched identity before reporting.

9. **Medium — Reload can resurrect another inspect command’s selection.** [inspect-dev.js:403](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:403)  
   A new inspect sequence clears only the in-memory selection. The persisted selection remains, while the persisted inspect sequence advances. Reload then restores the previous command’s selection under the new command. **Fix:** clear persisted selection on sequence changes and store/validate its owning sequence.

10. **Low — Apps without axes report the wrong tuple shape.** [inspect-dev.js:192](/tmp/rv-dt-js3r2/host/web-js/inspect-dev.js:192)  
    For a live plan node without app axes, the runner returns `modes: {}`; this code omits it, producing `tuple: null`. **Fix:** preserve the empty modes object.

11. **Low — The parity test does not establish its claimed node coverage.** [tamber-server.test.mjs:650](/tmp/rv-dt-js3r2/scripts/tamber-server.test.mjs:650)  
    Clicking a button’s centre selects its child text node, but the test never asserts the returned `testId`. It also discards `from` and `computed` and omits the dynamic, reload and live-edit cases above. **Fix:** assert selected identities, compare `from` through normalized site/instance identities, and add those regression scenarios.

The trim now preserves `computed`, and the selected node’s own style can correctly supply `from`. The SVG limitation is inaccurately described: [attribute_rows](/tmp/rv-dt-js3r2/host/web/src/element.rs:739) covers several SVG geometry attributes and radial-gradient coordinates; circle `r` is not that attribute path.

Release exclusion looks correct by source inspection. The module route is Host-gated, not authenticated to an individual page, and grants no cross-origin CORS access. The release test checks source strings rather than built artifacts. Tests were not run because they build and write files.

**Verdict: Request changes—round 1’s fixes are incomplete, and neither report parity nor complete input isolation holds.**
