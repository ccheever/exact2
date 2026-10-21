# Code review: LLP 1122 T7a-2 (copies at loopback names of their own; a pinned copy's pin holds over the app's own), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `2a82f19e03`.
- **Method:** one brief (sha256 `e04570691ea030393fe8f5b427acc6941b1d49feeb6f610eb723e483e7c12e84`), theme2's rulings inline, shared by both reviewers, each blind to the other's review; commit 2a82f19e03. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 BLOCKER, 2 SHOULD-FIX, 2 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - The app's latest choice is recorded at every call, equal to the pin or not, and is what is handed back (1).
  - The wasm runner holds the root size, setting the app's own aside, and applies it again when handed back (2).
  - Handing back puts back only the axes the pin set (3).
  - A setMode before the pin is recorded and said in `app` (4).
  - Tests: scheme, root size and mode on the JS page (5).
  - Port 80 (6).

I'll review the T7a-2 commit on HEAD as a read-only pass: correctness and security first, against the 21:04Z rulings, with no tree changes and no test runs.The review skill wants a defect-first pass over the full diff. I'll locate the exact2 repo and the T7a-2 commit, then read the gate, pin, and status changes against the rulings.The commit is small and isolated. Next I’ll read the gate, both dev pages, and the status path in full so the pin and host checks can be traced, not just diffed.Two blockers. The host gate does what the ruling asks. The pin does not: a later `setMode` back to the pinned value is forgotten on hand-back, and on the wasm page a pinned root font size does not override `setRootFontSize`.

## 1. BLOCKER — Hand-back reapplies an older `setMode` after the app has chosen the pinned value

`host/web/dev.js:453-459` (same logic in `host/web-js/dev.mjs:708`)

`holdPin` records a pinned axis only when the live value differs from the pin, and returns without touching `appMode` when nothing differs. Choosing the pinned value leaves the previous ask in place. Hand-back then writes that map over the mode (`host/web/dev.js:550-551`, `host/web-js/dev.mjs:714`).

Scenario: the copy is pinned `dark-style=black`. The app calls `setMode("dark-style", "dim")`. The pin is put back, and `app` says `dim`. The app then calls `setMode("dark-style", "black")`. The live mode is already black, so `holdPin` returns immediately and `appMode` stays `dim`. Every later status still says the app asked for `dim`. Hand-back sets `dark-style` to `dim`.

A run of values that all differ from the pin does keep the latest. This fails only when the latest value is the pin's. The same axis is also left stale when another pinned axis changes in that same read, because only the axes that still differ are written into `appMode`.

## 2. BLOCKER — On the wasm page a pinned root font size does not override `setRootFontSize`

`host/web/dev.js:543-549`

In a copy the pin sets inline `font-size: <px> !important` and then calls `exact_set_root_font_size(hostSize())`. That call is the host-size API. While the app has an absolute `setRootFontSize(px)`, the runner stores the new host size and leaves the kernel size alone (`runner/src/runner/root_font.rs:147-151`). The wasm document writes those kernel sizes as absolute `px` (`host/web/src/document.rs:623` via `css_text`, not `rem`). The root's computed `font-size` changes. The text and the layout do not.

Scenario: the app has called `setRootFontSize(20)` and the tool pins `rootFontSize: 32`. Status reports `32` because `facts` reads the computed root (`host/web/dev.js:434`). The wasm tree stays at 20.

A percentage misses in the other direction. `setRootFontSize("200%")` is `AppSize::Of`, so the same call sets the kernel to the pin times that ratio. Pin 20 with `"200%"` lays out at 40, while status still says 20.

The JS page does hold: its styles are `rem`/`em`, and the inline `!important` wins over both the `20px` rule and the `calc(var(--exact-host-font-size, 100%) * ratio)` rule (`host/web/navigation.js:989`, `host/web-js/dev.mjs:714`). Hand-back there removes the inline size and the app's stylesheet rule is what remains.

