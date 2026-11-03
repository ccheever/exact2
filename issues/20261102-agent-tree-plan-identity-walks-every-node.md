# A plan-tagged agent tree walks the instance tree once per node

**Status:** Closed
**Systems:** Runner agent tree (`runner/src/agent/tree.rs`), instance sites (`runner/src/instance.rs`, `runner/src/runner.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** An agent `tree` read with plan identity does one root-to-node walk of the instance tree per exported view, and clones every row key on the path it then throws away. A few thousand views make this quadratic. The default `tree` read does not ask for plan identity. The driver's source-map join does.

**Cause.** `tree_rows` calls `runner.site_of(node.id)` and writes only `site.0` (`runner/src/agent/tree.rs`). `site_of` is `Tree::site`, which starts at the instance roots and walks mounted nodes, region arms and collection rows until it finds the view (`runner/src/instance.rs`). The plan node id is already in `Ids::sites`, stored when the view is created (`ids.sites.insert`), and `Ids::site` is one map lookup. Event dispatch follows the kernel parent chain and does not do this walk.

**Reproduction.** `tree` with `plan` true on a flat screen of a few thousand views. Each node repeats a walk from the roots. A mounted collection clones each visited row key on every walk that passes it.

**Fix.** Resolve the plan node with `Ids::site` for the tree read. Keep `Tree::site` for the operation that prints the instance path.

**Acceptance.** A plan-tagged tree of N views does O(N) map lookups and the same JSON `site` values as today. A runner test with a few thousand sibling views finishes in time linear in N.

## Resolution

Agent tree plan metadata reads the stored view-to-site mapping directly. Full instance paths remain available to callers that need them.

## Verification

`a_large_plan_tagged_agent_tree_preserves_sites_and_navigation_flags` passed with 50,002 rows, validating every emitted site and activity flag.
