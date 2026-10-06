---
name: 20261005-media-actions
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-media-actions
pr_url: https://github.com/ccheever/exact2/pull/166
verified_commit: null
---

# Images and videos have a context menu, failure states and "Open original"

## Outcome

Every image and video the reference wraps in its media actions has the same context menu, tooltip,
keyboard access, progress toasts and failure states in the clone: copy path or URL, open in the
file viewer, save to disk, copy the image, and "Open original" when a load fails. Rendered HTML
previews load page assets from external hosts the way the reference's sandboxed frame does.

## Scope and exclusions

Surfaces (call sites of `MediaActions` found in the reference): inline media in chat Markdown, the
expanded image/video dialog, the Files panel image and video previews, and the video player
component (`MediaVideoPlayer`, which wraps its player when it receives an `actionsSource`). No
composer call site was found; confirm in the oracle before leaving composer chips out.

1. **Menu.** Right-click, the Menu key, or Shift+F10 opens a native menu (at the pointer, or at the
   element's bottom-left for keys). One menu at a time; opening it closes the tooltip. Items, in order:
   file media: "Copy full path", "Copy relative path" (only when the file is inside the workspace);
   URL media: "Copy URL"; then "Open in file viewer" (when the surface offers it); "Save image" or
   "Save video" (disabled if neither a source nor an asset exists); "Copy image" (images only,
   disabled when unavailable).
2. **Toasts.** "Path copied" / "URL copied"; save: loading "Preparing image download…" (or video),
   then "Download started"; copy: loading "Copying image…", then "Image copied"; failure: title
   "Could not <item label in lower case>" (default "Could not complete media action"; menu failure
   "Could not open media menu") with the error text or "The media action failed.". Errors:
   "This media is unavailable. Try reopening the preview.", "Reconnect to this environment and try
   again.", "The environment returned an invalid media URL.", "The file could not be fetched (HTTP N).",
   "This link returned a web page instead of media. Open the original URL.", "This image is too large
   or has no usable dimensions. Try saving it instead." (over 64 000 000 pixels), "The image could not
   be converted to PNG.". Asset media mint a fresh URL (`assets.createUrl`) at action time. The file
   name is the reference's name, else the display name, else "image"/"video".
3. **Tooltip.** Path, URL, or name, in the code style; media take keyboard focus.
4. **Failure states.** Message video: a 16:9 slot "Video unavailable · <label>" with **Retry video**
   ("Retrying…" while busy, re-mints the URL) and the open link. Expanded image: "This image could not
   be loaded." with the open link, or "Image unavailable. The file may have been moved or deleted." when
   there is no original URL. Markdown media that cannot load show the "unavailable" label, which also
   carries the menu. The open link reads **Open original** (an `http`/`https` external host), **Download
   video** (a blob) or **Open in browser**; it opens the system browser.
5. **Rendered HTML previews load external assets.** A rendered `.html` file in the Files panel or an
   attachment loads stylesheets, scripts, images, fonts and fetches from external hosts. The reference
   frame is `sandbox="allow-scripts allow-forms allow-popups allow-modals"` with an opaque origin.
   The clone's web view already does this for `https` and IP hosts (`modules/apple/R6MediaPreview.swift`
   header), but `README.md:268` and `AGENT-HANDOFF.md` "Remaining gaps" #1 still say it loads only its
   token directory, and plain `http://` to a named host is refused by App Transport Security (X7).
   First reproduce which cases load (https, http IP, http host, mixed content) and correct the docs;
   then close the rest.

Excluded: composer attachment chips, PDF/audio preview (done), the Browser surface, saving rendered
HTML, media in PR bodies (`20261005-pr-links-previews-and-routing`).

## Context and guidance

