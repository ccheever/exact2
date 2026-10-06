---
name: 20261005-x03-root-font-size
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-interface-font-size, 20261005-interface-font-size-conversion]
upstream_url: null
reproduced_on: null
---

# X3: An app-settable root font size, the base of `rem`

## Summary

In T3 Code the Interface font size setting (12 to 20 px, default 16) writes the root element's `font-size`, so every `rem` size in the app scales at once. In exact2 the
kernel resolves `rem` against a root size that the host sets, and the macOS host always sends 16. The clone stores the setting and shows the slider, but the setting changes
almost nothing on screen. The fix needed is a way for an app to set the root font size, as CSS `:root { font-size }` does.

## Why this issue arose

### The T3 Code behavior
- **Setting.** Settings › Appearance › Interface font size is a slider row under "Interface font" ("Everything outside code blocks and the terminal."), range 12 to 20, step 1,
  default 16 (`apps/web/src/components/settings/SettingsPanels.tsx:1536-1556`, `packages/contracts/src/settings.ts:116-128`). Resetting the font family also resets the size
  (`SettingsPanels.tsx:1543-1548`).
- **Effect.** `applyAppearanceFontVariables` runs on every change and writes `root.style.fontSize = "<n>px"` on `document.documentElement`
  (`apps/web/src/appearanceFonts.ts:100-134`, the write at `:118`; the effect hook is `FontAppearanceSync`, `apps/web/src/routes/__root.tsx:304-338`). The change is live: no reload.
  The value is clamped with `clampInterfaceFontSize` (2 becomes 12, 96 becomes 20, NaN becomes 16, fractions round).
- **What scales and what does not.** Everything sized in `rem` follows the root: text sizes, control heights, paddings, content widths. The comment in the source says the
  prompt and code sizes "stay in absolute pixels so they do not scale twice" (`appearanceFonts.ts:91-99`; they are the CSS variables `--font-size-prompt`, `--font-size-code`, `--diffs-font-size`).
  Items written in pixels do not scale: for example the 52 px top bar and the 12 px glass blur (`apps/web/src/index.css:116-131,178-185`) and the `[Npx]` classes (86 in `.tsx` files
  by my count on 2026-10-05, against 67 `[Nrem]` classes; the full per-class map is the work of `20261005-interface-font-size`).
- **Derived JS layout.** Some layout code reads the scaled size indirectly. The thread sidebar's minimum width is the width of a brand probe measured with a `ResizeObserver`
  (`apps/web/src/components/sidebar/SidebarChrome.tsx:88`, used at `AppSidebarLayout.tsx:239,318`), so it grows with the size. One reference quirk to keep: the thread details card
  reserves 280 px in JS (`threadDetailsCardLayout.ts:30`) while its CSS width is `17.5rem` (`index.css:129`).
- Persistence: the value is a stored setting (`settings.ts:380`); the sync hook applies it at startup.
- Reference tests: `appearanceFonts.test.ts:111-125` (`describe("font size clamping")`, 2 tests).

### What exact2 does today
- `EXACT2-GAPS.md` X3 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): "The kernel resolves `rem` against a root size that the host sets
  (`kernel/src/style/relative.rs`, `Kernel::set_root_font_size`). The macOS host always sends 16 (`host/apple/Sources/ExactKit/PageFacts.swift:75-81`)."
  Support needed there: "A way for the app to set the root font size, as CSS `:root { font-size }` does. Example: a `setRootFontSize(px)` host command on every host."
- Library (`20261005-platforms-v3`): `exactViewport()` supplies `prefersReducedMotion`, `prefersReducedTransparency` and `prefersColorScheme` (accessibility.md:36, design.md:63).
  A root font size fact or setter is not covered: unknown in this library. Whether a Contract property accepts `rem` for each property kind is also unknown there
  (to confirm at `issue-open`).
- Observed in the clone (mc-orch tree, 2026-10-05): the Contract files hold 1,622 `font-size=` attributes (`grep -c 'font-size='` over `*.contract`) and none of them is a `rem` value.

