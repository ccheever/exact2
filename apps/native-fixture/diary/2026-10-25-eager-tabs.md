surfaces: ios
agent: Codex gpt-6.1-sol
task: exercise a custom tab container that loads before adoption

### Rough
The earlier navigation regression depended on a document mount that main removed. The replacement checks the native container's viewport allocation.

### Lean in
The tab-container hook can load its view before returning, so the fixture can exercise adoption without a host change.

### Needed
- native tabs: provided

### Checkpoints
- editing the fixture module: smooth
