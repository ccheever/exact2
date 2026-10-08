---
name: 20261005-terminal-surface
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-surface
pr_url: https://github.com/ccheever/exact2/pull/150
verified_commit: null
---

# Terminal surface: the reference's Ghostty emulator in a native web view

## Outcome

A hooked box `t3-terminal` shows a terminal that runs the same emulator code as the reference
desktop app (libghostty-vt WebAssembly, Canvas 2D). The app builds and runs it from files vendored
in the example, with no network and no T3 Code checkout. The view accepts bytes to show. It reports
typed bytes, grid size, selection, link clicks, context-menu requests, focus, and keys it does not
handle. IME, clipboard, scrollback, mouse reporting and cursor behave like the reference surface.
The first deliverable is a written go/no-go verdict. `20261005-terminal-drawer`, `20261005-terminal-layout`, `20261005-terminal-integrations` and
`20261005-sign-in-terminals` start only after the user accepts a GO.

## Scope and exclusions

Included:

1. **Go/no-go spike** (first commit; verdict in `AGENT-HANDOFF.md` "Terminal spike"; each failed check
   gets a new `EXACT2-GAPS.md` entry, "new — record at prepare"). S1 a key that the page does not encode
   reaches the clone's key path (menu equivalents, key monitors); an encoded key does not leak. S2 cost of
   1, 4, 11 and 44 views (the reference keeps up to 10 hidden threads mounted, `ChatView.logic.ts:71,674-702`):
   processes, memory, idle CPU, a 5 MB output flood with no dropped byte and no stall, and process count after
   close. S3 WASM and font load offline from the app bundle through a custom scheme handler. S4 the agent
   screenshot shows the canvas, and a debug API returns the visible text. S5 agent `type`/`press` reach the
   page. S6 ⌘V clipboard read works without a prompt (`surface.ts:1121-1141`); if not, use the host-supplied
   read (`pasteFromClipboard(read, isCurrent)`, `surface.ts:935`). S7 Korean 2-Set composition commits once and
   the candidate window sits at the cursor (`surface.ts:1896`); check `R10Connect.swift` chord re-issue and
   `R9Input.swift` composition end. S8 the bundle has no `effect`, `@t3tools/*`, `tailwind-merge` or `zustand`.
   S9 views are inspectable in development builds only (`EXACT2-GAPS.md` X2 option 3). GO needs S1, S3, S7 and
   S8. S2 needs a budget: the implementer proposes it from the numbers and the user accepts it. NO-GO stops
   the terminal tickets; the user chooses the next step.
2. **Vendored assets** in `terminal-host/vendor/` with `VENDOR.json` and `verify-vendor.mjs`
   (see Implementation notes). No build step reads the reference.
3. **Host page, hook and bridge**: `terminal-host/src/entry.ts`, `T3Terminal*.swift`, the
   `t3-terminal` hook and data keys in `app.json`, and a typed JSON bridge.
4. **Ported pure modules with their tests**: `terminal-links.ts` (16 tests),
   `selectionActions.ts` (`resolveSelectionActionPosition`, 18 tests), and the emulator tests.

