# T3 Code for Exact on macOS

A macOS client for [T3 Code](https://github.com/pingdotgg/t3code) servers, written with Exact.
Contract draws the interface; TypeScript owns the client state, the projections and the server
connections (HTTP, WebSocket RPC, the saved environments and their credentials); an app-local
Swift module (`modules/apple/`) handles the composer's text view, the terminal and browser web
views, menus, notifications and window chrome. Provider execution, workspaces, Git and conversation history stay on the T3 server.
The app carries the official T3 server release (the CLI archive of the release built from the
reference pin, unmodified; `server-runtime/runtime-pin.json`) and runs it as its own local
server, "This machine". It does not modify T3 Code.

The reference is T3 Code `9c161462c9` (`referencePin` in `server-runtime/runtime-pin.json`;
the client mirrors Nightly `0.1.1-nightly.20261018.3302`, `version-skew.ts`). Ported files name
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
step that makes an app's assets first, so without `terminal-host/build.mjs` the terminal view
stays blank and its status names the load error.

Generated sources are built, not committed. `tools/generate/generate.mjs` makes them from the
packages pinned beside it (`tools/generate/package.json` and `bun.lock`, the versions the
reference locks; network once) and from committed inputs: the Shiki grammar tables
(`src/highlight/grammar*.ts`), the CSS word lists (`src/highlight/css-words.ts`), the Lucide and
Pierre icon data (`src/settings/settings-b-lucide.ts`, `src/shared/pages-icons.contract`,
`src/shared/timeline-files.contract` and `timeline-files-tables.ts`, `data/pierre-icons.tsv`,
`assets/composer-icons.json`), the ten icon tables (`icons`, `shell-icons`, `sidebar-icons`,
`timeline-icons`, `panel-icons`, `settings-icons`, `provider-icons`, `diff-glyphs`, `editor-icons`
and `settings-brands`, each a `.contract` in its feature's folder under `src/`, from the Lucide names
in `tools/generate/icon-tables.mjs` and the marks in `icon-marks.mjs`; their hand-written
components are in the `*-kit.contract` files), the morph plans (`src/shell/shell-morph.contract`)
and the terminal buffer vectors
(`macos/tests/terminal/vectors.json`, whose expectations are test data committed in
`gen-vectors.mjs`). The app build runs it before it bakes (`data/build.rs`);
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
`<T3 home>/userdata/logs/server-child.log`, and stops it when the app quits. A development build
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

Safari's Web Inspector (exact2 #101): in a development build the app's own web views (the
terminal, the browser surface, the rendered-HTML preview) are
`isInspectable`, so Safari's Develop menu lists them. A release build's are not: the packaged
build (that metadata), a legacy packaged marker retained for inspection compatibility, a production-trust bake or a distributed bundle (`--distribution`, what
`exact release` signs), as main #309 decides for Exact's own `iframe` web views
(`T3WebInspection.swift`; each web view writes a `t3.inspection:` line to stderr). There is no
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
connected server older than this client shows "Server update available" above the composer (and
a warning in the thread details card); its Update streams the update's progress, waits for the
server to come back on the new version and shares that state with Settings › Connections
(`server-update.ts`).

1. In T3 Code, open **Settings → Connections** and create a fresh pairing link.
   A command-line installation can also use `t3 pair` for a running server.
2. In this app's welcome wizard (or **Add environment**), paste the pairing URL.
3. Wait for synchronization, then pick a project and thread, or add an existing
   workspace by its absolute path **on the server**. Adding a project does not create
   the directory.
4. Choose a model and send. New threads use the project override or the server
   default for permissions, workspace and interaction mode.

## What it covers

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
  diagrams: each fence renders in the Mermaid page (`assets/mermaid/`, an `iframe` per fence,
  LLP 1020.001) with the Mermaid and DOMPurify builds the reference locks, which
  `tools/generate/` vendors into `assets/mermaid/vendor/`. A failed workspace
  preparation offers Retry (`prepared-run.retry`), and the failure row hides once a retry
  supersedes it (`workspace-retry.ts`, 737993303d). Code colouring follows Shiki for
  every declarator of a `const`/`let`/`var` list, object keys versus type annotations,
  import lines and keyword-named members (`ts-statements.ts`).
- Composer: send, stop, queue and steer while running (queued rows reorder by drag),
  model picker with search and favourites, reasoning and runtime options, Plan/Build,
  slash and @ menus, file and image attachments (videos as first-frame tiles that play in
  an expanded preview; removing an image the prompt references asks first and removes
  every reference), stash, multi-model drafts that start one worktree thread per model,
  the resume-with-less-context banner, `/usage-limits` (each account's windows as bars with
  pace and countdown, Manage usage, banked reset credits with "Use reset" behind a confirm;
  `usage-limits.ts`, `usage-bars.contract`) and Codex `/feedback [reason]` (uploads the
  thread with a banner, Copy ID and two local rows; `composer-feedback.ts`). Both long
  writes are detached requests whose reply joins the inbox (`composer-replies.ts`). The composer overlays the transcript, which keeps a
  measured reservation at its end (`composer-overlay.ts`); with Chat width Wide or Full
  the context strip's workspace control is the desktop Select. PNG, JPEG and HEIC images over
  2000 pixels on their long edge are scaled to that edge before upload; PNGs already within it keep their bytes.
- Requests: approvals, single/multiple-choice and free-text questions.
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
  (the workspace tree with search and expand-all, file previews with breadcrumbs,
  rendered Markdown/CSV and an editor that writes back with `projects.writeFile`; opened
  from Go to file, content search and file chips; its explorer and rendered/source
  choices persist; an `.html` file opens rendered in the sandboxed WebKit body (its sibling stylesheets, scripts and
  images load through the signed asset URL's token directory, and the page may load stylesheets, scripts,
  images, fonts and fetches from other hosts as the reference's sandboxed frame does (https, and `http` to an IP
  address or a named host: `app.json` allows arbitrary loads in web content, #106), `T3MediaPreview.swift`)
  from a signed asset URL, with the reference's Show HTML source / Show rendered page toggle,
  `files-rendered-html.ts`; regex literals in its scripts are coloured as Shiki does,
  `html-script-regex.ts`; every right-panel tab (Files, files, the
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
  `attachment-media*`, `T3MediaPreview.swift`: PDFs in PDFKit, HTML rendered in a sandboxed
  WebKit view that loads its token directory and external hosts as the reference's frame does, or as source coloured by Shiki's html grammar under the
  Pierre themes (`html-syntax.ts`), audio and video in the platform's players, "Unable to
  load audio/video." with Try again) and Device (the "Set up devices" wizard over
  `device.configure` / `subscribeDeviceState`; Escape closes it; after onboarding a row
  opens its simulator with `device.open`, then the workspace shows the hub's H.264 screen
  decoded with VideoToolbox (iOS: serve-sim's AVCC body, MJPEG when the description cannot be
  decoded or the decoder stays behind for a second, `T3DeviceDecodeBacklog.swift`; Android: serve-emu's SEMU-framed socket) as the 3D phone (SceneKit over the
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
- Pull request detail: besides the Summary and Timeline, the Code tab (`pages-pr-code*`: the
  change or one commit's change a slice at a time, Viewed ticks, line comments into the review
  being written, the host's conversations on their lines), hand-offs and quick actions
  (`pages-pr-handoffs*`, `pages-pr-quick*`) and the thread links (`pages-pr-links*`).
- Terminal: the thread terminal drawer below the chat column (`terminal.contract`,
  `terminal-drawer*.ts`; T3 Code's ThreadTerminalDrawer in its drawer mode): sessions on the
  server, split and close, the drawer's state kept per thread. The terminal is T3 Code's own
  Ghostty emulator page (`terminal-host/`, built into `assets/`) in a web view
  (`T3TerminalView.swift`); the sessions are native (`T3TerminalSessions.swift`).
  ANSI colours use VS Code's palette for the scheme. Shift+PageUp and Shift+PageDown page through
  the scrollback, ⌘⇧Home and ⌘⇧End jump to its start and the latest output (full-screen programs
  keep these keys), and ⌘A or the context menu's Select all selects the whole history; Jump to
  latest returns to the current output. The divider drags are native, outside the page, so the
  view forwards each mouse release in the window to the page, which sends the settled grid to
  the shell at once, as T3 Code's window listener does.
- Browser surface: a right-panel Browser tab on a `WKWebView` in the module
  (`T3BrowserSession*.swift`, `browser-*.ts`; T3 Code embeds Chromium, which exact2 does not
  offer, #100): tabs and the chrome row, history and local servers, zoom, appearance, the
  device toolbar, Annotate, screenshots, recording, downloads, a separate window and the
  floating player, profiles and cookie import, and an agent's `previewAutomation` requests.
- Command palette (threads, projects, actions, files, content search, New thread in…,
  Link pull request), toasts, the Nightly mobile-app notice, macOS notifications (after
  the user grants them).
- Settings: the reference's routes with project/environment scope, search, theme editor and
  VS Code theme import, keybindings, providers (sign-in and runtime install), connections
  (pairing, SSH, outdated-host update), scheduled tasks, SnapShots, diagnostics and licenses.

Return sends; Shift-Return inserts a newline; IME marked text stays an editor action.
⌘↩ sends from the button; in a new-thread draft it starts the thread in the background
when the server binds `composer.sendBackground`; ⌥⌘↩ on an existing thread sends and
opens a fresh draft in the same project. Other chords follow the server's keybindings
(⌘K palette, ⌘N new thread, ⇧⌘M model picker, ⇧⌘K copy PR number, ⌘B sidebar). ⌘B bolds instead
while the rich-text composer has the focus, as Tiptap does: a click into an existing draft
counts as focus (the window's first responder is followed, `T3ComposerInput.swift`), and ⌘B / ⌘I end
a composing syllable first and match by key code under a non-Latin source such as Korean 2-Set. Outside
the composer a ⌘ or ⌃ letter chord under a non-Latin source is re-issued with its key's Latin
character, as the reference's `resolveEventKeys` does (`T3LatinChords.swift`), so ⌘B, ⌘K and menu
equivalents still match. exact2 #168 matches declared chords by physical key itself, but not
the menu's standard items, the terminal's web view or the module's own key monitors, so the
re-issue stays. After the thread list re-renders or scrolls under a still pointer, the row that
slid under it is hovered, as a browser's synthetic mouse move does (the host's own, exact2 #139). The menu bar is the reference
desktop app's (`T3MenuBar.swift`): File shows only Close Window, View starts with Reload
and Force Reload (two Contract buttons the host files there, `-exact-menu="view"`, which run
`reload()`; settings-shortcuts.contract `ViewReload`), and the host's Develop and Go menus are removed, so ⌘D (diff), ⌘O (open in
editor) and ⌘1–9 reach the window; every button chord stays a hidden File key equivalent,
one dispatch button per chord (⌘N and ⇧⌘O both start a thread with no text field focused).
Edit › Undo sends ⌘Z to thread.undo when no editable text has the focus, and ⌘W closes the
active right-panel surface tab (its neighbour becomes active; the last tab closes the panel;
on the Pull Requests page the open pull request) before it closes the window. The surface launcher takes the focus when it mounts (`autofocus`), and
its letters are its rows' `aria-keyshortcuts` (`shell-panels.contract`), which Exact presses before type-to-focus; a Markdown table's Copy
menu is a window-level popup, placed from the trigger's `frame()` (scroll offsets applied),
that Escape and an outside press close (`table-menu.*`). The row context menus are Contract context
popovers, which the host presents as an `NSMenu` at the pointer, or for the ContextMenu key at the
focused row's centre (exact2 #314): the sidebar's thread, draft and selection menus, the legacy sidebar's
row, selection and project menus, the Files tree's, a panel tab's and a pull request number's. A draft
row's ContextMenu and Shift+F10, and a tab's Shift+F10, show theirs from the row, below it. The branch
picker's Copy branch name, the chat's file and web link menus, the media menu and the legacy sidebar's
choice of project members stay the module's (`T3ContextMenu.swift` and `T3Sidebar.swift` say why). A global hotkey of another
app (a launcher holding ⌘1, say) never reaches the window.

Saved environments and their credentials are one kept secret, `t3-catalog` (granted by `secret.keep
t3-catalog`, kept in the Keychain on macOS; `transport-catalog.ts`), each token scoped to the server
origin and environment. The Swift module atomically saves the versioned `t3-code.json` preference file (selections,
drafts, sidebar and page preferences, dismissed notices, pending operation identities)
under Exact's app data directory. **Disconnect** keeps the credential; **Forget**
removes it. The embedded server is the primary environment, "This machine"
(`local-primary.ts`): the client connects to it with the
bearer the server issued at start (memory only, never kept), never saves it and never
remembers its origin (its port can change every launch; the transport keeps a focus token
`primary` beside `t3.server.origin`). On launch the client reconnects by itself: to the primary
when it was the last focus, else to the last switched-on saved environment with its saved
credential, else to the primary while the Local environment switch is on (`relaunch-reconnect.ts`);
while the server starts, the window shows the connecting state (the host opens the first window
before the server is ready, exact2 #117). A launch reopens no thread: the reference's window loads `/`
every time (DesktopWindow.ts, ElectronProtocol.ts `getDesktopUrl`), whose IndexDraftLanding waits until
every switched-on environment's shell is in and opens a new draft in the most recently active project of
any environment (by latest user message; a visit is not activity). The clone does the same
(`home-landing.ts`), focusing that project's environment when it is not the one the launch reconnected.
The window keeps its frame across launches (the host's frame autosave, restored after the window's final
style since exact2 #113). Settings ›
Connections shows "This machine" (`this-machine.ts`): the Local environment switch, which asks
first, persists the setting and then relaunches the app through Contract's `relaunch()`, as the
reference does, and the Version row. Under it (`connections-network.ts`) are Network access ("Reachable at <url>" and its endpoints; the server
binds 0.0.0.0 while it is on, loopback otherwise, and refuses when no LAN or Tailscale IPv4
exists), Tailscale HTTPS (the server runs `tailscale serve` on the chosen port; the app reads
`tailscale status --json` only while network access or Serve is on, cached 60 s,
`T3LocalNetwork.swift`), and the Authorized clients fold: pairing links with their permissions,
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
## Known differences and framework gaps

The clone follows the reference wherever exact2 allows it. Where it cannot, the gap is an
exact2 issue. The open ones are records under main's `issues/` (the GitHub issues the clone
filed were moved there or closed). The gaps someone using this app runs into:

- Popovers flip near a window edge only as far as main's first slice goes
  ([`issues/20261009-popover-css-flip-fallbacks.md`](../../issues/20261009-popover-css-flip-fallbacks.md),
  #112). The clone's popovers, hover cards and colour picker use its `position-try-fallbacks`;
  the record stays open for the rest of the placement work.
- Data modules have no timers ([`issues/closed/20261009-typescript-failure-parity.md`](../../issues/closed/20261009-typescript-failure-parity.md),
  #124; closed with timers still deferred). Time is an argument, and debounces and cooldowns are
  Contract tasks.
- A backdrop blur sees only its parent ([`issues/closed/20261010-macos-backdrop-beyond-parent.md`](../../issues/closed/20261010-macos-backdrop-beyond-parent.md);
  main keeps the limit), so the composer is opaque where the reference's glass shows the
  transcript through it.

Main has closed records the clone was written around. The clone now uses these: a process
relaunch for the Local environment switch, Network access and Tailscale HTTPS (`relaunch()`,
[`issues/closed/20261009-native-process-relaunch.md`](../../issues/closed/20261009-native-process-relaunch.md)),
`border-style: dashed` ([`issues/closed/20261009-dotted-dashed-border-style.md`](../../issues/closed/20261009-dotted-dashed-border-style.md)),
`focusin`/`focusout` ([`issues/closed/20261009-bubbling-focus-events.md`](../../issues/closed/20261009-bubbling-focus-events.md))
and capture-phase key handlers for the keybinding recorder and the reply's Markdown
(`keycapture`, [`issues/closed/20261009-capture-phase-key-events.md`](../../issues/closed/20261009-capture-phase-key-events.md)),
and `repeating-linear-gradient()` with length stops for the usage bars and the diff's deletion bars
([`issues/closed/20261011-repeating-hatch-across-list-rows.md`](../../issues/closed/20261011-repeating-hatch-across-list-rows.md)),
and `-exact-menu` for View › Reload and Force Reload ([`issues/closed/20261009-contract-menu-bar-extensions.md`](../../issues/closed/20261009-contract-menu-bar-extensions.md)).
These it does not use, because on macOS they do not do what the reference does:

- `showModal(id)` / `close(id)` ([`issues/closed/20261009-gui-dialog-action-commands.md`](../../issues/closed/20261009-gui-dialog-action-commands.md), #282).
  A modal `dialog` is an AppKit sheet and an `alertdialog` is an `NSAlert`, not the reference's
  in-page popup; with Keyboard Navigation off its Tab stops only at fields; and a dialog opened
  by a click gives the focus back to nothing. So each dialog keeps its own Tab trap
  (`dialog-trap.contract` says why), and closing a dialog whose opener the view cannot name
  still leaves the focus nowhere (Base UI gives it back).
- The rest of the menu bar's app items ([`issues/closed/20261009-contract-menu-bar-extensions.md`](../../issues/closed/20261009-contract-menu-bar-extensions.md)).
  On main (checked at `44ad32092`) an `-exact-menu` item is still enabled only while its window is key and has
  no sheet (`ShortcutsMac.swift` `validateMenuItem`), app-menu items are still filed after About's separator
  (`sync`), and `-exact-menu-remove` still takes only its twelve standard ids. So Check for Updates… (always
  enabled and directly under About in the reference, with a box that has no parent window where a Contract
  `alertdialog` is a sheet), Paste as Text, View's zoom, the hidden Help book item, the ⌘Q hold and the
  trimmed File, Edit, Develop and Go stay the module's (`T3Menus.swift`, `T3MenuBar.swift`, `T3LatinChords.swift`).
  Settings… is the host's: it files the ⌘, dispatch button there.
- The notification click ([`issues/closed/20261009-notification-default-click-design.md`](../../issues/closed/20261009-notification-default-click-design.md)).
  Main's click needs a Contract `showNotification(navigate=)`, which always plays the system
  sound (`Notify.swift` posts with `.default` and presents with `.sound`; the reference's notification is
  `silent: true`, ThreadNotificationCoordinator.tsx:203-207), cannot be posted from the data module, and
  is delivered to a navigation root the app does not have. Badges and a permission fact stay refused
  (`rules/DEFERRED.md`). So the post, its click, the sounds, the Dock badge and the permission request stay
  in `T3Notifications.swift`.
- An inline link run as a Tab stop on macOS
  ([`issues/closed/20261012-macos-inline-link-run-tab-stop.md`](../../issues/closed/20261012-macos-inline-link-run-tab-stop.md)). This fix is the host's, so the
  clone gets it on main with no change of its own. No drive has tabbed to a link in a Markdown
  table cell since.

App Swift this clone keeps, where moving it onto what main offers would change what someone using
the app sees, or where main has nothing to move it onto (each is a framework capability to add):

- `T3MediaPreview.swift`: a PDF in PDFKit on Chromium's viewer surface, fitted to the width and
  top-aligned; an `iframe` would show WebKit's own PDF viewer. A rendered HTML attachment opens a
  `target="_blank"` link in the default browser and shows `alert` and `confirm` as sheets; an
  `iframe` refuses popups (LLP 1020 §5) and has no dialogs.
- `T3MediaActions.swift`: the media menu, Copy image and Save. Save opens in Downloads first and then
  in the session's last folder, as Electron's download dialog does, where `saveFile` names no
  folder; Copy image needs an image clipboard write (`ClipboardItem`), which main does not have;
  the menu's Menu-key form opens at the media's bottom-left corner, where a `contextPopover` opens
  at the focused node's centre.
- `T3ToolActivityIcon.swift`: a muted tool icon is drawn at the reference's `brightness(0.6)` in the
  light appearance; CSS `filter` applies only to SVG elements, and an SVG `image` is not built.
- `T3ImageAccent.swift` and the composer's image chips (`T3ComposerImageChip.swift`): an image's
  average colour needs its pixels, and readback is refused.
- `T3RowDrop.swift`: a legacy row takes any file and rings while a drag is over it; `drop` takes only
  `file_handlers` types and has no `dragenter` or `dragleave`.
- `RightPanelTabsInput.swift`: the device tab's rename field keeps Escape and ⌘ chords to itself;
  an `aria-keyshortcuts` button hears its chord before a field's `key` handler.
- `T3RemoteEditors.swift` (probing installed editors needs a process and file checks; `openURL`
  opens only http, https, mailto and tel), `T3InstalledFonts.swift` (no `queryLocalFonts`),
  `T3Locale.swift` (`exactTime().locale` is the preferred language, where Electron's
  `getSystemLocale` follows the Region setting), `T3WindowChrome.swift` (no
  `env(titlebar-area-*)`), `T3RootFont.swift` (the native views' rem), `T3PanelsNative.swift` (the
  Files editor's caret under the press that mounted it), and the data module's sleep
  (`timelineSleep`: data modules have no timers).
- The composer's editor core and what rides on it (`T3ComposerAttach`, `T3ComposerIntent`,
  `T3ComposerQueueKey` and the rest of `T3Composer*`): inline atoms, the caret's rectangle, pasted
  files and a video's first frame are not in main's Markdown editor or media surface.

Differences that belong to the app:

- The server connections are the data module's (`src/transport/`). Each environment's socket is a
  stream resource, admitted by `net.websocket ws:` and `wss:` since servers are named at run time
  (#703), whose mapper answers Pings and acknowledges chunks in the frame's own commit (#705, LLP
  1016.000 D7); HTTP goes through `fetch` (`net.fetch http:` and `https:`). The answer or command
  that asks for a request sends it on that socket itself and awaits the reply (#736, D7's amendment
  of 2026-10-18); a frame no turn may send yet (the socket is not open) waits in the outbox, which
  the mapper sends when the socket opens and after each frame. A data module has no timers, so a
  mutation the window runs, the tick, keeps the deadlines, Pings, probes and reconnect ladder: every
  250 ms while something is near, every second while connected, and once to publish the socket
  table each time an answer that may wait on the server starts or ends or the delivering answer
  reports a table the tick has not published. Nothing polls while an answer waits. A request's
  deadline counts from the tick's last reading of the clock, which can be up to a second old, so
  its timeout can come up to a second early. A connect replies once the socket's URL is in, not
  once the socket opens, since an answer may await only what this module's fetch, storage or open
  stream settles.
- The terminal's attach stream, the Browser's automation replies and the device hub's tickets keep
  a second, module-owned socket per environment (`T3Link.swift`), which the transport's `links` op
  tells the routes and bearers of the environments it has connected: the terminal's bytes go
  straight to its web view instead of through a commit each. The global pointer and key monitor
  that dates the last interaction stays native too (`T3ActivityInput.swift`); the activity reporter
  is TypeScript.
- Under the agent driver, a clock step that has a timer due first waits for every answer in flight,
  up to 20 s. Answers send their own requests, so that wait ends when the server replies, not after
  20 s; the app itself runs its timers on the wall clock. Under the agent the transport's
  ids also come from the driver's repeatable random stream, so drives of one persistent server use a
  different `--seed` each launch, or the server takes a repeated command id for the command it
  already ran and does nothing.
- The Browser surface is WebKit, not Chromium (#100 was closed as not planned). It has no
  "Open DevTools" item, and pages get no Notification API or geolocation.
- The host opens the first window before the embedded server is ready (#117 was closed as not
  planned), so the window shows the connecting state first.
- The launch's landing (`home-landing.ts`) waits for a background environment while it connects, but
  not for This machine while its server starts, nor for one that is retrying: the reference opens its
  window only once the server is ready, and keeps waiting for any switched-on environment until its
  third attempt has failed (`state/shell.ts` `allEnvironmentShellsBootstrappedAtom`). So a relaunch can
  open the draft in another environment's project when the most recent one is This machine's or a
  retrying one's.
- Code colours: eight languages are left out because of their licences (ada, gnuplot, nginx
  and racket are GPL-3.0; dax, dream-maker, glsl and tcl have no licence upstream), so they
  paint as plain text. Every other grammar is loaded at launch, so a fresh window colours
  an embedded language straight away, where the reference does so only after its first use.
- An ⌥-click on a reply's `#fragment` link opens nothing. Chromium downloads the page instead.
- Split view's empty side stripes every 10 pt, where the reference stripes every 8 px: each row
  of the virtualized list paints its own gradient, and main keeps `background-attachment: local`
  deferred, so a shorter period would break at every row edge.
- An image inside a link's text has no tooltip and no keyboard menu key.
- In the composer, a review comment's popover shows its comment and diff as plain text (the
  reference renders Markdown and a highlighted diff); a sent pull request chip has no hover card.

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
- With Reduce motion on, a legacy project's rows jump as it collapses but its chevron still turns,
  as the reference's (`transition-transform duration-150` with no motion-reduce variant,
  LegacySidebar.tsx:2405-2409).
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
- The legacy sidebar's thread machine glyph has no pointer tooltip, and neither does the
  reference's. Its meta slot is `pointer-events-none` and fades out on hover
  (LegacySidebar.tsx:489-493, 903-920); 'Lane Second' is only the glyph's accessible name.
- Mark unread changes nothing on a thread with no completed run: the server refuses it and the
  reference stays silent. On a completed thread the row gains the green 'Completed' label.
- The legacy thread menu has no Archive item, in the reference too (LegacySidebar.tsx:2241-2264).
  One row archives from its hover button, a selection from 'Archive (n)'.
- A chip pressed to open its preview stays selected after the preview closes, and a letter typed
  after Escape, Close or a backdrop click replaces the file or image chip (C14/C23). T3 Code's
  Tiptap editor keeps the chip's NodeSelection and Base UI's finalFocus gives the editor back the
  focus; measured on the reference web build. `macos/tests/composer/chipactions.swift` pins it.
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
first snapshot answer reported a code text waiting for its Shiki tokens, the root's highlight pump refreshed
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
of text." with the keyboard focus on that prompt (3 of 3). The chip now leads to the prompt: leading
to itself, it took VoiceOver's cursor to 'Go to threads' as it went with the old draft, and a focus
change posted for the prompt after the switch was not followed (`98be292b9`, replaced). The citation
chip's link still keeps VoiceOver on it ('lane note, selected'; 1 of 1).

**Checked with VoiceOver** (2026-10-17, bundle of `8dd82dc72`): VO-Space on a composer citation
chip's link, from the end of the thread, scrolls the timeline to the cited text and leaves
VoiceOver on 'View cited assistant text: …, link', with the keyboard focus on the prompt, as in
the reference (5 of 5). VoiceOver follows a link it presses to the link's linked elements; with
none, it took the element that appeared within about half a second of the press, the 'Scroll to
end' pill. The link now leads to itself (`T3ComposerChipElement.accessibilityLinkedUIElements`).

**Checked by agent drive and real input after main's #687 and #688** (2026-10-17, bundle of
`c4f9f4b38`): a press on the floating theme editor's swatch, its picker's plane and hue slider,
Inspect and the inspector each counts as an outside press, as the window's other presses do (the
window's count went 8 to 13, one per press). Escape in a new binding's recorder after a real
click into it keeps the field focused and the row up (F-2, 1 of 1; #688's placeholder fix).

**Fixed, not checked with real input or VoiceOver:**

- Context another panel adds to the composer (a review comment, a terminal excerpt, a thread
  dropped from the sidebar, a citation, a Browser annotation, an attached file or image) goes in
  at the start of the selection and deletes nothing, as the reference's `insertComposerText` at
  "cursor" (ChatComposer.tsx:6041-6120): a chip left selected after its preview closes stays in
  the draft. It replaced the selected chip before (session L, 3 of 3).
- A chip's details popover (a skill, a review comment, a preview annotation, a terminal excerpt)
  takes the focus as it opens, by pointer or VO-Space: its first tabbable (View instructions, the
  terminal output) or the popup, so VoiceOver reads '<label>, dialog'. Escape gives the prompt
  back the focus with the chip selected, and an outside press leaves the focus where the press
  put it. Measured on T3 Code (an isolated copy, VoiceOver, the Settings prompt sample): VO-Space
  or a click on 'Skill Frontend Design. Show details' reads 'Skill Frontend Design, dialog,
  Skill Frontend Design, and 2 more items, group' with the popup focused; Escape returns to the
  chip; a press on the Typography heading leaves the focus on the page. Before, VoiceOver kept
  reading the prompt (N11, N25).
- Mark unread shows nothing when the server refuses it, from every menu and environment, as the
  reference's `useMarkThreadUnread` (`reportFailure: false`): the flat row, multi-selection and
  More actions menus showed 'Failed to mark thread unread' before (`sidebar-mark-unread.test.ts`).
- Pairing a saved environment again reconnects it with the new credential, as the reference's
  `EnvironmentRegistry.register` restarts its connection: a background environment that failed for
  want of a credential connects and its row stops saying 'Connection failed'. Before, it kept the
  failure (`connections-repair.test.ts`).

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
  left with no input method, and keys reached the shell as the layout's characters (ㅎ ㅏ ㄴ or
  g k s). The terminal now reconnects its input context when the selected source changes
  (T3TerminalInputSource.swift). T3 Code under Chromium loses at most the first key in that
  case. Not yet investigated: in a freshly launched lane the native composer once sent the first
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
  does: its drawer reads the "Process" key's `key` ("ㅣ"), so a ⌃ chord typed during a terminal
  composition is no longer re-issued with its Latin letter (T3LatinChords). A composing syllable is
  not drawn, in the clone as in T3 Code, whose terminal input is a hidden textarea.

Real-input sessions should type IME rows with held keys: orca's zero-dwell press-key can make the
input method commit a jamo at once. They should switch input sources with the system shortcut or
menu, not TISSelectInputSource, before they judge IME rows.

**Not checked yet:** the context popover and progress bars under VoiceOver (the ACP stand-in's
threads now have a context meter), and an italic font face (no fixture has italic text). Also:

- Escape on the Merge confirmation over the thread-details popover in a narrow window.
- VO-Space on the chip kinds session K did not press (sample.py, an image, a video, doc.pdf, the
  citation and the thread chip pass).
- Once, in session L, VO-Right from Message skipped every composer chip; after the caret moved it
  read them in text order, every later time. Not isolated.

**Out of scope** (decided 2026-10-17): the device player and the device menus, a tailnet, Screen
Recording and other OS grants (FN24, the permission helper's real grant), provider installs and
sign-ins, Kotoeri's ⌃K, Reduce motion and trackpad momentum by a person, and GitHub writes.

**Next real-input session:** the fixes under "Fixed, not checked" (in the composer, a selected
file, image or review comment chip kept when a review comment, an image or an annotation is added;
VO-Space and a click on a review comment, annotation and the Settings sample's skill chip reading
the dialog, then Escape back to the prompt with the chip selected), and the rows under "Not
checked yet".

**Fixtures.** `tools/second-environment`, `tools/machines` and `tools/acp-standin` (each has its
README) give This machine plus two lane environments, a machines list, and an ACP agent stand-in.
`seed second --limits` adds pooled usage limits from a CLIProxyAPI hub stand-in; `seed second
--turns` threads on the stand-in with a completed turn, a turn that edits files (a turn diff), an
approval waiting and a context meter, and its `$frontend-design` skill; the seed itself an empty
project and a project action; `seed second --pull-request` a thread whose branch has an open pull
request, with a patch and a review thread, from a `gh` stand-in; and `machines.mjs up --older` a
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
state); its view is `T3Window` in `src/shell/app-window.contract`, which owns the window's view-only
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
of the generated sources (`generate/`), the embedded server's staging (`stage-runtime/`) and the
Shiki comparison. T3's MIT notice is retained in `LICENSE-T3`.

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
bun test examples/t3-code
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
`composer`, `menus` and `panels-native` tests define their own `exactModule`, so they leave
out `T3Module.swift` and its `T3Module+<area>.swift` op files; an `extension T3Module` goes in one
of those op files, never beside other code, or these three binaries do not compile) and the test
directory's sources, and every one Exact's `host/apple/modules/ExactTamber.swift`.
`EXACT_ASSETS` must be an absolute path: the module looks up `assets/` only under one, and three of
the composer's icon tests fail without it. Run from the repository root:

```sh
X=$(xcode-select -p); F="$X/Platforms/MacOSX.platform/Developer/Library/Frameworks"; L="$X/Platforms/MacOSX.platform/Developer/usr/lib"
R="$PWD/target/t3-tests"; mkdir -p "$R"; export T3_APP_DIR="$PWD/examples/t3-code"
T3_DK="$R" bun -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_DK + "/ExactDataKeys.swift");'
for d in examples/t3-code/macos/tests/*/; do
  n=$(basename "$d"); O="$R/$n"; mkdir -p "$O"
  [ "$n" = timeline-keyboard ] && continue # Actual host regression; separate recipe below.
  M=$(ls examples/t3-code/modules/apple/*.swift); case $n in composer|menus|panels-native) M=$(echo "$M" | grep -v /T3Module);; esac
  T=host/apple/modules/ExactTamber.swift
  xcrun swiftc -swift-version 5 -module-name "T3$(echo $n | tr -d -)Tests" -F "$F" -I "$L" -L "$L" \
    -Xlinker -rpath -Xlinker "$F" -Xlinker -rpath -Xlinker "$L" \
    host/apple/modules/ExactNativeModule.swift $T "$R/ExactDataKeys.swift" $M "$d"*.swift -o "$O/$n-tests" || continue
  [ $n = snapshot ] && { rm -rf "$O/fixture"; mkdir -p "$O/fixture"; export T3_SNAPSHOT_TEST_ROOT="$O/fixture"; }
  EXACT_ASSETS="$T3_APP_DIR" T3_BROWSER_TEST_DIR="$O" T3_COMPOSER_TEST_DIR="$O" T3_MENUS_TEST_DIR="$O" T3_PANELS_TEST_DIR="$O" T3_TERMINAL_TEST_DIR="$O" "$O/$n-tests"
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

`terminal` needs `bun examples/t3-code/terminal-host/build.mjs` first (it loads the page from
`assets/`); `T3_TERMINAL_SCALE=1` adds the 1/4/11/44-view cost table, and
`bun examples/t3-code/terminal-host/verify-vendor.mjs` checks the vendored binaries.
The SSH tests read
a temporary home, never `~/.ssh`; the live tunnel test skips unless `T3_SSH_COMMAND`
names an ssh test double, and agent runs of the app read hosts only from `T3_SSH_HOME`.
`macos/tests/ssh/fake-ssh.sh` is that double (its header lists the variables): the password
prompt tests run against it always, the live test when `T3_SSH_COMMAND` names it and
`FAKE_SSH_REMOTE_HOME` holds a running server's `.t3/userdata/server-runtime.json`.
`T3_MENUS_EVIDENCE` set to a directory also renders the quit pill in both appearances, and
`T3_PERMISSION_HELPER_EVIDENCE` the snapshot test's permission helper panel.
**Check for Updates…** (under About and in Help) reads the bundle's receipt: a build
without an update store (`deploy.store` is `"0"`) shows the reference's "Automatic
updates are not available right now." box; a build with one omits both items.
