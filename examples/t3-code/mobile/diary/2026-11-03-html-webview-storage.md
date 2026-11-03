surfaces: ios
agent: Codex gpt-6-astra
task: match attachment HTML WebView storage lifetime

### Rough

The attachment HTML viewers used a nonpersistent WebKit data store. The pinned
mobile AttachmentFileScreen keeps the ordinary WebView store, so a page's
storage disappeared whenever the native view was recreated.

### Lean in

Inline tool renders and full attachment-document HTML now use
`WKWebsiteDataStore.default()`. The legal-document viewer remains isolated and
nonpersistent because it has a separate privacy policy.

### Needed

- storage: provided by the WebKit default data store
- route ownership: unchanged; signed URLs and native callbacks remain guarded

### Checkpoints

- focused mobile TypeScript tests: passed (63 tests, 352 assertions)
- mobile Contract build: passed
- generated source check: passed
- strict app-target TypeScript: passed
- native iOS build: blocked by the offline Cargo cache missing `cipher v0.4.4`
