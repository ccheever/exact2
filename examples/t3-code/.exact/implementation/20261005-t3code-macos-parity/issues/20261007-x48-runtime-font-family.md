---
name: 20261007-x48-runtime-font-family
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261007-installed-font-picker]
upstream_url: https://github.com/ccheever/exact2/issues/318
reproduced_on: b896050d7 (main, agent mode, before filing)
---

# X48: Contract cannot apply a font family selected at runtime

## Summary

The T3 Code desktop Appearance picker discovers installed font families and applies the
selected name at runtime. Exact accepts installed families named as literals, but rejects a
`font-family` read from state or data. A picker cannot apply an arbitrary family discovered on
the user's Mac to ordinary Contract text.

This gap was reproduced on `fbce02624d2e33449ee2cde34497083d6fd47457` during the 2026-10-07
desktop audit. This local issue records the limitation; no framework fix or external issue
publication is included.

## Consumer and current support

The example's [installed font picker task](../tasks/20261007-installed-font-picker.md) records
the live discrepancy. Reference `1e2ecbd9758830669684b494d4398f626b0576e0` lists Arial in the
Code font picker and rejects its proportional metrics on selection. The clone lists only
the default SF Mono and returns `No fonts found.` for Arial.

The limitation is narrower than an inability to render installed fonts:

- `docs/contract-for-agents.md:1557` documents local installed families in literal fallback lists.
- `contract/lower/src/fonts.rs:197-282` converts literal family names and literal-choice arms
  to plan stack IDs. Other expressions fail with `lower-font-family-literal`.
- `kernel/tables/schema.json` stores `font_family` as a `u16`; the current plan owns the stack table.
- `host/apple/Sources/ExactKit/Text.swift:607-631` resolves source-less local-family rows through
  `TextEngine.installedFaces` in `FontFaces.swift:34`. The Apple host selects the family's real faces.
- `host/apple/tests/ExactKitTests/TextParityMacTests.swift:173` contains installed Georgia
  italic/bold face checks. This audit inspected that test but did not run the XCTest suite.

No installed-font enumeration command was found in the framework API. The example can
enumerate through its existing Swift native module. A standalone CoreText probe ran on this
Mac and found 247 families, including Arial, Menlo, Monaco and Georgia, with their actual faces.
Enumeration does not require the app to replace its Contract text renderer.

## Minimal reproduction

From the checkout root, compile this file with `contract build <file> --json`:

```contract
component App
  state family = "Arial"
  view
    text "Installed selected family" font-family=family
```

Observed exit code: `1`. Diagnostic ID: `lower-font-family-literal`.

> `font-family` takes a family name, or a choice (`?:` or `match`) whose every arm is one; a family computed at runtime cannot be resolved, because fonts are declared and resolved at compile time

Controls compiled with exit code `0` and diagnostics `[]`:

```contract
component App
  view
    text "Installed Arial" font-family="Arial, sans-serif"
```

```contract
component App
  state useArial = true
  view
    text "Installed selected family" font-family=(useArial ? "Arial" : "Menlo")
```

The current CLI was built with `cargo build --profile host-dev -p contract --bin contract`.
The three inputs, compiler outputs, exact exit codes, revision and CoreText probe are under
`target/desktop-audit/font-probe/`, especially `results.json`. They are local evidence, not
committed build artifacts. The snippets above reproduce the compiler result without that directory.

## Requested support

Allow an app's runtime-selected CSS family name or fallback list to reach the same text
measurement and rendering path used by literal families. Runtime selection must preserve
family identity, weight/style matching, fallback and inherited text metrics after a change.
The first consumer is the macOS example's Interface, Prompt and Code text; the web should
retain CSS's family-list semantics.

The existing finite literal-choice workaround can add a few named faces, but it cannot cover
arbitrary user-installed families absent at build time. Rendering the whole interface in
custom native views or Canvas would replace the current Contract text path rather than resolve it.
This issue does not request font downloads or a general font-management UI.

## Acceptance for the fix

- The state-bound reproduction compiles, and changing the state to another installed family
  changes resolved face and layout without rebuilding the app.
- A name supplied by a data/native result also works; the name need not occur as a source literal.
- Missing-family fallback, quoted names, fallback lists, bold/italic matching and inheritance
  retain the existing literal-family behavior.
- On macOS, resolved face names and measured text prove that Arial/Menlo select their own faces.
  Merely storing the chosen label or silently substituting a generic font is not sufficient.
- A runtime change invalidates affected text measurement and paint without leaving stale metrics.

## Deduplication and next action

X10 tracks ellipsis, wrap, balance, placeholder color and smoothing. X03 tracks root font
size and is adopted. Neither covers runtime family names. Keep X48 separate from those issues.

This is a reproduced local draft with no upstream URL. Framework implementation and any
upstream publication require their own authorized work. Once resolved, the installed-font
task removes the hardcoded generic-only catalog and verifies the actual faces in the app.

## Filed upstream (2026-10-08)

Filed as [#318](https://github.com/ccheever/exact2/issues/318) ([Design] `font-family` from a string at run time, so a font picker can apply any installed family), reproduced on main `b896050d7` in agent mode before filing (evidence under
`file-x48-x68/` on `t3-code-evidence`). Under the framework vs T3 split (user, 2026-10-08) the fix is framework work;
the clone's side waits for main fix of #318, then an adoption round.
