# LLP 1055: `box-shadow` and `text-transform`

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Kernel (style table, text runs), Contract (two attributes), Web host (CSS), Apple host (iOS and macOS presenters, captures), Linux host (painter)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1001 §1 (the style table), LLP 1034 D1/D2 (a colour pair the host resolves), LLP 1035.000 D4 (an inherited change touches its descendants), LLP 1007 (web CSS from rows), LLP 1008 (Apple presenter), LLP 1015 (Linux painter), LLP 1053 G2 (borders as the web paints them)

## Summary

A voice-journal port (grnl) had to drop two things from its design: the
warm elevation under its Record button, and uppercase section labels (it
upper-cased them in data). CSS spells both, and exact2 now does:

```
button box-shadow="0 6px 18px rgba(120, 60, 20, 0.35)"
text "Recent" text-transform="uppercase"
```

`box-shadow` sets the four shadow rows the kernel already had (bits 57–60),
which only the web ever drew. `text-transform` is a new row, bit 100.

## Motivation

The shadow rows existed and the web composed them, but no attribute set
them, and neither native host painted them. Case in data is wrong twice: it
is the author's copy, not a style, and it cannot follow a state or a class.

## Design

### D1 — `box-shadow` is one value; each of the four rows takes its part

The attribute binds its value to `shadow_color`, `shadow_offset`,
`shadow_radius` and `shadow_opacity`, as `gap` binds two rows. Each row's
dynamic parser, handed text, runs the one `BoxShadow::parse` in the kernel
and keeps its part. So a literal, a `style` block, a `class=(c ? A : B)`
choice and a computed string all behave alike, and a refusal reads the
same at compile time (the literal probed through the row) and at run time.

The grammar is CSS's single shadow: `none`, or `<color>? <length>{2,4}
<color>?` with lengths in `px` (unitless zero). `none` is opacity 0; a
shadow is opacity 1, the colour carrying its alpha, as CSS does. Refused by
name: a comma list, `inset`, a non-zero spread, and a missing colour (CSS's
default is `currentcolor`, which a shadow row cannot hold). A number is
refused at compile time.

*Rejected:* splitting the literal in the compiler, as `border-color` does —
a computed string would need a second parser at run time. A spread row —
no port asked for one, and every host would have to draw it. A list —
Core Animation casts one shadow per layer.

### D2 — Painted outside the border box only, blur radius = 2σ

CSS clips an outer shadow to outside the border box (Backgrounds 3 §7.1.1),
so it never shows through a translucent background, and its blur radius is
twice the Gaussian's standard deviation. Every native host holds to both.

- **Apple.** A contentless `ShadowCaster` layer, the node's lowest
  sublayer, casts the rounded border box through `shadowPath` (no offscreen
  pass) with `shadowRadius` = blur / 2, masked even-odd to outside the box.
  A node that clips its overflow would clip that layer too, so its children
  move into a clipping box of their own, as a scroll view takes them; a text
  node that clips keeps clipping (a box would not clip its own glyphs), so
  its shadow is clipped. macOS's `cacheDisplay` capture (the agent's
  screenshot, a canvas surface) draws views, not sublayers: there the node
  draws the shadow with Core Graphics before its box. iOS's GPU capture
  mirrors the caster's shadow and mask.
- **Linux.** Neither backend blurs. A Gaussian of an edge falls off as the
  normal CDF of the distance, and the outline's offset curves are its
  contours, so the shadow is a stack of offset outlines, each filled with
  the alpha that brings the stack to the CDF at its band — through the
  border fill both backends draw (a region inside a clip). Bands are at
  most 1.5 pt wide, at most 32. A shadowed node opts its frame out of flow
  damage, which never looks outside a box.

*Rejected:* the node's own layer shadow — `masksToBounds` clips it and it
shows through a translucent fill. A sibling layer in the parent — it must
follow motion, and its order among siblings breaks when they reorder. A real
blur on Linux — a raster pass and a picture per shadowed node.

### D3 — The web emits a scheme-aware shadow

The four rows still compose into one `box-shadow`. The opacity row folds
into each half of a `light-dark()` pair and the pair is emitted as such, so
the browser resolves it per element (LLP 1034 D2); it used to take the
light half. An invisible shadow is `box-shadow:none`.

### D4 — `text_transform`, bit 100

`enum:TextTransform` — `none`, `uppercase`, `lowercase`, `capitalize` —
inherited, a text row, a layout row: it changes what is measured.

### D5 — Applied once, where the kernel makes a paragraph's runs

`Arena::text_runs` transforms each run's string; `TextRun.text` is a
`Cow`, borrowed when nothing changes. Every measurer (Apple's CoreText
callback, Linux's cosmic-text, the monospace reference) and every painter
that reads runs (Linux, Apple content regions) reads that one string. Apple
paints a paragraph from its runs' `text` props, which carry the same string
(`NodeRef::shown_text`); an ancestor's change re-sends them. The web leaves
it to CSS: the browser measures and paints its own text, so it agrees with
itself, and with the kernel as far as Unicode's full case mappings without
language tailoring go — Rust's `to_uppercase` (`ß` → `SS`, a final `Σ` → `ς`)
is ICU's root mapping, which Chromium uses for a page with no `lang`.
`capitalize` is titlecase (`ǆ` → `ǅ`, `ß` → `Ss`, Georgian unchanged) of
each word's first letter; a word joins across an apostrophe, `.`, `:` or
`·` between letters, and "3d" stays "3d", as in Chromium. A word split
across two runs is one word. The agent's tree reports the authored text.

*Rejected:* a `TextStyle` field each host applies — three implementations
to keep equal, and one that forgets measures one string and paints another.
Swift's `capitalized` — it lowercases the rest of the word, which CSS does
not.

### D6 — Editable text is not transformed

A native field shows and edits its value; showing one string while editing
another needs a custom field on every platform. So the kernel never
transforms a field's run, the web restores the reset its UA sheet gives
`input` and `textarea` (which `all: unset` undid), and the compiler refuses
`text-transform` on either tag, directly or through a class.

## Not done

- Spread, `inset` and shadow lists (D1).
- Markdown source (`markup="markdown"`) is not transformed natively: hosts
  expand the source themselves, and transforming it would rewrite URLs. The
  web's CSS does transform the rendered Markdown.
- iOS's CPU capture (`layer.render(in:)`, where Metal is absent) ignores
  masks, so there a shadow shows under a translucent box.
