---
name: 20261005-x09-root-component-across-files
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-client-activity-reporting, 20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-hot-file-split, 20261005-interface-font-size, 20261005-legacy-sidebar, 20261005-live-automations-and-clones, 20261005-local-primary-environment, 20261005-managed-codex-chatgpt, 20261005-media-actions, 20261005-pr-code-tab, 20261005-pr-conversation-and-refresh, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-links-previews-and-routing, 20261005-pr-writing-and-metadata, 20261005-provider-settings-upkeep, 20261005-provider-sign-in-and-install, 20261005-right-panel-tab-menu, 20261005-server-update-banner, 20261005-settings-scoped-controls-and-theme-editor, 20261005-terminal-drawer, 20261005-terminal-integrations, 20261005-terminal-layout, 20261005-thread-commands-and-keys, 20261005-upstream-timeline-and-markdown, 20261005-upstream-ui-sync, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/108
reproduced_on: 4c893fef6
---

# X9: A child component cannot own a resource, and the root component cannot span files, so the app's data layer is capped at one 1,500-line file

## Summary

In T3 Code any component owns its data, so a feature lives in one folder. Exact2 requires every
`resource` to be declared on the root component, in one file. The clone's root already holds 41
resources, 149 state slots and 154 actions in 1,327 of the 1,500 lines the repository allows, and
each view gets its inputs through long prop lists. The clone splits views and TypeScript to make
room. Needed: resources owned by child components, or one root component declared across several files.

## Why this issue arose

### The T3 Code behavior
- Data is fetched by the component that shows it, and only while it is shown. Examples at
  `1e2ecbd975`: the PR hover card queries only while open
  (`apps/web/src/components/pullRequest/PullRequestLinkPreview.tsx`, `useEnvironmentQuery(open ? … : null)`);
  an expanded tool row fetches its output (`.../chat/V2ItemInspector.tsx:76-140`, `useTurnItemDetail`);
  `.../hooks/useLoadBalancedEnvironment.ts` is "only mounted for unresolved automatic drafts, so idle
  clients do not poll hosts".
- The reference has hundreds of component files (the 2026-10-05 audit counted 413). Adding a feature
  adds files, not lines to a shared one. The repository rule is "every source file ≤ 1,500 lines"
  (`CLAUDE.md`, enforced by `bun scripts/caps.mjs`).

### What exact2 does today
- GAPS X9 (EXACT2-GAPS.md, earlier sessions, framework source at exact2 `c1522fdac`, checked against
  `main` `d2cb661eb`): "A child component cannot own a resource (`type-child-resource`, LLP 1006). Every
  resource and root action lives in `app.contract` (41 resources, 149 root states, 154 actions, 1,327
  lines). The file cap is 1,500 lines." Support needed there: "Resources in child components, or one root
  component split across files (LLP 1091 modules)."
- Bundled library, topic `components` (revision `fd1c879072aa836bcd2002a1e41aa83c7607e836`): "Children
  can own state, derives and actions, but resources, mutations and scheduled tasks belong at the root.
  Pass their values and actions down."
- Clone README.md:292: "a child component may not own resources or assign root state".

### Where the clone hits it
- `app.contract` is 1,327 lines and `client.ts` is 1,455 (both measured in the mc-orch tree, 2026-10-05).
  Its 41 resources sit in `app.contract:61-485,1166-1171`. Many exist only to stand in for "fetch while
  mounted": they take flags such as `settingsOpen and settingsRoute == "providers"`
  (`app.contract:357-387`), which the root must also hold as state.
- Views live in `app-main.contract`, `app-settings.contract` and `app-overlays.contract` and receive root
  names as props of the same names. One call passes about 106 props (`SettingsWindow(...)`,
  `app.contract:1318`). Every new resource adds the gate flag, the prop and its plumbing.
- Workaround, planned in `20261005-hot-file-split`: move view sections to child files, keep state and
  resources in the root, move `client.command()` groups to `client-ops-*.ts`, with targets of
  `app.contract` ≤ 1,150 and `client.ts` ≤ 1,250 lines "so about 20 tickets can each add a few resources".
- What differs for the user: nothing visible. What differs for development: a hard stop at the cap, and
  parallel pull requests that all edit the same few lines.

## Why it must be resolved

The plan adds about 30 features; at the time of writing 30 task files cite X9 in their "Issue
assessment", and many add a resource (for example the turn-item fetch of
`20261005-upstream-timeline-and-markdown`, the PR detail refresh of `20261005-pr-conversation-and-refresh`,
the pooled usage view, the terminal session, the diff engine). `hot-file-split` buys room for roughly 20
small additions; features such as the diff engine or the terminal drawer may need more. The impact is
nonblocking until the cap and blocking after it. The cost of the workaround is permanent: gate flags that
can stick (a resource keeps fetching when its flag is not cleared), prop lists that grow, and merge
conflicts on root lines. The parity goal does not depend on this, but delivery does.

## Requested support

