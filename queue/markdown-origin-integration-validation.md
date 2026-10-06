Markdown origin integration validation (2026-09-18): the third full Rust test
run passed 1,555 tests, with four failures and eight ignored. Height/transform
hand-built plan tests still treat event tag 18 as unknown, but media now uses
it for `loadedmetadata` (three failures); the Apple ownership fixture's Cargo
project inherited the repository workspace because the diagnostic `TMPDIR`
was inside this worktree (one failure). A separate Messages Linux bake lost
its filesystem-helper reply after the preceding workspace build passed.
Full-workspace repair is stopped at the three-round rule; native reader
verification continues separately. Evidence: `target/markdown-comparison/
origin-integration/workspace-round3-*` in the original checkout.
