# Workspace media refresh

## Rough

Transcript Markdown images backed by workspace files reused a signed URL after a settled command or file change. The mobile renderer therefore could keep showing the old bytes even though the agent had rewritten the file.

## Lean in

The mobile media path now derives the latest settled workspace mutation from visible work items and checkpoints. That token is tracked beside the stable Markdown media signing key, so unchanged work keeps its URL while a real workspace mutation forces a fresh signature.

## Needed

The focused mobile regression covers assistant text and in-flight work, stable reuse, and re-signing after a completed file change. Device-side image rendering remains covered by the normal mobile build and drive.

## Checkpoints

- `workspace-mutation.test.ts`: focused mutation identity and URL invalidation coverage.
