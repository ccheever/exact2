# Apple text: a `pre-wrap` paragraph's last spaces take none of its max-content width

**Status:** Closed
**Resolution:** at the paragraph's end the spaces hang only as far as they would not fit, as before a forced break
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
wrap. CSS Text 3 §4.1.3 says so, but Chrome counts them in the max-content width and hangs
them only past the box's room.

## Fix

At the paragraph's end `pre-wrap`'s trailing spaces take the rule they already had before a
forced break: they count as far as they fit in the line's room. A max-content measure has
no limit, so they count; in a narrower box they still make no sideways overflow, which #1100
was for. A soft wrap's spaces still hang wholly.

## Acceptance

`TextCSSTests.testAWrappedLinesTrailingSpaceTakesNoWidth` (`host/apple/build.mjs --test`):
a `pre-wrap` "word   " measures as `pre`'s at max-content and in that width, and as its
room in a narrower one.
