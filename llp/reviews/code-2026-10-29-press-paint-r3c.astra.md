# Code review: web press paint keeps authored transitions, round 3 narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5f5335ab5b`.
- **Method:** one brief (sha256 `dac9612f6d3a7e890e6f172b69888a371abb984420102fafec61e54f1858c321`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [host/web/input-glue.js:348](host/web/input-glue.js:348): Kept declarations outlive author changes.** With `transition: all`, changing base opacity from `.8` to `.6` while held retains the `.8` formula. Release returns toward `.8`; removing the rule then exposes `.6` and starts a delayed author transition. Colour has the same problem. Clearing the pressed properties also leaves their kept declarations active. The new class-turn test changes no actual style, so misses this. [Transition rules](https://drafts.csswg.org/css-transitions-1/#starting).

- **MAJOR — [host/web/input-glue.js:309](host/web/input-glue.js:309): The no-image case changes base-colour clipping.** For `background-image:none; background-clip:content-box`, the generated rule replaces clipping with `border-box`. The author’s background colour immediately fills the padding/border area, even at paint progress zero. The bottom layer’s clip controls the background colour; its original clipping must survive. [CSS clipping rule](https://drafts.csswg.org/css-backgrounds-3/#background-color).

- **MAJOR — [host/web/input-glue.js:314](host/web/input-glue.js:314): Position-transition protection misses axis longhands.** `transition:background-position-x 1s` passes this guard, although writing `background-position` changes that property. With two differently positioned images, prepending the position list starts a transition and moves the author’s layers. The same applies to `background-position-y`. [Position longhands](https://drafts.csswg.org/css-backgrounds-4/#background-position-longhands).

- **MINOR — [host/web/input-glue.js:301](host/web/input-glue.js:301): The splitter treats quoted parentheses as syntax.** `url("a(b.png"), url("c.png")` is counted as one image. Consequently, the generated longhand lists have two entries for three actual layers, and the second author image receives the paint’s values through cycling.

- **MINOR — [host/web/input-glue.js:384](host/web/input-glue.js:384): `pressShown` reports omitted background paint.** When `layer()` returns `null` because of a position/size transition, this helper still composites the pressed colour. Inspection can report fully pressed blue while the actual background remains unchanged.

- **NIT — [llp/1061-press-feel-and-reduced-motion.rfc.md:271](llp/1061-press-feel-and-reduced-motion.rfc.md:271):** “Reads with the paint off” contradicts the new kept-declaration re-read.

The other listed corrections look sound on inspection. In-memory probes confirmed the declaration/list issues and correct `hold()` bookkeeping. Browser execution was blocked by the read-only sandbox’s temporary-file restriction.

**Verdict: Not ready; three MAJOR issues remain.**
