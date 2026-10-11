---
name: 20261007-x48-runtime-font-family
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261007-installed-font-picker]
upstream_url: https://github.com/ccheever/exact2/issues/318
reproduced_on: b896050d7 (main, agent mode, before filing)
---

# X48: Contract cannot apply a font family selected at runtime

Moved to main `issues/20261009-runtime-installed-font-family.md` (2026-10-09); tracked there.

## Summary

The T3 Code desktop Appearance picker discovers installed font families and applies the
selected name at runtime. Exact accepts installed families named as literals, but rejects a
`font-family` read from state or data (`lower-font-family-literal`). A picker cannot apply an
arbitrary family discovered on the user's Mac to ordinary Contract text.

## Why it arose

The 2026-10-07 desktop audit found it. Reference `1e2ecbd9758830669684b494d4398f626b0576e0` lists Arial in the
Code font picker and rejects its proportional metrics on selection. The clone lists only the default SF Mono and
returns `No fonts found.` for Arial. The [installed font picker task](../../tasks/20261007-installed-font-picker.md)
records the discrepancy.

Enumeration is not the gap: the example can enumerate families through its Swift native module (a standalone
CoreText probe on this Mac found 247 families, including Arial, Menlo, Monaco and Georgia, with their faces), and
installed families already render when named as literals. Applying an arbitrary runtime-selected family is the
gap; X10 (text rendering) and X03 (root font size) do not cover it.

## Clone workaround

None. The installed-font picker keeps its hardcoded generic-only catalog, and
[installed-font-picker](../../tasks/20261007-installed-font-picker.md) stays open and blocked. A finite
literal-choice (`?:` or `match`) can add a few named faces but cannot cover families absent at build time. Once main
applies a runtime family, the task removes the catalog and verifies the actual faces in the app.

## Evidence and history

- Reproduced locally on `fbce02624d2e33449ee2cde34497083d6fd47457` during the 2026-10-07 desktop audit:
  `contract build` of a state-bound `font-family=family` exits `1` with `lower-font-family-literal`; a literal
  `"Arial, sans-serif"` and a literal choice compile. Inputs, outputs and the CoreText probe were kept locally under
  `target/desktop-audit/font-probe/` (`results.json`), not committed.
- Filed as [#318](https://github.com/ccheever/exact2/issues/318) on 2026-10-08, reproduced on main `b896050d7` in
  agent mode (evidence under `file-x48-x68/` on `t3-code-evidence`).
