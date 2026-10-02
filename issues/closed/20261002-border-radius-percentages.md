# border-radius accepts only a number, so CSS's border-radius: 50% is refused

**Status:** Closed
**Resolution:** Fixed by 28d617918.
**Systems:** kernel, Contract lowering, hosts
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-02
**Related:** kernel/tables/schema.json border_radius_* (f32), llp/1001-kernel-v1.spec.md

`view width=32 height=32 border-radius="50%"` fails to compile:
``[lower-attr-value] `border-radius="50%"` is not a valid `border-radius`: expected number``.
`border-radius: 50%` is the web's usual way to make a circle, and agents reach for it
first (a fresh Claude Code session building a todo app did, 2026-10-02). The workaround
is half the box's size in pixels, which breaks as soon as the box's size changes.

The kernel stores one `f32` per corner (`border_radius_top_left` … in
`kernel/tables/schema.json`), so a radius is always a circular length. CSS also allows
percentages, resolved against the border box's width for the horizontal radius and its
height for the vertical one, which makes non-square boxes elliptical. It also allows
explicit elliptical radii (`10px / 20px`). LLP 1001 declares no deviation for any of this,
so under "the web is the standard" it's a gap rather than a choice.

Verified alongside: on whole-pixel radii the hosts agree. Radii 8, 14 and 16 on a 32 px
box and 56 and 64 on a 128 px box gave identical filled-pixel counts on the web and on
macOS (2026-10-02).

Options: carry horizontal and vertical radii per corner, with percentages resolved
against the laid-out box, so both percentages and the `/` form work as in CSS. Or keep
lengths only, declare the deviation in LLP 1001, and make the refusal name the
workaround.