### Where the clone hits it
- The row works and stores the value: `settings-core.ts:52,66` (default 16, range 12 to 20), `settings-appearance.ts:184-193` (the row).
- `settings-appearance-look.ts:55` copies `fontSize: prefs.fontSizeInterface` into the look object; no Contract view reads `look.fontSize`.
- The only reader is the sidebar's minimum width: `presentation.ts:127`, `r4-polish-sidebar-width.ts:68`, `r12-sidebar-width.ts:12-33`, `app.contract:486`. It uses a measured table of the brand
  mark width for each size 12 to 20 (`BRAND_MARK`, `r12-sidebar-width.ts:16`), because the clone cannot let the layout do it.
- Difference a user sees: moving the slider changes the sidebar's minimum width and nothing else. In the reference the whole interface scales.
- Workaround cost if the fix is refused: an app-side scale factor (`size / 16`) multiplied into every size attribute, as the contracts already do arithmetic such as `viewport.width - 640`
  (`app.contract:486`; whether multiplication in an attribute is accepted is to confirm). That means editing about 1,600 attributes plus native module metrics, and it would not follow the reference's `rem`/`px` split.

## Why it must be resolved

The parity goal needs the Interface font size setting to work: it is one of the Appearance settings people change on the first day, and it exists for readability and accessibility.
Without it, a person who needs larger text gets nothing. Waiting tickets: `20261005-interface-font-size` (the foundation: set the root, check `rem` support, class map, shared `style`
definitions) and `20261005-interface-font-size-conversion` (the per-area conversion). Both list this issue as `resolved framework issue`. The attended slider-feel row in
`20261005-interface-font-size` also waits for a live root. The `rem`-based approach is the web-standard one and keeps the clone's sizes in the same units as the reference;
the scale-factor workaround is larger, and it makes every later size edit carry the factor.

## Requested support

The web way: `:root { font-size: <length> }` and `rem` units that resolve against it. macOS host first, then every host (the kernel is shared).

- **A (preferred).** A host command `setRootFontSize(px)` callable from the app (EXACT2-GAPS X3's proposal), or an equivalent Contract/TS call, that updates the kernel's root size and
  re-lays out. It takes effect live without a reload and is observable through the agent (`layout` reports the new value).
- **B.** A root style in the Contract (`root { font-size = … }`) bound to state, so the app declares the size instead of calling a command. This is closer to CSS; the grammar change is
  larger.
- Needed in either case: the value space (a CSS length in px; invalid values refused with a log line, not silently ignored) and the statement of what `em` nesting and `rem` inside
  native components do (to confirm at `issue-open`).

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Minimal app: a box `width="10rem" height="2rem"` with text `font-size="1.5rem"`, and a slider whose handler tries to change the root size (no call exists).
2. Run the web host and the macOS host. Expected: `layout <box>` reports width 160 and font size 24 on both, and no call changes them.
3. Reference check in Chrome: `document.documentElement.style.fontSize = "20px"` makes the box 200 wide and the text 30.
4. Clone scenario: build the lane app (isolated `T3_LOCAL_HOME`, port 16xxx), open Settings › Appearance, move Interface font size from 16 to 20: only the sidebar minimum width changes; the oracle build scales the whole interface.

## Acceptance for the fix

- A conformance case against Chrome: a `rem` box, a `rem` font size and an `em` child under a changed root size give the same resolved sizes as Chrome at 12, 16 and 20 (`layout` on the web host and the macOS host).
- The change is live (no reload), survives a relaunch when the app sets it at startup, and does not change `px` sizes.
- An AppKit test shows the macOS host sends the new root size and native text re-lays out.
- A refused value logs a clear message.

## App adoption after resolution

Pin the example to a `main` that contains the fix. `20261005-interface-font-size` then sets the root from `clampInterfaceFontSize(fontSizeInterface)` at startup and on every change, turns the
unused `look.fontSize` into the source of that call (or removes it), and replaces the sidebar width table (`r12-sidebar-width.ts` `BRAND_MARK`) with the layout's own result if the layout can supply it.
`20261005-interface-font-size-conversion` converts the sizes the map marks as `rem`. Rows that then must pass: the 12/16/20 oracle pairs, the slider-feel attended row, and "at 16 the interface equals today's".
`issue-close` checks that the setting scales the interface as the oracle does at 12 and 20.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).
