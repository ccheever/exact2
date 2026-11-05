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
The app carries the T3 server built from the reference pin (the CLI archive upstream's release
workflow makes, built here from that commit, unmodified; `server-runtime/runtime-pin.json`) and runs
it as its own local server, "This machine". It does not modify T3 Code.

The reference is T3 Code `5d3d78ab93`, upstream main after v0.1.6, whose server and Beta app report
0.1.6 (`referencePin` in `server-runtime/runtime-pin.json`; `CLIENT_VERSION` in `version-skew.ts`). Ported files name
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

The embedded server. `tools/stage-runtime/stage-runtime.mjs` takes the archive named by
`server-runtime/runtime-pin.json`: a release pin's is downloaded and accepted only when its SHA-256,
the release's `SHA256SUMS` and the pin agree; a build pin's (no release publishes the commit) is
built as the pin's `build` steps say and copied into `.runtime-cache/`, and accepted when its SHA-256
is the pin's. Then it checks the signatures of its Mach-O files (ad hoc for a build), runs `t3 --version`,
starts it once on a scratch home and a lane port, and puts the archive and its manifest beside the pin
(`--offline` uses only the cache). The bundle carries that folder as
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

Safari's Web Inspector: every web view the clone shows is an Exact `iframe` (a Browser tab, a rendered HTML
preview, the terminal, a Mermaid diagram). Unasked, the web arm makes these `isInspectable` in a development build
only (main #309), so Safari's Develop menu lists them there. A Browser tab's ⋮ › Open DevTools opens that tab's
inspector, and makes it inspectable, in any build, a distributed bundle included (`openDevTools`, LLP 1020.002 D11,
ruled 2026-10-31). There is no View › Toggle Developer Tools: an inspector for the app's own UI stays out of exact2
(#101).

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
  (`highlights.ts`), audio and video in the platform's players (an expanded video preview has Chromium's
  controls over it, `preview-player.contract`), "Unable to
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
  floating player, profiles, and an agent's `previewAutomation` requests. A server-rendered tab's floating player pops
  it into Chromium's picture-in-picture window as Exact draws it (`requestPictureInPicture(…, presentation="window")`,
  LLP 1042 §9.1; `server-browser-pip.ts`): a stream of its own paints a hidden image that it floats, as the reference
  floats its canvas stream. A desktop tab's separate preview window is Exact's floating panel
  (`presentation="panel"`, `browser-desktop-pip.ts`) over the frame's captures, about 12 a second. As in the
  reference, each desktop tab keeps its own window beside the others and beside the server tabs' one window, which
  moves to the next server tab that pops out (LLP 1042 §9.2).
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
places, stepping by half a Chromium zoom level as a page zoom, `setPageZoom`; `ViewZoom`), and the window's root removes the Develop
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
draft in the most recently active project (by latest user message; a visit is not activity) of every
environment, reading each one's shell from the cache it kept since the last launch. The clone keeps no shell
between launches, so its landing (`home-landing.ts`) waits until every switched-on environment, This machine
included, has its shell in or has settled offline: refused, or waiting to retry after its third failed attempt
(`state/shell.ts` `allEnvironmentShellsBootstrappedAtom`). It also waits while This machine's server starts, as
the reference opens its window only once the local backend is ready (DesktopWindow.ts
`createMainIfBackendReady`). It focuses the picked project's environment when it is not the one the launch
reconnected.
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
share them; the keys an older clone kept in `t3-code.json` are carried over once. Where an
administrator's managed policy (`/Library/Application Support/T3Code/policy.json` or the
`com.t3tools.t3code` configuration profile) leaves `remote` out of `limits.clients`, or cannot be
read, the server starts local-only without Serve, as DesktopServerExposure does, and the saved
choices stay for when the policy allows them (`T3ManagedPolicy`, `T3LocalNetwork.swift`). The
server starts before any TypeScript runs, so this check is the module's. The default
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

- A data module's timer lives inside the answer that armed it (LLP 1027.000 D1), so the waits in the
  middle of a flow (View source's and Edit from here's polls, the Browser's paint and capture
  timeouts, the debounces) are the flow's own, on the app's clock. A flow whose answer is let go
  while it waits on a timer stops there, its `finally` unrun, so each flow that holds state across
  a wait has a send no other flow replaces (`app.contract`): View source, a transcript chip's or the
  composer's (`citationShown`); the Browser's Capture, whose Record start waits for two frames
  (`capturePressed`, queued); the
  checkout dialog's edits, whose next edit takes the debounce over (`prCheckoutTyped`); and the
  prompt links, whose switch to another machine's Scratch project polls (`fragmentChanged`, queued).
  Time is still an argument, and work outside an answer (the transport's tick, the sidebar's
  graces) is Contract tasks.
- Edit › Undo chosen from the menu while the undo notice claims ⌘Z is thread.undo, also with a field
  focused, where the reference's menu item undoes the field's text (its ⌘Z is thread.undo then too).
- Check for Updates… shows in every build. In a build with an update store its press would do nothing (the
  module has no hook into the host's check); no build has one while `deploy.store` is `"0"`.
- The managed policy's update keys are a known difference (the user, 2026-11-04). The clone is an open-source app
  built on exact2, not T3 Code's product run for an organization, so it has no administrator and no in-app updates
  (`deploy.store` `"0"`). The policy's `limits.updates` `mode: "off"` already matches, since nothing checks;
  `feedUrl` has no effect, since nothing updates.
- A backdrop blur sees only its parent ([`issues/closed/20261010-macos-backdrop-beyond-parent.md`](../../issues/closed/20261010-macos-backdrop-beyond-parent.md);
  main keeps the limit), so the Usage page's segment popover is opaque where the reference's glass shows the page
  through it. The composer's glass is a layer of its own in the chat column, placed at the card's box as last measured
  (`composer-editor.contract` ComposerGlass), so it moves with the card once the card's new size is measured.
- The Usage page draws a segment's popover in its scroll area, so the area clips it: a popover that v0.1.6 opens above
  its segment over the page's header (a weekly window's, near the top) opens below it here, and one at the area's edge
  is shifted inside the area, not the window.
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
  (`dialog-trap.contract` says why). A click on a Mac button and an accessibility press focus
  nothing, where Chromium focuses the pressed button and Base UI gives the focus back to it, so
  each opener names itself as it is pressed (`dialogOpener`, `app-window.contract`) and the
  dialog it opens gives the focus back there. Not covered: a dialog no press opens (the welcome
  wizard, the pairing and page-permission confirms, the SSH prompt, the ChatGPT plan notice)
  returns the focus held as it opened, as Base UI does; an opener in a menu returns it to the
  menu's trigger; and Usage's pooled reset-credit confirm returns it nowhere, as its opener is in
  a hover popover that has closed.
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
  source, LLP 1020.002 D8) and the agent's cursor drawn on the page's view need the page's web view; the artifact directory is the local server's own folder, outside the app's
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
  (app.contract `interacted`). The terminal's page reports what bubbles to its own document, as the
  reference's listeners hear its terminal in their page: a pointer, a press, the wheel and a key the
  surface leaves alone, not typed text or an arrow, which the surface stops (measured in v0.1.6).
  The Browser's web view keeps its events, as the reference's guest page does. The root
  hears a hovering pointer only while the window is key (ExactKit's hover tracking, LLP 1115.000
  D7); Chromium's page hears one while its window is key or main, so the two differ only over the
  main window while a panel of the app is key. Over a background window neither hears one.
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
  store, so a site that keeps its sign-in there asks again once. A hidden tab is not displayed: WebKit runs its timers
  about once a second and draws no animation frames, where the reference's tabs keep running (LLP 1020.002 §9); one
  that is recorded or shown in its separate window stays laid out, transparent, and keeps drawing. With
  the page focused, ⌘R reloads it, the zoom chords zoom nothing, neither the page nor the window, as in v0.1.6 (the
  page is asked `document.hasFocus()`, since a frame reports no focus), and ⌘L puts the focus in the address bar,
  selecting the address; ⇧⌘R reloads the window, as the
  reference's menu does. A page's dialog while
  an agent request runs on its tab is answered at once (OK for `alert`, Cancel for `confirm` and `prompt`); one left
  from an agent's run shows over the page as Chromium's tab-modal dialog and takes the person's answer. The console,
  network log and media scripts go in as each document commits, so what a page logs before that is not kept
  (clone/issues/browser-preload-without-taking-the-frame.md); the console, network log and audible state are read
  from the page every few seconds
  rather than pushed. The tab's icon is the page's first declared icon, loaded by URL, where the module redrew it.
