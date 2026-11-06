# Transcript Markdown prose wrapping

task: keep oversized prose tokens inside the mobile transcript width

### Rough

The mobile transcript used the Markdown renderer for prose but did not carry
the reference's anywhere wrapping rule. A long URL or unbroken token could
expand a row instead of wrapping inside the available chat width.

### Lean in

Transcript prose now has a 100% maximum width and breaks anywhere. Code blocks
and tables keep their existing horizontal scrolling policies; only ordinary
Markdown prose changes.

### Needed

A native screenshot pair with a long URL remains useful for pixel acceptance.
This source change makes no native frame-rate claim.

### Checkpoints

- Thread source regression asserts the width and wrap declarations.
- Focused Markdown, thread, and scroll tests pass after the change.
