---
name: 20261009-app-color-scheme
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-app-color-scheme
pr_url: https://github.com/ccheever/exact2/pull/369
verified_commit: 8dceddfa4b7e0cce534ad7899f856dd4479498d5
---

# Every surface draws in the app's appearance, not the macOS appearance

## Outcome

The app's Settings › Appearance mode decides the palette of every surface. System follows macOS; Light and Dark override
it. Today many surfaces pick their palette from `viewport.prefersColorScheme`, which on macOS is the system's
appearance, beneath the app's own. When the two differ, those surfaces draw the other palette. The audit found three
symptoms of this one root cause, in three areas.

Found by the 2026-10-09 desktop audit ([review](../../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
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

## Cause and fix

- **One resolved scheme.** `app.contract` derives `scheme` once, next to `viewport`: the device setting's mode when it is
  `light` or `dark`, else the system's `viewport.prefersColorScheme` (mode `system`, or before the setting is read, when
  `T3WindowChrome.setAppearance` also leaves the window on the system's). This is T3 Code's `useTheme` `resolvedTheme`.
  The root passes it to `T3Window`, and `T3Window` passes it to the nine views that drew a palette by the system fact:
  `ChatColumn`, `RightPanels`, `PagesCover`, `RightSheets` (`app-main.contract`), `WelcomeLayer`, `WindowOverlays`
  (`app-overlays.contract`), `SettingsWindow`, `ProjectDialogs`, `SettingsModals` (`app-settings.contract`). The
  palette's theme page (`paletteView`) takes it too.
- **Every site.** The 42 reads of `viewport.prefersColorScheme` in five `.contract` files are now `scheme`: the
  Usage page, its model and prices dialogs (PG-1), the pull request list, panel and dialogs, the diff and surface
  panels (the right panel's terminal, PA-11), the Mermaid preview (TH-7), the timeline rows and request banners, the
  composer dock and drawer, toasts, the details panel, the device setup, the welcome wizard, the project icon dialogs,
  the sidebar and its overlays. The terminal drawer and Settings › Providers already resolved the mode themselves
  (`data.look.mode == "dark" or (… "system" and …)`); they take the one `scheme` now. The scheme-split subtrees (one per
  scheme, so `light-dark()` in SVG glyphs re-resolves) now split on the app's scheme, which is the window's.
- **PA-11, dark palette.** The drawer's top edge and the terminal toolbar's outline were `light-dark(#e4e4e7cc,
  #ffffffcc)`: 80% white in the dark palette, a bright line over the dark drawer and toolbar, before and after the
  scheme fix. ThreadTerminalDrawer draws both with `border-border/80` (white at 6%, at 80%); the dark value is now
  `#ffffff0c` (`terminal.contract`). The drawer's edge now reads (22, 22, 22) over (10, 10, 10), the reference's pixel.
- **The theme editor (review follow-up).** The same root cause in TypeScript: `settingsCore` opened a create, edit or
  duplicate dialog on `device.appearanceMode === 'dark' ? 'dark' : 'light'`, so mode System on a dark Mac opened the
  Light appearance (seeded `#fcfcfc`, and the live preview painted the half the window did not show). The reference
  passes useTheme's `resolvedTheme` (`SettingsPanels.tsx:1189` ThemeLibrary `initialAppearance`, `CommandPalette.tsx:557`
  `themeEditor.toggle`). The `settingsCore` resource now takes the root's `scheme` as its last argument
  (`app.contract`, `app.ts` `args[13]`) and the draft opens on it (`settings-core-view.ts`). The palette's Toggle theme
  editor opens the same dialog (`settingsThemeDialog = "create"`), so it follows too. An edit opened on the edited
  theme's own appearance and a duplicate always on the resolved one; the reference hands an edit the resolved
  appearance too (`ThemeSettings.tsx:863-869`) and the panel keeps it when the source theme has colours for it, else
  takes the theme's own (`ThemeEditorPanel.tsx:379-383`). `sessionInputFor` now hands an edit the resolved appearance
  and the draft applies that rule (`sourceAppearance`, `theme-editor-session.ts`; a built-in has both palettes).

No new declared difference.

## Acceptance results

Agent mode on the branch bundle (lane `app-color-scheme`, base port 16280, 1280x840, `--epoch 2026-10-10T06:00:00Z`),
the same steps as the before build (the feature tip in `t3-code-evidence-base`, lane `app-color-scheme-before`) and
the reference (Electron production build over CDP, the mode set by `t3code:theme-appearance-mode`; the reference
follows the Mac's real appearance, Light). One clone session per build: Settings › Appearance mode, then `prefer
prefers-color-scheme`, then Usage › Cost › 30 days, the gpt-5.5 dialog, Timeline extras › Expand diagram, Audit work
thread with the terminal drawer and the right panel's Terminal surface. Each image is before | after | reference.

| Id | Result | Evidence |
| --- | --- | --- |
| PG-1 | pass (agent mode): macOS Dark, app Light: the Codex dot, line and area, the gpt-5.5 and gpt-5.4-mini bars, Output's black segment and Cost by speed (Standard light, Fast dark) are the light palette, on the page and in the model dialog, as the reference. Before: near-white (the Codex series almost invisible) and the speed segments swapped. macOS Light, app Dark: the dark palette (white Codex series, white Output); before: near-black on black, with bright grid lines. | [pg1-light-on-dark-mac.png](https://raw.githubusercontent.com/ccheever/exact2/10346dc56eab5e3999cd3628c9f788164692890d/app-color-scheme/pg1-light-on-dark-mac.png), [pg1-dialog-light-on-dark-mac.png](https://raw.githubusercontent.com/ccheever/exact2/87164ec6601750eb008cc6c2e602ff9ec563c2f5/app-color-scheme/pg1-dialog-light-on-dark-mac.png), [pg1-dark-on-light-mac.png](https://raw.githubusercontent.com/ccheever/exact2/2dd9f36e27181caf8c4498b55df9e90451117993/app-color-scheme/pg1-dark-on-light-mac.png), [pg1-dialog-dark-on-light-mac.png](https://raw.githubusercontent.com/ccheever/exact2/fb31011385aff62f5451b02c056f059a8e200f30/app-color-scheme/pg1-dialog-dark-on-light-mac.png) |
| PA-11 | pass (agent mode): macOS Dark, app Light: the drawer and the right panel's terminal are light (before: the panel's terminal black). macOS Light, app Dark: both dark (before: the panel's terminal white), and the drawer's edge and toolbar outline are the reference's faint border, not a white line. The drawer itself already resolved the mode under an explicit mode; the audit's dark drawer came from the unexplained agent-mode System state (see the review's "Observations not filed"), which did not reproduce here. | [pa11-light-on-dark-mac.png](https://raw.githubusercontent.com/ccheever/exact2/dd470dcf65a6fffc607810d24883fd07d061daaf/app-color-scheme/pa11-light-on-dark-mac.png), [pa11-dark-on-light-mac.png](https://raw.githubusercontent.com/ccheever/exact2/0a7334e64e519983ddbe664c4d8e8a8e1aa25216/app-color-scheme/pa11-dark-on-light-mac.png) |
| TH-7 | pass (agent mode): macOS Dark, app Light: the expanded diagram sits on a light card (before: black). macOS Light, app Dark: a dark card as the reference's dark mode (before: white). Caption and close button unchanged. | [th7-light-on-dark-mac.png](https://raw.githubusercontent.com/ccheever/exact2/873160acc0a4999f88f855ecb6b13aef5225c4cc/app-color-scheme/th7-light-on-dark-mac.png), [th7-dark-on-light-mac.png](https://raw.githubusercontent.com/ccheever/exact2/b9058b934795ad71dd9bdae922ea3479c7db0757/app-color-scheme/th7-dark-on-light-mac.png) |
| TE (review) | pass (agent mode): mode System, `prefer prefers-color-scheme dark`, Settings › Appearance › Create theme opens on Dark (Background `#0a0a0a`, Accent `#346bf1`), as the reference with the dark scheme emulated (CDP `emulateMedia`); before: Light (`#fcfcfc`, `#1b4ed8`) in the dark window. With `prefer … light` both builds and the reference open on Light. | [theme-editor-system-scheme.png](https://raw.githubusercontent.com/ccheever/exact2/988b5d69f78a0b6972eb01e12bcb924116c3a7e4/app-color-scheme/theme-editor-system-scheme.png) |
| TE-edit (review) | pass (agent mode): a two-palette custom theme ("Dusk": saved on Dark, then a Light palette added under the same name), mode System, `prefer … light`: Edit Dusk opens on Light, as the reference; before: Dark (the theme's own). A one-palette theme still opens on its own appearance (Bun test). | [theme-editor-edit-two-palette.png](https://raw.githubusercontent.com/ccheever/exact2/0d97a1c542fff69d96de7f1844f66a3d9e59b82e/app-color-scheme/theme-editor-edit-two-palette.png) |
| All | pass: `viewport.prefersColorScheme` is read once, by the derive (before: 42 reads in `app-main` 22, `app-overlays` 11, `app-settings` 4, `app-window` 4, `app` 1; after: 1 in `app.contract`, held by `app-color-scheme.test.ts`). Mode System still follows `prefer prefers-color-scheme`: with System, `prefer … dark` draws the dark Usage page and `prefer … light` the light one, the same as before. | [system-follows-mac.png](https://raw.githubusercontent.com/ccheever/exact2/f731363a15d7f8ab7a3543bae44bfa1bfc8f0666/app-color-scheme/system-follows-mac.png) |

Tests: `app-color-scheme.test.ts` (new): no `.contract` file reads `viewport.prefersColorScheme` but the root's derive,
and none resolves `data.look.mode == "system"` itself; the derive evaluated for every mode and system scheme (System
follows, Light and Dark override, an unread mode follows the system); `T3Window` hands `scheme` to the nine views; the
drawer, the surface panels, the Usage page and dialogs and the Mermaid preview take it; the drawer's dark border; the
`settingsCore` resource takes `scheme` and the theme editor's create and duplicate open on it (System and Dark on a dark
system open Dark, System and Light on a light one open Light; it fails on the old line); an edit opens on the resolved
scheme when the theme has that palette, else on its own, and a duplicate of a one-palette theme on its own (it fails on
the old rule).
`hover-layer.test.ts`: its two expected source lines take `scheme`. Checks: see the PR.

## Progress

2026-10-10: built (two commits; the drawer border after the first drive showed it), merged `feat(example)/t3-code`, the
bundle built, the before build and the reference driven, the branch driven once plus one retry (after the border fix:
every after image is from the retry), images composed and uploaded, the draft PR opened.

2026-10-10, review follow-up: the independent review found the theme editor's first appearance still taken from the mode
alone (`settings-core-view.ts:61`, should-fix) and the base moved to `284254a72` (should-fix). Merged
`feat(example)/t3-code` at `284254a72` (clean; none of its commits read `viewport.prefersColorScheme` or `look.mode`),
passed the resolved scheme into `settingsCore`, and applied the reference's edit/duplicate rule (second commit). The
bundle was built after each commit; the before build, the reference and the branch were driven through Create theme in
mode System and an Edit of a two-palette theme (lane `app-color-scheme`, base port 16280; the branch's first drive was
on the first commit's bundle, the retry on the final one, and every after image is from the retry). Then the base moved
to `ab220bfdb` (#352, the Browser surface part 2) and was merged too (clean; it adds no `viewport.prefersColorScheme` or
`look.mode` read, and the one-read test holds); the final checks ran on that head (see the PR).

## Not verified

- The audit's agent-mode state (mode System, Mac in Dark, the window drawing light while the fact reads dark) did not
  show in this lane: with System, `prefer prefers-color-scheme dark` drew the window dark in both builds. No repro, so
  nothing to give the coordinator.

## Found during the review follow-up (not this task; for the coordinator)

Seen on the evidence path of the edit scenario, against the reference, and not fixed here (theme library and editor
behaviour, outside the colour scheme):

- A one-palette custom theme made active paints the other appearance with its own palette: after "Dusk" was saved on
  Dark only, mode System on a light system drew the clone's window dark (`themeRoles` falls back to
  `own[own.appearance]`, `settings-appearance.ts:32`); the reference kept the light window. Create theme on Light then
  seeded `#0a0a0a`/`#346bf1` (the clone seeds from `prefs.themeLight`), where the reference seeded `#fcfcfc`/`#1b4ed8`
  (`ThemeSettings.tsx:667-670`, the light half's owner).
- The editor's submit label: the clone says "Save theme" for an edit and "Create theme" for a create whose name matches
  an installed theme; the reference says "Save changes", "Merge into “<name>”" and "Add <light|dark> palette"
  (`ThemeEditorPanel.tsx:1252-1266`).

## Real-input batch steps

None: every row is agent mode.

## Next action

None: review and merge.

## Delivery

Merged on 2026-10-10 as `8dceddfa4` (#369, squash). Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
