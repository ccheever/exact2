# Apple text: a `pre-wrap` paragraph's last spaces take none of its max-content width

**Status:** Closed
**Resolution:** at the paragraph's end the spaces conditionally hang, as before a forced break, and alignment places the ones that count inside the line
**Systems:** host/apple
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-31

## Problem

A word laid out as its own `pre-wrap` text with its space after it ("Say ") measures as
"Say" on the Mac and iOS. In a `flex-wrap` row of such words, each box ends at its last
letter and the next word starts there: "Say hello in plain words" draws as
"Sayhelloinplainwords" (T3 Code's sent message with an image chip, which lays out its
words one per text). Chrome sizes the same row with the spaces: at 14px Helvetica a
`pre-wrap` "Say " is 28.02px wide at max-content and "Say" 24.13px (Chrome 156).

## Cause

`TextEngine.uncountedTrailingWidth` (`host/apple/Sources/ExactCore/ParagraphGeometry.swift`,
from #1100) hangs `pre-wrap`'s trailing spaces wholly at the paragraph's end, as at a soft
wrap. CSS Text 3 §5 makes the end of a block a forced line break, before which §4.1.2
(Phase II, rule 4) has `pre-wrap`'s spaces conditionally hang, and §8.2 counts conditionally
hanging glyphs in a max-content size but not in a min-content size.

Alignment did not place the spaces that count: `TextEngine.penOffset` takes CoreText's pen
offset, which leaves out all trailing white space. A right-aligned `pre-wrap` or `pre`
"Say " in a 100px box drew "Say" at 75.87 where Chrome draws it at 71.98, with the space
ending at the edge.

## Fix

At the paragraph's end `pre-wrap`'s trailing spaces conditionally hang, as before a forced
break: they count as far as they fit in the line's room. A max-content measure has no limit,
so they count; in a narrower box they still make no sideways overflow, which #1100 was for.
A soft wrap's spaces still hang wholly. `penOffset` aligns a line as its ink and the spaces
its width counts, for every alignment and in right-to-left lines, where CoreText draws them
left of the pen.

## Acceptance

`TextCSSTests` (`host/apple/build.mjs --test`):
`testAWrappedLinesTrailingSpaceTakesNoWidth` measures a `pre-wrap` "word   " at max-content,
in that width and in a narrower one, and
`testAlignmentPlacesCountedTrailingSpacesInsideTheLine` checks the lines' starts against
Chrome's for right- and centre-aligned shrink-to-fit, wide right- and centre-aligned boxes,
`pre` and `normal`, an all-space line, a soft wrap, right to left, and min-content.
