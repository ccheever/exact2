# An agent view lookup walks the navigation tree once per row

**Status:** Closed
**Systems:** Runner agent tree (`runner/src/agent/tree.rs`), head (`runner/src/head.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** After a view grows to tens of thousands of nodes, `tree`, and any `state`, `tap` or `type` that first looks a name up as a view, takes minutes. The app still answers and draws. The driver's operations stall.

**Cause.** `tree_rows` asks `runner.inactive` of each row (`runner/src/agent/tree.rs`). `inactive_with` (`runner/src/head.rs`) climbs that row's ancestors. For each ancestor it copies the ancestor's children (`NodeRef::children` allocates) and calls `unselected`. For a navigation root, `unselected` calls `tab_panels`, which walks the root's subtree outside its routes, copying children again. When the row is inactive, `hidden_route` repeats the walk. Nothing is kept between rows, so one tree request costs rows × ancestors × that subtree walk. `head_of` has the same per-node `children()` copy on the document head, once per request.

**Reproduction.** A navigation root whose view holds about 50,000 nodes: `bun scripts/agent.mjs macos tree` takes minutes. A coloured code block of about 1,500 lines (on the order of 50,000 nodes) makes one view lookup take about a minute and a half.

**Fix.** Work out each navigation root's unselected routes once per request, one walk per root, and let each row read that set. Read children through `child_nodes` (or the arena slice) so the walk does not allocate an id vector per node.

**Acceptance.** A runner test with a navigation root over 50,000 rows builds the agent tree in time linear in the rows. `inactive` and `hidden` answer as they do now on the existing navigation fixtures.

## Resolution

Agent tree reads compute navigation activity once, inherit those flags during one traversal, and borrow child slices instead of allocating them per lookup.

## Verification

The runner suite passed, including the 50,000-child agent-tree regression and existing route, tab, sheet and split-pane activity cases.
