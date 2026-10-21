# Code review: Linux: a role drawn in the accent reads the nearest authored accent-color, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `da4e7b31690b85f2d8271efc9568c51cadfc812627f2b1f5393793efa364ec41`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: eight medium, one low; Grok: one high, two medium, three low.
- **Disposition:** the scope opened at the other paths in round 1; SVG descendants, region replay and theme aliases named

---

Review notes are at `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-d25516f8.md`.

The ordinary `node()` walk is correct: the thread-local defaults to `None`, drop restores nested scopes, and an authored accent wins over a theme tint the way the web does. `kernel/src/style.rs` is 1497 lines, three under the cap. Retained content and paint motion still resolve accent roles with no scope.

- **bug HIGH** `host/linux/src/paint/region.rs:254` — Content-region capture and replay resolve accent roles with no scope, so those colours stay the app tint.
- **bug MEDIUM** `kernel/src/motion.rs:281` — Paint-motion targets are resolved at commit time with no accent, then painted over the corrected static colour until the transition ends.
- **bug MEDIUM** `host/linux/src/paint.rs:883` — SVG elements, inline runs, and the kept-row image fast path resolve under an ancestor's accent, so a nearer `accent-color` is ignored.
- **suggestion LOW** `host/linux/tests/it/accent.rs:54` — The new test only samples boxes, so the missed paths stay green.
- **nit LOW** `kernel/src/style.rs:728` — The file is three lines under the 1,500-line cap.
- **nit LOW** `QUEUE.md:94` — The Android note still says the Linux painter never applies an authored accent.The reviewer finished. I'll check each finding against the paint paths before reporting it.The ordinary paint walk holds. Two paths the reviewer flagged check out, and the web self-reference rule does not. I'll record those and close the review.The ordinary Linux `node()` walk does what this commit says. A role the browser draws in the accent (`AccentColor`, `LinkText`, `-exact-link`, `-exact-symbol`) resolves to the `accent-color` scope that `node()` installs, the guard restores on drop and on unwind, and a real authored colour wins over the theme's `AccentColor`. `kernel/src/style.rs` is 1497 lines. There is one high finding and four medium ones.

## High

**Retained content regions still paint the app tint.** `host/linux/src/paint/region.rs:254`

`Picture::capture` runs before the paint walk and calls `BoxPaint::capture`, which resolves background, border, shadow, and gradient stops while the thread-local is still `None`. Text is resolved earlier, in `host/linux/src/content_region/source.rs:142`, and a scheme change re-resolves the stored role with no accent at `source.rs:44`. SVG scenes keep the `ColorValue` and resolve it in `Replay::paint`, which `node()` takes at `host/linux/src/paint.rs:876` and returns before the guard at line 896. A link or a tinted box inside a content region (the markdown reader's region) stays the app tint. The new test never builds a region.

Open `authored_accent` from each captured node's computed `accent-color` before `BoxPaint::capture`, `image_tint`, and each text run. Store that accent beside the saved `ColorValue` so `palette()` can re-resolve it when the scheme flips. For SVG, open the same per-element scope in `Replay::paint` before `paint_svg`, because those colours resolve at replay.

## Medium

**A colour transition paints the app tint, then snaps.** `kernel/src/motion.rs:281`

`color()` resolves the paint target with no node and no scope. `paint_sync` / `adopt_paint` call `color_targets` at commit time, and `BoxPaint::presented` paints that RGBA over the row for the whole transition. The static capture inside `node()` is the authored accent. When the transition ends, the colour snaps to it. Apple's `sync_paint` uses the same `color_targets`.

In `color_targets`, open `authored_accent` from that node's computed `accent-color` for the duration of the resolve. That also corrects Apple motion targets, which bake the same tint today.

**SVG children, inline runs, and a kept row's new picture ignore a nearer accent.** `host/linux/src/paint.rs:883`

SVG elements return here before the guard, and inline runs return at line 887. Both are painted later under the `<svg>` or the paragraph (`svg.rs` `ink` / `svg_text`, `inline.rs:21`). A child that authors its own `accent-color` still draws in the ancestor's accent. The same miss is on the kept-row fast path: `pictures_again` (`host/linux/src/paint/rows.rs:444`) resolves `image_tint` under the scroller's live scope. An ancestor accent with no nearer override is fine, which is all the new test checks.

Open the scope from each run in `text_palette`, `text_backgrounds`, and `run_looks`, from the item's node in `svg_item`, and from the image node in `pictures_again`.

**`accent-color="AccentColor"` drops an ancestor's authored accent.** `host/linux/src/paint.rs:897`

The web refuses to publish `--exact-accent` when the declared value is `var(--exact-accent, AccentColor)` (`host/web/src/css.rs:343`), so that declaration inherits the ancestor's custom property. The painter installs the computed `AccentColor` role as the scope, and `painted_accent` (`kernel/src/style/roles.rs:450`) clears the scope and resolves the role. The subtree paints the app tint, or the theme's `AccentColor`, instead of the ancestor's colour. With no ancestor, the tint is the right result. Descendants inherit that same specified role, so skipping the guard only on the authoring node is not enough.

When opening the scope, walk to the nearest computed `accent-color` that is not the `AccentColor` role. Leave the scope unset when every row on that walk is that role, so the root still falls through to `painted_override`.

**Android native fields resolve after the scope is gone.** `host/linux/src/presenter/hosted.rs:383`

`field_look` runs from `canvas/field.rs` after paint, once every `AccentScope` has dropped. `text_color().resolve` and the caret resolve at line 395 see no authored accent, so a field coloured `AccentColor`, `LinkText`, `-exact-link`, or `-exact-symbol` stays the app tint. The field's chrome is fine: `accent()` reads the `accent-color` row itself.

Open `authored_accent` from that field's computed `accent-color` around the text and caret resolves.

## Low

**The new test only samples boxes.** `host/linux/tests/it/accent.rs:54`

It checks a box fill, one `-exact-link` border, nesting, and the sibling after the subtree. It does not paint a text run, an SVG fill, a shadow, a content region, or a colour transition, so the misses above stay green.

**`kernel/src/style.rs` is three lines under the cap.** `kernel/src/style.rs:728`

1497 lines after the hook in `ColorValue::resolve`. `roles.rs` and `paint.rs` are under 1500. Move an existing block out of `style.rs` before adding anything else there.

**QUEUE still says the Linux painter never applies an authored accent.** `QUEUE.md:94`

Clause (2) of "Android's colours and channel" is stale for the ordinary walk. It is still true for content regions and for the terminal painter, which never opens a scope.

## Checked, and holding

The thread-local defaults to `None`, so Apple's Rust `resolve`, the terminal, and wasm are unchanged. `web_accent` is the browser set. `ButtonText` and `Highlight` stay themselves, matching `follows_accent` on the web. A real authored colour wins over the theme the way `var(--exact-accent)` does. Placed nodes and spatial islands re-enter `node()`. Focus rings and menus resolve the `accent-color` row itself. A non-role `resolve` is one match before the thread-local read. Drop restores the previous scope on unwind, and resolving the accent clears the scope first, so `accent-color="AccentColor"` does not recurse.

Review notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-d25516f8.md` and `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-d25516f8.md`. The range is the one commit `f9c97d3..HEAD` (6 files, 112 insertions, 2 deletions). `origin/main` is `77457568`, ahead of that parent.