The web way: a component owns its data and the data lives while the component is mounted.
- **A (recommended).** Allow `resource` (and, if simple, `mutation`) declarations inside a child
  `component`. A child resource starts when the instance appears, takes reactive arguments, drops late
  replies after an argument change, and is released when the instance is removed. Open points to confirm
  with the framework: how the bake handles a child resource that is not mounted at bake time, cache
  identity across instances, and how this interacts with reply ownership (X14).
- **B.** Keep resources at the root but allow one `component App` to be declared across several files
  (LLP 1091 modules), so each feature file contributes its own resources, state and actions. This solves
  the line cap and the merge conflicts. It does not remove the gate flags or prop plumbing.
A is the full answer; B is the smaller step and would make `hot-file-split` unnecessary for `app.contract`.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Minimal app: `component App` renders `Child`; `component Child` declares
   `resource r = someSource(1) as shape T`. Run `contract build app.contract --json`. Expected (web analogy):
   accepted, the source is called only while `Child` is shown. Actual (per GAPS): diagnostic
   `type-child-resource`.
2. For B: split `component App` over two files. Expected: accepted. Actual: no such syntax (to confirm).
3. Clone scenario: add one resource to the clone's root after `hot-file-split` and run `bun scripts/caps.mjs`;
   it fails once `app.contract` passes 1,500 lines.

## Acceptance for the fix
- A: the minimal app compiles with `[]`; `agent state` shows the child's resource pending then resolved
  only while the child is shown; hiding the child stops further source calls (`logs`); two instances with
  different arguments hold separate values; a late reply after an argument change is ignored.
- B: a two-file root compiles; `contract build` output equals the single-file build of the same app;
  `bun scripts/caps.mjs` passes for both files.
- Clone: the PR detail (or another feature chosen at `issue-close`) moves its resource into its own
  component with unchanged behavior, tests and pixel pairs.

## App adoption after resolution
- Move resources next to their views feature by feature; delete their gate flags and pass-through props
  from `app.contract`; shrink the 106-prop `SettingsWindow` call.
- Retire the targets of `20261005-hot-file-split` that existed only for this cap; keep its TypeScript split.
- Tickets that cited X9 drop it from their Issue assessment. `issue-close` verifies `caps.mjs` and that the
  moved features pass their original acceptance rows.

## Status and next action
Published 2026-10-06 as [#108](https://github.com/ccheever/exact2/issues/108) (reproduced on exact2 `4c893fef6` before filing). Decided upstream on 2026-10-08: see the last section.

## Fix built (2026-10-06)

**Superseded (2026-10-08):** #108 chose the root rewrite and defers child requests to D5; this branch is not pursued (recorded only; the branch is kept).
Built option A1 on exact2 `origin/main`, branch `daehyeon/fw-x9-child-resources` (worktree `~/orca/workspaces/exact2/t3-fw`), commits `19e731892` and `c4a2318c3` (review fixes). Not pushed.
- A child used outside every `when`, `each` and `match` may declare `resource` and `mutation`; the inliner lifts them into the root (`hits#1`). A use inside a region is still `type-child-resource`; a child `task` is still refused. A child state that a lifted resource reads moves to boot when it can (`type-child-resource-state` otherwise). Apps without child requests keep their plan bytes.
- Not built: a request that lives only while its arm is shown (the "fetch while mounted" half). Gate flags stay, but they can move into the feature's own file.
- Evidence: the five checks; contract tests (`child_resource.rs`, 11); a two-file scratch app (`use Search from "./search.contract"`) passes on the web JS target, the wasm web host and macOS; one independent review, its four findings fixed.
- Before main: LLP 1017.000 "P4c amendment" (proposed) needs Charlie's ruling and a `rules/DEFERRED.md` take or waiver. `Expand.lean` does not lift child requests; `contract-difftest` was not run (no Lean here).
- Clone adoption: needs the clone on exact2 main (`clone-on-exact2-main`) first.

## The cap reached (2026-10-08)

`app.contract` reached 1,499 lines on `e91fcfc65` with no same-file `use` lines left to merge.
[20261008-app-contract-room](../tasks/closed/20261008-app-contract-room.md) moved the root's view into
`T3Window` (`app-window.contract`), the last part the rules let leave the root: 1,459 lines, so
about 40 lines of room. Everything else is root-owned resources, mutations and tasks, or root state
and actions tied to them (that record counts them). Further room needs this fix (A1) or a rewrite
of root actions.

## Decided upstream (2026-10-08): a different design

[Charlie on #108](https://github.com/ccheever/exact2/issues/108#issuecomment-6055587663): "Use child state/actions to reduce the root now; defer request ownership to the existing D5 design.
… Use the T3 Code root rewrite already planned in #303 as the immediate remedy. … do not add partial-root syntax
merely to evade the cap."
- **Different design:** neither option under "Requested support" is chosen (A, child resources; B, a root across
  files). Request ownership waits for LLP 1035.005.000 D5 (navigation-entry ownership), with no date.
- For the clone: [app-contract-root-rewrite](../tasks/20261008-app-contract-root-rewrite.md) is the endorsed
  remedy. Resources, mutations and tasks stay in the root; view-only state and action halves move to area children.
- The local A1 branch ("Fix built" above) is superseded by the decision and not pursued.
- #108 stays open upstream for D5. Nothing to adopt.
