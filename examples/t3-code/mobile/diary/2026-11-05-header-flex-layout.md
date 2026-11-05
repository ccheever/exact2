# Mobile headers honor their authored flex layout

## Rough

The New Task and shared mobile root headers supplied `align-items` and
`justify-content` without declaring a flex display. The document subtitle wrapper
also supplied `flex-direction` on a block view, so the authored alignment was
ignored by the layout kernel.

## Lean in

The headers now explicitly use `display="flex"`, and the subtitle wrapper uses a
flex column. Button, title and subtitle ownership is unchanged; the fix only
restores the layout the Contract already described.

## Needed

A populated New Task and subtitle route still need a native screenshot comparison
for exact spacing. The source change removes ignored layout declarations without
changing route behavior.

## Checkpoints

- Added a Contract regression for both headers and the subtitle wrapper.
- Focused and full mobile checks plus the iOS build cover the change.