Excluded: sessions and server RPCs, the drawer (`20261005-terminal-drawer`); tabs, menus, which commands the key
policy sends to the app (`20261005-terminal-layout`); opening links, Add to chat (`20261005-terminal-integrations`).

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`, paths from the
repository root): `apps/web/src/terminal/ghostty/{surface,core,renderer,runtime,keyCodes}.ts`,
`README.md` there, `apps/web/src/lib/selectionActions.ts`, `apps/web/src/terminal-links.ts`,
`apps/web/src/components/ThreadTerminalDrawer.tsx:107-120,745-782` (the `beforeKey` policy).
The surface needs from its host page: two `fetch` + `WebAssembly.instantiate` loads and a
WebKit table workaround (`runtime.ts:46-67,190-219`); `FontFace` and `document.fonts`
(`surface.ts:66-67,120,672`); Canvas 2D, `ResizeObserver`, a DPR media query, rAF
(`:637,668,869-902,1194`); a hidden `textarea` for IME; the CSS variables `--app-scrollbar-width`,
`--app-scrollbar-thumb`, `--app-scrollbar-thumb-hover`; optional `navigator.keyboard.getLayoutMap`
(`keyCodes.ts:221-233`). `surface.ts:1` imports `lib/utils.ts`, which imports contracts, effect,
tailwind-merge and `composerDraftStore`. The page bundle must replace that import and the
`appearanceFonts.ts` import (`:18`) with small shims, and replace the `?url` imports
(`runtime.ts:1-2`, `surface.ts:17`) with scheme URLs.
Library revision: `20261005-platforms-v3`. Selected topics: capabilities and foundations (WKWebView hosts, hooks, bundle
assets and custom schemes are unknown in the library; the basis is the clone's own code: `modules/apple/R6MediaPreview.swift` is
a hook that mounts a WKWebView and exposes a `status` for the agent; asset lookup follows `T3Notifications.swift:73-83`), platforms
(keyboard/focus, pointer, native controls), testing-and-debugging, performance (named workload, repeated runs), accessibility.
Consumer framework revision: the `main` pin of `20261005-clone-on-exact2-main`. Toolchain: Xcode 27.0, Bun 1.4.2; Zig is not needed.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `lane-backend.sh`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (`target/t3-ui-parity/electron-oracle.mjs` for the render pair) | pending |
| recorded decision | Apparatus approval: `verify-vendor.mjs`, optional `refresh` script (spec open decisions) | none | User approves | pending |
| recorded decision | GO verdict of this ticket's spike | none | User accepts the verdict and the S2 budget | pending |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../../issues/README.md), 2026-10-05. They are not reproduced on the pin and not searched upstream. No prior attempt or review.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X8](../../issues/closed/20261005-x08-agent-pointer-native-views.md) | Pointer phases for native views in the agent | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)` rows) | Keep mouse, wheel and IME rows attended |
| [X25](../../issues/20261005-x25-keyboard-keyup-code-capture.md) | keyup, `code`, capture phase for keys | `EXACT2-GAPS.md` X25 | nonblocking (workaround: native key monitors) | S1 shows the real path |
| [X15](../../issues/closed/20261005-x15-non-latin-key-equivalents.md) | Chords under a non-Latin input source | `R10Connect.swift` re-issues chords | nonblocking | S7 checks ⌃C under Korean 2-Set 2026-10-07: #110 closed by main #168, which covers declared chords and the host's command items only; `R10Connect.swift` and the key-code fallbacks stay (adopt-main-fixes-input). |
| `EXACT2-GAPS.md` X2 option 3 | Inspectable web views in development builds | Not reproduced | nonblocking | S9 |
| new — record at prepare | Agent text/key input into a native web view; agent screenshot of web view content | Unknown in the library | blocking for agent rows only if S4 fails; else none | S4, S5 decide; rows become attended on failure |

## Implementation notes

- Layout: `terminal-host/vendor/` (binaries, `VERSION`, two license files, `VENDOR.json`),
  `terminal-host/src/` (copied sources with header notes, `shims/`, `entry.ts`),
  `terminal-host/build.mjs` (Bun bundle to `assets/terminal-host.{html,js}` and byte copies of the
  WASM and font into `assets/`; flat names; generated files are gitignored), `verify-vendor.mjs`.
  Run `build.mjs` from the example's `app.json` `commands` (the library says: add project commands there).
- Vendored files (copied from T3 Code `1e2ecbd975`; paths under `apps/web/src/terminal/ghostty/` unless stated):

