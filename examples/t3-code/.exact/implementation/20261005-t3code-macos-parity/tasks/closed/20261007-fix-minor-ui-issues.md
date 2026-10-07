---
name: 20261007-fix-minor-ui-issues
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-fix-minor-ui-issues
pr_url: https://github.com/ccheever/exact2/pull/191
verified_commit: f876fb29cf496b55d019c51972bd0a45d343b82a
---

# Small UI and behavior differences from T3 Code (Nightly) are fixed

## Outcome

A side-by-side pass on 2026-10-07 put the installed T3 Code (Nightly) `0.0.46-nightly.20261005.2676`
and the clone on the same local server (the Nightly's own, `127.0.0.1:3773`, through a one-time
pairing credential) and went through every settings route, the composer and workspace-card
menus, the sidebar menus, the command palette and Add project, the Pull Requests and Usage pages,
the right panel, the Diff and a settled thread. The differences below are fixed so the clone
behaves and looks like the reference; features the clone lacks are recorded as tasks.

## Fixed

| Area | Reference | Clone before | Change |
| --- | --- | --- | --- |
| Settings › Model effort (`settings-core.ts`, `settings-rows.contract`) | TraitsPicker: "Reasoning" and "Service Tier" groups, "Default" badges, the checked row filled, the option's description, a ⚡ before the label when the tier is Fast | effort only, check marks, no bolt; choosing an effort wrote `options: [reasoningEffort]` and dropped the saved Fast tier | every select trait in its own group (`TraitsMenu`), the bolt from the tier, and a pick replaces one option and keeps the others |
| Settings › inheritance icon | `LayersIcon size-3`, `text-primary` only when overridden for the project, `text-warning` when mixed | 14pt, accent also for "Set on the environment" | 12pt, accent only when overridden, amber when mixed |
| Settings › Providers › accent "+" | `Button variant="ghost-muted"` | foreground | muted, foreground on hover |
| Composer menus (`composer-controls.contract`, `r5-composer-menus.ts`) | MenuPopup / SelectPopup `side="bottom"`, flipping above only without room | traits, runtime and overflow menus always above the trigger | estimated menu heights; below the trigger when it fits (the draft hero), above otherwise (a docked composer) |
| Sidebar › project filter (`sidebar-overlays.contract`) | popup content-sized by the search input's intrinsic width (209pt) | 145pt, "Search projects..." clipped | the input keeps 185pt |
| Add project palette (`palette-add.ts`) | provider readiness asked again after a failure | one failed `server.discoverSourceControl` (an answer let go) was cached for the session: GitHub read "Setup Required" and sorted last | a failed discovery is not kept; it is asked again once the client's state moves (`client.revision`) |
| Thread › "Context compacted" (`timeline.contract`) | ContextCompactionRow: text-xs, `Minimize2Icon size-3` | the 14pt work row | 12pt label, 12pt Minimize2, 6pt gap (the live row keeps its shimmer) |
| Sidebar › settled rows (`r4-integrate.contract`, `sidebar-row.contract`) | `[&:not(:hover):not(:focus-within)_*]:text-secondary-label/70` recedes the PR badge too | the badge kept its state colour | the badge recedes until the row is hovered |
| Right panel › surface chooser (`shell-panels.contract`) | rows start-aligned | an enabled row's label centred (a button's text-align) | `text-align="left"` |
| Diff (`diff.ts`, `diff.contract`, `r4-surfaces-panel.ts`, `shell.ts`) | DiffPanel `!isGitRepo`: "Turn diffs are unavailable because this project is not a git repository."; the stat only with files, refresh only for a repository, expand-all and the tree only with files; the Diff surface needs a repository | "No net changes in this selection." with +0 −0, refresh, expand-all and tree | the notice from the workspace's streamed status, the reference's header conditions, Diff unavailable outside git |
| Thread header menu (`shell.ts`) | useThreadActionMenu: Un-settle only for `settledOverride === "settled"`, snoozed by `effectiveSnoozed`, Snooze disabled by `!canSnooze` | Un-settle for auto-settled threads, a passed snooze read as snoozed, Snooze disabled only without presets | the reference's three rules |

