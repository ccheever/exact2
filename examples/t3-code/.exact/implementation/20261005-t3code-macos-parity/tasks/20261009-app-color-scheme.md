---
name: 20261009-app-color-scheme
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

# Every surface draws in the app's appearance, not the macOS appearance

## Outcome

The app's Settings › Appearance mode decides the palette of every surface. System follows macOS; Light and Dark override
it. Today many surfaces pick their palette from `viewport.prefersColorScheme`, which on macOS is the system's
appearance, beneath the app's own. When the two differ, those surfaces draw the other palette. The audit found three
symptoms of this one root cause, in three areas.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PG-1 | Usage series colours resolve in the app's theme. Codex is `var(--contrast-foreground)` (near-black on light). Cost by type's Output is black. Cost by speed: Standard light, Fast dark. The Codex chart area and model bar are clearly visible. | `UsagePage` and `UsageModelLayer` take their palette from `viewport.prefersColorScheme`. With the page light and the Mac in Dark, the Codex dot, chart line and area, the gpt-5.5 bar and the Output segment use the dark palette (near-white, almost invisible), and Cost by speed's segments are swapped. After `prefer prefers-color-scheme light` the page is right. | Usage (⌘U) › Cost › 30 days with synthetic Codex and Claude transcripts. Compare the chart, dots, share bars and model bars. Open gpt-5.5 for the dialog. | `target/t3-audit/evidence/pages/PG-1-ref.png`, `PG-1-clone.png`, `PG-1-clone-prefer-light.png`, `PG-1-ref-dialog.png`, `PG-1-clone-dialog.png` |
| PA-11 | With macOS in Dark and the app showing Light, the terminal is light (light background, dark text). | Under the same conditions the terminal is black with light text. It turns light only after the colour-scheme fact changes (`prefer prefers-color-scheme light`). | macOS Dark, app light. On work › Audit work thread, open the terminal drawer. Then run `prefer prefers-color-scheme light`. | `target/t3-audit/evidence/panel/PA-11-ref.png`, `PA-11-clone.png`, `PA-11-clone-default.png`, `PA-11-clone-after-prefer-light.png` |
| TH-7 | "Expand diagram" shows the Mermaid diagram on a white card in light mode, with the caption "Mermaid diagram" and a close button. | The diagram sits on a black rectangle (the arrow is hard to see) in the light window. Same caption and close button. | Seed the extras fixture. Open "Timeline extras". Press "Expand diagram" (`mermaid-expand-12`) in light mode. | `target/t3-audit/evidence/thread/TH-7-ref.png`, `TH-7-clone.png` |

## Cause (read from the source)

- On macOS, `viewport.prefersColorScheme` is the system's appearance, beneath the app's own
  (`host/apple/Sources/ExactKit/DisplayPreferences.swift:88-95`: "the *system's* appearance, beneath any `setScheme` the
  app chose", LLP 1034 D3 as amended by LLP 1069.000 D1). The web's `prefers-color-scheme` means the same.
- The clone sets the window's appearance from the app's mode (`modules/apple/T3WindowChrome.swift:51-60`: `system` →
  nil, else `.aqua`/`.darkAqua`).
- Many views still branch on the system fact. Examples: `app-main.contract:316-319, 340-345` (UsagePage,
  UsagePricesLayer, UsageModelLayer), `app-main.contract:248, 251, 378, 382` (DiffPanel and SurfacePanel `scheme`),
  `app-overlays.contract:174, 194` (CommandPalette, DiagramPreviewDialog). `grep -n prefersColorScheme
  examples/t3-code/*.contract` lists every site.
- The terminal drawer already resolves the mode (`app-window.contract:580`: `data.look.mode == "dark" or (data.look.mode
  == "system" and viewport.prefersColorScheme == "dark")`).

Audit state: the audit ran in mode System with the Mac in Dark, and the agent window drew light while the fact read dark.
Two auditors saw this (pages and panel notes). Under that state even the drawer resolved dark. That agent-mode state is
not explained (see the review's "Observations not filed"). Verify this task under an explicit mode, which is normal use.

## Scope and exclusions

Included: one resolved scheme (`mode == "system" ? viewport.prefersColorScheme : mode`), made once and passed to every
view that now reads `viewport.prefersColorScheme` for its palette; the three symptoms above.

Excluded:
- The Usage page's other rows (PG-2 to PG-5): [usage-and-pr-pages](20261009-usage-and-pr-pages.md).
- The agent-mode mismatch itself (host code). If it still shows after this task, give the coordinator a one-file repro.
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`): `components/usage/usageProviders.ts:17-21` (Codex colour),
`UsageProviderChart.tsx`; `components/ThreadTerminalDrawer.tsx` (terminal theme from the app's resolved theme); the
Mermaid preview dialog in the chat Markdown.

Clone (`examples/t3-code`): `app-main.contract`, `app-overlays.contract`, `app-window.contract:580`, `terminal.contract:146`
(`scheme` and `terminal-theme` on the `t3-terminal` hatch), `pages-usage.contract`, `pages-usage-shapes.contract`,
`settings-appearance.ts` (modes `system`, `light`, `dark`).

Shared files: `pages-usage*.contract` with [usage-and-pr-pages](20261009-usage-and-pr-pages.md), which starts after this
task merges; `app-window.contract` with [settings-escape-and-nav](20261009-settings-escape-and-nav.md) (line 590), which
the plan puts in the wave before this one, so start from a base that has it.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`). Set the Mac's appearance with `prefer prefers-color-scheme <dark|light>` and the app's
with Settings › Appearance.

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PG-1 | macOS Dark, app Light: the Codex series, the gpt-5.5 bar, Output and Cost by speed use the light palette, in the page and the model dialog. Then macOS Light, app Dark: the dark palette. | `pg1-light-on-dark-mac.png`, `pg1-dark-on-light-mac.png` | agent |
| PA-11 | macOS Dark, app Light: the terminal drawer and the terminal panel surface are light. macOS Light, app Dark: both dark. | `pa11-drawer.png`, `pa11-panel.png` | agent |
| TH-7 | macOS Dark, app Light: "Expand diagram" shows a light card. macOS Light, app Dark: a dark card as the reference's dark mode. | `th7-mermaid-preview.png` | agent |
| All | No `viewport.prefersColorScheme` remains as a palette choice outside the one resolved scheme (a Bun test that reads the `.contract` files). Mode System still follows `prefer prefers-color-scheme`. | text: grep before/after | agent |

## Next action

Prepare a branch from `feat(example)/t3-code`. Build and test. Then do one batched live drive at the end for every row's
before/after pair. Close every row in this PR, or record the blocker of a row that cannot pass.