- A server Browser tab's PDF is Chromium's PDF viewer drawn by the viewer page over PDF.js, which reads its signed
  URL (`server-pdf-viewer.ts`, `file-preview-host/src/pdf/browser-pdf-viewer.ts`), where the reference embeds the URL
  in Chromium's own viewer: an embed would run an HTML answer as a page. Print prints the PDF's own pages through
  the platform's print panel, a sheet on the window, as Electron's viewer opens the system's (LLP 1020.002 D5); under
  the agent it is a `print` request, and `tap @t ok` saves the job under `app:/tmp/print/`. Two things differ.
  Chromium's drawing tools (Draw, Undo, Redo) are not there. Present fills the tab, not the screen. Neither viewer
  has a find bar (Electron draws none). A PDF from another origin loads only when its server
  lets the page fetch it (CORS), as the T3 server does. A rendered HTML preview is a sandboxed `iframe` (LLP 1020.002):
  a right-click shows WebKit's menu, not the desktop shell's. Every new window it asks for opens in the default
  browser, as the reference's do.
- The host opens the first window before the embedded server is ready (#117 was closed as not
  planned), so the window shows the connecting state first.
- The launch's landing waits for every switched-on environment's shell (`home-landing.ts`), where the
  reference picks at once from the shells it cached. An environment that stays unreachable through three
  attempts is left out of the clone's pick; the reference still offers its cached projects.
- Code colours: a code text paints plain until the data crate's lexers answer for it, a few frames
  through the root's pump (`app.contract` highlightStep), where the reference's loaded highlighter
  colours it in the render that shows it. The lexers share the data crate's thread with the chat
  Markdown, so each round lexes about 17 ms of code: a file of 1M characters takes about 30 rounds,
  where the reference's worker pool lexes it whole beside the page.
- Split view's empty side stripes every 10 pt, where the reference stripes every 8 px: each row
  of the virtualized list paints its own gradient, and main keeps `background-attachment: local`
  deferred, so a shorter period would break at every row edge.
- A reply's web link shows no tooltip with its URL when the pointer rests on it, where the reference's anchor has one
  (its Tooltip, side top): each word of a link is a node of its own (`FlowRuns`), so no one box holds the link to
  place it by. An image inside a link shows its URL on a keyboard focus, as the reference's does.
- A focused timeline button shows AppKit's focus ring outside it, where the reference's tool rows ring inside their
  box (`ring-2 ring-inset`) and its inline buttons draw Chromium's outline.
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

- Settings › About › Update track saves only this app's own track. In v0.1.6 the switch also moves every
  connected server between Stable and Nightly, after a database-migration check
  (`UpdateTrackSwitchDialog.tsx`). The clone is updated through Exact delivery and has no desktop
  update feed (`settings-a-about.ts`), and Nightly is not cloned. The row keeps v0.1.6's description but not
  its "N machines still run … Review" line: Review opens that switch in v0.1.6, and here it would move no machine.
- A two-finger swipe steps the window's history with AppKit's gesture (`trackSwipeEvent`, LLP 1115.000 D5.5),
  which follows System Settings › Trackpad › Swipe between pages; v0.1.6 recognises the swipe over wheel events
  (`HistorySwipeNavigation.tsx`) whatever that setting says. The window root hears AppKit's tracking
  (`historyswipe`) and draws v0.1.6's edge arrow from it (`history-swipe.contract`), but the distances are AppKit's:
  a lift at rest steps from about 250 points of travel, where v0.1.6 steps from 120 px, and a quick flick steps
  sooner, which the arrow cannot know, so it may look unarmed as the step goes.
  The window root offers only the steps ⌘[ and ⌘] take (`navigationHistory`), so no swipe is tracked and no arrow
  drawn under the palette or a dialog other than Settings, past the history's 50 visits, or onto an archived thread.
  v0.1.6's swipe differs in two of these, measured on the oracle: it ignores a swipe only with the pointer over the
  palette or dialog (beside it, over the backdrop, it steps the page under the open palette), and it steps onto an
  archived thread's route, which opens a new-thread draft. The clone's Back and Forward skip both
  (`sidebar-members.contract` `historyGo`, `keyboard-dispatch.ts` `liveEntry`), and its swipe follows them.
- A pressed chip in either composer takes v0.1.6's 2 px ring (`index.css` `.ProseMirror-selectednode`) in the
  default theme's focus colour: an atom kind's paint is a literal, so a custom theme's focus colour does not reach
  it. A selection that holds a chip and more text draws the chip as AppKit's selected token, where v0.1.6 lays
  its 30% `Highlight` over the chip.
- The legacy sidebar is removed, as in v0.1.5. In v0.1.5 a saved `legacySidebarEnabled` opens the boring
  sidebar, which is a Labs feature, and ⌘N there starts the thread in the current project. The clone
  has no boring sidebar, so the same setting opens the default sidebar, where ⌘N with several projects
  opens "New thread in…". For the same reason, the Working section's description leaves out v0.1.5's
  "The boring sidebar in Labs has no Working section."

- After VO-Space on a composer thread chip, the reference's VoiceOver cursor ends on the window's close
  button (VO-F3 "close, button", then VO-Right "minimize, button"; 3 of 3), though it has just read the
  new thread's prompt: Chromium drops the pressed link. The clone's stays on the prompt it read, which
  has the keyboard focus, as VoiceOver's cursor does in an AppKit window.
- The Browser tab's ⋮ › Open DevTools is missing on this Mac's own tabs, the common case. The reference
  draws those tabs natively in Electron and offers the row there; the clone opens them as server tabs, the
  pinned server's headless browser drawn from its frames, which have no web view to inspect. Only a tab of a
  paired environment, or of a server without a browser, is a web view in the clone and has the row. The web
  build has no row, as the reference offers it only through the desktop bridge. What the row opens is Safari's
  Web Inspector, not Chrome DevTools. The pinned server offers no way in either: it launches its Chromium through
  Playwright over a pipe (`--remote-debugging-pipe`) with fixed flags and reads no setting that adds a debugging port
  (reference `apps/server/src/preview/ServerBrowserContexts.ts:88-104`).
- No development web view inspection on Android: the clone does not run on Android yet.
- Settings › About has no Legal › Privacy policy row. v0.1.6's reads "How we handle your data, including the anonymous
  usage data T3 Code collects"; the clone collects no usage data (telemetry is out of scope), so that row would be false.
- Settings › Devices › Device sources never lists Simlock pools: the reference offers them only after Settings › Labs ›
  Simlock device hosts is on (`LabsSettingsPanel.tsx` `SimlockHostsRow`, `DeviceSourcesSettings.tsx`
  `supportsSimlock && simlockEnabled`), and Labs is not cloned.