| File | Bytes | sha256 | License |
| --- | --- | --- | --- |
| `vendor/ghostty-vt.wasm` | 630,932 | `51b016a6aa3c29ead71c7c8acf8c01d43b064bae19957a9c9f9e2bf469267629` | MIT, Mitchell Hashimoto and Ghostty contributors |
| `vendor/ghostty-write-pty.wasm` | 112 | `75cb147e98ede3f85f3cd6236a30f6d12565b0b237e1d8db941f5f3e8ad3d903` | MIT, T3 Tools Inc. |
| `fonts/SymbolsNerdFontMono-Regular.woff2` | 1,177,576 | `a8e2fc5ae3c2525812151b95da80c5beab0befa84aca84fc33aaed94317502df` | MIT, Ryan L McIntyre |
| `native/libghostty-vt/VERSION` (repo root) | 41 | `3fc03e29c86b5100e6738d20cb1c97c3969ab1044debbe8a4cd7d49cdc9cf321` | content `9f62873bf195e4d8a762d768a1405a5f2f7b1697` |

- `VENDOR.json` records for each file: example path, reference path, sha256, bytes, license id; the reference commit; the Ghostty
  repository and revision; Zig 0.15.2 and the build flags (`-Demit-lib-vt -Dtarget=wasm32-freestanding -Doptimize=ReleaseSmall
  -Dstrip=true -Dlib-version-string=0.1.0-dev+<revision>`, `apps/web/scripts/build-libghostty-wasm.sh:104-110`; trampoline source
  `ghostty-write-pty.zig`, 224 B). The reference does not record the Nerd Fonts release: write `unknown`. Copy
  `native/libghostty-vt/LICENSE` and `fonts/LICENSE` as `LICENSE-ghostty` and `LICENSE-nerd-fonts`. Sources keep the T3 Tools MIT header (`LICENSE-T3`).
- `verify-vendor.mjs` checks every sha256 and size in `VENDOR.json`. It instantiates
  `ghostty-vt.wasm`, calls `ghostty_build_info`, and compares the revision with `VERSION`, as
  `runtimeAbi.test.ts:17-47` does. It fails on any mismatch. Do not rebuild the WASM in the normal
  build (the reference downloads Zig without a hash check, `build-libghostty-wasm.sh:63-65`; no
  rebuild reproducibility is shown). An optional `refresh.mjs` may read a reference checkout or
  Zig; it is not part of the build, and it needs apparatus approval.
- Shims: `isMacPlatform` (from `lib/utils.ts:15-17`), `isMonospaceFamily` with `cssFontFamilies` and
  `areFontAdvancesMonospace` (from `appearanceFonts.ts`), scheme URLs for the WASM and font.
  Record each change in the file header. The Settings › Licenses list must show libghostty-vt and
  Symbols Nerd Font Mono, as the reference does (`third-party-licenses.config.json:99-110`).
- Bridge: page to native `ready, data, resize, selection, link, contextmenu, chord, focus, error`; native to page `write,
  resetAndWrite, theme, font, visible, focus, chords, clearSelection, readSelection, paste, dispose`. Resize arrives 150 ms after
  the grid settles (`surface.ts:911-921`). The page gets the chords it must not encode. `T3Terminal.swift` batches `write` per frame.
  The agent `status` lists each view: ready, cols, rows, visible text, selection, last error. The scheme handler serves only the
  assets; the web view cancels other navigation, uses a non-persistent data store, and loads no remote URL.
