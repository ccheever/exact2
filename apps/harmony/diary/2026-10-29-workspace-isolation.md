surfaces: web
agent: T3 Code, GPT-6.1 Sol
task: isolate optional native dependencies from public tool provisioning

## Rough

An anonymous `cargo fetch --locked` in the public root failed through Harmony's
native shells, which pulled the private Snapback source into root resolution.
All three shells now belong to the existing Snapback workspace because the app
resolver chooses one workspace for every platform.

Moving workspace ownership also moves compilation, lint and formatting coverage.
The existing Harmony async stage now checks its shells before the web tests, and
the Linux stage retains the relocated shell's cross-target check.

The first lock refresh selected 11 newer versions among imported dependencies.
Review compared Harmony's former root graph with the nested graph, and precise
updates restored those versions. Harmony now shares Snapback's existing
`tinyvec` 1.12.0 and `unicode-normalization` 0.1.25 pins, rather than its former
1.13.3 and 0.1.26. Every final nested package identity existed in an original lock.

## Lean in

Explicit Cargo workspace ownership already supplies app discovery, lock checks,
target placement and profile injection through the existing Exact commands.

## Needed

- dependency isolation: provided, through Cargo workspaces and the existing resolver

## Checkpoints

- dependency provisioning: smooth after the fix; fresh anonymous fetch exits zero and creates no Git cache
- workspace and async regression tests: smooth; 59 focused tests pass
- framework validation: smooth; all six required check groups pass on macOS
- host-shell compilation, Clippy and formatting: smooth for all three relocated shells
- driving the web app: smooth against the test's own temporary in-memory server
- Linux cross compilation: unavailable locally because Zig and the Rust target are absent; command tests cover routing and failure aggregation