- A composer reference's icon is v0.1.6's for a file (its Pierre type icon in the scheme's tint), a folder and a
  thread whose project has a favicon. An image chip shows a photo glyph, not the image's thumbnail: the staged
  image is drawn by the snapshot module's tile (`T3SnapShot.swift`), not from a file an image source can name.
  The other chips, and a thread whose project icon is a monogram, an emoji or a Lucide glyph, show the nearest
  SF Symbol to v0.1.6's Lucide glyph.
- A citation's comment opens on ⌥-click of its pill, or by itself after Cite; the reference's
  chip has a pencil button inside it, and here a pill is one press target (LLP 1045.000 §6).
- A composer chip's name underlines under the pointer only while the window is key, as AppKit tracks
  hover; an unavailable reference underlines too, where v0.1.6's is a plain span that does not. An atom
  kind's `:hover` is the kind's, not one chip's.
- With Markdown formatting off a reference shows its source: the editor draws atoms only where
  it styles Markdown (LLP 1045.000 D3). (A literal answer has no chips, as in the reference.)
  Settings › Appearance's prompt sample styles Markdown so its chips draw, where the reference's
  disables rich syntax.
- A character typed right after a mention or a skill makes its source longer, and deleting the
  space after one makes it text again; the reference keeps the chip until its document is
  rebuilt (LLP 1045.000 §6).
- A folder dropped or pasted on the local server or a saved loopback one is referenced by the path
  `storage.fs.realpath` gives its `doc:` handle (LLP 1069.010 D1, 2026-11-04), which Exact
  canonicalizes when the folder is dropped: one under `/tmp` is `/private/tmp/…`, where the reference
  keeps the spelling of the drag. Folders under the home directory are the same.
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
- With the focus in a server tab's page on this Mac (a press on its stream), ⌘=, ⌘- and ⌘0 zoom the window; the
  reference draws those tabs natively and zooms nothing. After the press the focus is on the right panel's region, not
  on the stream's key field (`server-browser.contract`), so the chord reaches View ▸ Zoom In. A desktop tab's page,
  which holds the focus itself, zooms nothing, as in the reference (2026-11-05).

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
  focuses its Diff scope menu, as the oracle does (agent drive, 2026-11-02; the panel itself before).
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
  returns the focus to the row and F6 reaches it. Once the rename field has gone, the row's button
  now takes the focus back, and F6 enters the sidebar at that row (`shell-members.contract`).
  Checked again: Tab from Search to a row, F2, a typed character, then Escape leaves the title
  unchanged and the focus on the row (2 of 2). F6 from the composer then reaches the row first (2 of
  2); further presses go Conversation, composer, row.

The rows the first session left (below, the second session) were checked on 2026-10-31 too.

**Checked with real input (2026-10-31, second session)** (bundle of `7656e94b94`, a lane copy on its own bundle id
and its own `HOME`, This machine on the fixture's local home with no folder for threads without a project, `second`
seeded with `--more-turns --pull-request --limits` and started with `--scratch`, `third` on the 0.1.4 release behind
the older-server stand-in reporting 0.1.4; the v0.1.5 oracle paired with the same two lane servers). Presses, drags,
right-clicks and keys from the HID event source; each result read back from the accessibility tree, the lane
server's API or VoiceOver's caption panel; the oracle driven the same way in the same state. 11 rows hold, 3
fail (one is the video chip of the chips row) and 2 are not reachable with these fixtures; all three were fixed
and hold when checked again (below). The oracle's doc.pdf and sample.py
chips were not opened under VoiceOver: an XCTest run of another lane took the front window during that step.

Hold:

- Effort words and bars. The stand-in's effort menu lists Low, Medium, High and Max; High reads "Effort: High" on
  the composer control and the next turn replies "effort high" (1 of 1). Appearance › Effort display › Bars draws
  rising bars: Low fills one of three, Max is named (1 of 1 each). The oracle draws the same and its turn replied
  "effort medium" after Medium (1 of 1).
- Shift-click Viewed on the pull request's Code tab: Viewed on the first file, then Shift on the last, marks all
  three, and the server's `pullRequests.filesViewed` reads all three viewed (1 of 1). The oracle does the same (1 of 1).
- Allow edits from maintainers, in the pull request's actions menu, unchecks and checks again; the server's detail
  reads `maintainerCanModify` false, then true (1 of 1). The oracle's menu offers the same item and its toggle reads
  back the same (1 of 1).
- Settings › Environments › Update all on Lane Third (0.1.4) shows "Downloading…" and leaves the row with no version
  once the stand-in reports 0.1.5 (1 of 1).
- ⌘-click send while a turn runs: the send tooltip reads "Click to steer, ⌘-click or ⌘Enter to queue", and ⌘-click
  queues the message (the queue reads "2 queued") (1 of 1). The oracle queues it the same way (1 of 1).
- Decline on an approval: the turn ends with "Stand-in reply: left the notes as they were" and no notes/approved.md
  (1 of 1); the oracle the same (1 of 1).
- The palette's New thread without a project, with This machine offering no folder: the thread starts in No project
  on Lane Second, whose scratch project the server lists (1 of 1). The oracle the same (1 of 1).
- The Usage page's bars under VoiceOver: interacting with the page reads, per window, "Session", "58%", "left", the
  pace ("Under pace: headroom left for the rest of the window, image"), the reset credit line and each account's
  segment ("ST: 20% left, resets in 38m, 13% of the window left, dialog pop up collapsed, button"; with "2 reset credits
  banked" where it has them), for Codex and Claude (1 of 1). The oracle speaks the same strings, as one group per
  window.
- VO-Space on composer chips: sample.py opens its file preview with Disable word wrap focused (Save file since
  2026-11-02, below), and doc.pdf the PDF
  preview reading "1 / 1 pages" (1 of 1 each). VO-Right from Message reads the three chips in text order, and Escape
  gives the prompt back with the chip selected. In the oracle the chips are reached by interacting with Message
  (VO-Right from it goes to Attach files); doc.pdf and sample.py were not opened there (below).
- An italic face: with Interface font Georgia, the stand-in's answer draws its emphasis in Georgia Italic, and bold
  with italic in Georgia Bold Italic (1 of 1). The oracle the same (1 of 1).
- The Thread title instructions textarea's grip: a drag of 60 pt from the grip grows the field by 60 pt (1 of 1); the
  oracle's grows the same (1 of 1).

Failed, then fixed and checked again with real input (2026-11-01, bundle of `5954688bbc`, a lane copy on its own
bundle id and `HOME`, `second` seeded with `--turns --more-turns`, `third` on the 0.1.4 release behind the older-server
stand-in; the v0.1.5 oracle paired with the same two lane servers; presses, moves and keys from the HID event source).
Both hold:

- The model picker's "Codex: Unavailable. Could not start Codex CLI …" tooltip stayed on screen after Escape closed the
  picker, and after the pointer moved away. The hover layer draws it at the window level, and the rail button went with
  no pointer leave. The root's hover surface now counts the picker (`hoverSurface`, app.contract), and its change ends
  the tip. Checked again: hovering Codex shows the tip, Escape removes it, and 3 s after the pointer moves away it is
  still gone (3 of 3); the oracle the same (1 of 1).
- The update dialog listed Lane Third under "1 skipped", "t3@0.1.5 is not on npm yet". The npm check was a fetch the
  shell's answer made and waited on, and the runner lets that answer go on each revision and each 500 ms tick: a trace
  of the old bundle shows the request forgotten 85 ms after it was sent, after which the check read "not published"
  for five minutes. The check is now the `npmRelease` resource's answer, keyed by the version and its five-minute
  window (`sidebar/npm-release.ts`), outside the ordered fetch queue. Checked again: just after pairing, the footer
  reads "Update 1 server" within a second, and its dialog lists Lane Third as a Server row; again after a relaunch
  (2 of 2). The old bundle in the same state shows no pill (1 of 1). The oracle lists Lane Third as a Server row
  (1 of 1); its own machine adds a Codex provider row, which the clone, with no local server in this lane, has not.

Also seen in this session, then fixed and checked again (bundle of `d9bb57b588`, the same lane):

- The footer's "Check for updates" tooltip stayed on screen once the update pill appeared, also after the pointer moved
  away. With the pointer resting on the idle pill as the app starts, npm's answer replaces that pill with the update
  pill under the pointer, which gets no pointer leave, so the root's sidebar hover stayed on the idle pill. A root task
  now ends it when the idle pill gives way (`sidebarUpdateTipGone`, app.contract). Checked again: four relaunches with
  the pointer resting on the pill show no tooltip, then or after the pointer moves away (4 of 4); opening and
  cancelling the update dialog leaves none (1 of 1). The previous bundle keeps "Check for updates" (1 of 1). The
  oracle shows the update pill's own tooltip, "Update 1 server and 1 provider", while the pointer rests there, and none
  once it moves away (1 of 1); the clone's update pill shows no tooltip while the pointer rests where the idle pill
  was.

Failed, then fixed in the macOS host and checked again with real input (2026-11-01, a bundle of this change with
exact2 #1225 merged, a lane copy on its own bundle id and `HOME`, the fixture's samples and a PNG attached as chips;
keys from the HID event source, VoiceOver read from its caption panel; the v0.1.5 oracle with the same files
attached). It holds:

- VO-Space on the clip.mp4 chip focused Close, and VoiceOver read "Close video preview, button", then half a second
  later "Expanded video preview, dialog" (3 of 3); the image preview read the same way (1 of 1). The oracle reads
  "Expanded video preview, dialog, with 3 items, Close video preview, button" (1 of 1). VoiceOver names a dialog before
  the control that takes the focus only when a web area is among the dialog's ancestors, as every dialog in Chrome
  has; the macOS host had none, and named a dialog drawn in the page itself, after the control. Each such dialog now
  has a web area of its own as its parent, and the host announces nothing (exact2 #1225, LLP 1080.003 D3.1). Checked
  again: VO-Space on the clip.mp4 chip reads "Expanded video preview, dialog, with 22 items, Close video preview,
  button", with the focus on Close (5 of 5); on the photo.png chip "Expanded image preview, dialog, with 5 items, Close
  image preview, button" (3 of 3; the oracle "with 3 items", 1 of 1); on the sample.py chip "sample.py, dialog, with 9
  items, Disable word wrap, button" (3 of 3; the oracle focuses Save file and reads "sample.py, dialog, with 8 items,
  Save file, button", 1 of 1). The counts are the clone's: its previews serve more children than the oracle's (the
  video preview's player controls, the image preview's second Close and its zoom). The Settings sample's skill chip
  reads "Skill Frontend Design, dialog, with 2 items" (3 of 3; before, "Skill Frontend Design, dialog"), and the font
  picker "Interface font family Search fonts..., dialog, with 2 items, Search fonts, expanded, combo box" (3 of 3;
  before, the field, then "Interface font family, dialog"). The 2026-10-31 "no control focused" was not seen (0 of 11
  opens by VO-Space).
- Matched on 2026-11-02 (a lane copy of that day's bundle, the same PNG and sample.py pasted as chips; VoiceOver read
  from its caption panel). The image preview reads "Expanded image preview, dialog, with 3 items, Close image preview,
  button" (2 of 2: VO-Space and an accessibility press): its children are now the oracle's, an sr-only title, Close,
  and one group holding the picture, its zoom and its name, with the backdrop's second Close out of the tree. The
  sample.py preview reads "sample.py, dialog, with 8 items, Save file, button" (2 of 2), as the oracle does: it has
  the oracle's sr-only title, opens on Save file, and mounts its wrap and copy buttons a moment later, as the oracle
  mounts them once the text has loaded (10 children then, as in the oracle). Closing either by Escape gives the focus
  back to the control that opened it, the image's Preview button (2 of 2) or the prompt for the file chip, an atom of
  the field. A dialog opened by a real click also gives it back to its opener: Settings › Appearance › Add theme,
  Escape, the focus on Add theme (1 of 1; the agent drive: on the Settings dialog before). The welcome screen's Add a
  computer moves the focus to the Pairing link field, as the oracle's does (agent drive and the oracle, 1 of 1 each;
  Continue kept it before).

Not reachable with these fixtures:

- The fallback model notice. The 0.1.5 server sets `usageLimits` only on providers that read an account's usage
  (Codex app-server, Claude, Cursor, Grok, OpenCode, Antigravity), each signed in to a real account; the ACP stand-in
  reports none, so no default provider is ever out of usage.
- Escape on the Merge confirmation over the thread-details popover, and the narrow-window Merge row: the window
  resizes through AX, but the pull request thread's details show no Git actions with Merge (its branch is not checked
  out in the lane), so there is no Merge to press there.

