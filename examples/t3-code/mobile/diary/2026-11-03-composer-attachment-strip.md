surfaces: ios
agent: Codex gpt-6-astra
task: match the pinned ComposerAttachmentStrip tile surface

### Rough

The strip had a second compact retained-file presentation and generic Open labels. The pinned mobile source uses one 72 point tile treatment for every draft attachment, keeps the native horizontal scroll behavior, and labels image actions as previews.

### Lean in

The Contract now uses one 72 point tile for images, videos, PDFs and documents, with the source gap, scroll persistence, hidden indicator and horizontal touch hint. Image tiles expose an image-button Preview action; other files expose a button Open action. Remove controls retain the existing ownership operations and now have stable test targets.

### Needed

Native presentation-source identifiers, document-specific preview/share routing and upload progress or Retry overlays remain open because the current mobile snapshot does not carry those source states.

### Checkpoints

- Focused attachment Contract assertions pass.
- Full mobile and iOS build checks remain to be run before landing.
