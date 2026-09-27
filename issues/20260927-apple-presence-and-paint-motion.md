# Apple motion leftovers: a removed layout-transition keeps its offset, exit colours don't show, inherited colour is cached past retirement, and more

**Status:** Open
**Systems:** Apple host (`host/apple/src/presence.rs`, `style.rs`, `paragraph.rs`, `Sources/ExactKit/…`), Linux host (`host/linux/src/paint/presented.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062, LLP 1063, LLP 1059, LLP 1061

Each item is confirmed by reading.

1. **A removed `layout-transition` leaves its last offset and scale.** `presence.rs:185-188` calls `remove_property` without presenting identity, so the presenter keeps `layoutOffset`/`layoutScale` (`PresenterIOS.swift:719`). It also happens when an ancestor becomes `display:none` mid-move.
2. **An exit animation's `color` never shows.** `paint_over` changes `text_color` without setting its mask bit (`style.rs:211`), and `restyle_presented` starts from an empty mask (`:243`), so the colour is never sent.
3. **Inherited colour is cached past retirement.** When a parent's colour transition is cancelled, its engine property is removed, and `present_colors` skips the descendant walk that clears `paint.runs` (`paragraph.rs:330`). Inline text and inheriting children keep the mid-transition colour. Destroying a node doesn't clear its entry.
4. **Size can go negative (P3).** A layout spring that overshoots gives a negative width or height on Apple (`presence.rs:263`, `Surface.swift:58`) and Linux (`presented.rs:86-87`). The web clamps to 0. Fix: clamp on every host.
5. **Presenter reset keeps exiting views (P3).** Reset never clears `leaving` (`PresenterIOS.swift:271`, `PresenterMac.swift:710`), and `.destroy` checks `endExit(id)` first (`:701`). After a reload that reuses an id, a new view's destroy can be consumed.
6. **Tab-bar state survives a reset (P3).** `SegmentHost.reset()` restores only `controls`, and `bars`/`members`/`hidden` survive a dev reload (`SegmentsIOS.swift:298`).
7. **The agent's `prefer` reaches only one session (P3).** It sets the static `DisplayPreferences.agent` and tells only the addressed session (`Agent.swift:237-248`). Another session's runner keeps the old bits while its press feedback follows the new ones.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max, Grok 4.7 xhigh, Opus 5.5 max. Verification: confirmed by reading.
