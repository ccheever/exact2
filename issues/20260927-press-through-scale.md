# The press composes through CSS `scale`, never `transform`, and is kept under reduced motion

**Status:** Open
**Systems:** Web (`host/web/index.html`, `host/web/src/css.rs`, `host/web/input-glue.js`), Apple (`PressFeedback.swift`), Linux, kernel schema bit 147
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1061 (the 2026-09-27 ruling), LLP 1055.000 §8 ruling 3 (`transform` owed), LLP 1001

The web shell's `[data-pressed] { transform: scale(var(--exact-press)); }` (`host/web/index.html:63`) writes CSS `transform`. It will replace an author's `transform` when the owed HTML row lands, and it already can on an SVG element that carries `transform`. Charlie ruled that the press composes through CSS's separate `scale` property, as Apple's host already does, and that it is kept under reduced motion.

**Do:**
- Emit the web press as `scale: calc(<row scale> * var(--exact-press))` from `css.rs`, or the equivalent, so it multiplies with the row's `scale` and leaves `transform` alone.
- Make the press hit box undo exactly that factor about `transform-origin` (`input-glue.js:65`, from the review's Grok #8).
- Remove the reduced-motion drop on every host (LLP 1061 D5), and update the schema comment on bit 147 (it still says "about its centre").
- Test: a pressable SVG element with an authored `transform`, and a box with a `scale` row, both keep their geometry while pressed. Test the press under `prefer prefers-reduced-motion reduce`.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
