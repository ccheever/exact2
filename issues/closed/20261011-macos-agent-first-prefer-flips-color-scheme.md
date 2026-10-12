# macOS agent: the first `prefer` turns `prefers-color-scheme` light whatever it names, because the agent's light appearance is not told at launch

**Status:** Closed
**Resolution:** fixed by #477
**Systems:** host/apple macOS, agent API
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-11
**Related:** https://github.com/ccheever/exact2/pull/468 (T3 Code clone task pr-list-scroll-and-sidebar-sweep, PS-1); the clone's `examples/t3-code/EXACT2-GAPS.md` X58; LLP 1069.007 D2 (display preferences come from the driver, `light` by default, "applied at launch rather than on the first `prefer`") and its "As built" list; LLP 1061 D5 (`prefer`); issues/closed/20261011-macos-scheme-switch-stale-text-raster.md (it notes "The agent pins light at its first operation")

## Summary

On macOS, an agent session starts by telling the runtime the machine's appearance as `prefers-color-scheme`. From
the first operation on, though, the window draws the agent's light (Aqua). So on a Mac set to Dark, the app reads
`viewport.prefersColorScheme == "dark"` while its window is light. The session's first `prefer` re-tells the
preferences, whatever fact it names (`has-focus` below), and `prefers-color-scheme` turns `light`. Every subtree
chosen by the scheme then mounts again. In the repro, a `scroll` under `when viewport.prefersColorScheme == "dark"`
was at offset 600. After the `prefer` it is a new node (id 3 → 104) at offset 0, and the runner's log says
`preferences (1 asked again) → epoch 3 (+101 −101 ~2)`. Later `prefer`s keep the offset.

The web agent reads `light` from its first operation, and a `prefer has-focus` there changes nothing else. On a Mac
set to Light both sources would say light, so nothing should flip (not run).

## Expected

- A `prefer` that does not name `prefers-color-scheme` leaves it as it is. `Agent.prefer`'s doc says "A fact not
  named stays as it is" (`host/apple/Sources/ExactKit/Agent.swift:395-404`).
- The session's `prefers-color-scheme` is the agent's `light` from launch, matching the window. LLP 1069.007 D2's
  table gives display preferences the defaults `no-preference`, `no-preference`, `light`, `no-preference`, "applied at
  launch rather than on the first `prefer`". Its "As built" list says "display preferences fixed at launch".

## Why this arose

In the T3 Code clone, the Pull Requests list went back to its top "when the window was focused again". This happened
in agent drives of pr-list-live-refresh (investigation A, attempt 4). PR #468 traced it here. Each of those drives
had made its first `prefer` (`has-focus false`, then `true`) at that point. The clone mounts the Pull Requests page
once per scheme (`PagesCover`: `when scheme == "dark"`), so the re-told scheme mounted the page again with its scroll
at 0. After one `prefer` made first, refocus after refocus kept the list's place on the same scroll node. The clone
needs no workaround: a drive that measures a scroll across a `prefer` makes one `prefer` first.
By PR #468's reading, real input does not take this path: a real refocus tells only the page facts
(`didBecomeKey` → `Session.tellPage`).

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `7e86209e20a32d5660ed182925d6b46f68ea20b8` and built
with `bun exact.mjs mac`. `contract build` gives no diagnostic. The Mac is macOS 26.6.2 (25G83) on Apple Silicon, set
to Dark (`defaults read -g AppleInterfaceStyle` → `Dark`).

```text
// X58: the macOS agent's first `prefer` re-tells the system appearance.
shape Viewport
  width: number
  height: number
  prefersColorScheme: string
  prefersReducedMotion: bool
shape Page
  hasFocus: bool
  visibilityState: string
shape Row
  n: number
shape Rows
  items: list<Row>

component X58app
  resource viewport = exactViewport() as shape Viewport
  resource page = exactPage() as shape Page
  resource rows = rows() as shape Rows
  view
    main testId="root" width="100%" height="100%" display="flex" flex-direction="column"
      text `scheme ${viewport.prefersColorScheme}, focus ${page.hasFocus}` testId="facts"
      when viewport.prefersColorScheme == "dark"
        scroll testId="list" flex=1 min-height=0
          each r in rows.items key=r.n
            text `dark row ${r.n}` testId=`row-${r.n}` height=24
      else
        scroll testId="list" flex=1 min-height=0
          each r in rows.items key=r.n
            text `light row ${r.n}` testId=`row-${r.n}` height=24
```

