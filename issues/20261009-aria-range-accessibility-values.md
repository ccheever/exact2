# A progress value for assistive technology: determinate `progress` (`value`, `max`) or `aria-valuenow`

**Status:** Open
**Systems:** kernel schema, Contract, GUI hosts, accessibility
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/279

## Current scope

Add approved aria-valuenow/min/max/text first, then determinate progress. Check bound values/percentages on Chrome/AppKit/UIKit and retain indeterminate state. Coordinate #278; values stay separate from descriptions.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

A download, an install or an upload shows how far it has got. HTML's determinate `<progress value max>`, or ARIA's `progressbar` with `aria-valuenow`/`aria-valuemin`/`aria-valuemax`/`aria-valuetext`, hands that value to assistive technology, so a screen reader announces "30 percent".

Contract admits only the indeterminate `progress` (LLP 1069.001, amended 2026-10-07: "Not built: a determinate bar (`value`, `max`)"), and carries none of the ARIA value attributes. An app can draw a bar but cannot expose its value.

### Current and expected behavior

- **Current:**
  - `progress value=30 max=100` is refused (`lower-attr-tag`: "`progress` takes no `value`: a `value` makes HTML's determinate progress bar, which Exact does not draw yet").
  - `aria-valuenow`, `aria-valuemin` and `aria-valuemax` on a `box role="progressbar"` are refused (`lower-unknown-attr`: "ARIA's, and Contract does not carry it yet").
  - A drawn bar with `role="progressbar"` and `aria-label` has a name and no value. On macOS it is not exposed at all; that part is #278.
- **Expected:** at least one of these, with the value exposed as the platform's (NSAccessibility's value on a progress indicator, UIAccessibility's `accessibilityValue`, the DOM's attributes) and printed by the agent's `tree` and `tree --ax`:
  - HTML's determinate `progress` (`value`, `max`), drawn as each platform's bar;
  - ARIA's value attributes on any node with a range role.

### Reproduction and evidence

Two one-file Contract sources, compiled with `bun exact.mjs contract build <file> --json` in an app made by `bun scripts/exact.mjs new <dir>`:

```text
component ProgressValue
  view
    main testId="root"
      progress value=30 max=100 aria-label="Download" testId="bar"
```

```text
component AriaValue
  view
    main testId="root"
      box role="progressbar" aria-valuenow=30 aria-valuemin=0 aria-valuemax=100 aria-label="Download" testId="bar" width=200 height=8
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Determinate `progress` | `contract build progress-value.contract --json` | compiler (any host) | main `0365ad1a4` | `lower-attr-tag` on `value` | accepted; a bar at 30 % with its value exposed | compiler JSON |
| ARIA value attributes | `contract build aria-valuenow.contract --json` | compiler | same | `lower-unknown-attr` on `aria-valuenow`, `aria-valuemin`, `aria-valuemax` | accepted and exposed | compiler JSON |
| Indeterminate `progress`, for comparison | a `progress aria-label="Working"`; `bun exact.mjs agent macos "tree --ax"` | macOS 26.6.2 | same | `progressbar "Working" value="0" [busy]` | (works; it has no value by design) | `tree --ax` |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- `progress value=30 max=100` (or the ARIA attributes) compiles, and updates when the bound value changes.
- macOS: `tree --ax` lists a `progressbar` with value 30 (of 100), and VoiceOver reads it as a percentage.
- iOS: `accessibilityValue` is "30%". Web: the DOM element carries the value. Linux: the agent's `tree` prints it.
- `progress` without `value` stays the indeterminate indicator.

### Constraints and related work

- Workaround: a drawn track and fill with `role="progressbar"`, `aria-label` and the percentage in `aria-description` or visible text. A screen reader reads it as a description, not a value, and on macOS it is not exposed at all.
- LLP 1069.001 §"Amended 2026-10-07" built the indeterminate form and lists the determinate bar as not built. No `rules/DEFERRED.md` entry refuses it: `progress` is HTML's element (§Components, restated 2026-09-27).
- Related: #278 (macOS exposes no `progressbar` role on a drawn box).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:28Z

**Decision: Add ARIA range values first; then determinate progress.**

Keep open with the bounded scope below.

aria-valuenow/min/max/text lets existing custom progress and range controls expose real values, and complements #278. Determinate HTML progress is a useful subsequent control slice.

Verify bound updates in Chrome/AppKit/UIKit, the displayed/accessibility percentage and the indeterminate form. Keep numeric values separate from labels/descriptions.
