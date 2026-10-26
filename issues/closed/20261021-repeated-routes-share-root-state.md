# A screen that can be open twice has no state of its own: resources are root-only

**Status:** Closed
**Resolution:** Guidance, not a new lifetime: a screen written as its own component inside `for e in stack(nav) key=e.id` already has `state` per entry, disposed when the entry is popped (LLP 1017.000 P4c); only resources, mutations and tasks are root-only. docs/recipes/navigation-stack.md gains "A screen open twice", and scripts/agent-test.test.mjs proves two entries keep separate state and a popped entry's is gone. LLP 1035.005.000 D5 (route-owned requests) stays unselected until a second consumer.
**Systems:** Contract (`type-child-resource`), runner router, docs
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Brooks four-way port's friction log
**Date:** 2026-10-21

## Symptom

Brooks' shoe Finder is a quiz that can be open in two tabs at once. Both copies shared one phase, one set of picks and one Health result. After a native Back the root still held the Health profile, so the next Finder opened with it. Finding and repairing this cost the lane about 20 minutes, and its single-flow tests did not catch it.

## Cause

Only the root `App` may declare a resource, mutation or task (`type-child-resource`), so a screen's working state lives at the root and outlives the route that uses it. A native pop does not run the app's cleanup for that state. The port keys each temporary record by navigation entry, prunes entries whose routes were removed, and clears with a gated root task.

## Fix

Give a route its own state lifetime: component-scoped shaped reads or per-entry resources, released when the route leaves the stack. If that is not wanted, document the per-entry pattern (state keyed by navigation entry, pruned on removal) in a recipe with a repeated-screen example.
