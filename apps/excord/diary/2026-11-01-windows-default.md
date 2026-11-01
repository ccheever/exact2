surfaces: windows
agent: Codex / Astra (gpt-6-astra)
task: qualify the default painter

### Rough

Excord compiled, then packaging refused `packages/dock/windows/lib.rs` as
an input with no source identity. Rust had recorded its Windows namespace
prefix; Cargo's source roots had none. The receipt now normalizes that
spelling before assigning the source its identity.

### Lean in

The manifest needs no painter key to use ink on Windows.

### Needed

- nothing new.

### Checkpoints

- build: native compilation passed; qualifying packaging and the window.
