# A package that only data code calls is not in the plan

**Status:** Open
**Systems:** Contract lowering (used modules), JS bake (`usedPackages`), agent permissions
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's adopt-native lane
**Date:** 2026-10-30
**Related:** LLP 1133.004, LLP 1133.011 D12

**Problem.** The T3 Code clone posts notifications from data code (`src/shell/shell-notify.ts`, `src/pull-requests/pages-pr-guide.ts`). No Contract file reads a name of `@exact/notifications`. Without such a read, the package has no rows in the plan. Under the agent, `--permission notifications=granted` is refused with "prefer permission: no module here asks for `notifications`", and data code's calls are refused with `NotSupportedError`, so nothing posts. Outside the agent, the calls are linked and work, so the gap shows only in drives and tests. A `use NotificationStatus from "@exact/notifications"` (a shape) does not help. The clone keeps a `notificationStatus()` read in `app.contract` only to put the package in the plan.

**Cause.** The lowering puts a module in the plan only when a Contract source reads one of its names, the root hears its event, or it has a name that only data code calls (`"contract": false`): `contract/lower/src/lib.rs:588`. Every `@exact/notifications` name is also a Contract name, so a use from data code alone matches none of the three. The bake finds the import (`usedPackages`, `js/bake/src/typescript.mjs:1169`) but uses it only to link the implementation. LLP 1133.004 "As built" records the behaviour (`llp/1133.004-notifications.rfc.md:677`). The refusal is at `runner/src/runner/modules/keyed.rs:325`.

**Reproduction.** An app whose TypeScript data code calls `@exact/notifications` and whose Contract reads none of its names: `bun scripts/agent.mjs macos --permission notifications=granted tree` is refused.

**Fix.** A package that data code imports enters the plan as a used module, as one that Contract reads does. Either the bake's `usedPackages` result, or a `use` of the package in any Contract file, marks the module used in the lowering.

**Acceptance.** The reproduction grants the permission, and the data code's notification posts under the agent. The clone drops its `notificationStatus()` read.
