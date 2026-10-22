# Code review: the design tool's Gallery view, G4, round 1 of 2 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `cd06f539c2be3d38196c9302adb6649454f46df687191a52be44972343d57af0`), shared by both reviewers, each blind to the other's review; HEAD `fb05064ee7`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (1 SHOULD-FIX, 1 NIT).
- **Disposition:**
  Nothing blocks from either reviewer. All taken, in the round 1 commit:
  - Astra 1, Grok 1 (a copy reloaded at the app's place stays there under the gallery): when the framed page changes, the grid forgets where copies were sent or reloaded. Tested: a copy reloaded at /post/3 frames /__dev/gallery/ once the gallery is on.
  - Astra 2 (framing refusal): the JS loop's gallery page carries the loop's frame-ancestors (the tool's origin and its own), asserted on the real loop (dev-gallery.test.mjs).
  - Astra 3 (the inspector through the join): tamber-inspect's join of a gallery cell gives its literal row the gallery's reason, and its token row the app's token rung. A contract test shows no row, param, detach or state control for it and the token edit kept.
  - Astra 4, Grok 2: the stub's restart clears its gallery copies.

1. **SHOULD-FIX** — A grid cell already reloaded at an app route keeps that route when Gallery is on, so it never frames `/__dev/gallery/`. `apps/design-tool/grid.ts:162`
   Tile `src` is chosen before any build is known: if `reloaded` has that cell, the URL is the copy's origin plus the stored app path, and `gridPage`'s `/__dev/gallery/` pathname is dropped. `reloaded` is only removed for cells that leave the grid (`grid.ts:160`), not when the framed page changes. The new guard at `grid.ts:217` skips later navigate and reload updates for `page === 'gallery'`, but the iframe is already the app, so the build that registers is not a gallery build and the guard never applies. Copies that can navigate are unaffected; a cell that could not (the T7 reload fallback) stays on the app for the rest of the Gallery session. The grid contract test never turns the toggle on, so it does not catch this.

2. **NIT** — Restart leaves the gallery copy list in place. `apps/design-tool/stub/tamber-stub.ts:451`
   `/__stub/restart` clears `noNavigate`, `copyDelays`, and `refuseLater`, and does not clear `galleryCopies` (set at `tamber-stub.ts:486`). The next `/__stub/copies` overwrites it. A copy frame that registers after restart and before that post is still labelled `page: 'gallery'` and has no `navigate`.

The rest of the plan holds. `galleryFrame` is only `hello.frame` plus `/__dev/gallery/`, and `hello.frame` still comes from `pageOf` (loopback, no path, not the tool's port). The toggle and the note are local-only and hidden in States; paired mode keeps the gateway frame. The note says the gallery follows each kept edit, not each scrub frame. The wasm loop's `/__dev/gallery/` answer is `text/plain` 404 with the reason (`host/web/dev.mjs`), and the iframe shows that body. `page: 'gallery'` is stored on the build and kept across `build-status` (`shown()` always sends `page`). The build row is labelled "the gallery". `behind` is that build's own pin revision, and the Live chip is the server's proposal class. There is no design or plan comparison, and gallery builds are not counted into it. In the grid, a gallery build does not lead and is not sent `navigate`; pins and holds are unchanged, and give-back still runs. `rowRung` and `stateRung` return no write rung for both cache paths, ahead of the own-file check. Promote, tokenize, and detach are still derived in `inspector.ts` from a template or param-with-template rung, which a gallery file does not get. A whole token or param on a base row still takes the token or param rung, which writes the app design.