`app.ts`'s sources are `rows: () => ({ items: Array.from({ length: 100 }, (_, n) => ({ n })) })`.

The drive:

```sh
bun exact.mjs agent macos --size 600x500 "clock +2000 real" "tree facts" "screenshot before.png window" \
  "layout row-10" "tap list wheel 0 600" "layout row-10" "prefer has-focus false" "prefer has-focus true" \
  "tree facts" "screenshot after.png window" "layout row-10" "tap list wheel 0 600" "layout row-10" \
  "prefer has-focus false" "prefer has-focus true" "layout row-10" "tree facts" "logs"
```

| Step | macOS agent, main `7e86209e2` | Web agent (JS target, Chrome), same app and main |
|---|---|---|
| First operation | `Text#2 [facts] "scheme dark, focus true"`, while the `window` screenshot shows a light (Aqua) title bar and page with "dark row …" rows | `"scheme light, focus true"` |
| `tap list wheel 0 600` | `scroll … #3 0,600` | `scroll #3 0,600` |
| `prefer has-focus false` (the session's first) | reply `"media":{"prefers-color-scheme":"light",…}`; log `t=2000 preferences (1 asked again) → epoch 3 (+101 −101 ~2)` | `"prefers-color-scheme":"light"` (unchanged) |
| After `prefer has-focus true` | `"scheme light, focus true"`, rows read "light row …", `scroll … #104 0,0` (a new node at the top) | `"scheme light, focus true"`, `scroll #3 0,600` (kept) |
| A second wheel, then a second `prefer` pair | `#104 0,600` kept; the log has only `page (1 asked again)` lines | — |

Not run: iOS (its agent sets the scheme through the scene's trait override, and `reportScheme` tells the
preferences there), Linux, and a Mac set to Light.

## Where it comes from

This is read from the source. Then step 2 was checked by a one-line patch, built and driven, and not kept.

1. At boot the session tells the runtime its preferences (`primePreferences` and `tellPreferences` →
   `preferenceBits`, `host/apple/Sources/ExactKit/Session.swift:1260-1288`). On macOS, `preferenceBits` reads
   `DisplayPreferences.systemDark` (`DisplayPreferences.swift:89-95`), which is `Agent.systemAppearance` when set and
   otherwise the user default `AppleInterfaceStyle`. Nothing has set `Agent.systemAppearance` at boot, so the
   runtime is told the machine's scheme: `dark` on this Mac.
2. The agent is made on first use (`agentInstance`, `Agent.swift:814-820`). Its `init` (`Agent.swift:28-35`) pins
   the system appearance to light: "The system appearance is the agent's from its first operation: light, as
   `prefer` would set it, never the machine's (LLP 1069.007 D2)". `systemScheme(dark: false)`
   (`Mac/AgentMac.swift:1016-1020`) sets `Agent.systemAppearance` and `NSApp.appearance` to Aqua. The window repaints
   light, and `viewDidChangeEffectiveAppearance` (`Mac/ExactViewMac.swift:249-254`) gives the session its paint
   scheme (`session.scheme(dark:)`, for `light-dark()`). But nothing tells the runtime's preferences again.
   `systemScheme` does not call `DisplayPreferences.changed()`, although that function's comment counts "the agent's
   system scheme" among its callers (`DisplayPreferences.swift:38-42`).
3. Every `prefer` (`Agent.swift:405-458`) assigns `DisplayPreferences.agentContrast` and `DisplayPreferences.agent`.
   Their `didSet` posts `changed()` (`DisplayPreferences.swift:18-29`), and each session's observer runs
   `tellPreferences()` (`Session.swift:525`). That re-reads `DisplayPreferences.systemDark`, which is now the pinned
   Aqua, so the runtime is told `light`.

The check: with `DisplayPreferences.changed()` added at the end of `systemScheme(dark:)`, the same drive reads
`"scheme light, focus true"` at its first operation. The log has `t=0 preferences (1 asked again) → epoch 3 (+101
−101 ~2)` when the agent is made, and `prefer has-focus false/true` keeps `#104 0,600`. That patch still mounts the
scheme's subtree once, at the first operation, because boot told `dark`. Applying the agent's light before boot, as
LLP 1069.007 D2 asks, would not.

## Acceptance criteria

- In the repro on a Mac set to Dark, the first operation reads `"scheme light, focus true"`, matching the window.
- `prefer has-focus false` / `prefer has-focus true` keeps the list's node and its `sy 600`, and the log has no
  `preferences` line for it.
- `prefer prefers-color-scheme dark` still switches the scheme, its subtree and the window.
