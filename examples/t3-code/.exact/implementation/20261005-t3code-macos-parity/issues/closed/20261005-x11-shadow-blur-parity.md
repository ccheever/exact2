---
name: 20261005-x11-shadow-blur-parity
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-composer-fidelity, 20261005-managed-codex-chatgpt, 20261005-provider-sign-in-and-install, 20261005-server-update-banner, 20261005-settings-scoped-controls-and-theme-editor, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/129
reproduced_on: null
---

# X11: Negative-spread box shadows draw too faint, and `backdrop-filter` blurs only the parent's paint, not the window below

Moved to main `issues/20261010-macos-backdrop-beyond-parent.md` (2026-10-10); tracked there.

The main file carries case B only (a backdrop beyond the parent's subtree). The other parts are settled: the negative-spread
shadow already matched Chrome on `4c893fef6` (not filed), the backdrop edges were fixed by main #221 (#129), and `saturate()`
landed with main #232 (#225) and is not adopted (user decision).

## Summary

T3 Code's menus, popovers, dialogs and composer are glass panels: a soft shadow with a negative spread, and
a blurred, saturated view of whatever scrolls behind them. On exact2's macOS host `backdrop-filter` samples only the
parent's paint and earlier siblings, so the clone draws an opaque composer and flat popovers.

## Why it arose

### The T3 Code behavior
- Shadows: menus, popovers, selects and comboboxes use `box-shadow: 0 16px 40px -18px rgb(0 0 0/55%)`
  (dark `0 18px 44px -18px rgb(0 0 0/80%)`) (`apps/web/src/components/ui/popover.tsx:93`, `menu.tsx:51`,
  `select.tsx:148`, `combobox.tsx:169`); toasts add `shadow-xl shadow-black/25`
  (`ui/toast.tsx:731`); tooltips `shadow-md/5` (`ui/tooltip.tsx:112`).
- Glass: `backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-saturation))` with
  `--glass-blur: 12px`, `--glass-opacity: 80%`, `--glass-saturation: 1.14` (overrides of 16 px / 1.08
  further down) (`apps/web/src/index.css:116-118,146-147,333-397`). 39 uses of `backdrop-blur` or
  `backdrop-filter` across `apps/web/src`.
- The composer: a `::before` layer filled with the card colour at 80 % alpha and
  `backdrop-blur-(--glass-blur) backdrop-saturate-(--glass-saturation)`, so the transcript is visible,
  blurred, through the composer as it scrolls (`components/chat/ComposerSurface.tsx:22,69-93`;
  `chat/ComposerBanner.tsx:62,95`). When the browser lacks `backdrop-filter` the reference falls back to an
  opaque background (`index.css:339-341`, `ComposerSurface.tsx:23`).

### Where the clone hit it
- Observed in the clone (mc-orch, 2026-10-05): README.md:270-274 and AGENT-HANDOFF.md gap #2 ("dialog and
  popover shadows faint or missing", "opaque composer") record both, from pixel pairs.
- Shadows: `shell-tip.contract:49`, `sidebar-overlays.contract:97,129` and every other `box-shadow=` on a
  dialog or popover in the contract files.
- Glass: the composer stack is absolute over the transcript (`r4-composer-overlay.ts`), and
  `T3Timeline*.swift` draws the transcript natively, so the composer sits below what it would blur.

## Clone workaround

The flattened glass: the composer card, model picker, toasts, PR tooltips, the confirm dialog and the alert stack draw
an opaque fill instead of the reference's blur, since each is nested below what it would blur (a declared difference).
The reference's `saturate(1.14)` is not adopted (user decision). Once main blurs beyond the parent's subtree, the clone
replaces the opaque fills with the reference's tokens (`--glass-blur`, `--glass-opacity`, `--glass-saturation`) and re-shoots the affected cells.

## Evidence and history

- Filed as [#129](https://github.com/ccheever/exact2/issues/129) (2026-10-06; the negative-spread shadow already matched Chrome on
  `4c893fef6`, so only the backdrop was filed). Main #221 (`846a844da`) closed it: where macOS blurs a backdrop (a panel
  that is a sibling of what it covers), the chain mirrors the box before the blur, as Chrome does. In the feature branch since main `261dd4e10`
  ([adopt-main-fixes-r5](../../tasks/closed/20261007-adopt-main-fixes-r5.md)).
- Adoption (r5): nothing to remove; the clone's real backdrops changed with no clone edit. The composer command drawer
  (`ComposerDrawerLayer`, `blur(16px)` over the chat column) had a dark band at its bottom edge before (the blur read the
  card's shadow below the box) and a clean edge after (agent drive, before/after image). The dialog backdrops
  (`blur(4px)`) and the SnapShot menu take the same mirror.
- Case B and `saturate()` continued in [#225](https://github.com/ccheever/exact2/issues/225); main #232 closed #225 on 2026-10-07 with `saturate()` only
  (in the branch since round 5, not adopted: user decision). Case B had no upstream issue until main #386 filed it on 2026-10-10.
