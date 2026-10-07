# Cross-browser conformance has never been calibrated for most apps

**Status:** Open
**Systems:** host/web-js/conform.mjs, host/web-js/conformance/known-firefox.json, host/web-js/conformance/known-webkit.json, scripts/agent-playwright.mjs, scripts/agent-launch.mjs, scripts/async.mjs
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

The async lane's first full run (M5 mini, main at 931f9fb7d) failed the Firefox step 566 times and the WebKit step 156 times. Chrome is the oracle; the browsers are exactly Playwright 1.63.0's (firefox-1543, webkit-2359), so this is not a version mismatch.

**Why so many.** The Firefox and WebKit steps were added on 2026-10-02 (6a0354e41). The lane last ran on 2026-09-24/25, so they have never had a full run. `known-firefox.json` names differences for 5 apps (caltrain, interaction-gallery, fieldnotes, markdown, realworld). The lane compares 24 apps plus the synthetic fixtures.

**What the failures are.**
- **Most: layout drift that `line-height: normal` permits.** Firefox's rows are 24 px where Chrome's are 22, and the difference accumulates down a list (synthetic-router, synthetic-fields, carousel, markdown-stress, duo-lab, native-fixture, …). These belong in the known lists as the existing entries do.
- **`resources.viewport.pointer`: Chrome `fine`, Firefox and WebKit `coarse`** (weatherlight and others). Headless Firefox and WebKit report a coarse pointer under Playwright. Either launch them with a fine pointer or name it as known.
- **Tree differences that may be real**, each worth a look before it is named known:
  - Firefox, carousel `wheel strip 0 700`: rows in a different order after the wheel;
  - WebKit, video-player `clock +60000`: `0:10` / `-0:00` against `0:00` / `-0:10` (playback time);
  - WebKit, synthetic-startend `tap say`: a virtualized window 400 rows apart (`line-389` against `line-789`).

**To do.** Calibrate each app: name the permitted layout differences, fix or name the pointer, and inspect every tree or state difference before naming it known. Logs: `~/exact2-verify/exact2-async/target/async/931f9fb7de91/conform-{firefox,webkit}.log` on the mini.

## Progress, 2026-10-07 (lane/xbrowser)

Measured on the M5 mini, all 24 apps plus the synthetic fixtures, Playwright 1.63.0:

| | Firefox FAIL lines | WebKit FAIL lines |
|---|---|---|
| The async lane at 931f9fb7d | 15,078 | (not counted per line) |
| After this lane | 1,763 | 925 (507 KNOWN) |

Four causes were in the harness, not in exact2, and are fixed there:

- **Pointer.** The Firefox and WebKit contexts used Playwright's `hasTouch: true`, which makes `(pointer: coarse)` and `(hover: none)` true, while Chrome's oracle reports `fine`. Every app that reads `viewport.pointer` failed every step. Their input is the mouse's in any case; the contexts now have no touch. The touch phases were already refused on this carrier.
- **`line-height: normal`.** Each engine resolves it from its own font metrics. In plain HTML, a 16px system-ui block is 18 px tall in Chrome and WebKit and 20 px in Firefox (20px font: 23 / 23 / 25), so rows drifted 2 px each down every list and moved virtualized windows. In `--browser` mode, both engines now take the same body line height (`body { line-height: 1.2 }`, an adopted sheet in place before any page script; authored line heights still apply), as the agent already hides scrollbars on both. The 912 per-step KNOWN entries that named this drift are removed.
- **Media time.** The frozen media clock (8b02d4c0f) now applies to Firefox and WebKit too, not only Chrome.
- **Wheels.** Firefox's default action scrolls one wheel event at most a page: in plain HTML, a 1000 px wheel moves a 300 px port 270 px, and no pref lifts that in headless Firefox. The steps wheel past a port on purpose (lists.steps). In Firefox the trusted wheel still reaches the page's handlers whole. Its default is taken over by a full-delta scroll on the scroller Chrome would chain to, unless a handler cancelled it. An off-viewport wheel point, which reaches no element in either engine, scrolls the page as Chrome's compositor does. synthetic-lists went from 7,334 to 0, carousel from 338 to 0, and realworld's document scroll now matches.

Blind review of the approach (Astra and Grok, both SOUND WITH CHANGES; folded): no `!important`; the sheet installed before boot; the unused touch pref removed; the media freeze's scope documented; obsolete KNOWN entries pruned.

## What remains, by cause

