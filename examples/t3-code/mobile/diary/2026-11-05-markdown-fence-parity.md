# Markdown fence parity

## Rough

Mobile Markdown splitting only admitted a fenced code block after it found a
closing fence. The pinned Remark parser keeps an opening fence as a code block
through the end of the document, including when another marker run appears
inside it.

## Lean in

The mobile fence scan now accepts an opening line at end of input, emits its
remaining bytes as code, and keeps closing-marker matching tied to the opening
marker and width. Nested marker runs therefore remain literal code content.

## Needed

Reference definitions that cross a code-card boundary still need a renderer
context that preserves source presentation while giving each native Markdown
node the document-wide definitions.

## Checkpoints

- Added regressions for unclosed backtick and tilde fences, nested markers, and
  an empty fence at end of input.
- Existing table and Markdown projections now treat an unfinished fence as a
  code partition, matching the pinned parser.