Parent specification: [spec](../spec.md). Reference (T3 Code `1e2ecbd975`):
`apps/web/src/components/media/MediaActions.tsx`, `.../media/OpenMediaLink.tsx`,
`.../media/MediaVideoPlayer.tsx:120-215`, `.../media/mediaContent.ts`,
`apps/web/src/components/chat/ExpandedImageDialog.tsx:140-290`, `.../ChatMarkdown.tsx:1560-1720`,
`.../files/FilePreviewPanel.tsx:154-171`, `.../files/BrowserDocumentFrame.tsx`,
`packages/client-runtime/src/mediaActions.ts`, `mediaReference.ts`, `mediaSource.ts`.
Port with their names and tests: `mediaFileReference`, `mediaUrlReference`, `mediaReferenceFileName`
(`mediaReference.test.ts`), `resolveMediaSource` (`mediaSource.test.ts`: "keeps the authored URL and
decodes the display name once", "accepts extensionless image embeds only when asked", "joins relative
paths to the workspace and records the relative reference", "cannot be loaded without a thread to mint
through", and the others), plus `resolveProtocolRelativeMediaUrl`.
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (a layout box is not
a hit target; offscreen targets), accessibility (focusable media, labelled controls, announce progress
once), state-and-data (await each step; a late old reply must not win), motion (toast timing; reduced
motion), design, testing-and-debugging. **Unknown in the library:** `NSMenu`, `NSPasteboard`,
`NSSavePanel`, `WKWebView`; the clone's own modules are the basis: `T3ContextMenu.swift` (native menu
at the mouse), `contextmenu=` on nodes (`sidebar-row.contract:56`), `saveFile` and its panel
(`timeline-plan.ts:80`, `T3PanelsNative.swift`, `R6DeviceStream.swift:516`), pasteboard write
(`T3Module.swift:122`), asset URLs with re-mint (`r5-panels-attach.ts:90`, `timeline-attachments.ts:57`),
inline image blocks (`markdown.contract:298`), expanded dialog (`timeline-attachments.contract:91-120`),
video slots (`r6-media-video.contract`).
The Electron main window has no download handler (only the Browser preview has one,
`apps/desktop/src/preview/Manager.ts:3576`), so how Electron saves a blob download is not read from
source: observe it in the oracle first.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | feature branch | Merged | satisfied (the clone is on `feat(example)/t3-code`) |
| merged task PR | 20261005-desktop-oracle-and-trace | none | Merged | waived: the oracle will not be built; oracle and trace-diff rows are not run |
| merged task PR | 20261005-main-fix-adoption | pending | Merged (the Menu-key and Shift+F10 path needs main's key events with modifiers, `preventDefault()` and `tabindex` on media nodes) | #155 merged |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | #147 merged |
| decision | What the oracle does on Save | reference source | Observe the Electron oracle (save dialog or direct download, folder, name) and copy it | answered from source: the main window installs no `will-download` (only the Browser preview does, `apps/desktop/src/preview/Manager.ts:3570`, whose comment says Electron opens a native Save dialog for a download with no save path), so Save is Electron's default: the Save dialog as a sheet, the media's file name, the session's last folder (Downloads first); "Download started" shows while it is open |
| decision | Hostname for the http-host check | none | Needs a dotted host name that resolves without editing system files, or a public host in an attended check | `localtest.me` (public DNS to 127.0.0.1, as #106 used) |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records only.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X7](../issues/20261005-x07-ats-keys.md) | ATS keys from `app.json`, so rendered HTML can load `http://` assets from host names | `EXACT2-GAPS.md` X7; r12-render finding | **blocking for the "Rendered HTML, http host" criterion only** (no workaround gives the reference result; the draft record says the same); nonblocking for every other criterion | Reproduce; keep the criterion in the plan as blocked until X7 is fixed or waived |
| [X26](../issues/20261005-x26-app-menu-control.md) | Menu at a given point (keyboard-opened menu) | X26 | nonblocking (workaround: native menu helper takes a rect) | none |
| [X29](../issues/20261005-x29-video-pdf-app-files.md) | `video` from `app:/` files | X29 | nonblocking (workaround: AVPlayerView) | none |
| [X30](../issues/20261005-x30-ts-announce-readback-picker.md) | Bytes, image transcode and save picker | X30 | nonblocking (workaround: native modules) | none |
| [X9](../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap | 1,327 of 1,500 | nonblocking until the cap | Put the media component in its own `.contract` file |
| new — record at prepare | Copy image has no "clipboard unsupported" case natively; the reference's wording "Image copying is unavailable. Use a secure browser connection or save the image." does not apply | MediaActions.tsx | nonblocking (declared deviation: item always enabled when media exists) | Declare in `EXACT2-GAPS.md` |

## Implementation notes

- One `MediaActions` component in a new `media-actions.contract` wraps any media node with
  `contextmenu=`, a `tabindex`, the Menu/Shift+F10 key handler and the tooltip. Menu rows come from a
  ported pure function (`mediaMenuItems`) so tests do not need the UI.
- New Swift ops (hunks for `T3Module.swift`): `copyImage` (decode, PNG, general pasteboard; refuse over
  64 000 000 pixels) and `saveMedia` (download the signed URL, then `NSSavePanel` as the oracle does).
  Reuse the existing menu and toast helpers. AppKit tests use a private named pasteboard.
- Reuse the unavailable-media label and the video slot geometry already drawn; add Retry and the open link.
- Rendered HTML: do not widen the wrapper policy beyond the reference (`R6MediaPreview.wrapperPolicy`
  already admits `frame-src http: https:`). Add fixtures for each external case.

## Acceptance and reproduction

Every row, attended or not, runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773; see
`20261005-embedded-server-runtime`). "Oracle" is `target/t3-ui-parity/electron-oracle.mjs`.

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Menu items | Thread with an image attachment, a workspace image path, a URL image, a video | Right-click each (attended session); also Menu key and Shift+F10 | Items, order, disabled states as above; menu at pointer / element corner | macOS 1280×840 and 840×620, light and dark; real pointer | native-chrome capture vs oracle `(attended session)`; agent `tree` for the key path |
| Menu logic | — | `bun test` ports of `mediaReference.test.ts` and `mediaSource.test.ts` (original names) and a `mediaMenuItems` test | Pass | host machine | log |
| Copy path and URL | Workspace image | Choose each | Pasteboard text equals the path / relative path / URL; toasts "Path copied" / "URL copied" | macOS | pasteboard read; screenshot |
| Save | Image and video assets on the lane server | Save each | Loading toast then "Download started"; bytes equal the source (sha256); panel behavior equals the oracle's | macOS; save panel `(attended session)` | hashes; trace shows `assets.createUrl` at action time |
| Copy image | PNG, JPEG, SVG, 70 000 000 px image | Copy | PNG on the pasteboard with equal pixel size; large image toast "This image is too large or has no usable dimensions. Try saving it instead." | macOS; AppKit binary `macos/tests/media-actions` | binary log |
| Failures | Remove the asset; expire the URL; refuse the network | Save, copy, open | Error toasts with the reference text; no menu left open | macOS | screenshots; `logs` |
| Video failure | Message video that cannot load | Open the thread | Slot, label, Retry video re-mints (trace), "Retrying…" busy state, open link wording | macOS both sizes | pixel pair vs oracle |
| Image failure | Expanded image with and without an original URL | Open | Both texts; Open original opens the default browser (never under the agent) | macOS | pixel pair; attended click |
| Menu keyboard, Escape and motion | Any media | Menu key opens the menu at the element's corner; arrows and Enter pick; reopen and press Escape; set prefers-reduced-motion | Focus returns to the media after the menu closes; `aria-label` on every icon button (Retry, open link); toasts appear without movement under reduced motion | macOS | `tree --ax`; film (`over 300 every 30`) in both modes; `(attended session)` for real keys |
| Tooltip and focus | Any media | Tab to it; hover (attended) | Focus ring and tooltip show path/URL/name; menu opening hides it | macOS | `tree --ax`; screenshot |
| Rendered HTML | Local fixture server on 127.0.0.1:16xxx serving a page that loads an https asset, an http IP asset, a script and a fetch | Open the page in Files | Each case loads or is refused exactly as in the oracle; the stale README text is corrected | macOS | screenshots; oracle comparison; server request log |
| Rendered HTML, http host | Page loading `http://<named host>/…` | Open | Loads as in the oracle | macOS | **blocked on X7**; record, do not claim |
| Clone checks | `git add -A` | Usual list, `bun scripts/caps.mjs`, five checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

States covered: loading (progress toasts, Retrying…), empty (no source), error (every message above),
disabled (Save with no source), hover (tooltip), keyboard focus (media and menu), permission (save
panel, network), reduced motion (toasts without movement).
Task-owned source paths: new `media-actions.contract`, `media-actions.ts`, `media-reference.ts`,
`media-source.ts` (+ tests), `timeline-attachments.*`, `r6-media-video.contract`, `markdown.contract`,
`r4-surfaces-files.*`, `modules/apple/T3ContextMenu.swift`, hunks for `T3Module.swift`,
`modules/apple/R6MediaPreview.swift`, `macos/tests/media-actions/`.
Required environment: Xcode 27.0, pinned Bun 1.4.2, oracle desktop build, Screen Recording permission
only if the user grants it for native-menu capture.

## Progress

2026-10-06, branch `feat(example)/t3-code-media-actions` (wave 3). Implementation: implemented; verification: unverified.

Built:
- Ports with the reference tests: `media-reference.ts` (mediaFileReference, mediaUrlReference, mediaReferenceFileName),
  `media-source.ts` (resolveMediaSource, classifyMarkdownImageSource, mediaMimeType and the markdownLinks helpers);
  `media-actions.ts` (mediaMenuItems, the menu flow with its toasts and the reference's error sentences, re-signing an
  asset at action time, OpenMediaLink, resolveProtocolRelativeMediaUrl); `media-views.ts` (what each surface hands the wrapper).
- `media-actions.contract` `MediaActions`: right click, Menu key and Shift+F10 (`chatlocal:media-menu`), `tabindex=0`,
  tooltip; `MediaVideoFailure` (16:9 "Video unavailable · <label>", Retry video / "Retrying…", open link);
  `FilesMedia` (Files image and video previews, new: "Unable to load workspace image."); `media-markdown.contract` (chat
  Markdown image lines: direct URLs, host paths through `media-file` assets, "Image unavailable · <alt>", videos).
- Surfaces wrapped: chat Markdown images and videos (user and assistant), the Files image and video previews, the
  expanded image/video dialog (unavailable image text, video failure with Retry), the message video tile.
- Native (`T3MediaActions.swift`, `T3Module+Media.swift`): `mediaMenu` (NSMenu at the pointer or the focused media's
  bottom-left; the agent answers `T3_AGENT_MEDIA_PICKS`), `mediaCopyText`, `mediaSave` (fetch, refusals, Save panel sheet
  as Electron's default download; agent: exports/), `mediaCopyImage` (PNG as is, else decoded and re-encoded, 64,000,000
  pixel limit; agent: a private named pasteboard).
- Rendered HTML: reproduced (AppKit test): http IP stylesheet, script and fetch, a named http host (`localtest.me`, unbundled
  binary) and a public https image load. README, AGENT-HANDOFF gap #1 and EXACT2-GAPS X7 corrected; differences declared.

Not built / limits: composer chips (excluded); Files' rendered Markdown preview media (the Files Markdown blocks get no
signed URLs; chat Markdown does); an image whose bytes fail to decode keeps its box (no `image` error event, #121);
the tooltip is AppKit's, not the code-style popup; Copy image is never "unavailable" (declared); http named host in the
bundle (#106, #135).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 implement | `ff5425066` on `9670b0723` | `bun test examples/t3-code` 1888 pass / 0 fail / 1 skip (base 1829; +59: media-reference 19, media-source 21, media-actions 19); strict tsc clean; `contract build` `[]`; `cargo test -p t3-code-macos --lib` 10 pass; AppKit `media-actions` 7/0, `r6-media` 5/0, `contextmenu` 4+13/0, `sidebar` 5/0; caps pass; five checks build/test/clippy/fmt/caps/boot all 0; bundle build ok | lane fixture under `target/lane` (not committed) | X7 (#106) for http named hosts in the bundle |
| 1 drive | AFTER `ff5425066`, BEFORE `9670b0723`, 1280×840, fixture server :16260, HTML fixture :16262 | AFTER exit 0: `View [markdown-image-actions] label="<lane>/repo/screens/logo.png" (contextmenu, key)`; `[markdown-video-actions]` → "Video unavailable · Clip", Retry video, Open in browser; Files `View [file-image-actions] … Image [file-image-picture]`; contextmenu picks: "Path copied", second relative path, Save → "Download started" (exports/logo.png sha256 064acdc2… = source), Shift+F10 → Copy image → "Image copied" (private pasteboard: PNG 480×300, 134417 bytes = source); `broken.mp4` → `[file-video-failed-label] "Video unavailable · media/broken.mp4"`, Retry re-signed and failed again; `page.html`: fixture log `GET /style.css Host=127.0.0.1:16262`, `GET /named.svg Host=localtest.me:16262`, `GET /data.json` (Origin null), Google favicon painted. BEFORE: the Markdown media draw nothing; logo.png shows "Failed to read workspace file"; its video step could not reach the tree under the provider-update toast (3 tries) | PR screenshots (before/after pairs) | real right click, real Menu key, real Save panel: unverified (attended) |


## Next action

Review the PR. Attended checks: a real right click and the Menu key on each surface, the Save panel's folder and name,
Escape returning focus to the media, reduced motion. Oracle and trace-diff rows: not run (no oracle). The "Rendered
HTML, http host" row stays blocked on X7 (#106).
