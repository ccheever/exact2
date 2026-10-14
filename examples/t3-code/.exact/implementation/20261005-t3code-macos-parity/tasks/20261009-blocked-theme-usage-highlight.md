---
name: 20261009-blocked-theme-usage-highlight
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-appearance-theme-tools
pr_url: null
verified_commit: 3edaae26884e28b820506e0b54dc5cee030baf7e
---

# Theme editor Inspect, colour usage outlines, and “N uses”

## Outcome

The floating theme editor can select a colour role from a label, swatch, or inspected app
paint. It queries mounted app nodes that declare that role, outlines their frames, and reports
the resulting count. Opening a swatch selects the role; pressing its label again clears the
selection. Counts come from the scene rather than reference screenshot numbers.

The 2026-10-09 discrepancy is retained below as historical evidence. X68 and X69 are now
adopted: `elementsFromPoint` supplies ordered hits, and `querySelectorAll` finds declared
paint-role data words. This implementation changes the example only.

## Original findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| S1-7 | Each row label is a toggle ("Show Accent usage"). Selecting it, or opening a row's colour picker, outlines every element painted with that role, and the header reads "Accent color · 4 uses" / "Background · 27 uses". | Row labels are plain text. Opening a colour picker or pressing a label shows no outline; the header stays "Select a color below". | Settings › Appearance › Create theme › press the "Accent" label (or the Background swatch). | `target/t3-audit/evidence/settings-1/S1-7-ref.png`, `S1-7-clone.png` |

## Implementation — 2026-10-14

Local commits: `6accb80fbc021fad73772a302e2af4dbf46326ed` and
`3edaae26884e28b820506e0b54dc5cee030baf7e`.

- Palette-backed app elements declare `data-paint`, unique IDs, and a primary `data-pick`
  role. Primary role priority is background, then border, then foreground; conditional paint
  follows the active branch. Palette-coloured icons participate through `CorePaintIcon`.
- While the editor is open, the root queries each role and reads the returned nodes’ frames.
  The editor and its own controls are excluded. Labels toggle outlines; swatches select them.
- Inspect reads `elementsFromPoint` and selects the first matching tagged hit, then enables
  Advanced if needed, clears the colour filter, and reveals the selected row.
- Inspection owns Escape through shared root state. Settings Back gives up Escape until
  inspection ends. The editor keeps its normal accessibility exposure. Outside pointer
  events still dismiss other app popovers through the existing window handlers.

The declared tags cover app palette consumers; they are not a framework computed-paint probe
for platform-owned control internals. A future palette consumer must declare its paint role
and a unique ID to participate.

## Verification receipt

| Check | Result | Local evidence |
| --- | --- | --- |
| Full app Bun suite | 4,861 passed, one existing skip, no failures | `target/appearance/full-tests.log` |
| Contract, strict app TypeScript, staged caps | Passed on `3edaae268` | `contract-followup.log`, `typecheck.log`, `caps.log` under `target/appearance/` |
| Mounted roles and IDs | Native scene reports 20 roles; no duplicate IDs within any role | `target/appearance/inspector-drive.json`, `inspector-checks.json` |
| Background toggle and count | Native header and outlines match the 20 queried Background nodes; second label press removes outlines | Same receipt |

The rebuilt native drive on `3edaae268` passed all 33 checks. Escape cancels inspection while
Settings stays open. Picking the Appearance paintbrush selects Text, enables Advanced, and
reveals the Text row. `target/appearance/after-inspected-window.png` was visually reviewed.
The earlier `before-followup-inspector-*` receipt records the Escape failure that motivated
`3edaae268`; it is not the repaired path’s acceptance evidence.

Native build: `target/appearance/followup-build.log` (Cargo 228.2 s; Swift 1.2 s; exit zero).
The checks exercise Background and Text, plus unique IDs across all 20 roles. They do not
claim paired screenshot coverage of every role on every app route.

## Colour control accessibility (G5D3 / PNB1a / G413)

The hue slider now supplies raw `aria-valuetext` (zero or 221, without percent). Saturation
and brightness expose rounded numeric percentages. In the native drive the initial
Background brightness changed from 98.82352941176471 to 99; a saturation arrow reports 2,
and entering `#2563eb` reports hue 221. Evidence: `target/appearance/before-ax.json` and the
AX captures in `target/appearance/inspector-drive.json`.

The existing instruction text remains linked with `aria-describedby`; the macOS host exposes
it as accessibility help. VoiceOver speech and its update timing were not exercised. These
AX value checks do not prove the complete spoken-description parity requested by PNB1a.
