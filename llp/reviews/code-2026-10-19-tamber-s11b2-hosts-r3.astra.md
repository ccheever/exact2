# Review round 3, LLP 1121 stage 11 B2 (the hosts' columns), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING** — [host/web-js/grid.js:8](/tmp/rv-s11b2-r3-astra/host/web-js/grid.js:8): A bound `grid-template-columns` or `grid-template-rows` naming `space.md` becomes `var(--exact-space-md)`, but `gridValue` permits only `--x-*` variables and rejects this as `unset`. Unset-base aliases also fail in release output. A runtime probe reproduced the rejection. Permit the nine base-scale properties in the track validator; test direct names and aliases in release and development builds.

All round-1 and round-2 findings are addressed, including Unicode trimming, option/list frame typing, SVG aliases, and scrub compatibility. No further bytecode-walk or column-wiring defect found.

Caps pass (`emit.rs`: 1,499 newlines). Six existing-binary tests and three JS batch tests pass. Saved outputs match for all 14 apps against B1. Fresh Cargo testing was blocked by sandbox denial of the target-directory lock.