Also seen on the `7656e94b94` bundle: with the Diff panel open, the header's thread details button opens
the details as a popover, as upstream's `PanelLayoutControls` does in that layout. An agent drive
had seen no change from that button. After the origin/main merge, opening a
thread, sending a message, the ⌘K palette (a query lists Threads and Settings results; Escape closes
it) and Settings › Appearance behave as before.

**Checked with real input (2026-11-02, the v0.1.6 follow round)** (the bundle of `ed3b4a1e52`, then this round's fixes;
a lane copy on its own bundle id and `HOME`, launched with `-AppleEnableSwipeNavigateWithScrolls YES
-AppleICUForce24HourTime YES`; `second` seeded with `--more-turns --pull-request --limits`, its `shared-notes` checked out
on `feature/notes` with two commits; the v0.1.6 oracle paired with the same lane server and driven the same way).
Presses, drags, wheel and phased scroll events, side buttons and keys came from the HID event source; each result was read
back from the accessibility tree, the clipboard, the saved preferences or VoiceOver's caption panel. 9 rows, 56 checks:
37 hold, 12 failed and are fixed, 1 failed and its fix was held back by a framework limit, 3 failed and were left, and 3
are not reachable with these fixtures. The four were taken up in a second session (below): three are fixed, one of
them with a ring left that Exact draws, and the swipe's failure was the input tool's.

Hold:

- The model and effort picker: a press on an effort level picks it and keeps the picker open, a press on a model picks it,
  Show more lists the older models and reads Show less, and Escape and a press outside close it (1 of 1 each). → moves from
  the search to the levels and ← back (1 of 1). The oracle does the same.
- Side chats: `/side` and `/btw` offer themselves at the start of a message, and Return on the highlighted row then a
  question opens a side chat that answers it (1 of 1 each). "Ask before editing the notes" in it shows "Side chat needs
  approval" with Open approval on the parent, which opens the side chat on its approval (1 of 1). Bring back says "Brought
  back"; Delete asks `Delete side chat "…"?` and closes the tab (1 of 1 each). The oracle the same.
- Projects: a project's pin in the sidebar filter and in the palette pins and unpins it, ⇧⌘P on the palette's highlighted
  row pins it, and the rows keep their order until the menu closes (1 of 1 each). Settings › Projects › Archive marks the
  project Archived, and it leaves the palette and the filter; Unarchive brings it back (1 of 1).
- Back and forward: the header's arrows, ⌘[ and ⌘], and the mouse's buttons 3 and 4 step the history (2 of 2 each).
- Stop and Force stop: with the stand-in's processes stopped, Stop reads "Stopping", then "Agent not responding to Stop"
  with Force stop; Force stop ends the turn with "Run interrupted · Force stopped by user" (1 of 1). The oracle the same.
- Messages: an image alone, a file alone (`sample.py`, "PY · 81 B") and a 39 KB paste ("Pasted text 38.5 KB") send and draw
  as the oracle's; a 4 KB paste stays in the prompt and draws with Show full message; two notes sent while a turn runs show
  the Steer label once and queue the second (1 of 1 each).
