---
name: 20261010-audit-wave-followups
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Differences the audit fix agents found outside their tasks

## Outcome

The first audit fix wave (2026-10-10) reported four differences from the reference that were outside each agent's task.
Each was seen on the base and the branch alike. This task fixes them as the reference does.

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FU-1 | On a server thread with no messages, the composer is docked at the bottom with "Send a message to start the conversation." | The composer sits in the middle of the window, as on a draft. | composer-provider-state-and-details (#356) | Make a thread with `thread.create` only (no turn); open it in both apps. |
| FU-2 | The right panel's open state is per thread: a fresh draft opens with the panel closed. | A fresh draft shows the launcher when the panel was open on the previous thread. | right-panel-launcher-and-files (#355) | Open the right panel on a thread, then ⌘N; compare. |
| FU-3 | ⌘↩ in a Files preview's comment draft saves the comment (as in the Diff). | Same cause as PA-12 before #360: `r4-surfaces-files.contract` passes the draft's focus ops but does not track them, so ⌘↩ reaches the composer's Send instead. | diff-panel-parity (#360) | Files › a source file › comment on a line › type › ⌘↩. Agent mode first; real keys in the batch. |
| FU-4 | Settings' provider page remounts on every route change, so a provider's open "Add custom model" field closes. | Two root-level provider links in a row while Settings stays open (A → B → A) can show A's still-open Add field again. | settings-escape-and-nav (#361) | Open Add on provider A; follow two provider links from inside Settings; compare. |
| FU-5 | Escape in the command palette opened over Settings closes only the palette. | Escape closes the palette and Settings too (agent keys, base and #364's branch alike): #361's `escapeOwned` has no palette term. | settings-appearance-and-skill-chip (#364) | Open Settings, ⌘K, Escape; compare. Real keys in the batch. |

## Scope and exclusions

Included: the five rows. Excluded: framework changes; anything already in another open task.

## Context and guidance

- FU-1: reference `ChatView.tsx` (empty-thread composer placement and hint), clone `chat-canvas-layout.ts`,
  `composer-resting-layout.ts`.
- FU-2: reference right-panel state keyed by thread (`rightPanel` store), clone `right-panel-tabs.ts`,
  `r4-surfaces-panel.ts`.
- FU-3: the PA-12 fix in #360 (`diff.ts` `noteDraftFocus`, `draftHoldsCommandEnter`) is the pattern; apply it to the
  Files preview draft (`r4-surfaces-files.contract`, `r4-surfaces-files.ts`).
- FU-5: `app-settings.contract` `escapeOwned` (#361) and the palette's Escape in `palette.contract`.
- FU-4: #361 keys the field by the Models block key (`modelAdding == block.key`); clear it on a provider route change.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FU-1 | agent drive on a no-turn server thread | before / after / reference image |
| FU-2 | agent drive: panel open, ⌘N | before / after / reference image |
| FU-3 | Bun test as #360's PA-12 test; agent ⌘↩; real ⌘↩ in the batch | before / after image |
| FU-4 | Bun test of the route change | text |
| FU-5 | agent drive: Settings, ⌘K, Escape (Settings stays); real Escape in the batch | before / after / reference image |

## Next action

Start after diff-panel-parity (#360) merges (FU-3 reuses its focus tracking).
