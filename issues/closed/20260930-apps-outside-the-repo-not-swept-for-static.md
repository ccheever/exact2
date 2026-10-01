# Weird Castle and Interview are not swept for the `position: static` default

**Status:** Closed
**Resolution:** both apps were brought onto today's exact2 on branches of their own (`fix7/drive` in each repo) and driven against it; neither needed a change for the static default
**Systems:** apps outside the repo (Weird Castle, Interview)
**Severity:** P2
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 §3.5, LLP 1001 §5, feedback: Charlie 2026-09-29 (breaking outside apps is fine; pick the ideal design)

LLP 1074 made `position: static` the default. In the repo only Messages needed changes (29 boxes). Apps that consume exact2 by path were not swept: Weird Castle (`~/projects/weird-castle`) and Interview.

What to do in each, once they move to a main at or after d8a4a0ebf, found with the same temporary compiler audit (a stack of "is this ancestor positioned" during lowering; the code is in this session's transcript and is easy to re-add):
- give `position="relative"` to the parent of every `position="absolute"` element (and of a `dialog`) that relied on the parent as its containing block;
- give `position="relative"` to every element that has `top`/`left`/`right`/`bottom`/`inset` and no `position`, since a static box's insets do nothing;
- a `z-index` on a static box that is not a flex or grid item does nothing now; add `position="relative"` where it mattered.

The compiler already lowers `relative` onto boxes that clip, scroll, transform or animate, so those need nothing.

## Sweep, 2026-09-30

Weird Castle's main, Login, password and account-menu containing boxes are
explicitly positioned; its only static `z-index` box is a flex item, where CSS
applies it. Interview's main and Screen containing boxes are positioned, and its
current Contract compiles to 462 nodes. No static-default migration was needed
in these inspected paths, and neither external repository was edited.

A full Weird Castle drive is blocked by its existing uncommitted Contract
changes: `app.contract:76` fails inference for the unused `setPassword(value)`
parameter. This is outside the exact2 ticket sweep; the external work was
preserved. Keep this issue open until that app builds and both external apps
can be driven against the integrated exact2 change.

**Static audit (2026-09-30, Claude, the follow-up lane):** a script over the Contract source (the parent of each `position="absolute"` element or `dialog`, resolved through `when`/`each`/`match`/`otherwise` regions; insets on a box with no position; `z-index` on a static box under a block parent) finds nothing in `~/projects/weird-castle/app.contract` or `~/projects/interview/app.contract`. The same script finds 35 boxes in Messages before LLP 1074 (`d8a4a0ebf~1`) and 0 after, so it sees what the hand sweep saw. The drive of both apps above is still owed.

## Driven, 2026-09-30

Neither app built against this main, for reasons that had nothing to do with
positioning, so each got a branch (`fix7/drive`, in `~/projects/weird-castle`
and `~/projects/interview`) holding its main checkout's uncommitted work as
found, then what today's exact2 asks for. Neither main checkout was touched.

**Weird Castle.** The Contract error at `app.contract:76` was exact2's: the
password field's `type=showPassword ? "text" : "password"` was a bound `type`,
refused since LLP 1069.001, and reported as an uninferable parameter. The
compiler admits a choice between text fields' literals again. The app was
also a month behind (the launcher under Node, the vendored cosmic-text and
wgpu-hal patches, the hosts' generated entry, the GPU surface's shared
encoder, the runner's new outcome kinds, a logic module for the web build's
JS target). On its branch: `cargo test` passes (7 + 1); macOS, the web build
and Linux build and drive through `scripts/agent.mjs`. The title, the login
(its `‹ Back` and the password's Show button, both absolute) and, against the
stand-in Castle, the signed-in screen with its account menu (absolute, under
the mark) are where the Contract puts them on macOS and on the web.

**Interview.** It needed the vendored wgpu-hal patch line and its web crate's
entry in today's form. With its backend running locally: the web build (the
JS target) and the macOS app drive from the home page through the
development sign-in; the brand, the notifications button, the compose button
and the tab bar (all absolute, in positioned boxes) are at their places on
both, with a phone-sized viewport.

Not driven: either app on iOS, Interview on Linux, and Interview's wide
layout (the rail and the Mac toolbar, which a 420-point viewport does not
show).
