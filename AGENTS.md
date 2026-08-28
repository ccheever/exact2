# Agent instructions

Read `rules/RULES.md` and `rules/NOT-DOING.md` first; they bind and this file does not.
`llp/1000-exact2-root.explainer.md` is the map. Design documents under `llp/research/`
are the predecessor's — research, never authority.

## The web is the standard

Where a default, a property name, a value vocabulary, or a behavior could follow CSS or
something else (React Native/Yoga, UIKit, AppKit), it follows CSS — even where a CSS
reset would usually override it. A bare node is `display: block`, `box-sizing:
content-box`, `flex-direction: row`, `flex-shrink: 1`. Rows are named `object-fit`,
`text-overflow`, `line-clamp`, not `resizeMode`, `ellipsizeMode`, `numberOfLines`.
The web is the dev loop and the parity oracle for every other surface; a kernel that
disagrees with a bare `<div>` reintroduces the four-disagreeing-default-layers bug
class the predecessor had. An unavoidable deviation (Taffy has no `position: static`)
is declared in `llp/1001-kernel-v1.spec.md` with the reason.

## Working here

- `kernel/tables/schema.json` is the one declaration authority; `kernel/build.rs`
  generates from it. Edit the table, never generated code.
- Every source file ≤ 1,500 lines. Stage (`git add -A`) and run `node scripts/caps.mjs`.
- The five checks: `cargo build --workspace` · `cargo test --workspace` ·
  `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check` ·
  `node scripts/caps.mjs` · `boot` (nothing to count until a host exists).
- Verify by running, never by grepping. Fix loops get three rounds, then stop and say so.
- Never `git stash`. Kill only PIDs you recorded. Agents remove apparatus freely and add
  none without a human saying so.
