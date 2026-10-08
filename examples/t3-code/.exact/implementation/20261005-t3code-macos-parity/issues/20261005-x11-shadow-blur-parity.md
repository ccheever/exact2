---
name: 20261005-x11-shadow-blur-parity
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-composer-fidelity, 20261005-managed-codex-chatgpt, 20261005-provider-sign-in-and-install, 20261005-server-update-banner, 20261005-settings-scoped-controls-and-theme-editor, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/129
reproduced_on: null
---

# X11: Negative-spread box shadows draw too faint, and `backdrop-filter` blurs only the parent's paint, not the window below

**Status (reclassified 2026-10-08):** Bucket 1, done on main: #129 (main #221) and #225 (main #232) are closed, both in the branch. `saturate()` is not adopted (user decision); a backdrop beyond the parent's subtree has no upstream issue.

## Summary

T3 Code's menus, popovers, dialogs and composer are glass panels: a soft shadow with a negative spread, and
a blurred, saturated view of whatever scrolls behind them. On exact2's macOS host the negative-spread
shadow draws faint, and `backdrop-filter` samples only the parent's paint and earlier siblings, so the
clone draws an opaque composer and flat popovers. The clone has no workaround for the shadows; the opaque
composer is its stand-in for the glass. Needed: Chrome-matching shadow strength and a backdrop that sees
everything painted behind the element in the window.

## Why this issue arose

### The T3 Code behavior
- Shadows: menus, popovers, selects and comboboxes use `box-shadow: 0 16px 40px -18px rgb(0 0 0/55%)`
  (dark `0 18px 44px -18px rgb(0 0 0/80%)`) (`apps/web/src/components/ui/popover.tsx:93`, `menu.tsx:51`,
  `select.tsx:148`, `combobox.tsx:169`); toasts add `shadow-xl shadow-black/25`
  (`ui/toast.tsx:731`); tooltips `shadow-md/5` (`ui/tooltip.tsx:112`). A negative spread shrinks the
  shadow rectangle, so the shadow shows mostly below the panel, not around it.
- Glass: `backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturation))` with
  `--glass-blur: 12px`, `--glass-opacity: 80%`, `--glass-saturation: 1.14` (overrides of 16 px / 1.08
  further down) (`apps/web/src/index.css:116-118,146-147,333-397`). 39 uses of `backdrop-blur` or
  `backdrop-filter` across `apps/web/src`.
- The composer: a `::before` layer filled with the card colour at 80 % alpha and
  `backdrop-blur-(--glass-blur) backdrop-saturate-(--glass-saturation)`, so the transcript is visible,
  blurred, through the composer as it scrolls (`components/chat/ComposerSurface.tsx:22,69-93`;
  `chat/ComposerBanner.tsx:62,95`). When the browser lacks `backdrop-filter` the reference falls back to an
  opaque background (`index.css:339-341`, `ComposerSurface.tsx:23`).

### What exact2 does today
- GAPS X11 (EXACT2-GAPS.md, earlier sessions, framework source at exact2 `c1522fdac`, checked against
  `main` `d2cb661eb`): "Negative-spread `box-shadow` draws faint; dialog and popover shadows are faint or
  missing. Lists, inset and spread are built (LLP 1077 D4, `ExactKit/BoxShadow.swift`), so the cause is the
  caster's blur/mask/spread, not a missing row (cause unconfirmed). Support needed: measure against Chrome and
  fix the caster." And: "Backdrop blur sees the parent's paint and earlier siblings only, not the whole
  window below the node (declared, LLP 1001:670-672, `ExactKit/Backdrop.swift`). The clone draws an opaque
  composer instead of REF's glass."
- Bundled library: not covered: unknown.
- Observed in the clone (mc-orch, 2026-10-05): README.md:270-274 and AGENT-HANDOFF.md gap #2 ("dialog and
  popover shadows faint or missing", "opaque composer") record both, from pixel pairs.

### Where the clone hits it
- Shadows: `shell-tip.contract:49`, `sidebar-overlays.contract:97,129` and every other `box-shadow=` on a
  dialog or popover in the contract files. Workaround: none. A user sees panels that look flat
  against the page where the reference's lift off the page.
- Glass: the composer stack is absolute over the transcript (`r4-composer-overlay.ts`), and
  `T3Timeline*.swift` draws the transcript natively. Stand-in: an opaque composer. A user sees a solid bar
  where the reference shows blurred messages behind it. Which exact2 limit the composer hits (parent-only
  paint, or the transcript being a native list that is not an earlier sibling) is to confirm at `issue-open`.

## Why it must be resolved