- Panels: thread details' menu adds Usage limits and Context window, and Context reads "12k / 200k" as the oracle's; the
  Diff pane's Commits lists the branch's commits ("No commits on this branch." on `main`) and picking one shows its diff;
  the Files explorer's edge drags 100 pt wider; an invalid address ("mailto:…") shows "Could not open this address / Enter
  an http or https URL."; Edit › Copy and Cut copy "selectme" and cut "cut" from a web-view tab's page; Open DevTools opens
  the Web Inspector; Open separate preview window opens the floating preview (1 of 1 each).
- Settings: a press on About's version copies "0.1.6" and shows "Copied!", and the Update track row shows Stable, as in
  the oracle; timestamps read 05:45 with 24-hour time forced, as the oracle's do with `AppleICUForce24HourTime` (1 of 1 each).
- A citation made with Cite and then Escape is removed from the prompt, in the clone as in the oracle (1 of 1 each).

Failed, then fixed and checked again with real input (the rebuilt bundle, the same fixture):

- Return on a focused effort level picked nothing (a Mac button takes Space only); it now picks the level, as v0.1.6's
  web button does (`model-picker.contract` EffortLevel). Checked again: ↓ → ↓ Return picks Max (1 of 1).
- Return on the model already chosen left the focus in the search; v0.1.6 moves it to the checked level after any pick.
  It now goes there at once, as no new levels draw (`app.contract` selectModel). Checked again: ← Return focuses Max (1 of 1).
- ⇧⌘M in a side chat focused the side chat's effort level, and its Permissions chip opened the menu over the window's
  composer: both measured the window's composer. The picker now counts the side chat's model and the menu its own chip
  (`composer-controls-members.contract`, `side-chat-members.contract`). Checked again: ⇧⌘M focuses Search models, and
  the menu opens over the side chat's chip (1 of 1 each).
- A side chat `/side` opened came back after a relaunch reading "This side chat was deleted.": its Delete was not counted
  as the person's choice, so the saved panel kept the tab (`panel-surfaces.ts`). Checked again: after Delete and a
  relaunch, the saved panel has no side chat tab (1 of 1).
- New thread in… highlighted its first row as it opened, so ↓ went to the second; v0.1.6 opens it as a pushed view with
  no highlight, and the first ↓ lights the first row (`composer-controls-members.contract`). Checked again (1 of 1).
- The sidebar's project filter took no ↑ or ↓, so no unpinned row showed its pin without the pointer; the arrows now move
  the highlight and wrap, as Base UI's list does (`sidebar-overlays.contract`). Checked again (1 of 1).
- The draft hero's project menu opened with no row highlighted and no backdrop blur; it now highlights the current
  project, whose pin then shows, over a blurred backdrop (`pages-hero.contract`). Checked again (1 of 1).
- The panel's "+" menu took no letter; B, T, F, D, P, L, M and S now choose their surface, as v0.1.6's
  `handleAddSurfaceMenuKeyDown` does (`panel.contract`). Checked again: S opens a side chat (1 of 1).
- Recently used read a visit made minutes before as "1h ago": the shell's clock moved only while a toast or a tick ran, so
  the visit was stamped late and the label read early. The shell now reads the window's minute clock too
  (`shell-members.contract`). Checked again: a visit a minute old reads "1m ago" (1 of 1).
- VoiceOver read a sent thread chip as "Thread, …, button"; the oracle reads "link, Thread, …" (ThreadContextChip is a
  router Link). It is now a link (`markdown.contract`). Checked again in the accessibility tree (1 of 1).
- VO-Space on a sent citation opened its details but left the focus on the chip, and the chip did not say it opens a
  dialog; v0.1.6's PopoverPopup focuses Go to source. The citation's and the terminal excerpt's details now focus their
  first tabbable, and both chips say "dialog pop up" (`markdown.contract`). Checked again: VO-Space reads "Quoted assistant
  text: …, dialog, with 2 items, Go to source, button", and Escape returns to the chip (1 of 1).

