# The iOS pointer backend and the driver expect Simulator.app, which Xcode 27 does not ship

**Status:** Closed
**Resolution:** Device Hub calibrates against its captured simulator framebuffer; the real held-drag smoke passes on Xcode 27 (2026-10-02).
**Systems:** Agent API, iOS simulator, host/apple/build.mjs
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** Crew port report D10 (2026-09-24); LLP 1035.003 (the simulator pointer backend)

The Crew port (report of 2026-09-24, D10) drove a booted simulator that nobody could see: `agent.mjs ios` never opened Simulator.app. The fix (`showSimulator()` in `host/apple/build.mjs`) runs `open -g -a Simulator` when the driver takes a simulator.

On this Mac, Xcode 27 ships no Simulator.app, so that falls back to Device Hub (`com.apple.dt.Devices`). Its "Devices" window comes up, but it has not been confirmed to show the simulator being driven. The same absence already broke something before this report: the iOS smoke logs `contact: unsupported — no Simulator window on screen`, so the simulator pointer backend (real contacts on the simulator, LLP 1035.003) finds no window and falls back.

Done when, under Xcode 27, driving a simulator shows that simulator's screen to a person at the Mac, and the pointer backend finds its window again (or names why it cannot) in the iOS smoke.

## Closure audit (2026-09-30)

Retained open on this machine's selected `/Applications/Xcode.app`: its
Applications directory contains DeviceHub.app and no Simulator.app. `simctl`
sees booted iPhone 18 Pro (iOS 27.0) and iPhone Duo (27.1). The UIKit suite
executes successfully headlessly (101 tests); that does not prove the desktop
pointer can reach a device. Computer-use inventory confirms Device Hub runs,
but two attempts to inspect its window timed out, so no window ownership or
coordinate mapping was guessed. `pointer.swift` still names Simulator.

## Device Hub, as built (2026-09-30)

What Xcode 27's Device Hub does, found by running it:

- Launch arguments never reach a Device Hub that is already running, so
  `open -b com.apple.dt.Devices --args -CurrentDeviceUDID …` changed nothing
  once it was up. Its main window shows whichever device was last picked in
  its list (it showed another session's iPad throughout).
- It registers the URL scheme `devices`, and `devices://device/open?id=<udid>`
  opens a window of that device's own, titled with the device's name, showing
  the simulator's live screen (the action and its `id` parameter are named by
  Device Hub's own log when the parameter is missing).
- Its windows belong to `com.apple.dt.Devices`; the owner name a window list
  gives is the localized "Device Hub". Its Accessibility windows carry no
  frames and all the same title, so nothing here goes through Accessibility.

So `showSimulator` (`host/apple/build.mjs`) now opens that URL when there is
no Simulator.app, and the pointer helper (`host/apple/pointer.swift`) takes a
simulator window to be one owned by either app, by bundle identifier. It
lists every candidate, those titled with the device first, and the driver
calibrates against each until its app sees the hover (`scripts/agent.mjs`),
since Device Hub's main window is titled with a device too. The helper's
`activate` is gone; the driver raises the device's window through
`showSimulator`.

Verified on this Mac (Xcode 27.0, an iPhone 18 Pro simulator on iOS 27.0
made for the purpose): driving it opens a 356×775 "fix7-iphone" window
showing its screen; the helper's `window` finds that window by title, first
of three Device Hub windows; `bun scripts/smoke.mjs ios` passes.

What does not work yet: the contact itself. Tried once the screen was
unlocked (it was locked for most of the work, which the helper now reports
as its own reason: `the Mac's screen is locked`):

- `open` does not bring Device Hub's window above the app a person is in
  (Chrome was in front: the helper refused each hover, naming it, as it
  should). The Accessibility request does (`kAXFrontmostAttribute` on Device
  Hub's process; after it and the URL the device's window was first in the
  window list). It is not in the helper: taking the front from the person at
  the Mac is only worth it once the rest works.
- With the device's window frontmost and the pointer posted inside it, the
  app saw no hover (`layout.pointer` stayed null across four positions, in
  two runs). The driver's calibration is built on that hover, which
  Simulator.app delivered; under Device Hub the smoke prints `ios contact:
  unsupported — the app saw no pointer hover in the simulator window
  "<device>"`, or names the covering app.

So this stays open for one thing: a way to map the viewport onto Device
Hub's window that does not need a hover (a touch the app reports, or the
screen's rectangle found in the window's picture), then the raise above.
Each try moves the pointer of whoever is at the Mac, so it wants a Mac
nobody is using.

The same run found the iOS smoke failing at main since offscreen taps are
refused (9e82bd496): on a phone's viewport the opened deck's middle is below
the fold. The smoke scrolls it in, and back for the material buttons.

## Closed (2026-10-02)

Device Hub does not emit the hover used by Simulator.app. The driver now
matches `simctl`'s framebuffer against a ScreenCaptureKit picture of the
exact-name device window. It derives the scale and offset from those pixels,
then includes the viewport's reported screen offset. No bezel dimensions or
app-specific coordinates are assumed. Low-detail, ambiguous, changed and
unmatched pictures refuse; Screen Recording permission is named when absent.

A contact raises Device Hub through Accessibility and opens the device URL.
Before each down or move the helper checks the window id, frame and topmost
owner. Release uses the last successful desktop point even if geometry
changes or a move fails, so a failed recalibration cannot leave the button
held. Simulator.app retains its existing hover calibration.

Built and driven on the dedicated `exact2-oct2-pointer` iPhone 18 Pro
simulator (iOS 27.0, Xcode 27). `bun scripts/smoke.mjs ios --app-only` passes
in 29.7 seconds, including a real down, 200-point drag with layout read while
held (scroll exceeds 50 points), hold and up, and all three Contract tests.
The smoke restores scroll through the still-visible scroll view, since the
title used to initiate the drag is now offscreen. Unit tests verify two
placements/scales and rejection of uniform, unrelated and duplicate images.
The physical drive was at one window placement; replay at multiple physical
placements and the broader Messages gesture cases remain LLP 1035.003 work.
