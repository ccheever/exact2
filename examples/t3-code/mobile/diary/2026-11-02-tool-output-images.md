# Tool output image previews

## Rough

The desktop timeline projects image blocks from dynamic tool output into signed media resources. The mobile feed previously flattened those blocks into text, so an expanded tool result could not show or open its images.

## Lean in

The mobile projection now preserves the first eight supported image blocks, signs resources only for open tool rows, and renders the resulting previews in the existing thread media route. The route validates the selected tool image against the current thread before minting its asset URL.

## Needed

Native visual acceptance still needs a populated thread with a dynamic tool image and an iOS run. The focused tests cover source ordering, resource signing, ownership, and route preparation.

## Checkpoints

- Focused mobile tests: 100 passed, 0 failed.
- Contract build: `examples/t3-code/mobile/app.contract` succeeded.
