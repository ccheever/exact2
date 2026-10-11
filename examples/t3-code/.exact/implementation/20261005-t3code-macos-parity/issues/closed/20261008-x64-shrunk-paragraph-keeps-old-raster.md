---
name: 20261008-x64-shrunk-paragraph-keeps-old-raster
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/316
reproduced_on: main 475043d20 (after #305, 4fe878a13) and the feature branch's framework (c0475fbaa), one-file app
---

# X64: a paragraph that shrinks below the text-raster size keeps painting its old pixels (macOS)

Moved to main `issues/20261009-macos-small-text-stale-raster.md` (2026-10-09); tracked there.

## Summary

On the macOS host, a wrapped paragraph that the host draws from a text raster keeps showing that raster after an update
makes the paragraph small enough to be drawn directly (fewer than `TextRasterizer.minPixels`, 16,384 device pixels).
The view tree, `layout` and the agent's default `screenshot` (the capture path) all have the new text; the window's own
pixels show the old lines at their old geometry, clipped by a `line-clamp` box or overflowing an unclamped one.

## Why it arose

`ProviderInstanceCard` (`apps/web/src/components/settings/ProviderInstanceCard.tsx:907`) draws a provider list row's
status as `line-clamp-2` text. After Reconnect the row reads the provider's new status ("Authenticated · ChatGPT") at
once, as the editor does. In the clone:
- `providers.contract` ProvidersPanel: the list row's `line-clamp=2` status went from "Not authenticated · Sign in with
  ChatGPT to use Codex." (two lines) to "Authenticated · ChatGPT" (one line, small) and kept painting "Not
  authenticated · Sign…" until Providers was reopened (#298 bug 19). Switching Codex off reproduces it on any lane ("Not
  auth" under a "Disabled" tree).
- `providers.contract` ProviderEditor: the editor's status detail kept its words but moved up beside a shorter lead after
  Disconnect, and its old two-line raster ran under the Display name field.

## Clone workaround

[fix-provider-auth-state](../../tasks/closed/20261008-fix-provider-auth-state.md) (#312) keys both texts by their status
(`each status in [row.status] key=status`; the editor's line by its parts), so a new status is a new node. Other clone
texts whose strings change in place can still show it; none is known to shrink below the threshold in a reported flow.
Remove the status-keyed redraw once main fixes the raster.

## Evidence and history

- Reproduced in a one-file app (rows clamped to 1 and 2 lines, plain, `nowrap`, keyed and wide, toggled between a long
  and a short status) on the feature branch's framework `c0475fbaa`
  ([06-x64-one-file-app](https://raw.githubusercontent.com/ccheever/exact2/0574dbb319f94727f96392c60d55e220e77a646f/fix-provider-auth-state/06-x64-one-file-app.png))
  and on main `475043d20`, which contains #305's `4fe878a13`
  ([06b-x64-on-main](https://raw.githubusercontent.com/ccheever/exact2/d385ab70b347b1bed6e04a67227b7f2514c2d292/fix-provider-auth-state/06b-x64-on-main.png));
  the app's [view](https://raw.githubusercontent.com/ccheever/exact2/8c7f04f8bb6fac0d4264ceb0e46a3d9d3eac542f/fix-provider-auth-state/x64-one-file-app.contract.txt).
  The rows that shrank below the threshold (`clamped-2`, `plain`) kept their old lines; keyed, `nowrap` and wide rows
  updated.
- Not #300 (fixed by main #305): that one paints the right string without "…". Shares `TextRasterMac.swift` with #291
  and #300/#305.
- Filed as [#316](https://github.com/ccheever/exact2/issues/316) on 2026-10-08, reproduced on main `b896050d7`.