Taken up again with real input (2026-11-02, second session: the bundle of this round's fixes, a lane copy on its own
bundle id and `HOME`, launched with `-AppleEnableSwipeNavigateWithScrolls YES`; `second` seeded with `--more-turns
--pull-request --limits`; the v0.1.6 oracle paired with the same lane server and driven the same way; keys, presses and
phased scroll events from the HID event source; this Mac's input source Korean 2-Set; each pin read back from the lane
server's shell):

- Return in the Browser's address bar went to the page but left the field focused with its ring, where v0.1.6 leaves the
  field (AppKit selects a field's text again after its Return action, which undoes the blur). The field now takes the
  key (`fieldKey`), which exact2's plan count limit, raised to 2^26 (#1253), lets the plan hold. Checked: Return loads
  the page and the field lets go: typed keys and ⌘A no longer reach it (7 of 7). Exact's focus ring stayed drawn on the
  field until the next press elsewhere, where v0.1.6 shows none: the blur handler's new value restyled the field while
  AppKit still had its editor attached (exact2 #1257). Checked again on a bundle with that fix: Return loads the page
  and leaves no ring, and Escape restores the address with none (5 of 5 and 1 of 1).
- ⇧⌘P in the sidebar's project filter and in the draft hero's project menu pinned nothing (0 of 3; 0 of 5 on this
  session's bundles before the fix). Under Korean 2-Set the P key types ㅔ, and both compared the typed character with
  the shortcut, where the palette and v0.1.6 read the physical key. Both now count a letter by its physical key, in the
  search field as the reference's onKeyDown does (`menu-keys.contract` `pinChordPressed`). Checked: with the row
  highlighted by the arrows or the pointer, ⇧⌘P pins it and again unpins it, in the filter and in the hero menu
  (14 of 14). The oracle the same (4 of 4).
- Return 0.15 to 0.3 s after the window became active, with `/bt` and its menu open, inserted a newline instead of
  picking `/btw` (3 of 4; on this session's first bundle two Returns sent `/bt` and two did nothing, in 4). Returning to
  the window resubscribes the shell and thread streams (`transportSignal("foreground")`), and the client counted itself
  not ready until the server's completion markers came, 0.2 to 0.5 s: filmed every 0.1 s, the menu closed and Send
  turned off for that long. Those streams did not fail, so they now stay live while they resume from their cursors
  (`client.ts` `retryStreams`), as the reference keeps its snapshot through a foreground resubscription; a slow resume
  still reads "Syncing messages...". Checked: Return 150, 175, 200, 225, 250, 275 and 300 ms after activation picks
  `/btw` (14 of 14), and filmed, the menu and Send stay (1 of 1). The oracle picks it at 150, 200, 250 and 300 ms
  (4 of 4).
- A two-finger swipe stepped nothing (0 of 11). The cause was the input tool. Its phased events (a small began event,
  then 11 changed ones) are cancelled by AppKit's own swipe tracking in a bare AppKit window too (`trackSwipeEvent`
  reports 0.025, then cancelled), since that tracking reads only the began event's delta from posted events. A began
  event that carries the swipe completes there (0.3, ended) and steps the clone's history back and forward (4 of 4; the
  previous bundle 1 of 1), while the 12-event shape steps nothing on either (0 of 2). v0.1.6 reads the wheel events
  itself and steps with both (2 of 2). A trackpad's swipe was not checked.

Not reachable with these fixtures: a terminal excerpt in a sent message (the fixture makes none; the app tests cover its
details), the fallback model notice (the stand-in reports no usage limits), and the narrow-window Merge row (the stand-in's
pull request has a running check, so v0.1.6 offers no Merge; neither app shows one).

**Checked with real input against a CLIProxyAPI hub (2026-11-02)** (the bundle of `8aee49c9b6`, then this round's fixes;
a lane copy on its own bundle id and `HOME`; `second` seeded with `--turns` and given a real CLIProxyAPI hub with four
Claude and three Codex accounts as its usage limit source, as `seed --limits` adds its stand-in; the v0.1.6 oracle paired
with the same lane server and driven the same way). Presses, hovers, keys and VoiceOver's keys came from the HID event
source; each result was read back from the window, the lane server's settings or VoiceOver's caption panel. No model
turn ran through the hub, and nothing was written to it. 7 rows, 19 checks: 12 hold, 3 failed and are fixed, 1 failed
and is left as a known difference, and 3 are not reachable with a hub; the oracle was driven through each.

Hold:

- Usage › Limits draws a Claude pool (Session with the two accounts that report a session window, Weekly and
  Weekly · Fable with four) and a Codex pool (Weekly with three), one segment per account, with the same window labels,
  figures ("40% left", "↻ +11%"), pace marks and arrows, reset times and banked credit counts as the oracle at the same
  minute (2 of 2 reads). Hub accounts carry no accent colour in either app, so each segment takes its pool's colour.
- A segment's popover: Plan "ChatGPT Pro 200 Subscription", Via "Lane Second · CLIProxyAPI hub", Left, Resets, Even pace,
  Restores and "2 banked · expires in 28d 14h" with Use reset (not pressed), the email blurred (1 of 1). Escape closes a
  pinned one (3 of 4: the first Escape of the session closed nothing and was not seen again; the oracle 1 of 1).
- The same hub added a second time under another URL on its host keeps both entries (`cliproxy-127.0.0.1-8317-2`,
  "Hub two"), and Limits still draws one segment per account, with the plan of the freshest read; Remove hub leaves
  the first (1 of 1). The oracle the same (1 of 1).
- `/usage-limits` on a Codex thread shows "Usage limits 3 accounts", each account's Weekly row, its credits line and
  Use reset, as the oracle's (1 of 1).
- Thread details: with Usage limits and Context window turned on, a Codex thread with hub accounts only shows no Usage
  limits section in either app (the section reads the instance's own limits), and a stand-in thread's Context reads
  "12k / 200k" in both (1 of 1 each).
- VoiceOver on Limits reads, step by step, "Session", "41%", "left", "Under pace: headroom left for the rest of the
  window, image", "↻ +10%", then each segment ("…: 81% left, resets in 1h 38m, 33% of the window left, dialog pop up
  collapsed, button"), then the next window, in the oracle's order and words (16 of 16). The oracle reads a figure's
  digits and its "%" as separate runs and adds "clickable" to the pace image.

Failed, then fixed and checked again with real input (the rebuilt bundle, the same lane server):

- A segment's popover was 286 pt wide, so its plan and source wrapped, where v0.1.6's is 320 pt (the w-72 column inside
  PopoverPopup's padding). It is now that width (`usage-pooled.contract` `popCardWidth`).
- The first and last segments' popovers sat flush with the segment's edge; v0.1.6 centres every popover on its segment
  and shifts it only at the edge. It is now centred and shifted only to stay inside the scroll area (`popLeft`).
  Its side, worked out before from the page's layout as if unscrolled, is now measured when it opens (`popSide`, the
  segment's top in the scroll area). Checked again: the first Codex segment's popover is 320 pt wide, centred, with its
  plan on one line, as the oracle's (1 of 1).
- The composer's account label shrank to keep the whole plan; in v0.1.6 the label keeps its width and the plan is cut
  ("… · ChatGPT Pro 200 Subscript…"). The label now keeps its width (`usage-bars.contract` `UsageAccountLabel`).
  Checked again: the three plans are cut as the oracle's are (1 of 1).

Left as a known difference: a weekly Claude segment's popover opens above it in v0.1.6, over the page's header; the clone
opens it below, since its card is drawn in the page's scroll area, which clips it (below).

Not reachable with a hub: the combined skipped-records warning (neither app has a skipped record: Cost reads 0 sessions),
the ChatGPT card's connected accounts (it lists authenticated providers whose own limits carry an external usage link,
ChatGPT subscription sharing, which a hub account never is; the app tests cover it), and the fallback model notice
(below).

**Checked with real input and VoiceOver (2026-11-02, the text lane)** (the bundle of this branch with exact2
#1273 and #1274 merged in, and the before bundle of `8de4044fb6`, each a lane copy on its own bundle id and `HOME`,
paired with `second` seeded with `--turns`; keys from the HID event source, VoiceOver read from its caption panel;
the oracle on its own home and port):

- A thread switch in the focused prompt is silent. With drafts in "Turn with file edits" and "Background thread
  (completed turn)", ⌘⇧] and ⌘⇧[ with the prompt focused: before, VoiceOver spelled or read part of a draft at 6 of 6
  switches ("h r e a d", "thread" going forward, "Bravo" going back); now at 0 of 12. exact2 #1274 makes the app's
  write into a focused textarea post nothing VoiceOver reads from (LLP 1045 D5). The drafts stay with their threads.
- With Font smoothing on, the prompt's typed text draws `antialiased`, as the reference's does: a draft's ink is 14%
  lighter than before (4078.6 to 3502.0 at 2×), now drawn as the reference draws its own (exact2 #1273, LLP 1019 D12's
  Fields).

**Checked with real input (2026-11-04, the closeout round)** (the bundle of `26bd3b15f6`, a lane copy on its own bundle id
and `HOME`; `second` seeded with `--turns --pull-request --more-turns`; the oracle at `5d3d78ab93` on its own home, paired
with the same lane server; 1280 × 840 at 2×; macOS in Dark, the clone and the oracle set to Light for row 9; Keyboard
Navigation off. Presses, drags, keys and pointer moves came from the HID event source; results were read from the
screen, the accessibility tree, the saved draft, the server's trace and a test server's request log.) 10 rows: 7 hold
and 3 fail.

| Row | Clone | Oracle | Result |
|---|---|---|---|
| A folder dragged from Finder onto the composer, local server (#1456) | the draft holds `[notes-folder](/private/tmp/p2-1104-ri/drop/notes-folder) ` and a folder chip (1 of 1) | the same text and chip (1 of 1) | holds |
| A completed ⌘Q hold in full screen (#1460) | after a 1.8 s hold the window leaves full screen (1352 × 878, then 1280 × 840), its alpha goes to 0, then the app quits (1 of 1) | the same (1 of 1) | holds |
| `window.open(url, name, features)` in a rendered HTML preview (#1460) | the URL opens as a tab in Chrome, the default browser; the app opens no window (1 of 1) | the same (1 of 1) | holds |
| A reply link's focus tooltip by Tab, a linked picture's menu by the Menu key (#1457) | the link "the plan" takes the focus with a ring on "plan" only and no tooltip; the linked picture is one Tab stop, the link, and the ContextMenu key there opens the picture's menu (Copy full path … Copy image) (2 of 2) | a ring on the icon and "the plan", and the tooltip "https://example.com/plan"; the linked picture is two stops: the link (Open in integrated browser, Open in system browser, Copy Link) and the picture (its path as a tooltip, then its menu) | **fails** |
| The terminal button; a pointer move over the terminal (#1452) | a press opens the terminal drawer (1 of 1); after 55 s with no input, a move over the terminal sends `server.reportClientActivity` 0.38 s later, off the 25 s cadence (1 of 1) | the drawer opens; the report comes 0.28 s after the move (1 of 1) | holds |
| View ▸ Zoom In, Actual Size; ⌘=, ⌘-, ⌘0; a Browser tab's own zoom (#1453, #1454) | each scales the whole page: sidebar, icons, the preview's page (1 of 1 each); a tab at 125% stays at 125% through the window's ⌘= and ⌘0 | the same | holds |
| The narrow window (#1448) | both windows stop at 840 pt, so View's zoom (+1) made the page 700 CSS px wide. The sidebar is a sheet; with Settings open, the sheet over it closes on the first Escape and Settings on the second; Project settings from a row's menu opens Settings › Projects and closes the sheet (1 of 1 each) | the same (1 of 1 each) | holds |
| Markdown formatting off: a folded long paste, then ⌘Z (#1445, #1446) | the first ⌘Z removes the 38 KB paste's chip, the second the typed text (1 of 1) | the same (1 of 1) | holds |
| A look at 2× (#1449, #1461, #1420, #1425, #1457, #1436) | "Prose answer": the bubble, the prose, the list, the code block's lines, the placeholder and the search field on the same device rows as the oracle's. Off by device pixels: in "HTML images" the line with "Image unavailable · colon entity" and its image, 7 lower; in "HTML image layout" the rows under the first image row, 1 to 2 higher; the command menu's rows 4 lower (the last 2) and about 2 right. A work group's time stays after a press; Settings › Devices the same | — | **fails** (the offsets) |
| The README's "Next real-input session" rows | ⇧⌘P in the sidebar's project filter pins the highlighted project and unpins it, and the pinned one is listed first (3 of 3). Return in the Browser's address bar lets go of the field (2 of 2) but loads nothing (0 of 2): no request reaches the server | ⇧⌘P the same (1 of 1); Return loads the page and lets go of the field (1 of 1) | **fails** (the address bar) |

Not checked: ⇧⌘P in the draft hero's project menu; the two-finger swipe, which needs a trackpad; a terminal excerpt, the
fallback model notice and the narrow-window Merge row, which these fixtures cannot reach; a ChatGPT sign-in's page and
four waiting sign-ins, which need real accounts.

Seen besides the rows, once each:

- On a fresh home with Lane Second paired, the hero's "or start without a project" and ⌥⌘N did nothing; the
  palette's New thread without a project opened the thread. The oracle opens it from the hero.
- With the focus in a Browser tab's page, ⌘= zooms that tab to 125%; in the oracle it zooms nothing.
- Closing the right panel while a Browser tab shows a page leaves a white box, about 250 by 160 pt, at the window's
  lower right over the timeline; the oracle shows nothing there.
- Opening the Files panel focuses "Search files" in the oracle, and nothing in the clone.
- ⇧Tab from the composer: in the oracle the reply's Fork from this response and Copy message are stops, then the code
  block, Copy code and Disable line wrap; in the clone the first stop was the last link (once) or the timeline's
  scroll area (once), and in "HTML image layout" the last picture, not Copy message or Fork.
- After the first Escape over Settings and the sheet, the oracle rings Settings' search button; the clone shows no
  ring. With Settings opened over the sheet, the clone draws a ring at the window's top left, on the sheet's toggle
  under Settings' dimming.
- ⇧F10 opens no menu in either app: on the Mac, Chromium opens a context menu by the ContextMenu key.
- The local server rows of a new Browser tab show a globe; the oracle's show a page icon.
- Two mistakes in this session: the oracle's welcome wizard took a Codex sign-in press (its download was cancelled,
  its ChatGPT page was closed in Chrome, nothing was signed in), and a Return meant for the clone's address bar sent
  the composer's 38 KB paste to the stand-in in "Turn with file edits", which answered it (no model ran).

**Checked with real input (2026-11-05)** (the bundle of `791e9a251a`, which holds the fix round's #1467 to #1474, a
lane copy on its own bundle id and `HOME`; `second` seeded with `--turns --pull-request --more-turns`; the oracle at
`5d3d78ab93` on its own home, paired with the same lane server; 1280 × 840 at 2×; macOS in Dark, both apps in Dark
except the look rows, where both were set to Light; Keyboard Navigation off. Presses, keys and drags came from the HID
event source, with text typed key by key. Results were read from window captures, the clone's focused accessibility
element, the oracle's `document.activeElement` and a test server's request log.) 13 rows: 12 hold and 1 fails.

| Row | Clone | Oracle | Result |
|---|---|---|---|
| Return in the Browser's address bar (#1467, #1468) | an address typed in the empty bar loads (1 of 1); a click in a shown address selects all of it, and a typed address and Return load (2 of 2); the field lets go | the same (2 of 2) | holds |
| A new tab's Local servers rows (#1468) | the page drawing: three window dots over two lines | the same | holds |
| ⌘= with the focus in a Browser tab's page (#1468) | after a press in a Lane Second tab's page, ⌘= twice, ⌘- and ⌘0 change nothing in the window's capture; with the focus in the composer, ⌘= zooms the window and ⌘0 sets it back | ⌘= twice changes nothing | holds |
| The right panel closed over a Browser tab (#1468) | the tab floats as a card at the window's lower right, in the oracle's place and size, and no other box stays (1 of 1); the card draws the page at full size | the same card, with the page scaled down to fit it | **fails** (the card's page scale) |
| The hero's "or start without a project", a fresh home with Lane Second paired (#1470) | opens New thread in No project on This machine, at the first landing and again from agent-edits' hero (2 of 2) | the same (1 of 1) | holds |
| ⌥⌘N on agent-edits' hero, Lane Second on screen (#1470) | New thread in No project on This machine (1 of 1) | the same (1 of 1) | holds |
| Opening the Files panel (#1470) | "Search agent-edits files" takes the focus, with a ring (1 of 1) | "Search files" takes the focus, `:focus-visible` (1 of 1) | holds |
| Settings over the narrow window's sheet (#1470, #1471, #1473) | 865 pt wide and View zoom twice: after ⌘, then ⌘B no ring at the top left and the focus on the sheet's toggle; the first Escape rings Search settings; the second closes Settings (2 of 2). ⌘B with Settings closed rings the toggle (1 of 1) | the same focus and order; a faint blurred glow at the top left (1 of 1) | holds |
| A reply link's Tab stop, tooltip and ring (#1469, #1472, #1474) | ⇧Tab onto "the plan" rings the icon and both words and shows "https://example.com/plan" above them, its top on the oracle's device row ±1 (1 of 1) | the same | holds |
| A linked picture's two Tab stops and their menus (#1469) | ⇧Tab reaches the picture (its path as a tooltip; the ContextMenu key opens Copy full path … Copy image), then the link (its URL as a tooltip; Open in integrated browser, Open in system browser, Copy Link) (1 of 1) | the same | holds |
| ⇧Tab from the composer in "Prose answer" (#1469) | Fork from this response, Copy message, the code, Copy code, Disable line wrap, what changed, the plan, plan.md (1 of 1) | the same | holds |
| "HTML images" and "HTML image layout" at 2× (#1469) | every image and the "Image unavailable · colon entity" line on the oracle's device pixels | — | holds |
| The command menu after "/" (#1469) | its rows' text starts on the oracle's device rows (1192, 1248, 1304, 1361) and columns (686; the last row 748 against 747) | — | holds |

Seen besides the rows, once each unless the item says otherwise:

- ⇧Tab from the composer skips a reply's changed-files card. In "HTML images" the oracle stops on shots, Open diff
  and Expand all folders after Copy message, and in "Turn with file edits" on README.md, notes, Open diff and Expand
  all folders; the clone goes from Copy message to the pictures, and to the user message's buttons (2 of 2 threads).
- After the hero's "or start without a project", the toolbar's Go back does nothing in the clone (2 of 2); in the
  oracle it returns to agent-edits' hero.
- ⌘N's project picker in the clone lists This machine's scratch folder as a project ("No project", Local,
  `…/t3home/scratch`, ⌘2) under the "No project ⌥⌘N" row; the oracle's lists only that row.
- A linked picture's link tooltip sits 11 device px higher in the clone (its top at row 692, the oracle's at 703). The
  oracle rings the picture's stop with a thin ring and the clone draws none; on the link's stop the clone draws
  AppKit's ring, wider than the oracle's.
- After a visit to "Prose answer", "HTML images" and "HTML image layout", those sidebar rows showed the project badge
  without "agent-edits" in the clone; the label came back after Settings opened and closed. The oracle kept it.
- In the oracle, the new tab menu's Browser added a Browser tab and left Files shown; the clone showed the new tab with
  its address bar focused.

**Checked with real input (2026-11-05, second)** (the bundle of `35d2dafc2d`, which holds the second fix round's #1481
to #1487, a lane copy on its own bundle id and `HOME`; `second` seeded with `--turns --pull-request --more-turns`, and
one more stand-in thread, "Plan sample", whose turn sends the stand-in's proposed plan; the oracle at `5d3d78ab93` on its
own home, paired with the same lane server; 1280 × 840 at 2×; macOS and both apps in Dark; Keyboard Navigation off.
Presses and keys came from the HID event source, with text typed key by key. Results were read from window captures,
the clone's focused accessibility element, the oracle's `document.activeElement` and a test server's request log.)
9 rows: 6 hold and 3 differ.

| Row | Clone | Oracle | Result |
|---|---|---|---|
| The floated Browser card (#1482, #1484) | with the right panel closed over a tab showing a 4 × 6 link grid, the card draws the page scaled down; a click on link 10 as drawn loads `/hit/10`, and the next page reads 539 × 748 in the card (1 of 1) | the same (1 of 1) | holds |
| Go back after the hero's "or start without a project" (#1481) | No project on This machine; Go back returns to agent-edits' hero on Lane Second, Go forward to No project, Go back to the hero (1 of 1) | the same (1 of 1) | holds |
| ⌘N's project picker with a Lane Second thread open (#1481) | agent-edits, No project ⌥⌘N, empty-project, background-only, shared-notes | the same | holds |
| Sidebar rows at View zoom 120 % (#1486, #1487) | after two ⌘= and clicks on "Prose answer", "HTML images" and "Italic answer", every row keeps "agent-edits" | the same | holds |
| ⇧Tab over a reply's changed-files card (#1483) | "Turn with file edits": Fork from this response, Copy message, README.md, notes, Open diff, Expand all folders, Worked for 144ms; "HTML images": Fork, Copy message, shots, Open diff, Expand all folders, then the pictures | the same stops in the same order | holds |
| The linked picture in "HTML image layout" (#1483) | the link tooltip's top on device row 704 and its bottom border on 754–755; the picture's path tooltip on the oracle's rows; on both stops a dark outer ring and the 2 px ring in the oracle's device columns | the link tooltip's top on row 703 | holds (1 device row, as #1483 measured) |
| ⇧Tab over a tool row and its Open diff ("Turn with file edits", work group open) | skips the "Changed 1 file" row and, with the row opened by a click, its Open diff: Expand all folders, then Worked for 144ms | stops on the row (Return opens it), and with the row open on its Open diff first | **differs** |
| ⇧Tab over a plan's buttons ("Plan sample") | plan.md, outline.ts, then Worked for 78ms | plan.md, outline.ts, Plan actions, then Worked for 78ms | **differs** |
| ⇧Tab over an approval's buttons ("Edit awaiting approval") | one stop on the "File change approval" group, with no ring drawn, then the user message's buttons | More approval options, Approve, Decline, the "File change" code, agent-edits/notes/approved.md, then the user message's buttons | **differs** |

Seen besides the rows, once each unless the item says otherwise:

- ⇧Tab from a thread's first timeline stop goes in the clone to the sidebar's footer (Check for updates, Usage, Pull
  requests, Settings), and in the oracle to the chat header (Thread actions, New thread in agent-edits, Toggle right
  panel) (2 of 2 threads).
- After a press on "Worked for 144ms", the clone keeps the group's time (4:06 PM) shown with the pointer gone; the
  oracle shows none. The 2026-11-04 session saw the clone do the same.
- In Dark, the link tooltip's fill is rgb(23, 23, 23) in the clone and rgb(17, 17, 17) in the oracle.

**Fixed, not checked with real input or VoiceOver:**

- Pairing a saved environment again reconnects it with the new credential, as the reference's
  `EnvironmentRegistry.register` restarts its connection: a background environment that failed for
  want of a credential connects and its row stops saying 'Connection failed'. Before, it kept the
  failure (`connections-repair.test.ts`). Checked by that test only: no fixture makes the failed
  credential cheap.

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

**Not checked yet:**

- A terminal excerpt's details in a sent message, the fallback model notice and the narrow-window Merge row (above, the
  2026-11-02 session's "Not reachable with these fixtures").

**Out of scope** (decided 2026-10-17): the device player and the device menus, a tailnet, Screen
Recording and other OS grants (FN24, the permission helper's real grant), provider installs and
sign-ins, Kotoeri's ⌃K, Reduce motion and trackpad momentum by a person, and GitHub writes.

**Next real-input session:**

- ⇧⌘P in the hero's project menu, and the two-finger swipe on a trackpad.
- A terminal excerpt's details, once a fixture sends one.
- The fallback model notice. A hub cannot reach it: v0.1.6 reads the default provider's own usage limits
  (`resolveNewThreadModelSelection`), never a hub's pools. It shows on a new thread's draft with no model chosen when the
  default model's provider reports a window at 100% that has not reset, Settings has a fallback model, and the fallback's
  provider is enabled, installed, signed in, available, not itself exhausted and allowed in the project. It needs a
  fixture provider that reports its own usage limits; it was not forced.
- The narrow-window Merge row, with a pull request whose checks have all finished.

Open native context menus with a HID right-click, not `orca computer`'s. VoiceOver's keys need the modifier keys'
own events (Control and Option down, then the key), not only the flags on the key's event, and ⌘F5 from the HID
source needs the Fn flag. Every copy of this bundle keeps its preferences and drafts in `~/Library/Application
Support/Exact/com.exact.t3code.macos/data/t3-code.json` whatever its bundle id (the host names the data root by the
app's id), so a lane copy needs its own `HOME`, with a `Library/Keychains` link to the user's keychains or macOS asks
to reset the keychain. While VoiceOver runs,
its caption panel covers the composer of a window 840 pt tall, and `orca computer` refuses a click
there (`window_not_focused`): make the window shorter first.

**Fixtures.** `tools/second-environment`, `tools/machines` and `tools/acp-standin` (each has its
README) give This machine plus two lane environments, a machines list, and an ACP agent stand-in.
`seed second --limits` adds pooled usage limits from a CLIProxyAPI hub stand-in; `seed second
--more-turns` a second approval, an italic answer and a turn held open, besides `--turns`' threads; `seed second
--turns` threads on the stand-in with a completed turn, a turn that edits files (a turn diff), an
approval waiting and a context meter, and its `$frontend-design` skill; the seed itself an empty
project and a project action; `seed second --pull-request` a thread whose branch has an open pull
request, with checks, comments, a review thread and a patch, from `gh` and GitHub API stand-ins; `machines.mjs up --older` (or `older <name>`) a
server older than the client that streams a scripted self-update; `env.mjs start --runtime <version>` an older release
from `.runtime-cache`, `--scratch` a lane that offers a folder for threads without a project, `no-scratch <home>` This
machine without one; and `env.mjs samples <dir>` a Python file, a PDF and a video to attach.

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
**Check for Updates…** (under About and in Help) shows the reference's "Automatic updates are
not available right now." box. Its reason is the managed policy's when one turns updates off
("<organization> manages T3 Code updates.") or cannot be used ("Updates are off because T3 Code
cannot use the managed policy at <path>. …"), read from the same file or profile as the local
server's exposure (`T3ManagedPolicy.updatesDisabledReason`). Otherwise the bundle's receipt
decides: a build without an update store (`deploy.store` is `"0"`, as this app ships) has no
update feed, so a policy's `feedUrl` (an electron-updater feed) has nothing to point; a build
with one does nothing (the module has no hook into the host's check).
