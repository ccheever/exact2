# Code review: the design tool's Gallery view, G4, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `cd06f539c2be3d38196c9302adb6649454f46df687191a52be44972343d57af0`), shared by both reviewers, each blind to the other's review; HEAD `fb05064ee7`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (3 SHOULD-FIX, 1 NIT).
- **Disposition:**
  Nothing blocks from either reviewer. All taken, in the round 1 commit:
  - Astra 1, Grok 1 (a copy reloaded at the app's place stays there under the gallery): when the framed page changes, the grid forgets where copies were sent or reloaded. Tested: a copy reloaded at /post/3 frames /__dev/gallery/ once the gallery is on.
  - Astra 2 (framing refusal): the JS loop's gallery page carries the loop's frame-ancestors (the tool's origin and its own), asserted on the real loop (dev-gallery.test.mjs).
  - Astra 3 (the inspector through the join): tamber-inspect's join of a gallery cell gives its literal row the gallery's reason, and its token row the app's token rung. A contract test shows no row, param, detach or state control for it and the token edit kept.
  - Astra 4, Grok 2: the stub's restart clears its gallery copies.

No BLOCKER found.

1. **SHOULD-FIX — [grid.ts:162](/tmp/rv-dt-g4r1/apps/design-tool/grid.ts:162): Gallery toggling preserves an earlier app reload URL.** After a copy refuses navigation and falls back to `/post/3`, enabling Gallery leaves that cell at `/post/3`; `reloaded` overrides the new `gridPage`. Confirmed with an in-memory probe. Clear page-specific navigation state when `at` changes, and cover toggling after a fallback reload.

2. **SHOULD-FIX — [app.test.contract:2235](/tmp/rv-dt-g4r1/apps/design-tool/app.test.contract:2235): The required foreign-origin framing test is missing.** Changing hello’s page to `example.com` tests URL filtering, not refusal to frame `/__dev/gallery/` from an unauthorized origin. Add that framing case; the current test cannot detect missing `frame-ancestors` enforcement.

3. **SHOULD-FIX — [tamber-inspect.test.mjs:783](/tmp/rv-dt-g4r1/scripts/tamber-inspect.test.mjs:783): The required gallery inspector test stops at rung helpers.** It never exercises the selection join or client-derived promote/tokenize/detach controls, and contains no token or param case. Cover a gallery selection through the inspector: all recipe write controls absent, token and param edits still targeting the app’s design.

4. **NIT — [tamber-stub.ts:451](/tmp/rv-dt-g4r1/apps/design-tool/stub/tamber-stub.ts:451): Restart does not clear `galleryCopies`.** The other copy configuration resets here, but gallery labels and missing navigation capability survive until another `/__stub/copies` request. Reset this list too for fixture isolation.

No files changed; no builds or test suites run.
