# A textarea cannot be resized: Contract refuses every `resize` but `none`

**Status:** Closed
**Resolution:** the compiler admits every keyword; the Mac and iOS draw the grip and `Runner::resize_box` writes the dragged size (LLP 1001, "CSS form and text vocabulary")
**Systems:** contract/lower, runner, host/apple
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-30

## Problem

`textarea resize="vertical"` fails to compile, so a textarea cannot be dragged taller.
Tailwind's preflight gives every textarea `resize: vertical`, and an app cloned from a
Tailwind page (T3 Code's settings) loses the grip its reference has.

## Cause

`contract/lower/src/values.rs:468` refused `both`, `horizontal`, `vertical`, `block` and
`inline` (`lower-css-resize`) because no native presenter drew a grip or could change a
box's size, though `kernel/tables/schema.json:4069` declares the row. The Mac's textarea
is an `NSTextView` in an `NSScrollView` with no grip (`Mac/TextAreaMac.swift` `makeTextArea`).

## Fix

The compiler admits every keyword. On the Mac and iOS a textarea with `resize` draws
Chrome's two strokes in its padding box's bottom end corner (`ResizeGrip.swift`); a drag
sends the border box the pointer reached (`covers` kind 16), and `Runner::resize_box`
writes `width` and `height` in the box's `box-sizing`, as Chrome's inline style does.
The web writes the CSS and the browser does the rest.

## Acceptance

`cargo test -p exact-apple --test it box_resize`; a drag of the grip on the web and the
Mac grows the box by the drag, within `min-height` and `max-height`.
