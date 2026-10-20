# Review round 2, LLP 1121 stage 9 icon-alias fix, Astra (gpt-6-astra, max)

Reviewed: 4ed1b22ed4.

No **BLOCKING** or **SHOULD-FIX** findings.

- **NIT — [tamber/src/icon.rs:233](/tmp/rv-rl-alias2/tamber/src/icon.rs:233):** The successful chain declares targets before aliases, so it resolves in one pass. Reverse the declaration order to exercise propagation across multiple passes.

Round-1 findings check out:

- `sparkle` restores the corpus’s expected `22:15` diagnostic.
- Invalid target literals remain unexpanded: the target reports its specific error, while the alias reports “an icon is…” at its own position.
- `Folding.design` documents resolved icon values.
- The role-axis regression tests both `propose` and `transaction`, with aliases both with and without params.

The fold paths converge on `check_design`; parameter overrides still precede alias resolution. IR/compiler readers consume resolved sources, while explanation and dependency readers retain authored aliases through the original parsed source. Unknown names, theme-token references, and cycles remain refused.

Static, read-only review of `4ed1b22ed4`. No files changed; tests weren’t rerun because this checkout has no built test binaries.

After round 2: the NIT is taken (the unit test's chain is declared alias first, so it resolves over two passes), in abb270eb5e.
