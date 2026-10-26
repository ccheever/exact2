# Code review round 3: per-column shadow values (c82ec4ceeb), Astra

**RIGHT**

No BLOCKING, SHOULD-FIX, or NIT findings.

1. **Round 2 is fixed.** [scripts/tamber-remote.test.mjs:774](/tmp/rv-shcols-astra/scripts/tamber-remote.test.mjs:774) now expects `tamber/1.16`; the stale comment is removed.

2. **The version bump is complete.** The server, protocol header, Versions section, and all real-server hello assertions agree on `1.16`. The [1.15 entry](/tmp/rv-shcols-astra/docs/tamber-protocol.md:118) is unchanged from `origin/main`; the new [1.16 entry](/tmp/rv-shcols-astra/docs/tamber-protocol.md:139) accurately describes this addition. The design tool displays the returned version; its `1.13` fixture matches its stub. Figma’s version check accepts `1.16`. LLP 1122 references introduce no conflicting version claim.

3. **The implementation matches the rulings.** [Shadow text folding](/tmp/rv-shcols-astra/contract/cli/src/tamber_base.rs:73) uses the kernel’s `fold_lengths` and column table. Per-shadow residual numbers use the same substitution and calculation, with matching shortest decimal values and normalized zero. App-defined bases are folded before this path. [Explain](/tmp/rv-shcols-astra/contract/cli/src/tamber_base.rs:560) selects the first tuple choosing each line; resolve handles each tuple independently. Absent keys, terminal exclusion, inset, text shadows, preserved colours, and mixed lists are consistent. D1’s length path remains unchanged, including terminal support.

Read the complete plan and all three commits through `c82ec4ceeb`. Read-only review: no builds, tests, simulators, or file edits. `git diff --check origin/main...HEAD` passed.
