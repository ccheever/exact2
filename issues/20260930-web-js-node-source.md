# The JS web target cannot name a node's source

**Status:** Open
**Systems:** web JS target, agent API
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1012.001.000 D6, LLP 1035.002 D6, LLP 1071

Since 2026-09-30 a development bake writes its plan's source map (`bake/src/receipt.rs` `source_map`), and `layout <node>` names the Contract line on macOS, iOS and Linux with no `--plan`, for example `@ apps/caltrain/app.contract:223:13 (Content) · compatible source map`. The web page still says `source unavailable`. The JS target's node reply (`host/web-js/agent.js` `nodeDetail`) has no `site` (the plan node index) and no `planDigest`, and its elements carry nothing to recover a site from: view ids are assigned at first ask (`host/web-js/rt.js` `viewId`), and `emit.rs` writes no per-element site.

Two ways, and the choice is the cost:

- **Emit the site on every element** (a `data-s` attribute or a property set at creation). This works on every page, and costs bytes in every app's `app.js`, which LLP 1071 budgets. Measure it on Caltrain and RealWorld before choosing.
- **Emit a side table only in the agent's chunk.** No boot bytes, but it needs a stable way to map a live element to its template position, which the JS target does not keep today.

The plan digest is the easy half: the driver can hash the `dist/app.plan` it serves and stamp it on the node. The map is not made yet either: after `bun host/web/build.mjs caltrain-web` the bake output holds only the macOS map (checked 2026-09-30), so the JS build's plan needs its own development map, written outside `dist/` so it is never published.
