# Per-question attachment shelves

## Rough

The mobile pending-input card could collect text and choices, but it had no
question-owned image shelf. The source keeps each question's images separate,
uploads them before Submit, and allows a custom-answer question to be answered
by images alone.

## Lean in

Question shelves now use an environment and thread scoped draft key. The card
shows each image's upload state, accepts only supported native images, releases
rejected staged bytes, and offers retry for a failed upload. Submission waits
for every shelf to have an upload owned by the current environment and sends
`attachmentsByQuestionId` beside the ordinary answers. Resolved requests prune
their shelves and queue local byte cleanup.

## Needed

Native byte-level upload progress remains separate from the truthful
Uploading… state. Snapshot capture and drag/drop targeting can share these
shelves in a later parity slice.

## Checkpoints

- Focused pending-input tests: 103 pass, 0 fail.
- Full mobile suite: 4,288 pass, 2 skipped, 0 fail.
- Strict mobile TypeScript, Contract build, caps and boot pass.
