# T3 Code for Exact on macOS

A macOS client for [T3 Code](https://github.com/pingdotgg/t3code) servers, written with Exact.
Contract draws the interface; TypeScript owns the client state, the projections and the server
connections (HTTP, WebSocket RPC, the saved environments and their credentials). Exact's
Markdown editor owns composer text, selection, styling and undo; TypeScript supplies the
source's block and reference projection and guarded edit intents. A drop, a paste and Attach files
are Contract's `drop`, `paste` and file input, their files staged in TypeScript
(`composer-attach-stage.ts`), and a large text paste folds into a file in the composer's `paste`.
The terminal is T3 Code's own page in an `iframe`, with its own server socket. An app-local Swift
module (`modules/apple/`) adapts type-to-focus, modifier events, the browser web view, menus,
notifications and window chrome. Provider execution, workspaces, Git and conversation
history stay on the T3 server.
The app carries the official T3 server release (the CLI archive of the release built from the
reference pin, unmodified; `server-runtime/runtime-pin.json`) and runs it as its own local
server, "This machine". It does not modify T3 Code.

The reference is T3 Code `6d450a4284` (`referencePin` in `server-runtime/runtime-pin.json`;
the client mirrors the stable release `0.1.5`, `version-skew.ts`). Ported files name
the reference files they follow, and T3 Code's MIT notice is kept in `LICENSE-T3`.

macOS is the only target: there is no web, iOS or Linux build of this app.

## Build and run

Requires macOS 14 or later, Xcode, the repository's Rust toolchain, the Bun version from
`package.json`, and the native TypeScript Hermes toolchain described in the
[root setup instructions](../../README.md#1-install-the-tools). Run from the Exact repository
root:

```sh
bun install --frozen-lockfile
export EXACT_APP_DIR="$PWD/examples/t3-code"
bun examples/t3-code/terminal-host/build.mjs                 # the terminal page into assets/ (ignored)
bun examples/t3-code/tools/stage-runtime/stage-runtime.mjs   # the embedded T3 server into server-runtime/ (ignored; network once)
bun host/apple/build.mjs t3-code-macos --bundle --run
```

The terminal page is a build output: the Apple bundle copies `assets/` as it is and has no
step that makes an app's assets first, so without `terminal-host/build.mjs` the terminal frame
stays blank and the host logs that `assets/terminal/index.html` is not an app asset.

Generated sources are built, not committed. `tools/generate/generate.mjs` makes them from the
packages pinned beside it (`tools/generate/package.json` and `bun.lock`, the versions the
reference locks; network once) and from committed inputs: the code colours' lexers and tables
(`data/highlights.wasm`, `src/highlight/highlights-tables.ts`, `syntax-ink.contract`), the Lucide and
Pierre icon data (`src/settings/settings-b-lucide.ts`, `src/shared/pages-icons.contract`,
`src/shared/timeline-files.contract`, `timeline-files-tables.ts` and `data/pierre-icons.tsv`),
the ten icon tables (`icons`, `shell-icons`, `sidebar-icons`,
`timeline-icons`, `panel-icons`, `settings-icons`, `provider-icons`, `diff-glyphs`, `editor-icons`
and `settings-brands`, each a `.contract` in its feature's folder under `src/`, from the Lucide names
in `tools/generate/icon-tables.mjs` and the marks in `icon-marks.mjs`; their hand-written
components are in the `*-kit.contract` files), the morph plans (`src/shell/shell-morph.contract`),
the terminal buffer vectors
(`tests/terminal/vectors.json`, whose expectations are test data committed in
`gen-vectors.mjs`), and the document viewers' page (`assets/file-preview/`: the reference's PDF.js,
large-text, Office, OpenDocument, notebook, archive and SQLite viewers, bundled from
`file-preview-host/src/` with their wasm, fonts and licences by `gen-file-preview.mjs`; 25 MB, the
wasm, fonts and CMaps gzipped, so the bake's 64 MiB capture of the app's sources and assets holds it).
The app build runs it before it bakes (`data/build.rs`);
run it yourself before the tests and checks below:

```sh
bun examples/t3-code/tools/generate/generate.mjs           # writes what differs; --check fails instead
```

The app is its own Cargo workspace (`Cargo.toml` here, building into `examples/t3-code/target`),
outside the root's members: its build script bakes the whole app on one core for minutes, so
exact2's six blocking checks leave it out and the async lane (`scripts/async.mjs`'s
`t3-code` and `t3-code-clippy` steps) builds, tests and lints it.

The embedded server. `tools/stage-runtime/stage-runtime.mjs` downloads the release archive named by
`server-runtime/runtime-pin.json`, accepts it only when its SHA-256, the release's `SHA256SUMS`
and the pin agree, checks the signatures of its Mach-O files, runs `t3 --version`, starts it once on
a scratch home and a lane port, and puts the archive and its manifest beside the pin (`--offline`
reuses the cache in `.runtime-cache/`). The bundle carries that folder as
`Contents/Resources/t3-runtime` (`app.json` `host.macos.resources`). At the first launch the app
unpacks it into `<T3 home>/runtime/versions/<version>` and starts `t3 --bootstrap-fd 0` with the
desktop envelope; it restarts the server with T3 Code's backoff, keeps its failure log in
`<T3 home>/userdata/logs/server-child.log`, and stops it when the app quits. Closing the window
(File › Close Window, the red button) does not quit: `app.json` sets
`host.macos.window.keepRunning`, so the window hides and the app, its session and the server keep
running, as the reference's (`DesktopLifecycle.ts`: `window-all-closed` quits only off macOS). The
reopen event (a Dock click, `open -a`) and a `t3 app` request show the window again; with it
hidden, ⌘Q quits at once, as the reference's menu Quit does with no page to hold it. The reference
builds a new window on the most recent project's draft, its other threads' drafts kept. The clone
opens that same draft on a close/reopen, using `exactPage().restores`; minimizing or hiding the app
and showing it again keeps the current thread and draft. A development build
starts it only with an isolated home and a lane port, never `~/.t3` or 3773:

```sh
T3_LOCAL_HOME=/tmp/lane/t3-home T3_LOCAL_PORT=16437 bun host/apple/build.mjs t3-code-macos --bundle --run
```

Without both variables its status is `refused` and nothing starts. Only a distribution bundle with both the exact `t3code` `/open?u=%s` handler and its
registered bundle scheme uses `~/.t3` and the port scan from 3773. The app manifest marks the
handler `distribution_only`, so ordinary development and `--release` builds remain refused
without their isolated variables. No `distribution.json` marker is required or accepted for
home admission.
An agent drive (`bun scripts/agent.mjs macos --size 1280x840 --storage <name> …`, with `EXACT_APP_DIR`
and both variables set) runs the development binary, which carries no bundle resources and so
no server: build the bundle, then point `EXACT_MAC_BIN` at its `Contents/MacOS/T3 Code (Exact)`
(the driver then skips its stale-build check, so rebuild after every source change).

Safari's Web Inspector (exact2 #101): every web view the clone shows is an Exact `iframe` (a Browser tab, a
rendered HTML preview, the terminal, a Mermaid diagram), which main #309 makes `isInspectable` in a development
build, so Safari's Develop menu lists it, and never in a production-trust bake or a distributed bundle. A Browser tab's
⋮ › Open DevTools opens its page's inspector in any build (`openDevTools`, LLP 1020.002 D11). There is no
View › Toggle Developer Tools: an inspector for the app's own UI stays out of exact2 (#101).

## Desktop command

An installed distribution exposes **Settings → General → t3 command**. Install creates the
launcher in `<T3 home>/bin/t3` and an owned link in a writable command directory; Remove
removes only links written by T3 Code. Another `t3` earlier on PATH is preserved and reported.
The launcher passes `T3CODE_CLI_PATH`, preserves an explicit `T3CODE_HOME`, and invokes the
app's installed server executable. Development builds hide this row. Availability uses the
app's distribution-only handler and scheme metadata, shared with the local backend's
home/port admission policy described above.

The desktop bootstrap secret stays in native memory and the server's stdin envelope.
Its HMAC credential changes every twelve hours, including a fresh exchange after suspend.
Project icon selection also accepts macOS `.icns` files; the embedded server converts them.

## Connect to a server

Start an existing T3 installation with `t3`, or use its normal source-checkout startup command,
or use the embedded server ("This machine"). Configure and authenticate at least one provider in
T3 Code. This client implements orchestration protocol 2 and requires the server's
`serverResolvedCommandContext` capability for writes. A server on another protocol is listed as
outdated or newer; one that can update itself offers **Update** in Settings › Connections. A
connected server older than this client shows a warning in the thread details card, and the
sidebar's update pill offers its update; a notice above the composer appears only while the update
runs or after it failed. Update streams the update's progress, waits for the server to come back
on the new version and shares that state with Settings › Connections
(`server-update.ts`). The sidebar's Update button combines one-click provider and remote server
updates in one confirmation, with per-machine progress and Retry. Provider installers finish
before servers restart; a slow server keeps its own progress without holding up the other
machines. Servers installed from npm appear after the target release is published on npm.
The app has no desktop self-update feed, so its own download/restart step is absent.

1. In T3 Code, open **Settings → Connections** and create a fresh pairing link.
   A command-line installation can also use `t3 pair` for a running server.
2. In this app's welcome wizard (or **Add environment**), paste the pairing URL.
3. Wait for synchronization, then pick a project and thread, or add an existing
   workspace by its absolute path **on the server**. Adding a project does not create
   the directory.
4. Choose a model and send. New threads use the project override or the server
   default for permissions, workspace and interaction mode.

## What it covers

- Prompt links: packaged builds register `t3code://new?prompt=…` and
  `t3code://threads/<environment>/<thread>`. Links fill a draft and never send it;
  hidden characters are removed with a warning, and replacing a draft or checking
  out a pull request requires a choice. The palette and user messages offer Copy
  prompt link. Development and lane bundles register no scheme; test the handler
  through `/open?u=<percent-encoded-link>` directly.
- Web links: packaged builds also claim `http`, `https` and HTML files, so a person can
  pick the app in System Settings › Desktop & Dock › Default web browser; the app never
  asks to become the default. A link or a page opens as a new Browser tab beside the
  thread on screen when its panel shows a Browser tab, else in a new thread without a
  project with the panel maximized (`prompt-link-handler.ts`).
- Sidebar: grouped projects, Working/active/Settled/Snoozed shelves, search, pin,
  settle/un-settle, snooze (presets and custom), Woke and Done pills (Woke dismissal
  syncs through `thread.visit`), project glyph overrides and favicons, Project order,
  rename and confirmed removal of project entries and groups, drag between shelves.
  Pressing a row's Settle, Un-settle or Wake button and dragging applies it to every row of
  that section between the press and the pointer ("Settled 3 threads, ⌘Z to undo";
  `row-sweep.ts`, reference 1826fb55cc). Draft rows have the reference's context
  menu (Copy ▸ Path / Branch, Project settings, Discard draft), and discarding a draft from
  the menu or an X goes behind the undo notice ("Discarded N draft(s), ⌘Z to undo"; Undo
  restores the text, images and worktree choice; `draft-rows.ts`, 95edeb753b). The
  version pill shows only when it fits beside the brand (9efb016900).
