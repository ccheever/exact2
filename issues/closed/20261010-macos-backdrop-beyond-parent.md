# macOS: `backdrop-filter` blurs only its parent's subtree, so a glass panel one wrapper deeper shows the content behind it sharp

**Status:** Closed
**Resolution:** Kept the declared parent-subtree limit under ruling 12 after the time-boxed plate attempt found no reliable insertion point; documented in LLP 1053.000 §3, vocab and pitfalls
**Systems:** host/apple macOS, backdrop-filter
**Severity:** P2
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261005-x11-shadow-blur-parity.md; https://github.com/ccheever/exact2/issues/129; https://github.com/ccheever/exact2/issues/225

## Summary

On macOS a `backdrop-filter` node blurs only what its parent and its earlier siblings paint. Anything an
ancestor above the parent paints shows through the panel sharp. In CSS (Filter Effects 2) the backdrop is
everything painted behind the element up to its backdrop root, at any depth. The web host blurs both
cases; macOS blurs only the first.

This is "case B" of GitHub #129 and #225. #129 closed with the edge fix (main #221); #225 asked for a
ruling on case B and `saturate()`, and closed when `saturate()` landed (main #232). #232's own text says
"The separate macOS cross-parent backdrop sampling problem remains open", but no issue tracks it. This
file is that issue.

## Why this arose

T3 Code draws its composer, menus, popovers, toasts and dialogs as glass: `backdrop-filter: blur(12px)
saturate(1.14)` over an 80 % fill (`1e2ecbd975`, `apps/web/src/index.css:116-118, 146-147, 333-397`;
the composer's `::before` layer, `components/chat/ComposerSurface.tsx:22`). The composer floats over the
scrolling transcript, so messages show blurred through it. It sits several wrappers deep, not beside the
transcript.

In the clone the panels that are siblings of what they cover blur correctly since #221 (the composer
command drawer, dialog backdrops). The rest are nested below what they would blur, so the clone draws them
opaque: the composer card, the model picker, toasts, pull-request tooltips, the confirm dialog and the alert
stack. A user sees a solid bar where T3 Code shows blurred messages. Making each panel a direct sibling of
the content behind it would constrain every overlay's component structure.

## Reproduction

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` exports an empty `sources`.

```text
component X11app
  view
    main testId="root" position="relative" width="100%" height="100%" background-color="#ffffff"
      row position="absolute" left=0 top=0 width=400 height=180
        box width=40 background-color="#e53935"
        box width=40 background-color="#1e88e5"
        box width=40 background-color="#e53935"
        box width=40 background-color="#1e88e5"
        box width=40 background-color="#e53935"
        box width=40 background-color="#1e88e5"
        box width=40 background-color="#e53935"
        box width=40 background-color="#1e88e5"
        box width=40 background-color="#e53935"
        box width=40 background-color="#1e88e5"
      // A: the glass is a sibling of the stripes
      box testId="sibling" position="absolute" left=20 top=40 width=160 height=100 background-color="#ffffff33" backdrop-filter="blur(8px)"
      // B: the same glass one wrapper deeper
      box testId="wrapper" position="absolute" left=220 top=40 width=160 height=100
        box testId="nested" width="100%" height="100%" background-color="#ffffff33" backdrop-filter="blur(8px)"
```

| Scenario | Commands | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|---|
| A and B, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 400x180 "screenshot x11b.png window"` | macOS 26.6.2 (25G83), Apple Silicon | main `abc1eadff`; #327 head `14493d253` the same | A blurred; B sharp: across the stripe edge at x = 320 the pixels step `#56a0eb #56a0eb #ec6560 #ec6560 #ec6560` | B blurred as A: `#6795d1 #808bbc #9e7fa0 #bd7386 #d36972` (the web's) | image, record |
| Same, web | `bun exact.mjs agent web --size 400x180 "screenshot x11b.png"` | Chrome 155 (the agent's) | main `abc1eadff` | A and B both blurred; A on macOS is within 4/255 of A on the web | (the reference) | image, record |

![Backdrop case B: web, macOS main, macOS #327](https://raw.githubusercontent.com/ccheever/exact2/d4109d63e545054c04bda13fea11338245556553/fw-issues-20261010/x11b-web-macos-pr327.png)

Record (contract, commands, pixels across a stripe edge in A and B on each host):
https://raw.githubusercontent.com/ccheever/exact2/d4109d63e545054c04bda13fea11338245556553/fw-issues-20261010/x11b-record.txt

Not tested: iOS and tvOS (a system material, declared separately), Linux.

## Constraints

- Declared today: LLP 1053.000 §3 (`llp/1053.000-backdrop-filter.rfc.md:131`), LLP 1001 §1
  (`llp/1001-kernel-v1.spec.md:722-728`), and `host/apple/Sources/ExactKit/Backdrop.swift:11-17`. AppKit's
  public `backgroundFilters` see only the superlayer's subtree.
- Private `CABackdropLayer` cannot ship (`Backdrop.swift:20`; #225).
- The web is the parity oracle (CLAUDE.md), and LLP 1115 puts what the author writes first
  (`llp/1115-write-the-web-ship-the-platform.principle.md:16`): the author wrote `backdrop-filter`, and
  someone who knows the Mac would notice sharp text through a glass panel.
- Cost: #225 judged a per-frame window snapshot (option 3 below) to work against the frame budget.
- #327 changes no backdrop code (its head behaves as main).

## Acceptance criteria

- In the repro, B on macOS matches A on macOS and B on the web: across the stripe edge at x = 320 the
  pixels grade as the web's, within the bound LLP 1053.000 §3 records for case A.
- A, the parity page (`scripts/fixtures/backdrop.contract`) and #221's edges are unchanged.
- A nested glass panel over a scrolling list stays blurred while the list scrolls.

## Decision needed

#225 listed three options for case B and no ruling was recorded before it closed:

1. **Keep it declared**, and document that a glass panel must be a direct sibling of what it blurs.
2. **Restructure the layer tree** so a backdrop node's layer sits where its backdrop root's content is
   in its superlayer's subtree (a host layering change; z-order risk). A spike would show whether it is
   safe.
3. **Snapshot the window's layers behind the node** and blur that (public API; costs a render per frame
   while the content behind moves).

## 2026-10-12 attempt (Codex)

Charlie admitted a time-boxed ancestor plate through clear wrappers, without
reparenting or snapshots, with the parent-subtree sample kept for blocked paths.
The attempt stopped under `rules/RULES.md`'s three-correction-round limit. No
plate implementation or `native.backdropSample` diagnostic is shipped; this
issue remains open and the declared parent-subtree limit still applies.

The regression first failed because no ancestor plate existed. The prototype
then created a plate but placed it at `(0, 0, 160, 100)` instead of
`(220, 40, 160, 100)`: the box calculated from backing layers disagreed with
the authored frame. Computing from view frames did not establish a reliable insertion
point. The third run of
`EXACT_TESTS=1 swift test --jobs 2 --package-path host/apple --scratch-path
target/clients/86599762ed9a44d2f30a8b0d/com.exact.caltrain/macos/aarch64-apple-darwin/embedded/development/tests
--filter ClipMacTests` failed at the new test's plate lookup with:

```text
testNestedBackdropSamplesBehindClearWrapper:
XCTUnwrap failed: expected non-nil value of type "CALayer" - unmounted branch layer
Executed 16 tests, with 1 failure
```

That prototype and its failing structural test were removed. The next attempt
must first establish a stable insertion point in AppKit's actual layer tree,
then verify composited window pixels for A/B, the unchanged parity page, a
scrolling list, and a clipped fallback. No composited-pixel acceptance is
claimed for this attempt.

The guide, pitfalls and `contract vocab backdrop-filter` now give the working
authoring advice: make the glass a direct sibling of what it covers.
`scripts/fixtures/backdrop-nested.contract` keeps the A/B reproduction, a list
under sibling/nested glass, and a clipped wrapper for that future verification.

## 2026-10-13 closed under ruling 12

Charlie's ruling (2026-10-12): try a time-boxed backdrop plate on provably
clear paths, and "stop and keep the limit if the clear path is too narrow".
The plate attempt stopped at the three-round limit (above) without a
reliable insertion point, so the declared limit stands: on macOS a
`backdrop-filter` samples only its parent's subtree; make the glass a
sibling of what it covers (LLP 1053.000 §3, `contract vocab
backdrop-filter`, docs/agent-pitfalls.md). Reopen with a new approach
(layer restructuring or a window snapshot) if a consumer needs more.
