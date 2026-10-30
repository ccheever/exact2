# Code review: web press paint keeps authored transitions, round 3 narrow re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `5f5335ab5b`.
- **Method:** one brief (sha256 `dac9612f6d3a7e890e6f172b69888a371abb984420102fafec61e54f1858c321`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

**MAJOR** `host/web/input-glue.js:348` — A kept opacity or colour declaration is copied for the rest of the press, so a clear or a base change never lands.

Once `state.kept.opacity` or `state.kept.color` is set, every later `paintRule` copies that string into `next` (colour at line 359) and skips `own()`, `later()`, and a fresh base. `own()` reads the inline style, which the paint rule does not hide, so a clear is visible without sampling the painted computed value. Clearing `--exact-press-opacity`, `--exact-press-opacity-to`, or `--exact-press-color` while held leaves the old `!important` declaration up; the web paragraph still says a property cleared while held stops showing (`llp/1061-press-feel-and-reduced-motion.rfc.md:293`). The clear test uses a node with no transition on that property, so this path stays untested. An authored change to the underlying opacity or colour leaves the baked `calc` in place, and that declaration holds the computed value, so the node's transition on it never starts and the paint is never left out for it. The release eases to the base captured on the first write. A re-press during the release reuses the same `state` and key, so the stale declaration survives the turn. The reduce-to-kept read is doing its job for everything else: unkept background and mask lists are dropped before `getComputedStyle`, and the first paint bakes its base with no rule up.

**MINOR** `host/web/input-glue.js:314` — A transition on an axis position longhand still shifts the layer.

`transitioned` for `background-position` and `mask-position` matches that longhand, `all`, and the `background` / `mask` shorthands (`HOLDS` at line 290). It misses `background-position-x`, `background-position-y`, `mask-position-x`, and `mask-position-y`. Those still receive the prepended list, which changes the axis and starts the interpolation this round drops the layer to avoid.

**MINOR** `llp/1061-press-feel-and-reduced-motion.rfc.md:271` — The web paragraph still says the glue reads with the paint off.

The same paragraph now says a written declaration is kept for the press, and the code re-reads with those declarations left on. Line 271 still says every turn, style or class change, and scheme flip reads with the paint off so the release eases back to the look as it is then. The "inline !important" wording at line 280 matches the cascade.

Round-3's six fixes hold; the kept short-circuit still ignores a mid-press clear or base change of opacity or colour.
