surfaces: ios
agent: Codex gpt-6-astra
task: preserve attachment route ownership during native sharing

### Rough

The attachment share helper accepted a route key but dropped it when it built the native request. The native media presenter therefore had to fall back to the currently visible controller. The focused bridge tests made the missing field clear; no workaround was needed.

### Lean in

The existing native share presenter already validates a supplied route key, so the parity fix stayed at the mobile bridge seam and did not add a second owner or transport.

### Needed

- share sheet: provided; route ownership now reaches the native presenter
- files: provided

### Checkpoints

- focused mobile tests: passed (52 tests, 302 assertions)
- full mobile suite: passed (4,278 tests, 2 skips, 0 failures)
- TypeScript: passed with the app's ES2024 target; the documented ES2023 command cannot type `Promise.withResolvers` in the existing voice controller
- Contract build: passed
- caps: blocked by the checkout environment because `cargo` is not on `PATH`
- boot: passed
- iOS build: blocked by the checkout environment because the offline Cargo cache is missing `cipher v0.4.4`
