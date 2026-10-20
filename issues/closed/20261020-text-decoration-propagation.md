# text-decoration does not reach nested text on Apple, nor in its box's colour on Linux

**Status:** Closed
**Resolution:** fixed: Apple runs (live paragraphs, region runs, widget snapshots) take their decorating ancestors' lines; live and region paragraphs on Apple, and Linux, draw each line in the nearest decorating box's colour, on visible text only, as Chrome; a widget snapshot draws them in the run's colour
**Systems:** host/apple, host/linux
**Author:** Daehyeon Mun
**Date:** 2026-10-20

## Problem

CSS draws a box's `text-decoration` lines through its inline descendants' text, in the box's
colour (CSS Text Decoration 3 §2.1). On macOS and iOS a decorated `text` draws no line through
its nested `text`. Linux draws the line, but in each glyph's colour, where Chrome uses the box's.
The T3 clone passes its `~~strike~~` flag down to each nested run instead.

## Cause

Apple builds a run's paint from its own style only:
`host/apple/Sources/ExactKit/NodeText.swift:106` and `Text.swift:62-63` (live paragraphs),
`host/apple/src/content_region/wire.rs:102` (region runs), `HeadlessSession.swift:368`
(snapshots). Linux walks the ancestors for the lines but fills them with the run's colour
(`host/linux/src/paint/text_decoration.rs:63,75`).

## Fix

Each run takes the lines of its decorating ancestors, each in the nearest such box's colour,
as Chrome paints them; hidden text draws none, a hidden box's lines still cross visible text.

## Acceptance

`<p style="text-decoration: line-through">a <span>b</span> <b>c</b></p>` and an underlined link
holding monospace code draw as Chrome does on macOS, iOS and Linux, line colours included.
