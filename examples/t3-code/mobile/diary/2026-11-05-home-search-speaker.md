# Home search speaker labels

## Rough

The mobile Home projection kept the server's matched message text but dropped
the speaker prefix that the reference shows beside a search excerpt. That made
user and assistant matches look identical and removed the distinction from the
row's accessibility name.

## Lean in

The projection now carries the existing matcher label into the excerpt: “You:”
for a user message and “Agent:” for an assistant message. The Contract already
uses the excerpt for both the visible result and its accessibility label, so the
same string keeps those surfaces aligned.

## Needed

Native filtered-state screenshots and VoiceOver readback remain needed for full
acceptance. This source correction makes no performance claim.

## Checkpoints

- Home, Home-state and incremental-search tests pass.
- The focused regression checks both speaker labels and the existing scoped
  search behavior.
