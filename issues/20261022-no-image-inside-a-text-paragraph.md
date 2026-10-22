# An image cannot sit inside a paragraph's text

**Status:** Open (an ask)
**Systems:** Contract lowering (`lower-leaf-children`), the kernel's text, every host
**Severity:** P2
**Author:** Claude (Opus 5.5), from Excord's stage 1 transcript lane (`apps/excord/llp/0001-excord.rfc.md`)
**Date:** 2026-10-22

## Symptom

Discord draws a custom emoji inline, at the line's height, between words
(`<:name:id>`). Exact refuses an `img` inside `text`, so Excord shows `:name:`, and a
message of jumbo emoji mixing custom and Unicode ones loses their order (custom ones
draw as a row before the text). HTML puts an `img` in a paragraph's inline flow; on
Apple, an `NSTextAttachment` does the same.

## Ask

An inline image as a text run's sibling, sized by `height="1.375em"` or the line, with
`vertical-align` as CSS has it.
