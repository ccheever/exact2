---
name: 20261010-x74-macos-heading-inside-button
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X74: On macOS a heading inside a button is not exposed, so Settings › Legacy features' heading sits beside its button

Moved to main `issues/20261010-macos-a-heading-inside-a-button.md` (2026-10-10), where it is tracked. Main PR
[#405](https://github.com/ccheever/exact2/pull/405) filed it (merged `2002d5a31`).

## Summary

The macOS host makes every button an accessibility leaf: `NodeView.accessibilityChildren`
(`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift`) returns nil for a node that `actsAsButton`. A heading the
author puts inside a button is therefore not in the accessibility tree. On the web, Chrome keeps it as button ›
heading [level=2], both for hand-written `<button><h2>` and for the Exact web build. Chrome does not apply ARIA's
"children presentational" to a button's heading, so the web, the parity oracle, exposes it.

## Why it arose

Task `20261010-settings-headings` (T3 PR #404, merged `917ddd341`) made Settings' section and row titles headings
at the reference's levels. General's "Legacy features" is `LegacyFeaturesSection` (`SettingsPanels.tsx`): a Base
UI `CollapsibleTrigger`, which is a button, holds `<h2>Legacy features</h2>` and the chevron. Playwright's ARIA
snapshot of the running reference shows button "Legacy features" › heading "Legacy features" [level=2]. On the
Mac the clone could not put the heading inside the button. The task recorded this as a declared difference in
`EXACT2-GAPS.md` ("Settings headings: a heading beside its button") and left filing to the coordinator.

## Clone workaround (settings-headings)

`CoreSections` (`examples/t3-code/settings-rows.contract`) puts an sr-only level-2 heading right before the Legacy
features button: `SettingsSrHeading(title=section.title, level=2)` (`settings-kit.contract`, a 1-pt text with
`role="heading"`, `opacity=0`, `pointer-events="none"`). The heading list and levels match the reference. The
heading is the button's sibling instead of its child. When main fixes the host, move the heading into the button
and remove `SettingsSrHeading` (`EXACT2-GAPS.md` X74).

## Evidence and history

- 2026-10-10, task settings-headings, review round 2: the reference's tree is
  [ref-legacy-aria.txt](https://raw.githubusercontent.com/ccheever/exact2/98e34bd659cfadd18ece51a01a5f85fc61828137/settings-headings/ref-legacy-aria.txt).
  The clone's tree is
  [evidence-legacy-ax-r2.txt](https://raw.githubusercontent.com/ccheever/exact2/33bba53928f9e4fbd90ca39b2eddaae965bd22f6/settings-headings/evidence-legacy-ax-r2.txt).
- 2026-10-10, main PR #405 reproduced it on main `d413487a8` in a one-file app: a `button` holding
  `text "Legacy features" role="heading" aria-level=2`, beside a plain heading. `tree --ax` on the Exact web in
  Chrome 155 showed button › heading [level=2]. On macOS 26.6.2 the `AXButton` had no children, and the plain
  heading was an `AXHeading`. Hand-written `button > h2`, `button > span[role=heading]` and `div[role=button] > h2`
  in Chrome 155 (CDP `getFullAXTree`) all keep the heading. An AppKit `NSButton` is a leaf `AXButton` with a
  title. A SwiftUI `Button` with a header-trait label was not read; that probe failed. Main PR #327 changes
  neither `accessibilityChildren` nor `updateRoleAccessibility` (read from its diff). Image:
  [x74-heading-in-button-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/5dcf1f60e9b0c24fea18103b894a6ce5f0f5e40d/fw-issues-20261010f/x74-heading-in-button-web-macos.png);
  record: [x74-record.txt](https://raw.githubusercontent.com/ccheever/exact2/5dcf1f60e9b0c24fea18103b894a6ce5f0f5e40d/fw-issues-20261010f/x74-record.txt).