## Found and not fixed here

| Difference | Where it is tracked |
| --- | --- |
| Red "the answer was let go…" / "fetch() called outside an answer" banners; "Select ref"; the composer's workspace strip missing after a let-go branch read | PR #190 (let-go-banner) |
| Files tree, pull request link and chat file-link context menus | [20261007-context-menu-gaps](20261007-context-menu-gaps.md) |
| Usage › Limits: in-bar labels, hatching, reset chip, trend arrows, "Claude" title | [20261005-usage-pooled-view](../20261005-usage-pooled-view.md) |
| Settings › Connections as the local primary (Local environment, Network access, Tailscale, T3 Connect) | [20261005-local-primary-environment](20261005-local-primary-environment.md), [20261005-this-machine-network-access](../20261005-this-machine-network-access.md) |
| Integrations › Browser, the surface chooser's Browser row | X1 (browser-surface) |
| Provider and GitHub account text blurred until hover | X42 |
| Placeholder colour | X10 |
| Changed-files card header not sticky while scrolling | X32 |
| Composer textarea and palette search focus ring | #179 |
| The traits menu's "2x speed, increased usage" wraps in the composer menu | the measured width (`effortMenuWidth`) against the drawn text: X10 |

Not differences: relative ages read "now" only under the agent's fixed 2026-01-01 epoch; faint
palette key glyphs only when the agent reports `prefers-color-scheme: dark` while the window draws
light; the Pull Requests list content (the reference's list was its cached copy).

## Follow-ups from review

Recorded from the independent review ([review](../../evidence/20261007-fix-minor-ui-issues/review.md)); small enough for a later fix pass:
- Settings traits: boolean traits (Claude's `fastMode`) as an On/Off group, the trigger label joining every select label with " · ".
- Settings traits with a selection mixed across environments: a pick keeps no saved options.
- Composer menus open below from estimated heights (`traitsMenuHeight`); a wrapped note or description can run past the estimate in a short window.
- The live "Compacting context" row keeps the 14pt shimmer row; the reference is text-xs in both states.
- The settled row's recede ignores `:focus-within`.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Settings traits | T3 Code (Nightly) server with Codex `GPT-6-Astra`, tier Fast | open Settings, the Model effort menu | groups, badges, bolt; picking Low keeps `serviceTier` | macOS | `settings-core.test.ts` "traits picker"; before/after shot |
| Composer menus | the draft hero | open the effort and runtime menus | they open under the trigger | macOS | before/after shots |
| Add project | GitHub authenticated on the server | open Add project | "GitHub repository" ready, listed before Setup Required | macOS | `palette.test.ts`; before/after shot |
| Diff outside git | a thread whose workspace is not a repository | open the Diff | the reference's notice and header | macOS | `diff.test.ts`; before/after shot |
| Title menu | threads auto-settled / snoozed in the past | open the header menu | Settle and Snooze | — | `shell.test.ts` |

Checks: the verify runner's recipe ([recipe](../../evidence/20261007-fix-minor-ui-issues/recipe.json),
[report](../../evidence/20261007-fix-minor-ui-issues/attempt3-report.json)): `bun test examples/t3-code`
2195 tests, 0 fail (1 skipped) and the macOS bundle build, passed with the source unchanged.
Strict `tsc` on `app.ts` shows the base's 8 errors only (ES2020 lib in the terminal files);
`bun scripts/caps.mjs` within caps. Runtime proof: one drive per scenario on the base build
(`t3-code-evidence-base`, `0a7ca50ad`) and on this branch, paired with T3 Code (Nightly) on the same
server at 1352×848, light; images on the `t3-code-evidence` branch under `fix-minor-ui-issues/`.