## 3. SHOULD-FIX — Hand-back of a pin with no mode puts back the pre-pin snapshot

`host/web/dev.js:455` and `host/web/dev.js:550-551` (`host/web-js/dev.mjs:708` and `:714`)

`holdPin` returns immediately when the pin has no `mode`, so a `setMode` during that pin sticks. Hand-back still writes `before.mode`, taken at the first pin, for every axis. `appMode` never learned those calls.

Scenario: the tool pins `{scheme: "dark"}` only. The app then calls `setMode("dark-style", "dim")`. The copy shows dim. Hand-back sets `dark-style` back to whatever it was when the pin arrived.

Scheme and font do not take this path. Hand-back clears the pin sheet and the inline font, and the app's own `color-scheme` and `#exact-root-font-size` rule are what remain.

## 4. SHOULD-FIX — A `setMode` that lands before the pin is restored, and never appears in `app`

`host/web/dev.js:531` and `host/web/dev.js:455`

`before.mode` is the live mode at the first pin, and hand-back uses it when `appMode` is empty, so the launch choice does come back. `holdPin` never runs for it, because there is no pin yet, so `app` stays absent for the whole pin.

Scenario: on launch the app calls `setMode("dark-style", "dim")`. The tool then pins `black`. Status shows `black` and no `app`. Hand-back jumps to `dim` with the tile never having said the app asked for it.

Scheme and font do not have this gap: `appAsked` reads the inline `color-scheme` and the `#exact-root-font-size` rule directly, including values set before the pin (`host/web/dev.js:444-447`).

## 5. NIT — The new tests cover one `setMode` path and the gate's main refusals

`scripts/dev-copies.test.mjs:167-176`, `scripts/install-page.test.mjs:513-514`, `scripts/tamber-builds.test.mjs:727-730`

The browser test pins `black`, clicks Dim, checks `app.mode` and that hand-back becomes `dim`, on both loops. It never sets the scheme, never sets a root font size, never sets the mode back to the pinned value, and never pins a non-copy (`app-own`, the single canvas). The server test accepts one `app` object and rejects five bad shapes. It does not show that a later status with no `app` clears a stored one, or that a body over 1 KiB is refused. The gate test does show `exact-copy-3.localhost` at the server port is admitted and not local, and that `0`, `100`, the wrong port, `evil.localhost`, and a name that only ends in `.localhost` are refused.

## 6. NIT — A copy on port 80 is refused, because the browser omits the default port

`scripts/install-page.mjs:297`

Loopback names go through `URL.host`, which drops `:80`. The copy check is `^exact-copy-[1-9]\d?\.localhost:${port}$`, so a request whose `Host` is `exact-copy-1.localhost` does not match a server on port 80. Any other port is in the `Host` header and matches. A wrong port fails closed. No other name matches that pattern, and a copy is never `local`: `local` still requires the host to be `127.0.0.1` or `localhost` (`scripts/install-page.mjs:299`). An attacker-owned rebinding name still fails the host check. `.localhost` is not a zone an attacker is authoritative for.

Also small, on the font report: `app.rootFontSize` is the stylesheet body, `20px` or `calc(var(--exact-host-font-size, 100%) * 1.25)`, with no comparison to the pin (`host/web/dev.js:446-447`). Any app rule is reported while a font size is pinned, including one that computes to the same px, and a long `calc(...)` is sliced at 64 characters. `app.scheme` is not sliced; a `setScheme` longer than 32 characters makes the server reject the whole status (`scripts/tamber-builds.mjs:134`), and the page keeps sending it.

The single canvas, a tab, and an `app-own` frame are not copies (`host/web/dev.js:367`): no `!important`, `holdPin` returns immediately, and `app` is omitted. Native builds do not run this script. A sequence of `setMode`s that all differ from the pin keeps the latest, and the restore does not loop: the follow-up pass sees the pin value and stops.
