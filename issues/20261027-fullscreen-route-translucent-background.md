# A full-screen route that paints a translucent background covers what is under it

**Status:** Open
**Systems:** navigation (Apple, web)
**Severity:** P3
**Author:** Claude (Opus 5.5), from Excord's image viewer
**Date:** 2026-10-27

Status note: a ruling for Charlie; no accepted LLP decides it.

## Symptom

Excord's image viewer is a full-screen route (`navigationPresentation: modal`, full
screen) whose column paints `#000` and, on a drag down, animates that background toward
transparent. Discord's apps and Photos fade the black away to show the dimmed channel
under the picture. Here nothing under the route shows:

- iOS: the route's controller paints the route's background again
  (`RouteController.paintBackdrop`), and the presentation keeps the system background
  under it (`ModalController.paintSurface`). In dark mode the fade is invisible (black
  over black); in light mode it goes from black to white. The colour is drawn twice, so
  at half its alpha the screen is three-quarters dark.
- Web: the fade goes to the host's grey under the route.

## Background

A full-screen route covers the screen, so each host puts an opaque surface under it: the
platform's background where the route paints nothing (LLP 1115 D2). An author can already
ask for a clear route with `background: transparent` (LLP 1141.001 D3, for sheets). A
translucent background, or one animating toward transparent, is neither: today it lands
on the opaque surface. In CSS a translucent box shows what is behind it, which here is the
page the route covers.

## Options

- **A. A full-screen route that paints its own background leaves the surfaces behind it
  clear** (iOS: the presentation surface and the route controller's view, about four
  lines in `ModalIOS.swift` and `NavigationIOS.swift`, and an XCTest; the web host
  likewise). The author's translucent black then composites over the page underneath,
  as CSS has it. A throwaway build showed the dimmed `#general` through the fade in both
  appearances. A route that paints nothing keeps the platform's background.
- **B. Paint the colour once and keep the opaque surface.** The fade's curve becomes
  right, but it still fades to black or white, not to the channel.
- **C. Leave the hosts; the app writes `transparent` on the route and fades an inner
  backdrop.** That backdrop cannot reach the screen's edges until LLP 1123 D4's
  `-exact-safe-area` exists.

## Recommendation

A. It follows LLP 1115's precedence (what the author writes, a translucent background,
wins, and CSS composites it over what is behind), it is what a hand-built UIKit viewer
does (an `overFullScreen` presentation over a clear view; the host already presents a
full-screen route `overFullScreen`), and it changes nothing for a route that paints an
opaque colour or none. Confidence: about 80%. The doubt: a route that paints a translucent
colour at rest, not only during a fade, would show the page under it too, which is what
CSS says and may surprise an author who expected the platform's background.
