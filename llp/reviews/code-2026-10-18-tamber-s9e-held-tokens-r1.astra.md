# Review round 1, LLP 1121 stage 9 E (held rows name their tokens), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING — [runner/src/agent/looks.rs:30](/tmp/rv-s9e-r1/runner/src/agent/looks.rs:30): Explicit `unset` rows disappear.**  
   These rows have `written` set but no `mask` bit. Consequently, `:hover { opacity = "unset" }` reports an empty look, although holding it changes the presentation. Linux previously included `mask.union(written)`. Serialize both sets, represent explicit clears, and test the resulting node and held answers.

2. **BLOCKING — [host/web/input-glue.js:378](/tmp/rv-s9e-r1/host/web/input-glue.js:378): JS links report owner node `0`.**  
   The JS emitter gives `<a>` elements `data-view=""`. This expression converts that marker to `0`, bypassing `$exactViewId`; the agent’s null-only fallback cannot repair it. Prefer the JS runtime’s ID, or accept only nonempty numeric `data-view` values. Add a test using a compiled JS link; the added web assertions inject runner responses.

3. **BLOCKING — [scripts/tamber-inspect.mjs:234](/tmp/rv-s9e-r1/scripts/tamber-inspect.mjs:234): Owner spans bypass source-freshness checks.**  
   The join now reads the owner’s bindings, but `files` and `readOnly` still cover only the selected node. Selecting a label whose press owner uses an imported `:active` style can therefore publish that style’s stale span with `readOnly: null`. An in-memory reproduction confirms this. Resolve the owner before checking freshness and include its file, chain, and binding-span files.

4. **BLOCKING — [host/web/input-glue.js:409](/tmp/rv-s9e-r1/host/web/input-glue.js:409): JS held answers omit the owner’s instance path.**  
   Neither `inspect-dev.js` nor `agent.js` fills it in. Owners inside repeated or conditional regions therefore lack the promised `{site, instance}` address. Compute the path for `held.el` using the inspect table’s existing instance logic and include it in both reports. Test a label whose path differs from its press owner’s.

Caps and two focused Bun tests passed. In-memory checks reproduced the JS identity and join defects. Fresh Rust tests were blocked by the read-only sandbox.