- Conversation: bounded history with older-page loading, streaming assistant and
  tool rows, work-log inspector, checkpoints and changed files, fork dividers, answered
  questions, mention and file chips, Markdown with code blocks (wrap, copy) and Mermaid
  diagrams: each closed fence, also while its reply streams, renders in the Mermaid page
  (`assets/mermaid/`, an `iframe` per fence, LLP 1020.001) with the pele and DOMPurify builds
  the reference locks, which `tools/generate/` vendors into `assets/mermaid/vendor/`; pele lays
  a diagram out for the lane's width. With Settings › Appearance › Render math on, a display
  formula (`$$…$$` or `\[…\]` opening and closing a line, or a ```math fence) is typeset by
  the reference's KaTeX in the math page (`assets/math/`, an `iframe` per formula); inline math
  stays TeX source until Markdown text has an inline atom (draft PR #888). A failed workspace
  preparation offers Retry (`prepared-run.retry`), and the failure row hides once a retry
  supersedes it (`workspace-retry.ts`, 737993303d). Code is coloured by the reference's lexers,
  @pierre/highlights 0.2.0, which the data crate runs (`data/src/highlights.rs`, `highlights.ts`).
- Composer: send, stop, queue and steer while running (queued messages wait at the end of the
  timeline, with Send now, edit and remove; with no turn running, Send now starts one),
  model picker with search and favourites, reasoning and runtime options, Plan/Build,
  slash and @ menus, file and image attachments (videos as first-frame tiles that play in
  an expanded preview; removing an image the prompt references asks first and removes
  every reference), stash, multi-model drafts that start one worktree thread per model,
  the Compact chip for a stale Claude session, `/usage-limits` (each account's windows as bars with
  pace and countdown, Manage usage, banked reset credits with "Use reset" behind a confirm;
  `usage-limits.ts`, `usage-bars.contract`) and Codex `/feedback [reason]` (uploads the
  thread with a banner, Copy ID and two local rows; `composer-feedback.ts`). Both long
  writes are detached requests whose reply joins the inbox (`composer-replies.ts`). The composer overlays the transcript, which keeps a
  measured reservation at its end (`composer-overlay.ts`); with Chat width Wide or Full
  the context strip's workspace control is the desktop Select. PNG, JPEG and HEIC images over
  2000 pixels on their long edge are scaled to that edge before upload; PNGs already within it keep their bytes. An
  image uploads as it is attached, three at once, its percentage on its tile while the prompt is typed, and the
  uploads go on in other threads (`composer-uploads.ts`). Send and Return wait for them; a send that goes anyway (the
  Browser's annotate and send) waits for them itself. Their attachments last the session, so a relaunch uploads the
  draft's images again; other files upload at send. With Markdown formatting on, each line of the prompt reads in the
  direction of most of its block's words (`composer-text-direction.ts`); with it off, each line by its first strong
  letter, as Apple's plain fields read.
- Requests: approvals, single/multiple-choice and free-text questions. While an approval is pending the
  composer stays usable: Send declines the approvals it shows, then steers the turn with the message
  (`declineForRedirect` in `client-ops-composer.ts`), or, for an agent that reads feedback, declines the
  approval with the message as its feedback; a draft edited meanwhile stays unsent. An approval that full access
  still asked for says why. A typed answer
  stays with its thread until the question resolves. While the composer answers a question the server lets
  carry attachments, its shelf is that question's (`question-attachments.ts`): a dropped, pasted or picked image
  joins it without a chip, as does a SnapShot taken while the question is open (pinned when the capture is
  first seen); a question whose shelf is still staging files stays unanswered; the answer uploads each question's
  images beside its file chips, its shelves taking and giving up nothing meanwhile, and the drafts go when the
  request resolves.
- Version control in the workspace card (`src/details/git*`): the branch picker (search, create
  a ref with Enter, check out, "Start from origin"), the git actions quick action and
  menu, the commit dialog (file selection, message, "Commit on new branch"), the
  default-branch confirm, progress with hook output, the 10 s inline success, pull,
  the Publish repository wizard, the "Run on" picker across environments that share a
  repository, and the new-worktree start flow. On a local draft a ref search that parses as a
  pull request reference (`#42`, `42`, a pull request URL, `gh pr checkout 42`) leads both
  pickers with "Checkout pull request", which opens PullRequestThreadDialog
  (`pr-checkout.ts`, `pr-checkout.contract`): `git.resolvePullRequest` per edit, then
  Local or Worktree through `git.preparePullRequestThread` (Worktree passes the draft's thread
  id), and the draft moves onto the checkout. As in the reference, the field's text is selected
  when the dialog opens, each edit's lookup starts at once, and "Resolving pull request..."
  stays until 450 ms after the last edit before the answer or error shows
  (`command-waits.ts`).
- Right side: the workspace card (docked beside the chat from 984 pt, a header popover
  below), Changes (branch) and Uncommitted diffs plus per-turn diffs, lineage with
  merge-back, the surface chooser and a tab bar per thread (`src/panel/`): Files
  (the workspace tree with search and expand-all, which leaves ignored folders such as `node_modules`
  to open by hand, file previews with breadcrumbs,
  rendered Markdown/CSV and an editor that writes back with `projects.writeFile`; opened
  from Go to file, content search and file chips: a chip written from the workspace root as a bare name, or
  with a subfolder project's own folders repeated, opens the file the workspace search finds, and a `~/` link
  opens as a host file the server expands, `workspace-file-lookup.ts`; its explorer and rendered/source
  choices persist; an `.html` file opens rendered in a sandboxed `iframe` (its sibling stylesheets, scripts and
  images load through the signed asset URL's token directory, and the page may load stylesheets, scripts,
  images, fonts and fetches from other hosts as the reference's sandboxed frame does (https, and `http` to an IP
  address or a named host: `app.json` allows arbitrary loads in web content, #106), `media-frame.*`)
  from a signed asset URL, with the reference's Show HTML source / Show rendered page toggle,
  `files-rendered-html.ts`; every right-panel tab (Files, files, the
  pull request list, the Diff, the Device surface with its device, pull request details and
  attachments) is kept per thread across launches, `saved-panel.ts`, `saved-surfaces.ts`;
  diffs keep the project's requested cwd, including projects outside the server's launch directory), Linked pull requests (rows, Copy link, Open, Watch for changes / Stop
  watching, Unlink), the Pull request surface (`pr-surface.ts`, `pr-attachment*`: the thread's or a link's
  pull request in the Pull Requests page's detail panel, opened from the chooser's P, the
  details card's `#N` rows with "Show N more", and pull request links; `pr-row*`: with the
  host's detail the row splits into its state glyph, the checks segment and its popover
  (attention and running checks, "Show all", Details links), the tooltip card (state,
  base ← head, checks, files and diff size) and the one action worth taking: Resolve and
  Fix check the pull request out into a worktree with `git.preparePullRequestThread` and
  leave the task unsent in the project's draft (`draft-thread-id.ts`: the draft's thread id
  is allocated first and passed along, so the server runs the project's worktree setup
  script for that thread, and the draft later launches under the same id, across a
  relaunch), Ready and Merge (after "Merge pull
  request?") run `pullRequests.runAction`; a pull request tab without a linked snapshot
  takes its state from the loaded detail; the composer's context strip shows the thread's
  pull request chip, or a hand-off draft's from its branch status, with its tooltip list,
  +N and stack forms, `strip-pr-badge.ts`), sent attachment
  previews (Markdown, table or numbered source, images, Copy contents and Save file;
  opened from a sent file chip; the text is fetched from the signed asset URL, and Save file
  hands the bytes to the window's Save panel, `attachment-files.ts`, `save-file.ts`;
  `attachment-media*`, `media-frame.*`: PDFs in the viewer page's PDF.js (`file-preview-host/`), a draft past 16 MiB
  given to it as a copy in app:/cache (LLP 1020.001 D7), HTML in a sandboxed `iframe` that loads its token directory
  and external hosts as the reference's frame does, or as source coloured by the reference's html lexer
  (`highlights.ts`), audio and video in the platform's players, "Unable to
  load audio/video." with Try again) and Device (the "Set up devices" wizard over
  `device.configure` / `subscribeDeviceState`; Escape closes it; after onboarding a row
  opens its simulator with `device.open`, then the workspace shows the hub's H.264 screen
  decoded with VideoToolbox (iOS: serve-sim's AVCC body, MJPEG when the description cannot be
  decoded; a sustained decode backlog or stalled body resubscribes at a fresh IDR and keeps the last frame and 3D viewer, `T3DeviceDecodeBacklog.swift`; Android: serve-emu's SEMU-framed socket) as the 3D phone (SceneKit over the
  connected server's own device models, never bundled; a procedural body for other devices)
  or flat, with Home, Rotate (Android: Back, Recents and a Portrait / Landscape menu),
  appearance, text size, the Tools drawer (foreground app, open URL, launch, terminate,
  Simulator / Emulator settings, accessibility frames over the screen, location, permissions,
  push, the event log), Save screenshot, Float over chat, Close, Power off, 3D / Flat, the
  iPad's Magic Keyboard and Restore 3D view; the focused screen forwards keys (HID usages on
  iOS, key codes and text on Android), `device-session.ts`, `device-tools.ts`,
  `T3DeviceClient.swift`, `T3DeviceVideo.swift`, `T3DevicePhone.swift` and their siblings; the iPhone Duo's hinged 3D viewer on the fixed cover / inner panel feeds with its Fold shape and
  Device stance stands, and a foldable emulator's Fold / Unfold over serve-emu's `/api/fold` with its procedural
  hinged body, `device-duo.ts`, `T3DeviceDuo*.swift`, `T3DeviceFold.swift`). Sent videos are 4:3 tiles in the message's media grid;
  the tile and the video chip open the media dialog, playing.
- Pages: Pull Requests (list, detail, checks, copy actions), Usage (cost, tokens,
  limits, share bars that follow the selected metric, model dialog, custom model prices and
  Map to), welcome wizard. The palette's "New thread without a project" and the new-thread
  heading's No project open the machine's folder for threads without a project the same
  way (`scratch-project.ts`, 845ddd9354).
- Pull request detail: besides the Summary and Timeline (a review thread's replies under its
  finding, a page of conversations at a time), the Code tab (`pages-pr-code*`: the change or one
  commit's change a slice at a time, read again with the loaded slices kept when the server
  announces a change; Viewed ticks, line comments into the review being written, or to the agent
  where the reader may not comment; the host's conversations on their lines; what a failed diff
  read means, with Try again), hand-offs and quick actions (`pages-pr-handoffs*`,
  `pages-pr-quick*`) and the thread links (`pages-pr-links*`). A thread's pull request surface
  keeps the tab it was left on. Every +/- splits into code, tests, mocks, generated files and docs
  (`pr-change-breakdown.ts`, the server's `pullRequests.changeBreakdown`): a list row's on its first
  hover, the detail's at once (drawn by the window's hover layer, since the header folds), the
  details card's inline under the diff size.
- Terminal: the thread terminal drawer below the chat column (`terminal.contract`,
  `terminal-drawer*.ts`; T3 Code's ThreadTerminalDrawer in its drawer mode): sessions on the
  server, split and close, the drawer's state kept per thread. Each pane is T3 Code's own
  Ghostty emulator page (`terminal-host/`, built into `assets/terminal/`) as a bundled page in an
  `iframe` (`terminal-frame.contract`, LLP 1020.001). The page holds the session and its buffer and
  opens its own server socket with a ticket the transport mints for the terminal's environment
  (`terminal-host/src/session.ts`, `terminal-frames.ts`), as the reference's renderer holds the
  attach stream, so output never passes a commit. A pane shown again (a thread switch, a remount)
  attaches again and gets its history back from the server's attach snapshot. The theme, font,
  keybindings and visibility go to the page by `postMessage`; the selection popup and the context
  menu are a menu-shaped popover at the point the page names (an NSMenu).
  ANSI colours use VS Code's palette for the scheme. Shift+PageUp and Shift+PageDown page through
  the scrollback, ⌘⇧Home and ⌘⇧End jump to its start and the latest output (full-screen programs
  keep these keys), and ⌘A or the context menu's Select all selects the whole history; Jump to
  latest returns to the current output. The divider drags are native, outside the page, so the
  view forwards each mouse release in the window to the page, which sends the settled grid to
  the shell at once, as T3 Code's window listener does.
- Browser surface: a right-panel Browser tab is a partitioned `iframe` (LLP 1020.002; `browser-frames.*`,
  `browser-*.ts`; T3 Code embeds Chromium, which exact2 does not offer, #100): tabs and the chrome row, history and local servers, the address bar's search
  engine, zoom, appearance, the device toolbar, Annotate, screenshots, recording, downloads, a separate window and the
  floating player, profiles, and an agent's `previewAutomation` requests.
- Command palette (threads, projects, actions, files, content search, New thread in…,
  Link pull request), toasts, the Nightly mobile-app notice, macOS notifications (after
  the user grants them).
- Settings: the reference's routes with project/environment scope, search, theme editor and
  VS Code theme import, keybindings, providers (sign-in and runtime install; a ChatGPT sign-in's
  callback comes back to `openAuthSession`'s loopback receiver on the port T3's server chose, LLP
  1133.005 D9, `codex-setup-ops.ts` `receiveCallback`), connections (pairing, SSH, outdated-host
  update), scheduled tasks, SnapShots, diagnostics and licenses.

Return sends; Shift-Return inserts a newline, continues a Markdown list (renumbering the numbered
items below) or leaves an empty item or quote line, with rich text on or off; IME marked text stays an
editor action. A bracket or quote typed over a selection wraps it.
⌘↩ sends from the button; in a new-thread draft it starts the thread in the background
when the server binds `composer.sendBackground`; ⌥⌘↩ on an existing thread sends and
opens a fresh draft in the same project. With Settings › Send shortcut set so that Return needs ⌘,
⌘↩ follows the Follow-up behaviour setting while a turn runs and ⌘⇧↩ takes the other. Other chords follow the server's keybindings
(⌘K palette, ⌘N new thread, ⇧⌘M model picker, ⇧⌘K copy PR number, ⌘B sidebar). ⌘B bolds instead
while the rich-text composer has the focus, as Tiptap does: a click into an existing draft
counts as focus (Exact fires the field's `focus` on the click), and ⌘B / ⌘I end
a composing syllable first and match by key code under a non-Latin source such as Korean 2-Set. Outside
the composer a ⌘ or ⌃ letter chord under a non-Latin source is re-issued with its key's Latin
character, as the reference's `resolveEventKeys` does (`T3LatinChords.swift`), so ⌘B and ⌘K
still match. exact2 #168 matches declared chords by physical key itself, and AppKit matches the
menu's own items by the key's Latin letter, but neither reaches the module's own key monitors, so
the re-issue stays. After the thread list re-renders or scrolls under a still pointer, the row that
slid under it is hovered, as a browser's synthetic mouse move does (the host's own, exact2 #139). The menu bar is the reference
desktop app's (DesktopApplicationMenu.ts), built from main's menu hints (LLP 1021 §5.3.1): the app
menu has Check for Updates… directly under About and Help holds it alone (two app-wide Contract
buttons, `-exact-menu="about"` and `"help"`, enabled under a sheet or with no key window; settings-shortcuts.contract
`UpdateMenuItems`), File shows only Close Window, View starts with Reload and Force Reload (two
Contract buttons the host files there, `-exact-menu="view"`, which run `reload()`; `ViewReload`)
and then Actual Size ⌘0, Zoom In ⌘= and Zoom Out ⌘- (the app's own commands, which take the host items'
places, stepping by half a Chromium zoom level as the root font size; `ViewZoom`), and the window's root removes the Develop
and Go menus, the help book and Edit's template items (`-exact-menu-remove`, app-window.contract), so
⌘D (diff), ⌘O (open in editor) and ⌘1–9 reach the window. Every button chord is a shortcut with no
item (`-exact-menu="none"`) but one that stands in for a host item (⌘Z, ⌘,), one dispatch button
per chord (⌘N and ⇧⌘O both start a thread with no text field focused). While the sidebar's undo
notice claims ⌘Z, thread.undo stands in for Edit › Undo. ⌘W closes the active right-panel surface
tab (its neighbour becomes active; the last tab closes the panel; on the Pull Requests page the open
pull request) or the focused terminal, and never the window: File › Close Window has no key
equivalent, as in the reference, and AppKit's Close All goes with it (the window root's
`-exact-menu-keys` and `-exact-menu-remove`, LLP 1021 §5.3.3). The root's capture handler hears ⌘W
ahead of the menu (`app-window.contract` `modifierDown`) and prevents one that closes nothing, which
AppKit would answer with a beep; a held ⌘W's repeats are dropped.
The Quit shortcut setting's hold (Hold ⌘Q or press twice) is the window root's `key` handler, ahead
of the menu's Quit (`quit-hold.contract`). Edit's Paste as Text (⇧⌘V) stays the module's
(`T3Menus.swift`): it is the responder chain's `pasteAsPlainText:`, which a button's press is not. The surface launcher takes the focus when it mounts (`autofocus`), and
its letters are its rows' `aria-keyshortcuts` (`shell-panels.contract`), which Exact presses before type-to-focus; a Markdown table's Copy
menu is a window-level popup, placed from the trigger's `frame()` (scroll offsets applied),
that Escape and an outside press close (`table-menu.*`). The row context menus are Contract context
popovers, which the host presents as an `NSMenu` at the pointer, or for the ContextMenu key at the
focused row's centre (exact2 #314): the sidebar's thread, draft and selection menus, the Files
tree's, a panel tab's, a pull request number's, the branch picker's Copy branch name (on its trigger
and on each ref in the open picker, which stays open under it), and the chat's web and file link
menus (`link-menus.contract`; their rows are computed before the right-click,
`external-link-menu.ts` and `file-link-menu.ts`). A draft row's ContextMenu and Shift+F10, and a
tab's Shift+F10, show theirs from the row, below it. A mailto, irc, xmpp or fragment link has no
menu of its own, so its right-click and its ContextMenu key get the desktop shell's
(`T3TextContextMenu.swift`). The media menu stays the module's (`T3ContextMenu.swift` says why). A
file link on the pull request page shows its menu where the press was (LLP 1021 D2: the press's
`clientX` and `clientY` as the popover's `left` and `top`; a key's press is at 0, 0, the window's
corner, as the reference's `Menu.popup` puts it). A global hotkey of another app (a launcher holding
⌘1, say) never reaches the window.

Saved environments and their credentials are one kept secret, `t3-catalog` (granted by `secret.keep
t3-catalog`, kept in the Keychain on macOS; `transport-catalog.ts`), each token scoped to the server
origin and environment. The data module atomically saves the versioned preference file, `app:/data/t3-code.json`
(selections, drafts, sidebar and page preferences, dismissed notices, pending operation identities), through
Exact's `storage.fs` (`protocol.ts` `preferenceFiles`); a drive that names no scratch store keeps none of it. **Disconnect** keeps the credential; **Forget**
removes it. The embedded server is the primary environment, "This machine"
(`local-primary.ts`): the client connects to it with the
bearer the server issued at start (memory only, never kept), never saves it and never
remembers its origin (its port can change every launch; the transport keeps a focus token
`primary` beside `t3.server.origin`). On launch the client reconnects by itself: to the primary
when it was the last focus, else to the last switched-on saved environment with its saved
credential, else to the primary while the Local environment switch is on (`relaunch-reconnect.ts`);
while the server starts, the window shows the connecting state (the host opens the first window
before the server is ready, exact2 #117). A launch reopens no thread: the reference's window loads `/`
every time (DesktopWindow.ts, ElectronProtocol.ts `getDesktopUrl`), whose IndexDraftLanding opens a new
draft in the most recently active project (by latest user message; a visit is not activity) of the
environments whose shells are in, without waiting for a slow one; only a workspace with no project yet
waits until every switched-on environment's shell is in. This machine is always in that first pick: with the
Local environment switch on, the reference creates its window only once the local backend is ready
(DesktopWindow.ts `createMainIfBackendReady`), so the clone's landing waits for the local environment while
its backend is starting or its shell is not in yet. The clone does the same
(`home-landing.ts`), focusing that project's environment when it is not the one the launch reconnected.
The window keeps its frame across launches (the host's frame autosave, restored after the window's final
style since exact2 #113). Settings ›
Connections shows "This machine" (`this-machine.ts`): the Local environment switch, which asks
first, persists the setting and then relaunches the app through Contract's `relaunch()`, as the
reference does, and the Version row. Under it (`connections-network.ts`) are Network access ("Reachable at <url>" and its endpoints; the server
binds 0.0.0.0 while it is on, loopback otherwise, and refuses when no LAN or Tailscale IPv4
exists), Tailscale HTTPS (the server runs `tailscale serve` on the chosen port; the app reads
`tailscale status --json` only while network access or Serve is on, cached 60 s per set of
Tailscale IPv4 addresses, `T3LocalNetwork.swift`; an address in 100.64.0.0/10 is a Tailscale IP
only on Tailscale's adapter or when that status lists it), and the Authorized clients fold: pairing links with their permissions,
expiry, Share (endpoint choice, link, code, QR; HTTPS endpoints pair through the hosted app,
`pairing-urls.ts`) and Revoke, and the paired clients live from `subscribeAuthAccess`
(`auth-access.ts`). A change that needs a relaunch is persisted before Contract calls `relaunch()`;
the module's teardown stops the old embedded server. No drive has checked the relaunch of a
normally launched app.
The Local environment switch, Network access and Tailscale Serve (enabled, port) live where T3 Code
keeps them, `<T3 home>/userdata/desktop-settings.json` (`T3DesktopSettings.swift`: read once at the
first session, sparse compact writes through a temp file and a rename), so the clone and T3 Code
share them; the keys an older clone kept in `t3-code.json` are carried over once. The default
endpoint stays in `t3-code.json` (the reference's renderer localStorage); a created link's
credential stays in memory.
While the embedded server runs, `t3 app <dir>`
(the server's own CLI; T3 Code's `t3`, or `<T3 home>/runtime/versions/<version>/t3`) reaches the app
on `<$TMPDIR>/t3code-<uid>/<24 hex of sha256(<T3 home>/userdata)>.sock` (`T3AppControl.swift`,
`desktop-activation.ts`): the app comes to the front, adds the folder as a
project of This machine when it is new and opens its draft once the primary is connected and
loaded, and the CLI prints `Opened <dir> in T3 Code.` or the reference's error code. A lane build
listens for its own home (`t3 app <dir> --base-dir "$T3_LOCAL_HOME"`). Every paired server, a loopback one included, is a
saved environment under Environments with its switch and row menu (Icon, Copy trace ID, Remove
from this device…); a saved one with the primary's environment id (the same T3 home paired before)
is removed with its credential and GitHub sharing trust, as the reference's registry does (a window
focused on it moves to the primary), and pairing this machine's own server saves nothing. Load balancing and GitHub sharing count environments as
the reference's `loadBalancingEnvironments` does: this machine first, then every switched-on
saved environment; both sections show from two (`machine-list.ts`). Every running thread
of every connected environment keeps its detail stream (`keep-alive.ts`), so opening one shows it
at once. A pairing that
fails saves nothing: no row, no catalog entry, no remembered origin (the transport remembers an
origin only once its socket opens), and a failed first connection falls back to the saved
environment it replaced. A successful Add environment with nothing connected stays on
Settings › Connections with the reference's "Backend added" toast, and a pasted pairing URL
fills both Host and Pairing code (`pairing-fields.ts`); a failed pairing with nothing
connected leaves no client state naming its origin. A stored `onboardingCompletedAt` older than T3 Code itself (the 1970
values of older builds) reads as unset, so the real time is stored (`onboarding-time.ts`).
After a lost connection the client follows the reference reconnect ladder
and resynchronizes before permitting writes. A submission with an uncertain result keeps
its command identifiers and draft and is never retried automatically.
Providers follow the selected Settings environment. With a project or checkout selected, the
provider switch writes that project's checkout overrides; Reset uses the environment setting.
Provider update details show live installer progress and failures. The Models section can rename
built-in models and stores favorites, visibility, and order on that environment. Legacy device
preferences seed only an unset server list; an explicit empty list stays empty.

The model picker explains models that need a newer CLI and links to Provider settings. Workspace
skills refresh after five minutes. Muse Code is available in Add provider; Usage includes Pi
sessions and Antigravity quota groups. For a local ACP executable, choose ACP Registry → Enter
manually → Local command, enter its path, and add one literal argument per row. Secret fields
show Configured; enter a replacement or use Clear. Provider editor questions keep their prefilled
answer as literal text, including whitespace and an intentionally cleared answer.

## Known differences and framework gaps

The clone follows the reference wherever exact2 allows it. Where it cannot, the gap is an
exact2 issue. The open ones are records under main's `issues/` (the GitHub issues the clone
filed were moved there or closed). The gaps someone using this app runs into:

- Popovers flip near a window edge only as far as main's first slice goes
  ([`issues/20261009-popover-css-flip-fallbacks.md`](../../issues/20261009-popover-css-flip-fallbacks.md),
  #112). The clone's popovers, hover cards and colour picker use its `position-try-fallbacks`;
  the record stays open for the rest of the placement work.
- A data module's timer lives inside the answer that armed it (LLP 1027.000 D1), so the waits in the
  middle of a flow (View source's and Edit from here's polls, the Browser's paint and capture
  timeouts, the debounces) are the flow's own, on the app's clock. Natively a timer raced against a
  host call its answer awaits runs only once that call lands (main's `QUEUE.md`), so Annotate's 5 s
  crop deadline and a recording's capture timeout wait out a capture that hangs, where the
  reference gives up. A flow whose answer is let go while it waits on a timer stops there, its
  `finally` unrun, so each flow that holds state across a wait has a send no other flow replaces
  (`app.contract`): View source, a transcript chip's or the composer's (`citationShown`); the
  Browser's Capture, whose Record start waits for two frames (`capturePressed`, queued); the
  checkout dialog's edits, whose next edit takes the debounce over (`prCheckoutTyped`); and the
  prompt links, whose switch to another machine's Scratch project polls (`fragmentChanged`, queued).
  Time is still an argument, and work outside an answer (the transport's tick, the sidebar's
  graces) is Contract tasks.
- Text a platform view draws keeps AppKit's font smoothing (LLP 1019 D12): with Font smoothing on,
  the composer's and the fields' typed text is a shade heavier than the reference's, which draws
  all of its text `antialiased`.
- View's zoom scales the root font size, so every `rem` and inherited text size, but not `px` lengths or
  the module's native views, where Electron's page zoom scales the whole page.
- A completed ⌘Q hold leaves the window as it is until the keys go quiet: the reference leaves full
  screen and turns the window transparent first (concealPendingQuitWindow), and Contract has neither.
- Edit › Undo chosen from the menu while the undo notice claims ⌘Z is thread.undo, also with a field
  focused, where the reference's menu item undoes the field's text (its ⌘Z is thread.undo then too).
- Check for Updates… shows in every build. In a build with an update store its press would do nothing (the
  module has no hook into the host's check); no build has one while `deploy.store` is `"0"`.
- An external open the system cannot perform is not reported: Contract's `openURL` returns nothing, so the
  reference's "Unable to open link" and "Could not open the sign-in page" toasts do not appear.
- A backdrop blur sees only its parent ([`issues/closed/20261010-macos-backdrop-beyond-parent.md`](../../issues/closed/20261010-macos-backdrop-beyond-parent.md);
  main keeps the limit), so the composer is opaque where the reference's glass shows the
  transcript through it.
- A ChatGPT sign-in's browser page, once the callback is in, reads "Return to T3 Code (Exact)" and "You can close
  this tab." (the host's page, LLP 1133.005 D9), where the reference's adds "Your sign-in response has been received.
  T3 Code is finishing the connection."
- Two ChatGPT sign-ins wait at once, each on its port, as the reference's do; a third waits for one of them to end
  (the window's two receivers, `codex-setup-host.ts` `LOOPBACK_SLOTS`), where the reference receives any number.
- The notification sounds are WAV copies of the reference's two MP3s, since `playSound` reads WAV.

Main has closed records the clone was written around. The clone now uses these: a process
relaunch for the Local environment switch, Network access and Tailscale HTTPS (`relaunch()`,
[`issues/closed/20261009-native-process-relaunch.md`](../../issues/closed/20261009-native-process-relaunch.md)),
`border-style: dashed` ([`issues/closed/20261009-dotted-dashed-border-style.md`](../../issues/closed/20261009-dotted-dashed-border-style.md)),
`focusin`/`focusout` ([`issues/closed/20261009-bubbling-focus-events.md`](../../issues/closed/20261009-bubbling-focus-events.md))
and capture-phase key handlers for the keybinding recorder and the reply's Markdown
(`keycapture`, [`issues/closed/20261009-capture-phase-key-events.md`](../../issues/closed/20261009-capture-phase-key-events.md)),
and `repeating-linear-gradient()` with length stops for the usage bars and the diff's deletion bars
([`issues/closed/20261011-repeating-hatch-across-list-rows.md`](../../issues/closed/20261011-repeating-hatch-across-list-rows.md)),
and `-exact-menu` for View › Reload and Force Reload ([`issues/closed/20261009-contract-menu-bar-extensions.md`](../../issues/closed/20261009-contract-menu-bar-extensions.md)),
and the notification click ([`issues/closed/20261009-notification-default-click-design.md`](../../issues/closed/20261009-notification-default-click-design.md)):
`@exact/notifications` (LLP 1133.004) posts the thread and review guide notifications from the data module
without a sound, its click's `navigate` location opens the thread (`app.contract` `notificationOpen`), and it
holds the Dock badge and the permission's fact and request (`shell-notify.ts`); their sounds are the reference's
two, played with `playSound`.
It also uses an atom face's `ariaLabel`, which names each composer chip with the reference's `aria-label` while an
image chip's drawn label follows its upload
([`issues/closed/20261030-an-atom-is-named-only-by-its-drawn-label.md`](../../issues/closed/20261030-an-atom-is-named-only-by-its-drawn-label.md);
`composer-chip-presentation.ts` `chipAccessibleName`); a change to the opacity a view was mounted with, which is
now drawn, so a freshly saved EAS host's Edit and Remove draw `disabled:opacity-64` and then 1
([`issues/closed/20261030-a-node-mounted-at-one-opacity.md`](../../issues/closed/20261030-a-node-mounted-at-one-opacity.md));
`resize="vertical"` on each textarea Tailwind's preflight gives a grip
([`issues/closed/20261030-textarea-resize-refused.md`](../../issues/closed/20261030-textarea-resize-refused.md));
loopback sign-ins side by side, where a callback with no `code` or `error` is answered 400 and the receiver keeps
listening (#1169, LLP 1133.005 D9); and a prop or style binding that traps is unset while the runner goes on
([`issues/closed/20261030-binding-trap-poisons-the-runner.md`](../../issues/closed/20261030-binding-trap-poisons-the-runner.md)).
A code block's tokens and a find line's parts still carry their offset `at`, counted once by their producers: summed
per row, as the compiler's `lower-row-own-list` warns, a long highlighted block's later rows would pass the runner's
list-step bound and lose their find and citation marks.
These it does not use, because on macOS they do not do what the reference does:

- `showModal(id)` / `close(id)` ([`issues/closed/20261009-gui-dialog-action-commands.md`](../../issues/closed/20261009-gui-dialog-action-commands.md), #282).
  A modal `dialog` is an AppKit sheet and an `alertdialog` is an `NSAlert`, not the reference's
  in-page popup; with Keyboard Navigation off its Tab stops only at fields; and a dialog opened
  by a click gives the focus back to nothing. So each dialog keeps its own Tab trap
  (`dialog-trap.contract` says why), and closing a dialog whose opener the view cannot name
  still leaves the focus nowhere (Base UI gives it back).
- The menu bar's app items ([`issues/closed/20261009-contract-menu-bar-extensions.md`](../../issues/closed/20261009-contract-menu-bar-extensions.md)).
  Main's #947 (LLP 1021 §5.3.1) gave app-wide items, `-exact-menu="about"` and `"none"`, the Edit, Help, Go
  and Develop removals, View's zoom and a root `key` handler that holds ⌘Q, and the clone adopts them. Two
  things stay the module's: the "Updates unavailable" box, which has no parent window where a Contract
  `alertdialog` is a sheet, and Paste as Text (`T3Menus.swift`). Settings… is the host's: it files the ⌘,
  dispatch button there.
- An inline link run as a Tab stop on macOS
  ([`issues/closed/20261012-macos-inline-link-run-tab-stop.md`](../../issues/closed/20261012-macos-inline-link-run-tab-stop.md)). This fix is the host's, so the
  clone gets it on main with no change of its own. No drive has tabbed to a link in a Markdown
  table cell since.

App Swift this clone keeps (`modules/apple/`) does what main has no call for, or what moving onto main's
call would change for someone using the app. It is not work left while it works. A piece of it moves into
exact2 only when a library many apps can reuse needs it.

- `T3BrowserPage.swift`, `T3BrowserRecorder.swift`, `T3BrowserAgentCursor.swift` and `T3BrowserCapture.swift`: what a
  Browser tab's frame hands its `t3-browser-page` hatch (LLP 1020.002 tier 3). The recording (the frame as a capture
  source, LLP 1020.002 D8), the agent's cursor drawn on the page's view and the separate preview window's
  snapshots need the page's web view; the artifact directory is the local server's own folder, outside the app's
  files, where a capture moves in and Reveal in Finder acts, which has no framework call; a saved recording's bytes
  go to the environment from there, since the data module reads only the app's files.
- `T3BrowserPage.swift`'s `stage`: Copy image of a Browser artifact needs its pixels, and data code reads only the
  app's own files, where the artifact directory is the local server's. The module copies the artifact into
  `app:/tmp/copy-image`; data code draws it into a PNG and writes it through `@exact/clipboard` (`browser-capture.ts`).
- `T3MediaActions.swift`: the media menu, Save, and Copy image of an SVG. Save opens in Downloads first and then
  in the session's last folder, as Electron's download dialog does, where `saveFile` names no
  folder; the menu's Menu-key form opens at the media's bottom-left corner, where a `contextPopover` opens
  at the focused node's centre. Copy image of any other image is `@exact/clipboard`'s `write`
  (`media-copy-image.ts`, LLP 1133.006); the data module's image codec draws no SVG yet.
- `T3ShellMenu.swift`'s Copy Image, in the window's own context menu over any image: it copies the bitmap ExactKit
  decoded, which data code never sees, and the package's `write` is data code's alone (LLP 1133.006 §11 Q3).
- `T3Module+Sidebar.swift`'s `sidebarNotify`: the sidebar's undo notice, provider pill, search results and Update
  everything ask for the snapshot again after the answer that started them returned. A data module's timer ends with
  its answer (LLP 1027.000 D1), and data code has no other way to ask Exact to read a resource again.
- `T3RemoteEditors.swift` (probing installed editors needs a process and file checks; the client's
  opens are Contract's `openURL`, with the editors' schemes in `app.json` `open_url`),
  `T3InstalledFonts.swift` (no `queryLocalFonts`), `T3WindowChrome.swift` (the traffic lights at the
  reference's x = 16: the host's tall title row, `display_override`, leaves them where AppKit puts
  them, LLP 1039 D8) and `T3RootFont.swift` (the native views' rem).
- The composer's OS adapter (`T3Composer`) keeps type-to-focus. The send chords, the queued edit's ⌥↑ and
  the open menus' keys are the composer's own `key` action (`app.contract` `composerKeyEvent`); the
  layouts' window-space boxes are `frame` reads on `resize` (`layoutFrames`, `src/shell/layout-frames.ts`),
  and a composer menu's trigger is measured with `frame` as the menu opens. A drop on the chat column or a sidebar row, a paste's files
  and Attach files are Contract's `dragenter`, `dragleave`, `drop`, `paste`
  (`ClipboardEvent.files`) and `input type="file"`; TypeScript stages each file
  (`composer-attach-stage.ts`), reads it at send and removes it (`composer-staged-copies.ts`).
  An image within its limits keeps its bytes and type; one over them is encoded by
  `compressImage`. Paste as Text is the Contract's: Exact fires `paste` for `pasteAsPlainText:` as
  for Paste, and the composer's `key` handler arms the paste its ⇧⌘V brings, as the reference's
  keydown does (`isPasteAsTextShortcut`); Edit › Paste as Text chosen with the pointer arms it in the
  module (`T3Menus.swift`, the reference's `DESKTOP_PASTE_AS_TEXT_EVENT`), which TypeScript reads for
  the paste it holds or the files it attaches. Paste is enabled for files or image data alone where a
  `paste` is heard. A text paste of 32 KB or more (UTF-16 units or UTF-8 bytes), or one that
  would take the prompt past 120,000 units, is the composer's `paste` too (`composer.contract`
  `pasted`, `composerPasteDisposition`): lengths decide most pastes there, and a paste only its byte
  count can decide is held for TypeScript to measure. A held paste is prevented and goes, with the
  field's value and selection, through `attachFiles`' queue to `composer-editor-files.ts` `foldPaste`,
  which stages a `pasted-text.txt` file whose chip one guarded `setRangeText` puts in the paste's
  place, shows the reference's refusal, or puts the text in as the field's paste would. A paste
  Paste as Text armed, and every paste into a literal answer, stays inline; a question that takes
  attachments folds as the thread's draft does.
  A staged video or audio file keeps its extension (`composer-staged-copies.ts` `stagedName`),
  which AVFoundation needs to play it; a video's shelf tile is a muted, paused `video` of that
  copy sought to its first frame, and its display size for the preview is read from its header
  (`composer-video.ts` `videoFrameSize`). The composer is
  Exact's Markdown editor (`textarea
  markup="markdown"`, LLP 1045): it styles lists, tasks, quotes, headings, rules and fenced code
  as they are typed, owns ⌘B/⌘I (`format`), and keeps native undo. The app reads the source
  selection from its `beforeinput`, `input` and `selectionchange`, and edits it only with
  `setRangeText` (one undo step each) and `setSelectionRange`: completions, draft history, the
  code language picker, citations and Enter/Tab/paste in code. A reference (a file link or `@path`,
  a `$skill`, a citation, a context reference) is an atom of the editor (LLP 1045.000): its pill
  shows the label, glyph and tooltip `composerChips` gives its source, the editor keeps the caret
  out of it and deletes it whole, a copy takes its source, and a press opens it, anchored at the
  pill. Code languages are ordinary controls beside the field. Plain and literal answers stay
  ordinary textareas.

Differences that belong to the app:

- With Markdown formatting off the composer is a plain `textarea` (`markup="none"`), where a held paste's
  `setRangeText` (a folded chip, or text a held paste puts in) is no undo step, and Apple clears the
  field's undo history when its value changes (`docs/contract-grammar.md`, `setRangeText`): ⌘Z
  does not take it back, where the reference's undoes it. In the Markdown editor each call is one undo step.
- The server connections are the data module's (`src/transport/`). Each environment's socket is a
  stream resource, admitted by `net.websocket ws:` and `wss:` since servers are named at run time
  (#703), whose mapper answers Pings and acknowledges chunks in the frame's own commit (#705, LLP
  1016.000 D7); HTTP goes through `fetch` (`net.fetch http:` and `https:`). `app.json`'s
  `host.macos.appTransportSecurity.allowsArbitraryLoads` turns App Transport Security off, so an
  environment at a plain `http:` host name pairs, as in the reference's Chromium. The answer or command
  that asks for a request sends it on that socket itself and awaits the reply (#736, D7's amendment
  of 2026-10-18); a frame no turn may send yet (the socket is not open) waits in the outbox, which
  the mapper sends when the socket opens and after each frame. A data module's timer lives only
  inside an answer, which the sessions outlive, so a mutation the window runs, the tick, keeps the
  deadlines, Pings, probes and reconnect ladder: every 250 ms while something is near, every second
  while connected, and once to publish the socket table each time an answer that may wait on the
  server starts or ends or the delivering answer reports a table the tick has not published. Nothing polls while an answer waits. A request's
  deadline counts from the tick's last reading of the clock, which can be up to a second old, so
  its timeout can come up to a second early. A connect replies once the socket's URL is in, not
  once the socket opens, since an answer may await only what this module's fetch, storage or open
  stream settles.
- The module holds no server connection or bearer. The device hub's tickets are minted by the
  transport (`deviceHubAccess`) when the module asks for one (`presentation.deviceHub.want`) and
  reach the device view as its `t3-media` node's `data-device-origin` and `data-device-ticket`,
  which the node drops once the module reports the ask answered; the Tools drawer's feeds and
  accessibility reads reuse a ticket for 60 s, as the reference keeps its hub access. A
  Browser recording's upload is minted and cleaned up on the transport's socket; only its bytes go
  from the module (`browserRecordingUpload`), because the saved recording is in the artifact
  directory, outside the app's files the data module can read. Each terminal page opens a socket of
  its own (one per pane, the reference's renderer has one for all).
- The last interaction (the activity reporter's and the pull request surfaces' idle rule) is the
  window root's `pointermove`, `pointerdown`, `wheel` and `keycapture`, to the second
  (app.contract `interacted`). Keys and scrolls the terminal's and the Browser's web views take
  never reach the root: typing in the terminal alone does not keep the reader recent, where the
  reference's terminal is in its page (an `iframe` guest's events stay in its page). The root
  hears a hovering pointer only while the window is key (ExactKit's hover tracking), where
  Chromium's window listeners also hear one over a background window.
- While the device tab's rename field has the focus, the keybinding chords of the reference's
  bubble-phase window listeners stay in the menu bar disabled, and the composer's send chords and
  the Escape buttons beside the tab strip drop theirs (app.contract `renameHoldsKeys`): an
  `aria-keyshortcuts` button hears its chord before any field, where the reference's input stops
  the key's propagation. A held chord that is a text field's editing chord (⌘Z, ⌘C, ⌘A…) drops out
  of the menu instead, so the field edits with it.
- Under the agent driver, a clock step that has a timer due first waits for every answer in flight,
  up to 20 s. Answers send their own requests, so that wait ends when the server replies, not after
  20 s; the app itself runs its timers on the wall clock. Under the agent the transport's
  ids also come from the driver's repeatable random stream, so drives of one persistent server use a
  different `--seed` each launch, or the server takes a repeated command id for the command it
  already ran and does nothing.
- The Browser surface is WebKit, not Chromium (#100 was closed as not planned). Pages get no
  geolocation. A page's notification permission request is denied, as in the reference.
- A Browser tab is a partitioned `iframe` (LLP 1020.002), so the platform answers what the reference's Electron
  shell drew: `alert`, `confirm` and `prompt` are sheets titled with the page's origin (a `prompt` is answered), the
  file chooser and a download's destination are the platform's panels, another app's scheme asks in Safari's words,
  fullscreen takes the screen, and camera, microphone, geolocation and clipboard reads are refused. Each profile is a
  `persist:` partition per environment and Incognito an in-memory one, and cookies cannot be imported from other
  browsers (LLP 1020.002 D4). A profile's cookies from the module's old stores move into its partition once, and the
  page reloads for them; its local storage, IndexedDB and cache stay behind, since WebKit reads none of them out of a
  store, so a site that keeps its sign-in there asks again once. A hidden tab is not displayed, so WebKit throttles its
  timers; one that is recorded or shown in its separate window stays laid out, transparent, and keeps drawing. With
  the page focused, ⌘R reloads it and the zoom chords zoom it (the page is asked `document.hasFocus()`, since a frame
  reports no focus) and ⌘L puts the focus in the address bar, selecting the address; ⇧⌘R reloads the window, as the
  reference's menu does. A page's dialog while
  an agent request runs on its tab is answered at once (OK for `alert`, Cancel for `confirm` and `prompt`); one left
  from an agent's run shows over the page as Chromium's tab-modal dialog and takes the person's answer. The console,
  network log and media scripts go in as each document commits, so what a page logs before that is not kept
  (clone/issues/browser-preload-without-taking-the-frame.md); the console, network log and audible state are read
  from the page every few seconds
  rather than pushed. The tab's icon is the page's first declared icon, loaded by URL, where the module redrew it.
- A server Browser tab's PDF is the viewer page's PDF.js reading its signed URL (`server-pdf-viewer.ts`), where the
  reference embeds the URL in Chromium's viewer, which would run an HTML answer as a page. Its toolbar is T3 Code's
  PdfViewer (pages, previous and next, zoom, Fit width, full screen, Find in PDF), not Chromium's (thumbnails and
  outline, a page field, rotate, two-page view, download, print, properties, and the browser's own ⌘F), and it stays
  in the floating player, where Chromium's hides at that size. A PDF from another origin loads only when its server
  lets the page fetch it (CORS), as the T3 server does. A rendered HTML preview is a sandboxed `iframe` (LLP 1020.002):
  a right-click shows WebKit's menu, not the desktop shell's, and a `window.open` with features opens the platform's
  popup window, where the reference opens every new window in the default browser (a `target="_blank"` link or ⌘-click
  still does).
- The host opens the first window before the embedded server is ready (#117 was closed as not
  planned), so the window shows the connecting state first.
- The launch's landing (`home-landing.ts`) waits for a background environment while it connects, but
  not for This machine while its server starts, nor for one that is retrying: the reference opens its
  window only once the server is ready, and keeps waiting for any switched-on environment until its
  third attempt has failed (`state/shell.ts` `allEnvironmentShellsBootstrappedAtom`). So a relaunch can
  open the draft in another environment's project when the most recent one is This machine's or a
  retrying one's.
- Code colours: a code text paints plain until the data crate's lexers answer for it, a few frames
  through the root's pump (`app.contract` highlightStep), where the reference's loaded highlighter
  colours it in the render that shows it. The lexers share the data crate's thread with the chat
  Markdown, so each round lexes about 17 ms of code: a file of 1M characters takes about 30 rounds,
  where the reference's worker pool lexes it whole beside the page.
- An ⌥-click on a reply's `#fragment` link opens nothing. Chromium downloads the page instead.
- Split view's empty side stripes every 10 pt, where the reference stripes every 8 px: each row
  of the virtualized list paints its own gradient, and main keeps `background-attachment: local`
  deferred, so a shorter period would break at every row edge.
- An image inside a link's text has no tooltip and no keyboard menu key.
- In the composer, a review comment's popover shows its comment and diff as plain text (the
  reference renders Markdown and a highlighted diff); a sent pull request chip has no hover card.
- Appearance has no Environment identification row. The clone is the stable app only, and it does
  not draw the Nightly or Dev artwork. At v0.1.5 the stable app's default also shows no art, but the
  row lets a user opt into Nightly or Dev artwork ("Show Dev or Nightly artwork in the sidebar
  header."); its version pill is offered only on a Dev or Nightly stage.

Some checks need a person: physical modifier chords, right-click menus, real pointer drags
and hovers, macOS notification and screen-capture grants, provider installs and logins, and
GitHub writes. Agent drives cover the rest. Where a person's keys or pointer found what an agent drive
could not, the comment at the fix says what happened and why the code is so.

## Work left

As of 2026-10-17, after merging main's macOS fixes (exact2 PRs #636 to #640, #646, #647, #651,
#655, #663, #664, #676, #679 and #681, then #687, hover following the pointer once a context menu
closes, realinput-1017l failure 3, and #688, Shift-Tab wrapping from a key view loop's first stop
and a placeholder change keeping a field's focus, failure 2, then #712, a context menu's item chosen
by typing its name and Return pressing it), a session with real keys on the terminal, the
real-input and VoiceOver sessions G to L, an agent drive and real-input session on the bundle
of `94cc6831c`, and others on the bundles of `dd7643a86`, `3d6c75ad2`, `c067957ad`, `966bb0d79`,
`ea15f98f0` with `571d07c17`, and `6b1fc7dde`, each with the fixtures below. Each item says what is
wrong or missing; the open framework issues that someone using the app runs into are in the section
above.

**Known differences:**

- Settings › About › Update track saves only this app's own track. In v0.1.5 the switch also moves every
  connected server between Stable and Nightly, after a database-migration check
  (`UpdateTrackSwitchDialog.tsx`). The clone is updated through Exact delivery and has no desktop
  update feed (`settings-a-about.ts`), and Nightly is not cloned.
- The legacy sidebar is removed, as in v0.1.5. In v0.1.5 a saved `legacySidebarEnabled` opens the boring
  sidebar, which is a Labs feature, and ⌘N there starts the thread in the current project. The clone
  has no boring sidebar, so the same setting opens the default sidebar, where ⌘N with several projects
  opens "New thread in…". For the same reason, the Working section's description leaves out v0.1.5's
  "The boring sidebar in Labs has no Working section."

- After VO-Space on a composer thread chip, the reference's VoiceOver cursor ends on the window's close
  button (VO-F3 "close, button", then VO-Right "minimize, button"; 3 of 3), though it has just read the
  new thread's prompt: Chromium drops the pressed link. The clone's stays on the prompt it read, which
  has the keyboard focus, as VoiceOver's cursor does in an AppKit window.
- When a thread switch replaces a draft in the focused prompt, VoiceOver reads the old draft as it
  leaves ("see Turn with file edits now", about 0.6 s after VO-Space on a composer thread chip, cutting
  off "Message, edit text", or after ⌘⇧[; 3 of 3 each); the reference's reads none of it. Exact's
  textarea writes the new draft into the same text view (`TextAreaMac.swift` `writeValue`), for which
  AppKit posts only `AXSelectedTextChanged`, and VoiceOver speaks the text removed from an AppKit text
  view however it is replaced; Chromium reports the change as an `AXValueChanged` edit naming the
  removed text, which VoiceOver does not read.
- The Show in thread details menu, on the thread details card and in Settings › Appearance, closes
  after a choice, as a Mac menu does. The reference's is a Base UI menu whose checkbox items keep it
  open, so several items can be turned on or off in one visit.
- A server-rendered Browser tab has no picture-in-picture window: Exact has no source that turns its
  JPEG frames into a video, which the reference's `canvas.captureStream()` does. Its in-app
  floating preview works.
- No development web view inspection on Android: the clone does not run on Android yet.
- A composer reference's pill is the platform's token (LLP 1045.000 D4, §10 Q2): its kind's
  colours, with the platform's padding and radius and no border; its glyph is the nearest SF
  Symbol (a document for any file, not its file-type icon; no project favicon or image
  thumbnail), as the Apple editor draws only `symbol:sf/` icons; and every context reference
  (an image, file, thread, terminal excerpt or comment) takes one look, since a kind's paint is
  per scheme.
- A citation's comment opens on ⌥-click of its pill, or by itself after Cite; the reference's
  chip has a pencil button inside it, and here a pill is one press target (LLP 1045.000 §6).
- A composer chip's name does not underline while the pointer is over it, where the reference's
  `ContextChipLabel` does (`hover:decoration-current`). The chips are atoms of the Markdown field,
  and an `AtomFace` has no hover state: the macOS field draws the pill and takes its press itself
  (`TextAreaMac.swift` `atomHit`), so the app hears no hover per atom. Kept as a known difference by
  the user's ruling (2026-10-31). A chip in a sent message is a view of its own and underlines.
- With Markdown formatting off a reference shows its source: the editor draws atoms only where
  it styles Markdown (LLP 1045.000 D3). (A literal answer has no chips, as in the reference.)
  Settings › Appearance's prompt sample styles Markdown so its chips draw, where the reference's
  disables rich syntax.
- A character typed right after a mention or a skill makes its source longer, and deleting the
  space after one makes it text again; the reference keeps the chip until its document is
  rebuilt (LLP 1045.000 §6).
- Edit › Paste as Text chosen with the pointer is a Paste: no key comes before it to arm the
  composer, where the reference's menu item arms it over IPC before pasting. Its ⇧⌘V is Paste as Text.
- A folder dropped on the chat column or pasted into the composer is not mentioned in the prompt. The
  reference inserts its absolute path as an `@` mention (ChatComposer.tsx `addComposerFolders`); such a
  folder reaches the clone as a `doc:` handle with no path (LLP 1069.010 D1), so the clone shows the
  reference's own refusal for a folder whose path it cannot get: 'Couldn't get the path of "src"',
  "Type the folder path with @ instead." Files dropped or pasted with it attach.
- A whole-word `_text_` in the composer is drawn in italic, where the reference's composer keeps typed
  underscores literal: Exact's Markdown field paints CommonMark emphasis and has no option to leave `_`
  out (clone `issues/composer-editing-underscore-emphasis.md`). `node_modules` and an underscore in a
  code span stay plain, and the sent message renders the same in both.
- An image or a file chip copied from the composer pastes whole only within this app: the reference
  carries the chip's record in a clipboard type of its own, and Exact's clipboard holds text, so the
  clone finds the record by the id in the pasted link, in the draft that holds it or in the latest copy
  (`composer-copied-chips.ts`). The latest copy's image bytes stay until another copy replaces it, as
  the reference keeps that copy's files in memory.
- Leaving a quote with an empty `> ` line leaves a blank line under it, which the clone's composer
  shows: the reference's composer hides the blank line it writes there, so that the comment under the
  quote is its own paragraph and not a lazy continuation of the quote.
- An image over 10 MB is re-encoded as a PNG when it can hold transparency and else as a JPEG; the
  reference prefers WebP, which `compressImage` does not write. An image within its limits keeps its
  bytes and type, and a PNG or JPEG over 2000 px is fitted in its own type, as the reference's are.
- A dropped or pasted file's MIME type comes from its extension, by the names macOS gives
  (`composer-attach-stage.ts`), where Chromium's `File.type` reads its own table first; a
  picked file's is the picker's.

**Not failures:**

- The real clicks on a pull request's Code-tab chevron that did nothing in the session on
  `dd7643a86` (4 of 4) landed on a system privacy prompt an earlier lane copy had left on screen
  ("T3 Code Lane M would like to access files in your Documents folder", UserNotificationCenter,
  screen points 546–806 × 160–368). It covered the chevron's point and ignores synthesized clicks;
  the file name's centre lay outside it, and AX hit-testing, which asks only the app, still found
  the chevron there. With the window clear of it, real clicks fold and unfold the file on that
  bundle (4 of 4) and on `3d6c75ad2` (below), as the reference's chevron button does
  (PullRequestCodeTab.tsx:777-797). A real-input session should check which window is topmost at a
  point before it judges a click.
- Tab into a new binding's recorder records nothing (2 of 2 by real keys on the bundle of
  `dd7643a86`; with Keyboard Navigation off the first Tab from the window goes to 'Search settings'
  and the second into the recorder, 'Press shortcut'). A Tab pressed while it records is recorded,
  'tab', as the reference's captureKeybinding records Tab while recording
  (KeybindingsSettings.tsx:770-779); the earlier session's helper pressed Tab until the recorder had
  the focus, and the 'tab' it reported came from such a press.
- The settings search jump from another page lands on the row (agent drive, 'Time format' from a
  scrolled Appearance). It runs once its page is ready and laid out, one millisecond on the agent's
  clock (`clock +1`); the earlier drive's failure was a jump that waited for a one-second poll its
  clock never reached.
- Opening a thread marked unread does not clear it when the thread opened in between had
  nothing new (session K's FN4), as in the reference. ChatView's visit effect dedupes on
  `${routeThreadKey}:${updatedAt}` (ChatView.tsx:2866-2868) and returns early, without touching
  that key, for a thread with nothing new (2858); ChatView is not remounted between server
  threads (ThreadRouteView.tsx:184, _chat.tsx:209-221); and mark-unread leaves `updatedAt` as it
  is (Orchestrator.ts:2660-2670, 2789-2790). Reopening the thread hits the same key and sends no
  visit, as the clone's `sidebar-commands.ts` does.
- Settings › Keybindings: 'Preview: Zoom In' (`mod++`) shows ⌘ and two blank key caps, as the
  reference's KeybindingPill splits a literal '+' into empty parts (KeybindingsSettings.tsx:74-103;
  `keybinding-view.ts`).
- In the accent picker, VO-Right stops at 'Accent brightness' and Tab reaches 'Accent hue'
  (E-C3). The saturation and brightness sliders are inside the plane, a group as the reference's
  ColorSaturationValuePlane is, and VO-Right stops at a group's last child until VO-Shift-Up
  leaves it, as in any AppKit group. The hue slider reads as the reference's ('223, Accent hue,
  slider').
- A flat sidebar drag lands the row by the lifted row's centre, not the pointer's drop point
  (FN13), as the reference's closestCenter and sortable strategy do (Sidebar.drag.ts:1-47,
  Sidebar.tsx:5071-5082; the clone's `sidebar-drop.ts`). Each write went to the row's own
  environment.
- Shift-Tab from the window goes to 'Toggle main sidebar', the last view in tree order though it
  is drawn top-left (FN28, exact2 PR #646): AppKit's Shift-Tab with no first responder selects the
  last key view of the loop.
- The font picker and Add Environment name their dialog after the focused field or radio, about
  0.5 s later (H-10, J18, J21, FN32), the order exact2 PRs #664 and #679 chose; a caption window
  shorter than that misses the name.
- ⌘Z after snoozing a background row from another environment undoes it while the undo notice
  lasts, 5 s (`sidebar-commands.ts`); a ⌘Z after that leaves the row snoozed (FN29).
- A composer pull request chip is named 'Open Open pull request #132: …': the reference's own
  template, `Open ${kindLabel} …` with the kind label 'Open pull request' (contextChipParts.tsx:113).
- Return on a focused button that is not the default one, such as 'Edit Echo test', does nothing
  and Space presses it, as in any AppKit window (Return presses the default button).
- A press outside a chip's popover closes it and still does what it lands on (a press on a
  timeline image opens its preview), as Base UI's outside press lets the press through.
- Interface fonts: a picked family's bold face draws the bold text (Georgia Bold for the PR title
  and project names) and persists across a relaunch; no fixture has italic text, so the italic
  face was not seen (N17).
- Pull Requests in the lane: 'GitHub CLI returned no login for the authenticated account' and a
  first click on #144 that did not open it come from the github-lane fixture's `gh` stand-in;
  the list loaded and the second click opened it. The timeline's 'Image unavailable' for a table
  image and a linked image is the lane's missing media server.
- Mark unread changes nothing on a thread with no completed run: the server refuses it and the
  reference stays silent. On a completed thread the row gains the green 'Completed' label.
- A chip pressed to open its preview stays selected after the preview closes, and a letter typed
  after Escape, Close or a backdrop click replaces the file or image chip (C14/C23). T3 Code's
  Tiptap editor keeps the chip's NodeSelection and Base UI's finalFocus gives the editor back the
  focus; measured on the reference web build. The old native-handler test was retired with that
  owner; this reference measurement does not qualify the shared field's current behavior.
- Dragging a zoomed preview image past an edge (row 4, E-D4): ZoomableImage's drag keeps its
  press as the origin, so the image moves back only once the pointer passes where the edge was
  hit. Measured on the reference's own ZoomableImage.tsx in Chrome for Testing 154;
  `composer-preview-keys.test.ts` pins it.
- Escape on an existing binding's recorder brings back its pill and leaves the focus on no
  element (KeybindingKeyControl's showPill).
- After the accent picker closes, a second Escape in Create theme leaves Settings and the
  floating theme editor stays open (ThemeEditorPanel has no Escape handler; Settings'
  useEscapeToGoBack takes it).
- ⌘Z with the prompt focused is the prompt's own undo, as in the reference (thread.undo is
  `!terminalFocus && !editableFocus`).
- The right panel toggle moves from the chat header into the panel when the panel opens, as the
  reference's does.
- Sessions K and L did not run some rows, for want of fixture data or because the task left them
  out; none is a failure. Since checked (below): H-2's reset-credit confirm, the diff Turn submenu,
  the approval menu (FN5), a diff comment (FN7), the linked pull request's actions menu and Reply,
  and G-5's Remove project on an empty project (session L). Still not run: FN8's Usage part (the
  confirm covers 'Toggle main sidebar'; the driver does not read `inert`), FN17 (no real-input path
  makes a focused control disabled), the project scope search, FN32's dialog over Add Environment
  (none opens there), a zoomed image's drag that reverses within one press, and realinput-1015c
  row 24. J9, J11, J15, J16 and J17 need pre-fix builds or superseded framework test apps. J10 ran
  on the T3 lane instead of PR #676's test app (58 of 58 outside presses closed the popovers, and 9
  of 9 on the linked pull request's menu in session L), and J19's NSPopover path does not exist in
  the clone.

**Checked with real pointer** (2026-10-17, bundle of `6b1fc7dde`, the flat sidebar, a lane copy): with
the prompt focused, a right press on a draft row focuses the row ('draft for the row check, Unsent draft,
agent-edits') and opens its menu (Copy, …), and Escape leaves the focus on it. On a hovered card, a right
press on Unpin thread, Snooze thread or Settle thread, or on the draft row's Discard draft, focuses that
control, as Chromium focuses a button it presses, and opens the row's menu; Escape leaves the focus on
the control. A right press on the card's body still focuses the row (1 of 1 each).

**Checked with real pointer, keys and VoiceOver, and by agent drive** (2026-10-17, bundle of
`966bb0d79`, main's #712 in it; `second` seeded, its home as This machine's, a lane copy on an
isolated home): a right press focuses the row or header whose menu it opens, as Chromium focuses the
reference's (Sidebar.tsx:1759-1760 and 1924-1925, LegacySidebar.tsx:715-717 and 2373-2381), so
VoiceOver, told that the menu closed, reads the row and no longer re-reads a long draft in the
prompt. With that draft focused, a legacy project header's Rename says "Closing menu shared-notes,
expanded, button shared-notes, contents selected, Project title, edit text", then "Rename project,
dialog" (3 of 3). Group into… now lands on the 'Project grouping rule' pop-up button, as the
reference's lands on its combo box: "Closing menu shared-notes, expanded, button Use global default,
Project grouping rule, pop up button" (1 of 1). The header's menu reads "Group into…" and the app
and Help menus "Check for Updates…", as the reference's Electron menus draw their "...". Escape
from either dialog, or from the menu, gives the focus back to the header. On a flat thread row,
Escape from its menu leaves the focus on the row (3 of 3; in the agent drive the right press ran
`focus("thread-…")` and the prompt blurred), and Archive thread chosen by typing "Archive" and Return
opens 'Archive thread "Background follow-up"?' on Cancel, VoiceOver saying "Closing menu Background
follow-up, background-only, button, Cancel, button", then the dialog's name (3 of 3; it did nothing
before #712), with Escape back to the row. The reference's Rename (the Electron oracle, same steps)
never re-reads the draft either and says less: "Closing menu T3 Code (Nightly), web content" (3 of
4), "Rename project, dialog, with 6 items ref-proj, …, Project title, edit text" (1 of 4), and nothing
within 6 s in an earlier session (4 of 4). Not measured: Archive on the reference (its isolated home
has no keychain, so it cannot save a paired lane, and its own server has no thread to archive), and
where the focus lands after Confirm removes the right-clicked row.

**Checked with real keys, pointer and VoiceOver** (session K, 2026-10-17, bundle of
`c5574fdca`): everything the previous "Fixed, not checked" list held, but for the rows above.
That is: the font picker by VO-Space and by pointer; double-click rename across environments
(flat and legacy, 34 of 34, also onto a draft with an inline image chip); the empty prompt read
as 'Message, edit text' and the last-tile removal staying on the prompt; light dismiss of the
'Open in', file 'Open in', folder crumb and diff scope menus; VoiceOver landings in dialogs
(#655, #664, `dialogSeated`); the fields focused on mount; 'Toggle main sidebar'; the shelf's
Remove handing on the focus; Escape order at a narrow window and the focus back on the Git
options chevron; the chips as the text area's siblings in text order; Return on menu rows; a
clicked button taking no focus; AXPress answering success; the font picker's keyboard pick; the
legacy row names; the image preview as ZoomableImage; no 'HTTP 404' after a hover Settle; #647's
alertdialog, menu and group speech; the Git split button's chevron; settle, snooze, wake and pin
on a background environment's row; Mark unread from the native menu; the keybinding recorder as
a text field; a new binding by keys; the Project scripts menu; the composer's 'Preview <name>'
buttons; the Snoozed header and Wake; and Tab through the legacy Rename project dialog.

**Checked with VoiceOver after main's #681** (2026-10-17, FN15): the shortcut recorder, an
`aria-modal` row with no role, is served by its field. VO-Space on 'Edit shortcut for Chat: New'
reads 'Keybinding for Chat: New Press shortcut, edit text', and VO-F3 'Keybinding for Chat: New,
edit text, is in the VoiceOver cursor' (2 of 2; before, 'scroll area'). The clone needed no change.

**Checked with real keys, pointer and VoiceOver** (session L, 2026-10-17, bundle of
`9ee4e4e88`): each Settings page opening at its top after another was scrolled, and the search
jump landing from General (L1); Remove project on an empty project and Project grouping, read as
dialogs from an empty prompt (N3, G-5); light dismiss of the linked pull request's actions menu
and the script and Git menus (N5, J10); Reduce motion (N12, with the note above); the thread, pull
request, review comment and annotation chips by pointer (N14); the branch PR glyph, flat and
legacy (N16); Tab and Shift-Tab in the file and image previews (N20); an HTML preview's
own-section, `target=_top` and `target=_blank` links (N21); the image preview refitting after a
window resize (N22, FN23); Escape on 'Remove from this device…' and 'Delete action?' by keys
(N23); Escape on a git dialog over the thread-details popover in a narrow window (N24); a flat
sidebar drag (FN13); the shortcut recorder read as a text field (FN15); ⌘Z after a snooze from
another environment (FN29); and 한글 typed as the first word in an app the input method had not
met, in the terminal and the composer.

**Checked by agent drive** (2026-10-18, bundle of `51d58fe45` and the fix; This machine, `second`
seeded with `--turns` and `third`, paired in one kept store): quit with "Background follow-up" on Lane
Second open, each relaunch opens a new draft in Lane Third's `shared-notes`, the project with the latest
activity of any environment, as the reference's IndexDraftLanding does (3 of 3). The bundle of
`51d58fe45` alone opened Lane Second's most recent project (3 of 3).

**Checked by real input** (2026-10-17, bundle of `ea15f98f0` with `571d07c17`, `second` seeded with
`--turns`; a lane copy with Lane Second paired in Settings › Connections, quit by holding ⌘Q): relaunched as
it is, the copy opens with Lane Second's projects and no toast (3 of 3), as does the development bundle
itself paired the same way (2 of 2, on a build with the pump change alone). On the bundle of `c067957ad`
each relaunch opened with no projects and "native executor admission limit reached" (2 of 2). While the
first snapshot answer reported a code text waiting for its tokens, the root's highlight pump refreshed
`data` and `shell` every 16 ms. A refresh replaces the answer in flight, so the launch's snapshot answer,
which waits seconds on its connections, never landed, and the replaced answers' native calls filled the
executor's admission limit about 4 s after launch (`host/apple/src/executor_core.rs:491`). A fresh bundle id
avoided it only because its defaults hold no saved environment. The pump now leaves an answer in flight
alone (`571d07c17`): in saved traces of the first seconds, 92 snapshot queries and 89 replaced snapshot
requests in 2.8 s before, 11 and none in 4 s after.

**Checked by agent drive and real input** (2026-10-17, bundle of `3d6c75ad2`, `second` seeded with
`--turns --pull-request`; a lane copy for real input): the settings search jump to a legacy row by
Return ("legacy", Return from a scrolled Appearance) opens General at "Legacy features", expanded,
with 'Plan mode (legacy)' focused (agent and real input; `188ae6a05`, where `SettingsWindow` had
tested the result's id for "legacy-" though it is "search-result-legacy-…"). By real input: ⌘V with
only PNG data on the pasteboard attaches it, an 'image.png' tile on an emptied draft's shelf and no
chip (`1683137cd`: AppKit enables Paste only for a pasteboard the text view can read, and the
composer now reads PNG and TIFF); ⇧⌘V with a Markdown file and a PNG file on the pasteboard, after
"look ", keeps the prose, adds the 'File attachment, paste-notes.md' chip and puts photo.png on the
shelf with no image chip, as the reference's Paste as Text attaches them (ChatComposer.tsx
`onComposerPaste`, `skipImageInlineChip`; `753fb8b9f`); and a click on a pull request's Code-tab
chevron folds and unfolds the file (4 of 4, three HID clicks and one through orca), as a click on
its name does. The agent drive stopped twice before its other ops, at `tap Dismiss` (the "Nightly
needs the beta mobile app" notice, shown this time on Settings › General) and at `tap
pr-filter-state` (hidden or inert); real input covered what they would have checked.

**Checked by agent drive and real input** (2026-10-17, bundle of `dd7643a86`, the fixtures below; a
lane copy for real input): a file attached with Attach files to an empty draft draws as
'sample.py 1 KB' (agent), and by real input a file pasted into an emptied draft does too and a press
on it opens its preview (the fix keeps the files attached while the editor's empty state was in
flight; `composer-editor-paste.test.ts`). ⌘V of a copied file attaches it as a chip and pastes no
text (agent and real input), of an image file adds the image's chip and its shelf tile. A press on
the Files preview's gutter closes an open skill chip's details (`chip.open` true, then false). A
press on the floating theme editor's swatch, and on Inspect, closes the Appearance sample's chip
details (1 of 1 each); the picker's plane and hue slider cannot be pressed with the details open,
since the press that opens them closes the picker, and armed, Inspect's overlay takes every press.
The linked pull request's actions menu opens (16 items) and a press outside it closes it; Reply
opens a focused 'Reply to this conversation' field and Escape closes it. The machines popover,
with Lane Third behind the older-server stand-in: 'Ready to update to 0.0.46-nightly.20261004.1',
then after Update all 'Downloading…' (2 s) and 'Restarting…' (16 s), and after the restart the
banner is gone. The image preview: with Keyboard Navigation off it opens on 'Close image preview'
and Tab and Shift-Tab go to the zoomable image and stay there (3 of 3 each); with it on they cycle
Close and the image (6 of 6 each).

**Checked by agent drive** (2026-10-17, bundle of `94cc6831c`, the fixtures below): on Usage ›
Limits, a press on a segment pins its popover, a press in its card keeps it and a press elsewhere
on the page closes it; the popover's Use reset opens 'Use a reset credit?' with Cancel focused,
covering the window, and Escape closes it (H-2). Providers, Connections and Appearance each open at
their top after the page before was scrolled. A press on a diff gutter (line 3 of README.md in the
turn's diff) closes an open skill chip's details (`chip.open` true, then false). The diff scope's
Turn submenu lists 'Turn 1' and selects it. An approval's 'More approval options' menu (Cancel,
Always allow this session) closes on a press outside it (FN5). 'Comment on line 3' in the diff
opens its draft focused (FN7).

**Checked with real keys and pointer** (2026-10-17, bundle of `94cc6831c`, a lane copy launched
normally): F-2 by Tab: Settings › Keybindings, Add keybinding, Tab into the new recorder, Escape:
the field keeps the focus, its value goes back to Unassigned and the row stays (1 of 1; the 'tab' it
showed first came from a Tab pressed while recording, under "Not failures" above). The composer's file preview (sample.py): with
Keyboard Navigation off, Tab and Shift-Tab stay on 'Disable word wrap' (3 of 3 each), as AppKit's
loop does without it; with it on, Tab goes Disable word wrap → Copy contents → Save file → Remove
from draft → Close and back to the first, and Shift-Tab from the first goes to Close and back
through the rest. Escape on 'Remove Lane Third from this device?' returns the focus to 'More
actions for Lane Third' and keeps the environment (2 of 2). Escape on 'Delete action "Echo
test"?' closes it and keeps the action, and a second Escape closes the editor; both were opened by
an accessibility press, which leaves no focused trigger, so the focus went to the Name field and
then the window (session L's keyboard-opened run went back to Delete and 'Edit Echo test'). The
Usage page's pinned popover by a real pointer: press on the segment, a press in its card keeps it,
a press elsewhere closes it (2 of 2).

**Checked with VoiceOver** (2026-10-17, bundle of `c067957ad`, `second` seeded with `--turns`; a lane
copy): VO-Space on a composer thread chip ('link, Thread, Turn with file edits', in "see … now" on
Notes branch thread) opens the thread; VoiceOver says "Message, edit text, Insertion at beginning of
text." and stays on the new thread's empty Message, which has the keyboard focus, and VO-Right goes
on to 'Default, collapsed, button' (3 of 3). The reference (the Electron oracle on an isolated home,
its own server seeded with two threads, the chip picked from the `@` menu) says "Message, Ask for
changes, send follow-ups, or attach images, list box pop up, text entry area, Insertion at beginning
of text." with the keyboard focus on that prompt (3 of 3). At that revision the native chip led to the prompt: leading
to itself, it took VoiceOver's cursor to 'Go to threads' as it went with the old draft, and a focus
change posted for the prompt after the switch was not followed (`98be292b9`, replaced). The citation
chip's link still keeps VoiceOver on it ('lane note, selected'; 1 of 1).

**Checked with VoiceOver** (2026-10-17, bundle of `8dd82dc72`): VO-Space on a composer citation
chip's link, from the end of the thread, scrolls the timeline to the cited text and leaves
VoiceOver on 'View cited assistant text: …, link', with the keyboard focus on the prompt, as in
the reference (5 of 5). VoiceOver follows a link it presses to the link's linked elements; with
none, it took the element that appeared within about half a second of the press, the 'Scroll to
end' pill. That revision linked the chip to itself through
`T3ComposerChipElement.accessibilityLinkedUIElements`. The shared-field composer retires that
native AX projection; this historical drive does not qualify its current VoiceOver behavior.

**Checked by agent drive and real input after main's #687 and #688** (2026-10-17, bundle of
`c4f9f4b38`): a press on the floating theme editor's swatch, its picker's plane and hue slider,
Inspect and the inspector each counts as an outside press, as the window's other presses do (the
window's count went 8 to 13, one per press). Escape in a new binding's recorder after a real
click into it keeps the field focused and the row up (F-2, 1 of 1; #688's placeholder fix).

**Checked with real input (2026-10-30)** (bundle of `2f29f2074d`, a lane copy on its own home, `second`
seeded with `--turns --pull-request`, pointer and keys through `orca computer`, VoiceOver read from its
caption panel):

- Context added at the selection keeps a selected chip (2 of 2): with the `README.md` mention selected,
  a review comment from the diff went in before it; with that review comment selected, an image from
  Attach files went in before it. Neither chip was replaced.
- A chip's details popover, the parts that hold: Escape closes it and the prompt keeps the focus with
  the chip selected (3 of 3, a review comment), and a press outside it closes it and leaves the focus
  where the press put it (Search threads, 1 of 1).
- Mark unread on "Background thread (no run)" from the thread's actions menu: the server refused it
  ("has no completed run to mark unread", in its trace) and the app showed no toast or banner (1 of 1).
- The context popover under VoiceOver: the meter reads "Context window 6.2% used, dialog pop up,
  button", and VO-Space opens the popover, which reads "Context Window", "6.2% · 12k/200k" and
  "Context window usage, 6%, progress indicator" (1 of 1). The Usage page's bars were not checked.
- VoiceOver reads a composer image chip as "Image attachment, sunset.png, 13 KB", in the prompt's
  value and as the chip itself when VoiceOver interacts with the prompt (1 of 1).
- A freshly saved EAS host ("Realinput EAS", saved by a pointer press) draws Edit and Remove at full
  opacity, as the older host beside it does (1 of 1).

**Checked with real input and VoiceOver (2026-10-31)** (a lane copy on its own home, `second` seeded with
`--turns --pull-request`, pointer and keys through `orca computer`, VoiceOver read from its caption panel;
first the bundle of `1caaa582f5`, then that branch with exact2 #1180, the macOS host change that lets
VoiceOver reach the chips, which the clone now has from main):

- A chip's details popover takes the focus as it opens, as the reference's PopoverPopup does
  (`initialFocus`). Before, the focus stayed on the prompt: the shared field's press makes the prompt
  the first responder, and the popover's `autofocus` yields to a focused field. The root now moves the
  focus with `focus(id)` as the details open (`chipOpenFocus`, composer-members.contract). Driven
  targets: the popup ("Review comment, outline.ts L3, dialog" after a pointer press on a review comment
  chip, 6 of 6; "Skill Frontend Design, dialog" on the Settings prompt sample's skill chip, 7 of 7; an
  accessibility press (AXPress) on a review comment chip, 2 of 2) and a skill's View instructions ("View
  instructions, button" after a pointer press on a composer skill chip, 2 of 2). The terminal output
  target is not checked with real input: the fixture has no terminal excerpt. The app tests cover all
  three targets.
- VO-Right from Message reads each chip in text order and then 'Default, collapsed, button' (6 of 6:
  two review comments and an image chip, or a review comment and two skill chips), and VO-Left goes
  back the same way (2 of 2). On the Settings sample it reads the skill chip, both file chips and then
  the Monospace font heading (2 of 2). Before #1180, VO-Right went from Message straight to 'Default'
  and skipped every chip.
- VO-Space on a chip opens it with the focus inside: a review comment reads "Review comment, outline.ts
  L7, dialog" (5 of 5), the Settings sample's skill chip "Skill Frontend Design, dialog" (5 of 5), a
  composer skill chip "View instructions, button" (1 of 1), and an image chip opens "Expanded image
  preview, dialog" with Close image preview focused, as the reference's ExpandedImageDialog does (5 of
  5). Before #1180 VO-Space on a chip opened nothing.
- Escape gives the prompt the focus back with the chip selected: "Message …, edit text …, is selected
  Review comment, outline.ts L3. Show details" (27 of 27, after every popup and image preview above).
  On the Settings sample the chip's finish had never given the sample its focus back: it counted
  Settings itself, a route dialog, as a dialog over the sample.
- An outside press gives the prompt the focus back with the chip selected unless the press focused
  another element, as the reference's PopoverPopup does (`finalFocus`, true by default, which
  FloatingFocusManager applies when the press left the focus on the body or in the closed popup). A
  press on empty timeline ground returns it (3 of 3); the press focused the window's root, which counts
  as the body. A press on Search settings (2 of 2) or on the Typography heading (2 of 2) leaves the
  focus there, and on empty ground in Settings the Settings dialog keeps it (2 of 2). Before, the
  timeline's own pointerdown op replaced the details' close on their shared send, and the focus went
  nowhere; chip ops now have a send of their own (`chipChanged`).

**Checked with real input (2026-10-31, the v0.1.5 follow round)** (bundle of `fbf13e6f05`, a lane copy
on its own bundle id and home, `second` seeded with `--turns --pull-request`, the v0.1.5 oracle paired
with the same lane server). Presses, drags, wheel scrolls and keys came from the HID event source
(small CGEvent tools). `orca computer`'s accessibility right-click is not real input for native menus:
it opens the menu with its first item highlighted, and the next key or snapshot chooses that item.
22 rows: 17 hold and 5 failed; the five were fixed and hold when checked again (below).

Hold:

- A sidebar row's menu. Copy › Thread writes `[title](t3-context://v1/thread/thread_<id>)` (1 of 1).
  Turn into project, on a scratch thread with its own folder, makes the project, and the server
  moves the thread into it (1 of 1). Snooze lists the server's presets and In 1 hour snoozes the row
  (1 of 1). There are 5 presets, or 4 when This evening and In 3 hours fall on the same minute, as
  the Settings preview says. The row's clock popover lists the same presets (1 of 1). Escape on the
  menu chooses nothing (1 of 1).
- Custom snooze's Edit snooze options opens Settings › Threads with Snooze options in view (1 of 1).
- The thread title menu: Copy › Thread writes the same link and shows "Thread copied" (1 of 1);
  Turn into project on a scratch thread (1 of 1); Snooze › Tomorrow ("Snoozed, 18h left", 1 of 1).
- The palette's Copy thread row, and ⇧⌘C, copy the open thread's link (1 of 1 each).
- The sidebar tree: Tab from Search lands on a row, ArrowDown moves the focus ring, and F2 opens the
  rename field (1 of 1).
- F6 from the composer goes to the right panel, the terminal, the sidebar, the conversation and back
  (1 of 1). From the terminal, F6 reaches the sidebar and ⇧F6 the right panel (1 of 1). The panel
  takes the focus itself, where the oracle focuses its Diff scope menu (a known difference, #1187).
- The approval row: More lists Cancel and Always allow this session, Approve underlines on hover, and
  pressing Approve lets the turn complete with notes/approved.md (1 of 1). Decline was not pressed:
  the fixture seeds one approval.
- The composer's Permissions chip opens the four-mode toggle, hovering a mode describes it, and Full
  access leaves the chip as its icon alone, as the oracle draws it (1 of 1).
- Settings › Appearance › Thread details lists the ten items, and unchecking Servers reads "8 of 10
  on" (1 of 1).
- Thread details › Servers. A `python3 -m http.server` run in the thread's terminal shows as
  "localhost:16946, Terminal · Python". Hover shows Stop and "Open in the browser panel. ⌘-click for
  your browser.". A press opens the server in the Browser panel. Stop ends the server: the port no
  longer listens and the section goes (1 of 1).
- With that page live in the Browser panel, presses in the right half of the window reach their
  targets: the Diff tab, a link in the page, the timeline's Open diff, the details card's Changes and
  the tab's close button (5 of 5). Under the agent driver, #1186 saw these presses blocked.
- Dragging a sidebar row toward the bottom brings the dashed drop hint into view. Over Settled, the
  header lights up and the row shows Settle; the drop settles it ("Settled 1 thread, ⌘Z to undo")
  (1 of 1).

Failed, then fixed and checked again with real input (bundle of `7656e94b94`, a lane copy on its own
bundle id and home, `second` seeded with `--turns`; presses, right-clicks and keys from the HID event
source, each result read back from the accessibility tree or the clipboard). All five hold:

- Open diff on a tool call whose change has no recorded patch opened the Diff panel on the Tool call
  scope, as the oracle does, but read "No net changes in this selection." The oracle reads "No patch
  available for this selection." The panel now follows DiffPanel's `hasNoNetChanges` (a patch that
  resolved blank). Checked again: the README.md call's Open diff in "Turn with file edits" opens the
  Diff panel on Tool call, reading "No patch available for this selection." (1 of 1).
- Right-clicking the thread details card opened nothing (0 of 3: on its padding and on its Workspace
  row). On its padding the oracle opens Show in thread details; on the Workspace row, Copy full path.
  The card's menu was not menu-shaped (a check icon box beside each label), so macOS opened nothing;
  its rows are now text alone and the Workspace row has Copy full path (`shell-details.contract`).
  Checked again: a right-click on the padding between the card's sections opens the Mac's menu,
  headed Show in thread details, with the ten items and Reset to default. Choosing Project scripts
  hides the card's Add project script row, a right-click on the bottom padding shows the item
  unchecked, and choosing it again brings the row back. A right-click on the Workspace row opens Copy
  full path, which copies the lane repository's path and shows "Path copied" (3 of 3 menus).
- A thread chip in a sent message showed no tooltip on hover (0 of 2), and a double-click selected
  nothing (0 of 2), on inline code in an answer or a user message or on a plain word, though a drag
  selected text. The oracle shows "Open thread" and selects `selectme`. The session had opened a page
  in the Browser panel: a Browser tab neither shown nor held left its empty, transparent box over the
  window (1280 by 800 from the left edge while the panel is closed), which took the hover, the presses
  and the double-clicks meant for the timeline under it. The box is no longer displayed
  (`browser-frames.contract`). Checked again in three states: before any Browser tab; with a page
  open, after closing the panel, while its floating preview shows; and after closing that preview,
  which leaves the tab neither shown nor held, the state that failed. In each, hovering the chip shows
  "Open thread" (3 of 3), and a double-click selects `selectme` in the answer and in the user message
  and a plain word in each, as ⌘C then reads (12 of 12).
- After F2 and then Escape in the sidebar, the focus went to the window, and F6 never entered the
  sidebar after that: from the composer it stopped at the terminal (4 presses). In the oracle, Escape
  returns the focus to the row and F6 reaches it. Once the rename field has gone, the row's treeitem
  now takes the focus back, and F6 enters the sidebar at that treeitem (`shell-members.contract`).
  Checked again: Tab from Search to a row, F2, a typed character, then Escape leaves the title
  unchanged and the focus on the row (2 of 2). F6 from the composer then reaches the row first (2 of
  2); further presses go Conversation, composer, row.

Not checked:

- Effort words and bars: no provider has reasoning traits, and the lane's Codex is unavailable.
- Shift-click Viewed and Allow edits from maintainers: the fixture's pull request has no files. The
  oracle cannot load its detail either ("GitHub returned an unreadable getPullRequestDetail response").
- The update dialog, which needs a server older than 0.1.5.
- ⌘-click send, which needs a running turn.
- The fallback model notice.
- A palette move to another machine's scratch project.
- Decline.

Also seen: the model picker's "Codex: Unavailable…" tooltip stayed on screen after the picker closed,
until another tooltip replaced it.

Also seen on the `7656e94b94` bundle: with the Diff panel open, the header's thread details button opens
the details as a popover, as upstream's `PanelLayoutControls` does in that layout. An agent drive
had seen no change from that button. After the origin/main merge, opening a
thread, sending a message, the ⌘K palette (a query lists Threads and Settings results; Escape closes
it) and Settings › Appearance behave as before.

**Fixed, not checked with real input or VoiceOver:**

- Pairing a saved environment again reconnects it with the new credential, as the reference's
  `EnvironmentRegistry.register` restarts its connection: a background environment that failed for
  want of a credential connects and its row stops saying 'Connection failed'. Before, it kept the
  failure (`connections-repair.test.ts`). Checked by that test only: no fixture makes the failed
  credential cheap.
- The Thread title instructions textarea's grip (`resize: vertical`). Not checked: `orca computer
  drag` delivered nothing in this app (the grip twice, a drag-select in the prompt twice), and its
  scroll did not move a page either.

**Checked with real keys** (2026-10-16):

- Light dismiss on macOS (exact2 PR #676, rows H-6 and H-7): a real click in the composer, Search
  threads, Search settings, the terminal or a sidebar row closes an open popover menu (Project
  scripts, Git action options, Settings' Environment and Project scope), every time, with no
  change in the clone. A press dragged onto the menu keeps it open.
- A double-click on a sidebar row of another environment renames it: 23 of 23 runs in the flat
  sidebar and 21 of 21 in the legacy one, in both directions, with a text draft or an image and
  text draft on the destination thread (0 of 10 and 0 of 12 before). Rows are keyed by the thread
  with its environment, as the reference keys them, and a rename asked for before that
  environment's shell is in waits for the row.
- A press on the empty composer's placeholder line puts the caret in Message, as the placeholder
  takes no press, like the reference's `pointer-events-none` one (fresh bundle id).
- A Korean word typed (2-Set) with nothing editable focused composes whole in the composer:
  type-to-focus hands the key to the prompt instead of inserting its characters (fresh bundle id).

- Terminal, Korean 2-Set with VoiceOver on (C9): VoiceOver was not the cause. When another
  process selects the input source with TISSelectInputSource (an input switcher, or the
  real-input harness's own tool) while the terminal has the focus, WebKit's input context can be
  left with no input method, and keys reach the shell as the layout's characters (g k s). The
  native terminal reconnected its input context when the selected source changed; the terminal
  page's web view is the host's `iframe`, which does not yet (`clone/issues/terminal-page-socket-input-source.md`).
  Selecting the source before focusing the terminal composes. T3 Code under Chromium loses at most
  the first key in that case. Not yet investigated: in a freshly launched lane the native composer once sent the first
  2-Set syllable as jamo (ㅎㅏㄴ한글).

- Terminal, Korean 2-Set, first syllable: in an app it has not yet met, macOS's Korean input
  method edits Hangul in place without marked text (it inserts the jamo and then replaces the
  syllable before the caret). It switches to marked text only once the app's text stops holding
  what it inserted, and remembers that app until logout. The terminal page now keeps that text in
  its input and sends each syllable once, when the next one starts, so the first word after a
  launch composes whole (한, not ㅎㅏㄴ). ⌃C, ⌃L and ⌥Return behave as they do during a
  composition. This goes further than T3 Code: its surface under Chromium, in an app the input
  method has not met, sends ㅎ 하 ㅏ ㄴ한 (Chrome for Testing under a fresh bundle id). Re-checking
  needs a bundle id the input method has not seen.
- Terminal, Korean 2-Set, while a syllable composes: ⌥Return opens the Hanja list, as in T3
  Code: the syllable goes to the shell when the input method commits it, and the candidate picked
  (or the syllable again, on Escape) follows it, as under T3 Code's Chromium page. ⌃C commits the
  syllable alone, and a second ⌃C is the shell's. ⌃L commits it without clearing, as T3 Code
  does: its drawer reads the "Process" key's `key` ("ㅣ"). A ⌃ chord or ⌥Return the input method
  declines comes to the page with its own key right after the commit, where Chromium sends
  "Process"; the page counts it as the input method's (entry.ts `CompositionTracker`). A composing
  syllable is not drawn, in the clone as in T3 Code, whose terminal input is a hidden textarea.

Real-input sessions should type IME rows with held keys: orca's zero-dwell press-key can make the
input method commit a jamo at once. They should switch input sources with the system shortcut or
menu, not TISSelectInputSource, before they judge IME rows.

**Not checked yet:** the Usage page's progress bars under VoiceOver (the 2026-10-30 session had no
`--limits` seed), and an italic font face (no fixture has italic text). Also:

- Escape on the Merge confirmation over the thread-details popover in a narrow window. Not checked on
  2026-10-30: `orca computer` cannot resize the window, and its drags deliver nothing here.
- VO-Space on the chip kinds the 2026-10-31 session did not press (sample.py, a video, doc.pdf, the
  citation and the thread chip), and the terminal output as a chip's focus target.

**Out of scope** (decided 2026-10-17): the device player and the device menus, a tailnet, Screen
Recording and other OS grants (FN24, the permission helper's real grant), provider installs and
sign-ins, Kotoeri's ⌃K, Reduce motion and trackpad momentum by a person, and GitHub writes.

**Next real-input session:**

- Decline on an approval.
- Effort words and bars, with a provider that has reasoning traits.
- Shift-click Viewed, with a pull request fixture whose files the 0.1.5 server can read.
- VO-Space on the chip kinds not yet pressed, and a terminal excerpt's details.
- The textarea grip and the narrow-window Merge row. Drags from the HID event source reach the app,
  and the window can be resized through AX.
- The Usage page's bars under VoiceOver.

Open native context menus with a HID right-click, not `orca computer`'s. While VoiceOver runs,
its caption panel covers the composer of a window 840 pt tall, and `orca computer` refuses a click
there (`window_not_focused`): make the window shorter first.

**Fixtures.** `tools/second-environment`, `tools/machines` and `tools/acp-standin` (each has its
README) give This machine plus two lane environments, a machines list, and an ACP agent stand-in.
`seed second --limits` adds pooled usage limits from a CLIProxyAPI hub stand-in; `seed second
--turns` threads on the stand-in with a completed turn, a turn that edits files (a turn diff), an
approval waiting and a context meter, and its `$frontend-design` skill; the seed itself an empty
project and a project action; `seed second --pull-request` a thread whose branch has an open pull
request, with checks, comments, a review thread and a patch, from `gh` and GitHub API stand-ins; and `machines.mjs up --older` a
server older than the client that streams a scripted self-update.

**Framework issues opened by this app:** open on main is
[`issues/20261016-native-menu-pick-voiceover-old-focus.md`](../../issues/20261016-native-menu-pick-voiceover-old-focus.md)
(the clone no longer meets it: a right press focuses the row, checked above). Closed as the clone's is
[`issues/closed/20261017-macos-voiceover-s-press-moves-its.md`](../../issues/closed/20261017-macos-voiceover-s-press-moves-its.md)
(the citation link now leads to itself; checked above). Closed with #688 is
[`issues/closed/20261016-escape-in-recorder-blurs-on-macos.md`](../../issues/closed/20261016-escape-in-recorder-blurs-on-macos.md). The macOS issues opened from 2026-10-13 to 2026-10-16 are closed
on main, the light-dismiss and VoiceOver ones (exact2 PRs #663, #664, #676, #679 and #681)
included, and so is the pointer events one (`pointerdown` and `pointerup` now bubble to every
ancestor that hears them, and `stopPropagation()` stops them).

## Source and checks

The app root holds what the toolchain reads there: `app.contract`, `app.ts`, `app.json` and the
Cargo workspace. Every other Contract and TypeScript source is in `src/<feature>/`, and its tests are
in `tests/<feature>/`: `shell/` (the window's composition, header, toasts, tooltips and keyboard
dispatch), `sidebar/`, `composer/`, `timeline/` (the transcript, its Markdown and citations),
`highlight/` (the shared highlighter), `diff/`, `panel/` (the right panel's tabs and surfaces),
`details/` (the thread details card: Git, the pull request row, project scripts, lineage),
`pull-requests/`, `usage/`, `pages/` (the empty main view and the welcome wizard), `settings/`,
`appearance/` (themes and fonts), `connections/` (environments, pairing, This machine, SSH, server
updates), `browser/`, `terminal/`, `device/`, `palette/`, `providers/`, `media/`, `snapshot/`,
`client/` (the data source, protocol and event projection) and `shared/` (the dialog trap, menu
keys, icons and the shapes several features read). `tests/support/` holds the fixtures. A test
reads a source by its file name through `tests/support/app-files.ts`, so a file name is unique under
`src/` and `tests/`.

`app.contract` keeps the window's resources, mutations and tasks, the state they read and
the actions that write it or send (a child component may not own resources or assign root
state). Most of those members are written in members files beside their features
(`src/<feature>/*-members.contract`, LLP 1091.002): each `members "./src/…"` line in `T3Code`
stands for that file's `members T3Code` block, so the root stays under the line cap and compiles to
the one-file plan. Members files load after the files the root's `use` lines name, so those lines
still name what only the members read, which keeps the files' load order the one-file root's. A
test that reads the root's text reads it spliced through `appText` (`tests/support/app-files.ts`).
Its view is `T3Window` in `src/shell/app-window.contract`, which owns the window's view-only
state (the sidebar's hover and drag, the title menu, the settings editors' menus, Add
environment's fields) and lays out the components of `app-main.contract`,
`app-settings.contract` and `app-overlays.contract` beside it; each takes the root names it reads as
props of the same names, and an action that writes both sets the window's half and calls the
root's (`…Root`). `src/client/client.ts`, `protocol.ts`, `domain.ts` and `presentation.ts` own the
data source, commands and event projection; `src/transport/` owns the server connections (the
sessions, routes, saved-environment catalog, activity reporter and outdated hosts' update jobs, and
`transport.contract`, the slots app.contract gives them). A labelled `button` or `link` that may mount no text says `title=""`: since
LLP 1115 the Mac host shows the `aria-label` of a button whose mounted, not `aria-hidden` children
give no text as its help tag, and the reference shows only its own tips (`help-tags.test.ts`
applies the host's rule to the view tree). `modules/apple/` is the native module. `data/` is the
Rust data source (`t3-code-data`): the chat Markdown that `app.ts` leaves to Rust (`renderMarkdown`,
`renderPullRequestMarkdown`, `renderFileMarkdown`) over `apps/markdown/parse`, with its tests; it does
no I/O when it runs, so any host can link it, and its build script runs the generator below. `macos/`
is the macOS host crate (`t3-code-macos`: the bake and the Apple host's entry), its tests of the baked
plan, and the AppKit tests. `tools/` holds the lanes (`github-lane/`, `network-lane/`), the generator
of the generated sources (`generate/`) and the embedded server's staging (`stage-runtime/`). T3's MIT
notice is retained in `LICENSE-T3`.

Exact asks an answer again when a topic it watches changes. Since exact2 #183 a change
that arrives while the snapshot read is in flight lets that read's reply land and then asks
once more, so the topics go straight to Exact (the app's read gate is gone). An answer Exact
replaces for new arguments is let go with its replies unread, so each
WebSocket RPC carries a trace id the transport lists while it is pending, and "Some requests
are slow" counts only requests the server has not answered.
The transport follows the reference's reconnect policy (jittered 1 s·2ⁿ⁺¹ ladder capped at
five minutes, reset after 30 s connected; Retry, returning to the app and an offline report
probe the live socket instead of replacing it) and resubscribes a failed stream on the same
session after 250 ms doubling to 30 s.

Tests should exercise the client or execute Contract actions through the existing fixtures.
For example, invoke a menu action and assert its recorded hide, focus and dispatch calls;
matching those calls as source text does not check their execution. Keep source scans for
cross-file invariants, such as menu target ownership, dialog focus guards and heading levels.
Avoid snapshots of whole source lines, CSS values, component inventories or retired workarounds:
they duplicate the implementation and fail on harmless edits. Native focus, hit testing and
layout still need the AppKit tests or an app drive; passing a source scan does not prove them.

```sh
bun examples/t3-code/tools/generate/generate.mjs   # the generated sources, first
bun test ./examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o /tmp/t3-code.plan
cargo test -p contract --test it button_migration -- --ignored   # the async lane's sweep of this app: app.contract and its imports (minutes)
cargo test --manifest-path examples/t3-code/Cargo.toml --lib   # the app's workspace: the bake, then its tests (Hermes env of the root setup)
cargo test --manifest-path examples/t3-code/Cargo.toml -p t3-code-data   # the chat Markdown's tests alone, without the app's bake
```

A Cargo build alone does not compile or launch the Apple crate's Swift module. Use the app
build above for an integrated check.

The module AppKit/XCTest binaries under `macos/tests/<name>/` build with Exact's
module facade, the app's generated data keys, every file in `modules/apple/` (the
`composer` and `menus` tests define their own `exactModule`, so they leave
out `T3Module.swift` and its `T3Module+<area>.swift` op files; an `extension T3Module` goes in one
of those op files, never beside other code, or these three binaries do not compile) and the test
directory's sources, and every one the rest of Exact's module seam
(`host/apple/compose.mjs`'s `moduleSeam`: `ExactNativeViews.swift`, `ExactTamber.swift`,
`ExactExtensions.swift`) with empty extension and view tables (`ExactComposed.swift`, as
`host/apple/build.mjs` composes it for an app that uses no package).
`EXACT_ASSETS` must be an absolute path: the modules resolve their bundled assets under that
root. Run from the repository root:

```sh
X=$(xcode-select -p); F="$X/Platforms/MacOSX.platform/Developer/Library/Frameworks"; L="$X/Platforms/MacOSX.platform/Developer/usr/lib"
R="$PWD/target/t3-tests"; mkdir -p "$R"; export T3_APP_DIR="$PWD/examples/t3-code"
T3_DK="$R" bun -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_DK + "/ExactDataKeys.swift");'
T3_DK="$R" bun -e 'import { composedModule } from "./host/apple/compose.mjs"; await Bun.write(process.env.T3_DK + "/ExactComposed.swift", composedModule([], true));'
for d in examples/t3-code/macos/tests/*/; do
  n=$(basename "$d"); O="$R/$n"; mkdir -p "$O"
  [ "$n" = timeline-keyboard ] && continue # Actual host regression; separate recipe below.
  M=$(ls examples/t3-code/modules/apple/*.swift); case $n in composer|menus) M=$(echo "$M" | grep -v /T3Module);; esac
  T="host/apple/modules/ExactNativeViews.swift host/apple/modules/ExactTamber.swift host/apple/modules/ExactExtensions.swift $R/ExactComposed.swift"
  xcrun swiftc -swift-version 5 -module-name "T3$(echo $n | tr -d -)Tests" -F "$F" -I "$L" -L "$L" \
    -Xlinker -rpath -Xlinker "$F" -Xlinker -rpath -Xlinker "$L" \
    host/apple/modules/ExactNativeModule.swift $T "$R/ExactDataKeys.swift" $M "$d"*.swift -o "$O/$n-tests" || continue
  [ $n = snapshot ] && { rm -rf "$O/fixture"; mkdir -p "$O/fixture"; export T3_SNAPSHOT_TEST_ROOT="$O/fixture"; }
  EXACT_ASSETS="$T3_APP_DIR" T3_BROWSER_TEST_DIR="$O" T3_COMPOSER_TEST_DIR="$O" T3_MENUS_TEST_DIR="$O" T3_PANELS_TEST_DIR="$O" "$O/$n-tests"
done
```

The `timeline-keyboard` regression uses the actual ExactKit key loop and scroll
views. It compiles ExactKit's sources into its own binary, over ExactCore, which
ExactKit imports, built first as a module of its own. Both pass
`-package-name apple`, the name SwiftPM gives `host/apple/Package.swift`'s
package (their `package` declarations need one). After building the macOS
app (`bun host/apple/build.mjs t3-code-macos`, which writes
`examples/t3-code/target/aarch64-apple-darwin/host-dev/libt3_code_macos.a`), run it from the
repository root:

```sh
mkdir -p target/t3-tests
xcrun swiftc -swift-version 5 -package-name apple -module-name ExactCore -parse-as-library -I host/apple/Sources/CExact \
  $(rg --files host/apple/Sources/ExactCore -g '*.swift') \
  -emit-module -emit-module-path target/t3-tests/ExactCore.swiftmodule -emit-library -static -o target/t3-tests/libExactCore.a
xcrun swiftc -swift-version 5 -package-name apple -module-name ExactKit -I host/apple/Sources/CExact -I target/t3-tests \
  $(rg --files host/apple/Sources/ExactKit -g '*.swift') \
  examples/t3-code/macos/tests/timeline-keyboard/main.swift \
  -L target/t3-tests -lExactCore -L examples/t3-code/target/aarch64-apple-darwin/host-dev -lt3_code_macos -lc++ \
  -o target/t3-tests/timeline-keyboard-tests
target/t3-tests/timeline-keyboard-tests
```

The terminal page's tests are Bun's (`terminal-host/src/*.test.ts`, the vendored surface's among
them); `bun examples/t3-code/terminal-host/verify-vendor.mjs` checks the vendored binaries. The viewer
page's are too (`file-preview-host/src/**/*.test.ts`, the reference's PDF layout and find tests).
The SSH tests read
a temporary home, never `~/.ssh`; the live tunnel test skips unless `T3_SSH_COMMAND`
names an ssh test double, and agent runs of the app read hosts only from `T3_SSH_HOME`.
`macos/tests/ssh/fake-ssh.sh` is that double (its header lists the variables): the password
prompt tests run against it always, the live test when `T3_SSH_COMMAND` names it and
`FAKE_SSH_REMOTE_HOME` holds a running server's `.t3/userdata/server-runtime.json`.
`T3_PERMISSION_HELPER_EVIDENCE` set to a directory renders the snapshot test's permission helper panel.
**Check for Updates…** (under About and in Help) reads the bundle's receipt: a build
without an update store (`deploy.store` is `"0"`) shows the reference's "Automatic
updates are not available right now." box; a build with one does nothing (the module has no
hook into the host's check).
