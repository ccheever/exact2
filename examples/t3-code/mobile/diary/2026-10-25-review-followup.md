surfaces: ios
agent: Codex gpt-6.1-sol
task: repair cancellation tests and attachment scope validation

### Rough

Fresh tests need generated grammar and icon tables. The existing generator also
needs the mobile grammar step after the maintained desktop merge. The merge
preserves that step and the mobile theme pin. The first merged lock retained
orphan package entries and failed frozen installation. Using the maintained
graph with the mobile theme pin corrects the lock. Verification is queued behind
another worker in the shared resource lane; device QA is pending because the UI
lease belongs to another task.

The desktop keyboard fixture bakes the complete app before it runs. Its fresh
plan bake exceeded the 20-minute launcher limit before the fixture ran. A sample
landed in plan validation. The README workspace warning explains the full-app
prerequisite; a separate unit fixture would avoid it.

The first staged caps run reported the inherited 2,994-line desktop Contract
and a wasm private-field error. The coordinator later retired the shared Cargo
target. Fresh isolated validation is recorded in the handoff; the oversized
Contract remains with its desktop owner.

### Lean in

Injected queues let cancellation tests wait for codec and callback completion.
The file-hold worker can expose a completion barrier without granting file authority.

### Needed

- files: by hand, with native claims and durable release ownership.
- sidebar: missing; native split-sidebar presentation remains a framework dependency.
- background uploads: missing from the native question attachment handoff.

### Checkpoints

- editing app.ts: smooth; three scope boundaries now use one decoder.
- native favicon tests: smooth; 65 of 65 checks pass.
- focused data tests: smooth; 89 mobile and 78 integration tests pass, as do
  the 47 Rust app-data tests and focused strict TypeScript checking.
- driving the app: pending shared UI ownership.
- desktop recorder fixture: the US B/KeyB event now expects `mod+shift+b`,
  matching the maintained fixture. Korean layout coverage stays separate.
