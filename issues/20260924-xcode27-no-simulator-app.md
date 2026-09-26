# The iOS pointer backend and the driver expect Simulator.app, which Xcode 27 does not ship

**Status:** Open
**Systems:** Agent API, iOS simulator, host/apple/build.mjs
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** Crew port report D10 (2026-09-24); LLP 1035.003 (the simulator pointer backend)

The Crew port (report of 2026-09-24, D10) drove a booted simulator that nobody could see: `agent.mjs ios` never opened Simulator.app. The fix (`showSimulator()` in `host/apple/build.mjs`) runs `open -g -a Simulator` when the driver takes a simulator.

On this Mac, Xcode 27 ships no Simulator.app, so that falls back to Device Hub (`com.apple.dt.Devices`). Its "Devices" window comes up, but it has not been confirmed to show the simulator being driven. The same absence already broke something before this report: the iOS smoke logs `contact: unsupported — no Simulator window on screen`, so the simulator pointer backend (real contacts on the simulator, LLP 1035.003) finds no window and falls back.

Done when, under Xcode 27, driving a simulator shows that simulator's screen to a person at the Mac, and the pointer backend finds its window again (or names why it cannot) in the iOS smoke.
