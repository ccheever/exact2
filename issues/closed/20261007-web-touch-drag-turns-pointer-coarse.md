# Conformance: a web finger drag turns the page's pointer coarse; Linux stays fine

**Status:** Closed
**Resolution:** The web agent injects a finger without Chrome's touch emulation, so the page's (pointer) and (hover) stay the device's (Charlie, 2026-10-07); interaction-gallery's Linux reference is 6 / 6 equal, JS vs wasm 18 / 18.
**Systems:** scripts/agent.mjs, host/linux, host/web-js/conform.mjs
**Author:** Claude (Opus 5.5), split from 20261007-conformance-video-player-plays-in-real-time
**Date:** 2026-10-07

Strict JS-target conformance with `--linux` at 931f9fb7d: interaction-gallery's `drag swipe-card-1`,
`drag deck-handle` and `tap mode-reorder` differ on `resources.media.pointer`: the wasm page (Chrome)
reports `coarse`, the Linux reference `fine`. The web agent delivers a finger `drag` by turning on
Chrome's touch emulation (`Emulation.setTouchEmulationEnabled`, scripts/agent.mjs), which also flips
the page's `(pointer)` media to coarse for the rest of the drive; the Linux host's drag leaves the
device's pointer fine. One host reports the wrong primary pointer for the same launch: either the web
agent should restore its media after a touch gesture (or not change it), or the Linux host should
report the finger it was just driven with. A host-parity decision, not fixed here.
