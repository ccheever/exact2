---
name: 20261010-audit-wave-followups-3
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

# Differences the audit fix agents found outside their tasks (third set)

## Outcome

PRs #371 and #375 reported four more differences from the reference, outside their findings. This task fixes them as
the reference does. ([First set](closed/20261010-audit-wave-followups.md), [second set](20261010-audit-wave-followups-2.md).)

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FW-1 | Expand diagram shows the Mermaid diagram on the app's card colour: white in the app's Light mode, also when macOS is Dark. | On macOS Dark with the app Light, the expanded dialog shows the diagram on a dark card (the shared `DiagramPreviewDialog`, or the offscreen Mermaid render's theme). TH-7 (#369) fixed the dialog's chrome; this is what remains. | markdown-links-and-files-preview (#371) | Agent mode: `prefer` dark, app mode Light; a thread or a Files `.md` with a mermaid fence; Expand diagram. |
| FW-2 | A sent attachment's Markdown preview renders with the chat renderer (`ChatMarkdown`), as Files' rendered Markdown now does (#371). Check the reference first. | It uses the reduced parser (`r4-surfaces-render.ts`). | markdown-links-and-files-preview (#371) | Send (or seed) a `.md` attachment; open its preview in both apps. |
| FW-3 | Escape in a Pull Requests Filters submenu (Author, Labels, …) closes only that submenu; the Filters menu stays. | Escape closes the whole Filters menu (since fix-keyboard-focus). | usage-and-pr-pages (#375) | Pull Requests › Filters › → into Author › Escape. |
| FW-4 | With no author chosen, the Author submenu shows no check on "Anyone". | "Anyone" is ticked. | usage-and-pr-pages (#375) | Pull Requests › Filters › Author. |

## Scope and exclusions

Included: the four rows. Excluded: framework changes. FW-2 changes nothing if the reference's attachment preview also
uses a reduced renderer: then record the reference lines.

## Context and guidance

- FW-1: `timeline-mermaid.contract`, `timeline-mermaid.ts` (the render theme sent to the offscreen renderer),
  `app-overlays.contract` (`DiagramPreviewDialog`), #369's `scheme`.
- FW-2: reference attachment preview component (`apps/web/src/components/media` or the attachment panel); clone
  `r4-surfaces-render.ts`, the chat renderer path #371 uses for Files.
- FW-3, FW-4: reference `PullRequestFilters` (Base UI menu nesting); clone `pages-prs.contract`, `pages-prs.ts`.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FW-1 | agent drive with `prefer` dark and app Light | before / after / reference image |
| FW-2 | agent drive on an attachment preview (or the reference lines if unchanged) | before / after / reference image |
| FW-3 | agent keys; real Escape in the batch | before / after / reference image |
| FW-4 | agent drive | before / after / reference image |

## Next action

Start now (its files do not overlap the in-flight tasks).
