# Code review: conform --linux: synthetic-decorations' rangeFrame held to the Linux host's specified 'unavailable' (theme2 04:27), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `1cdb3d192c`.
- **Method:** one brief (sha256 `3fc654272dbbb9c2113470b680a9be5075b947cb948e13202198b16869fb5305`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking or should-fix; one nit (the QUEUE reader's rects are field-local)
- **Disposition:** taken in 984a608543

No blocking or should-fix findings. The change matches theme2’s ruling.

**Nit** — `QUEUE.md:1573` — `field.rs` caret and highlight rects are content-local (`caret` is `(x, line.top, width, height)`), not viewport. Viewport is `PaintedField::place` plus the field transform in `host/linux/src/paint/field.rs`, then the same scroll-to-viewport step `frame` uses, unclipped. Say that in the queue sentence so a reader is not built from the raw rects.

**(a)** Met. A `{slot, value}` entry runs the step, settles, and `fail`s unless `state().slots[slot] === value` (`conform.mjs:394-399`). A throw (stdio crash is `new Error`) `fail`s on that same step. Earlier steps do not match `'tap measure'`, so boot, `tap reset`, and `tap replace` still compare. Tree order is preorder, so those two presses come before Measure. After that, Linux is closed. The only later drive step is `clock +60000`, which would compare full state, including the now-divergent `geometry` string, so ending there is required. The measure step’s tree, layout, and screenshot are not equality-checked; they differ only by that specified string.

**(b)** Slot read is right. `state geometry = ""` is a string slot; `state()` emits `slots` as typed JSON (`runner/src/agent.rs:688-709`). `measure` sets `geometry` to `"unavailable"` when `caret.unavailable` is true (`decorations.contract:20-24`). `Geometry.unavailable` is the bool (`contract/types/src/geometry.rs:27`), and a host with no reader returns `GeometryAnswer::UNAVAILABLE` (`runner/src/geometry.rs:110-111`, `783-788`). `L.tap` returns only after the action lands; `L.clock('settle')` is the same settle the drive uses before a compare.

**(c)** Unchanged. Two-element entries have `apart[2] === undefined`, so the new block is skipped and the old stop-before-the-step path runs (`conform.mjs:393-401`). A `null` step still never opens Linux (`conform.mjs:783`).

**(d)** Mismatch and caught errors both call `fail(step, …)` with `tap measure`, and `fail` always pushes `report.failures` when no field is passed (`conform.mjs:281-291`). `L.close` runs after the try, including on failure; the drive `finally` closes it if that path is skipped (`conform.mjs:574`).

**(e)** The Apple claim is right. `InlineGeometry::range_frame` is implemented only in `host/apple/src/geometry.rs`. Linux uses `RunnerLinks::ALL` → `KERNEL` and never calls `set_inline_geometry`, so `scrolled.inline` stays `None` and `rangeFrame` is unavailable. The web reader is `page_range_frame` in `host/web-geometry`, not that trait. `host/linux/src/text/field.rs` is the right geometry (`lines`, `spans`, `caret`, `highlight`). Windows links `exact-linux`, so it is the same code.
DONE
