# Composer upload queue

## Rough

The source composer starts attachment uploads after selection and keeps the
worker alive while the user changes routes. The clone had no app-owned upload
state, so files and images waited until Send and the strip could not explain a
failed upload.

## Lean in

The mobile client already exposed the upload URL and native byte-transfer
operations. A session queue now uses at most three workers, adopts each server
ID into the draft before Send, persists that reference, and feeds the existing
Send admission's upload-state and provenance fields. Failed rows expose a real
Retry action that starts the same draft-owned operation again. A remote ID is
not treated as owned unless its environment provenance was saved by this queue.

## Needed

The native upload operation currently returns only when its request completes;
it does not publish byte-level progress. The strip therefore says
“Uploading…” instead of displaying a fabricated percentage. Native progress
events remain a separate parity slice.

## Checkpoints

- Contract build: `bun scripts/exact.mjs contract build examples/t3-code/mobile/app.contract --json --errors-only`
- Strict app target TypeScript: Bun 1.4.4, ES2024 Promise library
- Focused mobile tests: composer upload queue, attachment strip, Send admission/controller/root and New Task