- States: loading (the canvas paints the theme background first, `surface.ts:704-709`); error (the page failure text
  `<message> — close and reopen the terminal to retry.`, `ThreadTerminalDrawer.tsx:905-915`); hidden (`setVisible(false)` stops
  paint, bytes still parse); unfocused (steady hollow cursor); focused (blink 500 ms, steady under reduced motion, `surface.ts:26,637`).
  Keyboard focus enters by a click or a focus request; Tab is sent to the shell as in the reference, and the user leaves by the app chords or a click. No dialog,
  menu, empty, disabled or permission state here (menus and dialogs belong to `20261005-terminal-integrations` and `20261005-terminal-drawer`). Reduced
  motion: the cursor stops blinking. Accessible names come from the page (`Terminal input`, `Terminal scrollback`).

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Vendor integrity | Fresh checkout | `bun terminal-host/verify-vendor.mjs` | Every sha256 and size equals the table; revision equals `VERSION`; two license files present | macOS host machine | log |
| Self-contained build | Run under a read-deny of the reference checkout path (for example macOS `sandbox-exec` with a `deny file-read*` rule); no network | `bun terminal-host/build.mjs` and the app bundle build | Exit 0; bundle and build log contain no reference path | macOS | log, bundle grep |
| Bundle content | Two builds | Compare sha256; grep imports | Same sha256; none of `effect`, `@t3tools`, `tailwind-merge`, `zustand`; size recorded | macOS | hashes |
| Ported tests | — | `bun test examples/t3-code/terminal-host examples/t3-code/terminal-links.test.ts` | `terminal-links.test.ts` 16/16; `keyCodes` 5, `renderer` 8, `core` 12, `runtimeAbi` 9; the pure `surface.test.ts` describes (from `:471`) pass with original names; `resolveSelectionActionPosition` tests pass; DOM-bound tests (`surface.test.ts:42-470`, `observeSelectionActions`) run in the web view or are listed `n/a-ui` with a reason | macOS | test log, map in `AGENT-HANDOFF.md` |
| Spike S1–S9 | Fixture harness page feeds bytes and logs bridge messages | Run each check; open and close 44 views with `ps` before and after | Each has a pass/fail line, numbers for S2, process count back to baseline, and the verdict | macOS 26.6.2, 1280×840 | report, `--json` transcripts |
| Render | Harness bytes: colors, wide and emoji cells, Nerd glyph, alt screen, 2,000 lines | Agent `state`, screenshot; same bytes in the oracle drawer (`target/t3-ui-parity/electron-oracle.mjs`) | Same cols×rows for the same box and font; cell size within 1 pt; screenshot pair reviewed; glyph raster difference declared | macOS 1280×840, light and dark | `state`, pair |
| Keys and bridge | Harness focused | Agent `type`/`press` (if S5 passes) | Logged `data` equals what the vendored `core.encodeKey` returns for the same events | macOS | log |
| Input that the agent cannot send `(attended session)` — pointer parts are agent rows since exact2 #186 (adopt-main-fixes-r3): drag select, double/triple click, ⌘-click a link, wheel through the scrollback, scrollbar thumb drag and right-click run in `macos/tests/terminal/pointer.swift` with the agent's event shapes and in the r3 drive (`tap terminal-view drag … mouse`, `clicks 2/3 at`, `wheel 0 -600 at`, `contextmenu at`, `mouse at … modifiers Meta`; result pending the r3 drive); still attended: Korean 2-Set typing and its candidate window, ⌃C under 2-Set (an input method, not a pointer), ⌘C/⌘V against the shared pasteboard | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch | Drag select; double and triple click; ⌘-click and Shift-click on a link; wheel through 10,000 scrollback rows; scrollbar thumb drag; right-click; ⌘C, ⌘V; Korean 2-Set typing with candidate window; ⌃C under Korean 2-Set; resize drag | Matches the oracle behavior; each step is recorded | macOS, trackpad, US and Korean 2-Set | notes, shots |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, new `macos/tests/terminal` binary and the affected AppKit binaries); `bun scripts/caps.mjs`; the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `terminal-host/**`, `terminal-links.ts`, `terminal-links.test.ts`,
`modules/apple/T3Terminal*.swift`, `macos/tests/terminal/**`, `app.json`, `.gitignore` lines,
`LICENSE-ghostty`, `LICENSE-nerd-fonts`, the Licenses list, `AGENT-HANDOFF.md`, `EXACT2-GAPS.md`.
Required environment: Xcode 27.0, Bun 1.4.2, the oracle from `20261005-desktop-oracle-and-trace`, no T3 Code (Nightly) running; lane builds set
`T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

2026-10-06: implemented on `feat(example)/t3-code-terminal-surface` (rebased on `da40e6590`, after
hot-file-split #147). The spike verdict is **GO if the user accepts the S2 budget**; the full table,
numbers and the proposed budget are in `AGENT-HANDOFF.md` "Terminal spike". Scope 2–4 are in place
because the spike needed them: vendored assets with `VENDOR.json` and `verify-vendor.mjs`; the page
(`terminal-host/src/entry.ts`), its build and the `t3-terminal` view (a module view rather than a
hook, so the agent's `type`/`press` and a canvas snapshot reach it); the ported tests. A development
harness (`terminal.contract`, ⌃⌥⇧T) stands in for the drawer.

Not done: the render pair against the Electron oracle (desktop-oracle-and-trace will not be built:
not run); the attended rows (mouse, wheel, drag selection, links, right-click, scrollbar drag,
resize, ⌘C/⌘V through WebKit's clipboard, real Korean 2-Set typing and the candidate window,
⌃C under 2-Set, Safari inspection); the seven session-buffer `core.test.ts` tests (move to the
drawer with `state/terminal.ts`). Open decisions: the GO and the S2 budget; apparatus approval for
`terminal-host/build.mjs` and `verify-vendor.mjs`; X46 (no pre-bake step: run the page build before
the bundle build); the harness stays until the drawer replaces it.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (implementation + spike) | `b9e4d15a8` on `da40e6590` | `bun test examples/t3-code` 1457/0 (base 1321; terminal-host + terminal-links 136); strict tsc on `app.ts` clean, on `terminal-host/src/entry.ts` (ES2023, DOM) clean; `contract build` 2189 slots, 43 resources, 48356 nodes; `cargo test -p t3-code-macos --lib` 10/0; `macos/tests/terminal` 8 (1 skipped unless `T3_TERMINAL_SCALE=1`; the scale run passes too); every other AppKit binary passes (mermaid needs a server: not run); `verify-vendor.mjs` 7 files match; `build.mjs` under `sandbox-exec` (reference and network denied) exit 0, two builds same sha256; caps pass; macOS bundle builds with the page in `Resources/assets/` | `AGENT-HANDOFF.md` "Terminal spike" (S1–S9 table, S2 numbers); drive transcripts and pictures in the PR | User: GO and S2 budget. Live: canvas paint under the agent fixed after the drive, not re-driven; attended rows unverified |

## 2026-10-06 conflict, theme and evidence follow-up

Merged base `eb259ef281b224520b35faad61929f8be9cf40ed` in `61b7a428c`, retaining both
window-chrome and terminal presentation status. `b01d97a0f` connects selected appearance,
terminal theme colors and typography, including updates while the page loads.

[New evidence and limitations](../../evidence/20261005-terminal-surface/20261006-theme-parity/attempt.md):
1496 Bun tests pass; native terminal 8 pass / 1 scale skip; app Rust 10 pass;
strict TypeScript, Contract, app build/launch and all five gates pass. Four pairs compare
actual WKWebView output against the pinned original renderer in Chrome. All ten built-in
theme appearance halves match source palette values. App window captures show ANSI output
and actual key delivery to the loopback fixture. The default layer capture reproduced as
fully transparent and is preserved as a failed artifact. Always use the window capture here.

This does not close the task's full acceptance: no full Electron render pair, no physical
IME/pointer/clipboard sweep, no fresh scaling benchmark, and no accepted GO/budget.
The development harness still is not ThreadTerminalDrawer; PTY/session/UI integrations
remain separate work. Verification stays unverified for those unmet criteria.

## Next action

The user reads the spike verdict and accepts or rejects GO and the S2 budget. On GO, the drawer, layout, integrations and sign-in terminal tasks may start; the attended rows run in a person's session.

2026-10-07 (records sync): merged into `feat(example)/t3-code` by #150 (`404abd3fb`); verification stays as recorded above.
