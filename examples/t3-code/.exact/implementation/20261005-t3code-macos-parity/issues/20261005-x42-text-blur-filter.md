---
name: 20261005-x42-text-blur-filter
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: [20261005-provider-sign-in-and-install]
upstream_url: null
reproduced_on: null
---

# X42: `filter: blur()` on text and boxes (blurred redacted account text)

## Summary

T3 Code hides account emails and Source Control accounts behind a fake string of the same shape and blurs it with CSS `filter: blur(4px)` (Tailwind `blur-xs`) until the user clicks to reveal. Exact2 is not known to admit an element-level `filter: blur()` on a text or box node; the clone's only blur precedent is an SVG filter on SVG groups. Without it the hidden text is crisp (still unreadable as the real value). The requested support is the CSS `filter` property with `blur()` on any node, on the macOS host first.

## Why this issue arose

### The T3 Code behavior
- `RedactedSensitiveText` renders a `<button>` with the account value. Hidden: a deterministic fake string of the same length and punctuation (letters from `abcdefghjkmnpqrstuvwxyz23456789`, `@ . - _` kept), monospace 11 px, muted, `select-none` and `blur-xs`; revealed: the real value, muted, no blur. A click toggles; the tooltip reads "Click to reveal email" or "Click to hide email" (`apps/web/src/components/settings/RedactedSensitiveText.tsx:6-61`; the class string is at `:48`).
- It is used for the provider email (editor header status line, Account row, Codex rows), the Source Control account, the composer usage label and the Usage page account popover (`ProviderInstanceCard.tsx:188-200`, `SourceControlSettings.tsx:160-168`, `CodexSetupSection.tsx:74,592,776`, `ComposerUsageLimits.tsx:31`, `UsageLimitsPooled.tsx:173`).
- This is the only element-level blur in the reference: other `blur` classes are `backdrop-blur-xs` on overlays (`components/ui/sheet.tsx:19`), a separate property (see `EXACT2-GAPS.md` X11). Tailwind 4 supplies the radius (`apps/web/package.json:84` `tailwindcss ^4.0.0`; `src/index.css` defines no `--blur-*`), 4 px.
- Reference tests: `ProviderInstanceCard.test.ts:91` checks for `blur-xs` in the markup and that the real email is absent.

### What exact2 does today
- Not in `EXACT2-GAPS.md`. The bundled library (`20261005-platforms-v3`) does not cover `filter`: unknown. It documents `contract vocab --json <word>` for the admitted vocabulary (testing-and-debugging), which has not been run for `filter`.
- `EXACT2-GAPS.md` X11 states the nearby fact: "Backdrop blur sees the parent's paint and earlier siblings only, not the whole window below the node (declared, LLP 1001:670-672, `ExactKit/Backdrop.swift`)". That is `backdrop-filter`, not `filter`.
- Observed in the clone (mc-orch tree, 2026-10-05): SVG filters work on SVG groups: `g filter="url(#send-soft)"` (`composer-controls.contract:166`), `g filter="url(#soft)"` (`nightly.contract:44`), and a per-id blur (`settings-a-collections.contract:110`). Whether `filter` is accepted on a `text` or `box` node: not tested.

### Where the clone hits it
The clone has no `RedactedText` yet; `20261005-provider-sign-in-and-install` adds it (the existing redaction is a bullet string, `settings-source-control.contract:266-269`). Planned workaround: draw the same fake string without blur. A user sees a crisp but unreadable same-shape string where the reference shows a blurred one. A second candidate: draw the string as SVG `text` under an SVG `feGaussianBlur` filter (precedent above); this needs a text-measure check and loses selection semantics (the reference hides selection with `select-none` anyway). Not tried.

## Why it must be resolved

The goal is the reference's look and behavior. The redacted text appears at eight call sites in seven files (Settings › Providers, Source Control, the composer banner and the Usage page), so the difference shows in pixel pairs against the desktop oracle. It is small: the value stays hidden either way, so the issue is nonblocking. Keeping the workaround costs a measured SVG text path (or a declared pixel difference in `EXACT2-GAPS.md`) and a second code path if exact2 adds `filter` later. The issue ends when exact2 admits `filter: blur()` and the app adopts it, or the user closes it by decision.

## Requested support

The CSS `filter` property with `blur(<length>)` (and ideally the other filter functions) on any node, macOS first: web `element.style.filter = "blur(4px)"`. It must apply to a text node's drawn glyphs, respect `overflow` clipping, and follow the node's opacity. Alternative: only `blur()` on `text`. Other hosts: web has it; iOS can follow with Core Image.

## How to reproduce

To confirm on the pinned `main` at `issue-open`: `bun exact.mjs contract vocab --json filter`; then a one-node app `text "someone@example.com" filter="blur(4px)"`. Reference (web, Chrome): blurred glyphs, 4 px radius. Expected on exact2: the property is refused by the compiler (`contract build` diagnostic) or ignored. Clone scenario: after `20261005-provider-sign-in-and-install`, open Settings › Providers with an authenticated fixture provider and compare the hidden email against the oracle.

## Acceptance for the fix

- `contract build --json` returns `[]` for `filter="blur(4px)"` on `text` and on `box`.
- Agent: `layout <testId>` reports the filter row on the node; a screenshot pair shows the text blurred, and the pixel difference against Chrome at the same radius is within the project's tolerance.
- Conformance case against Chrome for `filter: blur()` on a text node inside an `overflow: hidden` box.
- A hit test on the blurred text still reaches the button (the reveal press works).

## App adoption after resolution

Add `filter="blur(4px)"` (hidden state only) to `RedactedText` in `redacted-text.contract`; remove any SVG-text fallback; mark the pixel cells of the redacted rows as matching. `issue-close` verifies: the hidden email in Settings › Providers and Source Control matches the oracle pair; reveal and hide still toggle by press and keyboard.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).
