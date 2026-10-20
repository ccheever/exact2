# Review round 1, Tamber: no answer to a remote client names this machine's directories, Astra (gpt-6-astra, max)

**WRONG**

1. **BLOCKING — [tamber-server.mjs:551](/tmp/rv-usespath-r1-astra/scripts/tamber-server.mjs:551), [600](/tmp/rv-usespath-r1-astra/scripts/tamber-server.mjs:600): failure replies bypass scrubbing.** Subscribing after failure, or polling after consuming the `down` event, returns raw `down` text from child stderr. Reproduced app/HOME disclosure in these 503 replies. Apply remote-safe error serialization to both exits and test them.

2. **BLOCKING — [tamber-server.mjs:176](/tmp/rv-usespath-r1-astra/scripts/tamber-server.mjs:176): external source paths still leak.** Only app/HOME prefixes are handled. A linked package outside both retains its absolute path in selection SSE events, long polls and `inspect` replies; reproduced all three. Normalize source metadata, nested locations and diagnostic paths through app/package-relative identities, including linked packages.

3. **BLOCKING — [tamber_serve.rs:615](/tmp/rv-usespath-r1-astra/contract/cli/src/tamber_serve.rs:615), [tamber-server.mjs:177](/tmp/rv-usespath-r1-astra/scripts/tamber-server.mjs:177): blanket replacement corrupts authored data.** A value containing `<appDir>/assets/icon.svg` becomes `assets/icon.svg`; returned source text changes too. Rust applies this before distinguishing clients, affecting local proposals and scrub previews. Replace path metadata and machine-generated diagnostics only; preserve authored values and source bytes. Add local and remote payload-integrity tests.

4. **SHOULD-FIX — [tamber_uses.rs:569](/tmp/rv-usespath-r1-astra/contract/cli/src/tamber_uses.rs:569), [tamber-server.mjs:817](/tmp/rv-usespath-r1-astra/scripts/tamber-server.mjs:817): external paths cannot reliably round-trip into outlines.** `shown` reduces external sources to a basename or `node_modules` suffix, while outline only expands against the app roots. The canonical external map entry therefore misses, or collides with another file. Reproduced the miss. Use unique package-relative identities and match against the map’s corresponding identities.

Local `hello.app` remains absolute; no new write traversal route found. Bun probes reproduced the transport defects; six eligible tests passed. Socket tests were sandbox-blocked. No Cargo builds or file edits.
