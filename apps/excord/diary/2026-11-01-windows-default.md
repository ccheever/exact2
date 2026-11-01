surfaces: windows
agent: Codex / Astra (gpt-6-astra)
task: qualify the default painter

### Rough

Excord compiled, then packaging refused `packages/dock/windows/lib.rs` as
an input with no source identity. Rust had recorded its Windows namespace
prefix; Cargo's source roots had none. The receipt now normalizes that
spelling before assigning the source its identity.

With `EXACT_INK_RICHEDIT=1`, the composer did not accept the same click,
selection and typing that worked with ordinary Ink and CPU. The app logged
`RichEdit UIA: device: Catastrophic failure (0x8000FFFF)`. This optional path
remains unqualified; the screenshots and log are in `target/win-default/`.

### Lean in

The manifest needs no painter key to use ink on Windows.

### Needed

- native editing: missing in this drive; investigate RichEdit input and UIA
  together, as recorded in `QUEUE.md`.

### Checkpoints

- build: rough until the package-source repair above; Shelf and Excord now
  package on Windows. The temporary CPU manifest builds and its smoke names
  CPU. The original manifest is restored.
- window: smooth with default Ink and the CPU override. Both scroll the long
  channel, accept composer text, open the account menu and resize at 100%
  and 150%.
  Ink uses the composition window; CPU uses the softbuffer window.
- cleanup: restored the prior unsent draft and 100% scale; sent no message.
