# Markdown reference context

## Rough

Each native Markdown text node is parsed independently. A reference definition
in a later preview chunk therefore could not resolve a link in an earlier chunk.

## Lean in

The generated Markdown adapter now finds document-level link and footnote
definitions and exposes their source as render-only context. File and thread
projections keep the original block text and append this context only when the
Contract builds the parser input, so definitions never become duplicate visible
source rows.

## Needed

The context is intentionally limited to definitions that the pinned Remark
parser can locate in the complete source. Native pixel comparison of a rendered
cross-chunk link remains a follow-up acceptance item.

## Checkpoints

- Added file and transcript regressions for a reference used before a fenced
  code chunk and defined afterward.
- Focused Markdown/thread tests pass, the full mobile suite passes, generated
  sources and strict TypeScript pass, and the normal iOS simulator build opens.
- Contract compilation completed with the existing warning set.
