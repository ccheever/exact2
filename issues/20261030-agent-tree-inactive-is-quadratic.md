# An agent op that looks up a view takes minutes on a long view

**Status:** Open
**Systems:** Runner (agent tree, `runner/src/head.rs`), agent driver
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's timeline lane
**Date:** 2026-10-30

**Problem.** In the T3 Code clone, a reply showed a coloured 1,500-line code block, which added 56,996 nodes. After that, each `state data.highlightPending` took about 90 s, because `state` first tries the name as a view. `tap` and `type` look up their targets the same way. A drive that sampled `state` each second timed out after 25 minutes. The app itself answered and drew; only the driver's ops stalled. A sample showed the owner thread in `tree_rows` → `head::inactive_with` → `unselected` → `tab_panels` → `NodeRef::children`.

**Cause.** `tree_rows` asks `runner.inactive` of each row (`runner/src/agent/tree.rs:100`). `inactive_with` (`runner/src/head.rs:98`) climbs the row's ancestors. For each ancestor it copies the ancestor's children, and for a navigation root `unselected` (`head.rs:223`) calls `tab_panels` (`head.rs:327`). `tab_panels` walks the root's subtree outside its routes to find tablists and tabpanels. Nothing is kept between rows, so one tree request costs the number of rows times this per-row work.

**Reproduction.** A navigation root whose view holds about 50,000 nodes: `bun scripts/agent.mjs macos tree` takes minutes. In the clone, at 56,996 added nodes, one view lookup took about 90 s.

**Fix.** Work out each navigation root's unselected routes once per request (one walk per root), and let each row read that set. Or cache `unselected` per root for the duration of one request.

**Acceptance.** A runner test with a navigation root over 50,000 rows builds the agent tree in time linear in the rows. `inactive` gives the same answer as before for each row of the existing navigation fixtures.