The goal is a complete clone, and a declared difference is not an end state. Both effects are in the first
screen a user sees (composer over a scrolling thread) and on every menu and dialog. `20261005-composer-fidelity`
adds a menu, a tooltip and the ultrathink ring on this surface and records the shadow difference in its
Issue assessment; `20261005-main-fix-adoption` and every pixel-pair gate carry the same difference. The
opaque stand-in also changes readability: text behind the glass never shows. Cost of the workaround: none in
code, but a permanent mismatch in every popover, dialog and composer cell.

## Requested support

Two separate requests.
1. **Shadow strength.** `box-shadow` with blur, offset, spread (positive and negative) and alpha that matches
   Chrome pixel for pixel (CSS Backgrounds 3: the blur radius gives a Gaussian of σ = radius / 2; spread
   grows or shrinks the shadow rectangle first; the shadow is clipped inside the border box). GAPS asks to
   measure against Chrome and fix the caster; the acceptance below gives the measure.
2. **Backdrop that sees the window.** `backdrop-filter: blur() saturate()` samples everything painted behind
   the element up to its backdrop root (CSS Filter Effects Level 2): ancestors' backgrounds, earlier siblings
   and their subtrees, including scrolled and native-drawn lists. Alternative: keep the parent-only rule but
   let a native list (the transcript) take part in the backdrop; this is smaller if the transcript is the only
   case, but it must be confirmed.
macOS host first; web and iOS hosts follow the same cases.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Shadows: a 240×120 box with `border-radius: 8px` and `box-shadow: 0 16px 40px -18px rgb(0 0 0/55%)` on a
   white page, in Chrome and in the macOS host. Expected: a soft shadow below the box, strongest near its
   bottom edge. Actual (per GAPS): much fainter.
2. Glass: a scrolling list of coloured rows, and an absolute panel over its lower part with
   `background: rgb(255 255 255/80%)` and `backdrop-filter: blur(12px) saturate(1.14)`. Expected: rows show
   blurred through the panel while scrolling. Actual: the panel shows a flat colour.
3. Clone: open a long thread and scroll under the composer (`agent` `screenshot`); compare with the reference
   desktop oracle (`target/t3-ui-parity/electron-oracle.mjs` from `20261005-desktop-oracle-and-trace`).

## Acceptance for the fix
- Conformance cases (Chrome vs host) for: positive spread, negative spread, no spread, dark alpha, with
  crops of the shadow band; pixel difference per channel under a threshold fixed at `issue-open`.
- A backdrop case: list under panel, host crop equals Chrome's within the same threshold, before and after a
  scroll step; also a panel over an earlier sibling in another subtree.
- Clone: the composer cell over a scrolling thread, the menu, popover, dialog and toast cells at 1280×840 and
  840×620, light and dark, match the oracle within the matrix tolerance.

## App adoption after resolution
- Replace the opaque composer fill and the faint-shadow values with the reference's colours, alpha and tokens
  (`--glass-blur`, `--glass-opacity`, `--glass-saturation`); keep the opaque fallback when
  prefers-reduced-transparency is on (the library names that preference in `accessibility`).
- Remove README.md:270-274 and AGENT-HANDOFF.md gap #2 lines for shadow and glass; update
  `20261005-composer-fidelity` rows and re-shoot affected cells. `issue-close` verifies the cases and cells.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval;
publication only after approval).

## Merged upstream in part (2026-10-07, adopt-main-fixes-r5)

Filed as [#129](https://github.com/ccheever/exact2/issues/129) (the negative-spread shadow already matched Chrome on
`4c893fef6`, so only the backdrop was filed). Main #221 (`846a844da`) closed it: where macOS blurs a backdrop (a panel
that is a sibling of what it covers), the chain mirrors the box before the blur, as Chrome does, so the edges no
longer read past the box. A backdrop beyond the parent's subtree (case B) and `saturate()` continue in
[#225](https://github.com/ccheever/exact2/issues/225). In the feature branch since main `261dd4e10`
([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)).

Nothing to remove; the clone's real backdrops changed with no clone edit. The composer command drawer
(`ComposerDrawerLayer`, `blur(16px)` over the chat column) had a dark band at its bottom edge before (the blur read the
card's shadow below the box) and a clean edge after (agent drive, before/after image). The dialog backdrops
(`blur(4px)`) and the SnapShot menu take the same mirror. Kept for #225: the flattened glass (composer card, model
picker, toasts, PR tooltips, the confirm dialog, the alert stack), since each is nested below what it would blur and
the reference's `saturate(1.14)` is refused. Main #232 closed #225 on 2026-10-07 with `saturate()` only (in the branch since round 5,
not adopted: user decision); a backdrop beyond the parent's subtree has no open upstream issue (reclassified 2026-10-08).
