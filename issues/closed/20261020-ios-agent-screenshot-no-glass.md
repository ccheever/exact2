# agent ios screenshots draw glass and backdrop blur as clear

**Status:** Closed
**Resolution:** A plain iOS screenshot over glass or a blur takes the display's pixels in the page's area (AgentIOS materials, devices.mjs withMaterials), or says why its materials are approximate
**Systems:** agent (scripts/agent.mjs), Apple host (iOS), materials
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

A view with `backgroundMaterial="glass"` over list text: `bun exact.mjs agent ios "screenshot a.png"`
shows the text behind the capsule crisp, as if the material were clear. During an ordinary drive
the app is not on the simulator's screen either, so `xcrun simctl io <udid> screenshot` shows the
home screen. Real compositing appears only with `--touch platform` (the app is then frontmost) and a
`simctl io screenshot` taken while the drive holds the app open.

The lane spent about 30 minutes, part of it tuning the material's opacity against the agent's
picture, which was wrong.

## Reproduction

```
row backgroundMaterial="glass" position="absolute" bottom=20 left=20 width=300 height=56
```

over a `list` of long texts; compare `agent ios "screenshot a.png"` with the same drive under
`--touch platform`, capturing `simctl io screenshot` during `clock +4000 real`.

## Expected

The agent's screenshot shows what the screen shows, materials included, or says plainly that it
does not render materials and how to get a composited picture.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-19),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet re-run on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` on Charlie's Mac.
