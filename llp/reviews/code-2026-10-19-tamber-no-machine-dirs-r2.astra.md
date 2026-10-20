# Review round 2, Tamber: no answer to a remote client names this machine's directories, Astra (gpt-6-astra, max)

**WRONG**

1. **BLOCKING — [tamber-paths.mjs:52](/tmp/rv-usespath-r2-astra/scripts/tamber-paths.mjs:52): external paths still leak through errors.** Only app/HOME prefixes are scrubbed. A linked package’s missing named design produces its absolute path at [sources/packages.rs:147](/tmp/rv-usespath-r2-astra/contract/cli/src/sources/packages.rs:147); the remote response preserves it. Confirmed at the JS boundary. Render embedded compiler paths through the path-identity rule and add an external-package failure regression.

2. **SHOULD-FIX — [tamber_uses.rs:486](/tmp/rv-usespath-r2-astra/contract/cli/src/tamber_uses.rs:486): linked-package motion sites stop matching.** `motion_sites` now returns `@package/file`, but `motion_instances` still compares canonical paths against that name or `app/@package/file`. Duration/easing uses consequently remain `site: null`, and outlines miss them. Keep canonical paths through the join; normalize the completed reply.

3. **SHOULD-FIX — [tamber_params.rs:57](/tmp/rv-usespath-r2-astra/contract/cli/src/tamber_params.rs:57): display names break reads of authored defaults.** `explain` still reads `dir.join(app.files[…])` at [tamber_explain.rs:117](/tmp/rv-usespath-r2-astra/contract/cli/src/tamber_explain.rs:117). For external packages that path is wrong, so `"4px"` falls back to parsed `4px`, losing its original spelling. Retain filesystem paths separately and normalize only emitted metadata.

Round 1’s down replies, authored path strings, and ordinary package selection/outline cases passed in-memory probes. Local `hello.app` remains absolute; no new write traversal found.

43 selected Bun tests passed. Three socket tests could not bind port-0 listeners. No files changed or Cargo builds run; Rust integration and full design-tool tests remain unverified.
