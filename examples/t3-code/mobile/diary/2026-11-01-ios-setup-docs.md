surfaces: ios
agent: Codex gpt-6-astra
task: make the iOS clone setup reproducible from the README

### Rough

The README had the right build entry point, but its dependency install block left a
reader inside the viewer subdirectory. Later commands were documented as root commands,
so copying the blocks into one shell could point `EXACT_APP_DIR` and the generator at
the wrong path. It also assumed the reader already knew the root toolchain setup and
how to pair the app with a T3 server.

### Lean in

The README now starts from an explicit repository root, runs `exact setup`, installs the
viewer dependency in a subshell, documents simulator selection and connected-device
requirements, and gives the pairing steps needed after launch.

### Needed

The clone still requires a reachable T3 server and remains an incomplete development
implementation; this change documents those prerequisites without changing the app.

### Checkpoints

The Apple build help accepts the documented `--ios t3-code-ios` invocation. The local
setup check reports this machine is using Bun 1.4.4 while the repository pins 1.4.2,
so the README continues to direct users to the pinned version.
