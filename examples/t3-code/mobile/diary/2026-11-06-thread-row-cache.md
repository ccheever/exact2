# 2026-11-06 — steady transcript row projection

## Rough

Native scroll events can request a fresh snapshot while the selected transcript
has not changed. The transcript and Markdown caches already reused their inner
work, but the mobile row adapter still rebuilt every row and activity for each
snapshot.

## Lean in

The completed row projection now stays resident for the current client
revision, thread, route, theme, HTML context and elapsed second. A revision,
local presentation change, or elapsed-label boundary invalidates it. This keeps
scroll redraws on the existing cache path without changing row ownership.

## Needed

A device trace with a populated long thread is still needed to publish a frame
rate comparison against the reference app.

## Checkpoints

- Focused thread and scroll-performance tests pass.
- The regression confirms same-second scroll redraws reuse the complete row
  array and that a local action invalidates the underlying transcript cache.