- **Modifier keys: fixed (lane/xbrowser-2, Charlie 2026-10-07).** Every host's driver now presses a chord's modifiers as their own keys; Firefox no longer differs from Chrome on any key log (synthetic-tabindex is clean in Firefox; synthetic-keys' remaining lines are layout). issues/closed/20261007-cross-browser-modifier-keys.md.
- **Native controls' intrinsic sizes (user-agent; candidates for per-node KNOWN, as markdown's file input already is):**
  - text inputs and selects: synthetic-rem `size`, synthetic-budget `in-*`, synthetic-mounted `pick`;
  - textareas: synthetic-keys `area` and `grid`, where Firefox's rows are taller;
  - file inputs: markdown `open-file` on steps other than boot, Firefox 194 against Chrome 164;
  - checkboxes and radios: synthetic-early and synthetic-controls, where Chrome stretches a checkbox's box across a stretching column (365 px) and WebKit keeps 12 px; synthetic-radios, 2–3 px.

  Each such entry should name the node and the followers it moves, with the plain-HTML evidence.
- **Text metrics (glyph advances and rounding):** small drift, at most 1.6–3.3 px, in synthetic-text, synthetic-styles, synthetic-failed, synthetic-blur, native-fixture, typetour and completion-storm. Larger wrap-driven differences in markdown-stress and textflow (the same text breaks lines differently), and realworld's favourite link, which follows the ♥ glyph's fallback width (5.7 px). In plain HTML, the same 16px system-ui string is 67.28 px wide in Firefox and 67 px in Chrome. **Decided (Charlie, 2026-10-07): a 4 px tolerance** for cross-browser box positions and sizes (scroll offsets 1 px; Chrome conformance unchanged), built in lane/xbrowser-2. It absorbs the 1.6–3.3 px class. **It does not cover accumulated drift**, which was measured and is reported here rather than widened: in markdown-stress, blocks down a 4,900 px reading column drift 4.5–5.5 px at boot, and the 12,000 px column after `toggle-single` differs by 66.5 px (text wrapping differently over a long document); synthetic-rem 4.8 px and synthetic-controls 6.8 px at the root from several such rows.
- **Viewport segments:** Firefox and WebKit have no Viewport Segments API (Chromium's), so synthetic-segments' `prefer segments` steps differ in state, tree and layout. This is a candidate KNOWN class for that fixture's segment steps.
- **Unclassified, to look at:**
  - Firefox grants `requestFullscreen` in headless mode (video-player's viewport becomes 1366 wide); Chrome's does not.
  - WebKit marks the root and a disabled view focused after Tab (synthetic-tabindex).
  - WebKit's media session readback is `playing` or `paused` where Chrome's is `none` (video-player, synthetic-media).
  - `mediasession episode seekforward` finds no box for view 2 in both engines.
  - weatherlight's live forecast changed between the Chrome and WebKit pages (05:15 against 05:30), because the fixture reads a live API.

## After the modifier and tolerance changes (2026-10-07, lane/xbrowser-2)

Full pass on the mini, all 24 apps and the synthetic fixtures (FAIL lines, before → after this lane): **Firefox 1,763 → 1,004, WebKit 925 → 666.**

| Firefox layout delta | ≤ 6 px | 6–10 | 10–20 | > 20 | non-layout |
|---|---|---|---|---|---|
| lines | 243 | 78 | 77 | 566 | 38 |

WebKit: 88, 29, 64, 373, and 110 non-layout. What is left is no longer glyph drift:
- **> 20 px: native controls and wrapping.** markdown-stress (274 Firefox lines: wrap over a 12,000 px column), synthetic-keys (130: Firefox's textarea is 52 px tall where Chrome's is 24, and the grid below moves with it), synthetic-budget (36), synthetic-mounted (18: an input 12 px wider), markdown (29), textflow (23), synthetic-segments (26). These are the native-control and wrapping classes above: per-node KNOWN entries with plain-HTML evidence, or a fixture that sizes its controls.
- **Non-layout:**
  - WebKit synthetic-radios (46): `ArrowRight` on the blue radio selects it and focuses it in WebKit, where Chrome keeps red. That is an engine's radio arrow-key behavior, or a carrier difference; to look at.
  - WebKit synthetic-tabindex (5): the root and a disabled view marked focused after Tab (still unclassified, above).
  - Media (synthetic-media 36 WebKit, 12 Firefox; video-player): the media-session readback and rates above.
  - synthetic-segments (12 each): no Viewport Segments API.
  - synthetic-keys `key field a for 700` (a held key's auto-repeat, #140): Chrome's field reads `go`, Firefox's `goaaaa`. Chrome's held-key path (`browserKey`) sends no `text` for a printable key, so neither the down nor its repeats type; the same on main at 0bd99f606 (main: 225 keys lines, of which the key logs; this lane: 137, only this step outside layout). Not fixed here.


## Finishing the calibration (2026-10-07, lane/xbrowser-finish)

Every remaining class was probed in plain HTML in Chromium, Firefox and WebKit (Playwright 1.63.0 on the mini) and then fixed, sized out of the fixture, or named known with that evidence. Three blind review rounds (Astra, Grok) narrowed the known entries to exact steps and fields, with geometry pinned to the measured value pairs and tree and state rows matched by anchored patterns.

| Item | Plain HTML | Outcome |
|---|---|---|
| Radios, ArrowRight on the last radio | Chrome and Firefox wrap to the first radio; WebKit stays | WebKit's convention: known at that step only; `radios.steps` taps red after it, so later steps start equal. No exact2 bug: the JS target and the wasm host follow Chrome. |
| WebKit tabindex: root and the "disabled" box focused | WebKit's Tab skips buttons; clicking a button focuses its nearest focusable ancestor | WebKit's convention: known, focus mark only, exact records. The `disabled` box has `tabindex=0`, which HTML keeps focusable. |
| Native control sizes (textarea, input, select, range, date, time, checkbox) | each engine's own intrinsic size (e.g. a checkbox in a stretching column: Chrome and Firefox 400 px wide, WebKit 12) | Fixtures size their incidental controls (budget, mounted, rem, controls, early; keys' textarea keeps its authored height). Apps keep theirs: markdown's path field (a text input's automatic minimum width in a flex row: Chrome 164, Firefox 194, WebKit 182.25 px, the app's numbers) and video-player's select (Firefox 156.83 against 141) are known at their measured pairs. |
| Viewport segments and posture | `window.viewport` exists only in Chrome; `navigator.devicePosture` nowhere without emulation | Known at the segment steps, exact. |
| Long-text wrapping | generic families differ: system-ui 16px 459.9 px (Chrome, WebKit) against 462.9 (Firefox) for the same 60 characters; `ui-monospace` is SF Mono only in WebKit | Known at measured pairs for markdown-stress (both), and textflow, fieldnotes, duo-lab and completion-storm (Firefox). WebKit's textflow scroll offset is not explained (its serif and system-ui match Chrome's) and stays failing. |
| Media session readback | WebKit updates `playbackState` from the page's media; Chrome and Firefox return the declared value | Known per step, exact. |
| Media rate counts | the harness's frozen clock fired its own `ratechange`s before or after the page's handlers | **Fixed in the harness:** the freeze is now invisible (`parityScript`): real rates 0, the page's own `playbackRate`/`defaultPlaybackRate` with real-setter validation and load-time resets, and the freeze's own trusted events swallowed. This also turned Chrome conformance (wasm vs JS) for synthetic-media, media-session and video-player green (27 failures on main to 0). |
| Firefox `<audio>` on a video-only file | Firefox refuses it (MEDIA_ERR_SRC_NOT_SUPPORTED); `<video>` plays | Known for `sounds`, exact. |
| Firefox headless fullscreen | innerWidth becomes 1366 | Known at the two fullscreen steps, measured pairs; playback still compared. |
| Firefox collapsed setSelectionRange | no select event, direction "forward" | Known, exact. |
| Firefox focus scrolling (synthetic-keys) | Chrome reveals a partly hidden focused field, Firefox does not | Known at measured pairs (the page column's scroll offset is not reported, so the comparison cannot cancel it). |
| Realworld's ♥ | 14px sans-serif `♥ 2`: Chrome and WebKit 43.69 px, Firefox 37.98 | Known at measured pairs. |
| Media session on the Playwright carrier | — | **Fixed:** `mediasession` goes through `exact.mediaSession.act`, as on Chrome, without needing a box. |
| Held key (#140) | — | **Fixed:** Chrome's `browserKey` sends a key's text with its down, so a held printable key types and repeats; Control and Meta chords carry none. Firefox's `key field a for 700` now matches. |

**Final counts** (all 24 apps and the synthetic fixtures): Firefox 0 failures (679 known); WebKit 9 failure lines from three causes (934 known), all left open on purpose:
- `synthetic-startend` `long.sy`: Chrome 91,784 against WebKit 91,782 at four steps. Plain HTML of a comparable 1,900-row list gives equal scroll heights in Chrome and WebKit, so this is not shown to be an engine difference.
- `synthetic-lists tap say`: `transcript.sy` Chrome 166 against WebKit 70, deterministic (3 of 3), though the two screenshots at that step show the same rows. Unexplained; worth a look at `scrollFollowEnd` in WebKit.
- WebKit `textflow` `tap pause` and `clock +60000`: `textflow.y` and the document scroll offset, 215 against 130 (above).

The issue stays open for those three.
