# A padded glass row with align-items=center places its children 3 pt high on iOS

**Status:** Open
**Systems:** kernel (layout), Apple host (iOS), materials
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

The bottom bar of the Linear port:

```
row height=56 padding=4 box-sizing="border-box" backgroundMaterial="glass" align-items="center"
  button height=48 …
```

`agent ios "layout tab-inbox"` put the 48 pt buttons at (4, 1) in the row, not (4, 4). A button
beside the row with no padding sat where expected. `align-items="flex-start"` puts them at y 4.

## Expected

CSS: the row's content box is 56 − 2 × 4 = 48 pt tall, so a 48 pt child centred in it starts
at y 4. A y of 1 is what a child 54 pt tall gives, (48 − 54) / 2 + 4, so the first thing to find is
whether the button lays out taller than its `height`. Compare the same tree in Chrome (the
layout oracle) and on iOS, with and without `backgroundMaterial`, before changing the kernel.

## Reproduction

The row above holding one `button height=48`; `agent ios "layout <button>"`.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-20),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet reproduced on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` (entries 21 and 22) on Charlie's Mac.
