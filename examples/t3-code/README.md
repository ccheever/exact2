# T3 Code for Exact on macOS

A macOS client for [T3 Code](https://github.com/pingdotgg/t3code) servers, written with Exact.
Contract draws the interface; TypeScript owns the client state and projections; an app-local
Swift module (`modules/apple/`) handles HTTP, WebSocket RPC, Keychain credentials, the
composer's text view, the terminal and browser web views, menus, notifications and window
chrome. Provider execution, workspaces, Git and conversation history stay on the T3 server.
The app carries the official T3 server release (the CLI archive of the release built from the
reference pin, unmodified; `server-runtime/runtime-pin.json`) and runs it as its own local
server, "This machine". It does not modify T3 Code.

The reference is T3 Code `1e2ecbd975` (`referencePin` in `server-runtime/runtime-pin.json`;
the client mirrors Nightly `0.0.46-nightly.20261004.1`, `version-skew.ts`). Ported files name
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
bun examples/t3-code/terminal-host/build.mjs   # the terminal page into assets/ (ignored)
bun examples/t3-code/stage-runtime.mjs         # the embedded T3 server into server-runtime/ (ignored; network once)
bun host/apple/build.mjs t3-code-macos --bundle --run
```

The terminal page is a build output: the Apple bundle copies `assets/` as it is and has no
step that makes an app's assets first, so without `terminal-host/build.mjs` the terminal view
stays blank and its status names the load error.

Generated sources are built, not committed. `tools/generate/generate.mjs` makes them from the
packages pinned beside it (`tools/generate/package.json` and `bun.lock`, the versions the
reference locks; network once) and from committed inputs: the Shiki grammar tables
(`r12-render-grammar*.ts`), the CSS word lists, the Lucide and Pierre icon data
(`settings-b-lucide.ts`, `pages-icons.contract`, `timeline-files.contract`,
`timeline-files-tables.ts`, `macos/pierre-icons.tsv`, `assets/composer-icons.json`), the morph
plans (`shell-morph.contract`) and the terminal buffer vectors
(`macos/tests/terminal/vectors.json`, whose expectations are test data committed in
`gen-vectors.mjs`). The app build runs it before it bakes (`macos/build.rs`);
run it yourself before the tests and checks below:

```sh
bun examples/t3-code/tools/generate/generate.mjs           # writes what differs; --check fails instead
```

The app is its own Cargo workspace (`Cargo.toml` here, building into `examples/t3-code/target`),
outside the root's members: its build script bakes the whole app on one core for minutes, so
exact2's six blocking checks leave it out and the async lane (`scripts/async.mjs`'s
`t3-code` and `t3-code-clippy` steps) builds, tests and lints it.

The embedded server. `stage-runtime.mjs` downloads the release archive named by
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

Without both variables its status is `refused` and nothing starts. Only the packaged build
(`distribution.json` in its Resources) uses `~/.t3` and the port scan from 3773, as T3 Code does.
An agent drive (`bun scripts/agent.mjs macos --size 1280x840 --storage <name> …`, with `EXACT_APP_DIR`
and both variables set) runs the development binary, which carries no bundle resources and so
no server: build the bundle, then point `EXACT_MAC_BIN` at its `Contents/MacOS/T3 Code (Exact)`
(the driver then skips its stale-build check, so rebuild after every source change).

Safari's Web Inspector (exact2 #101): in a development build the app's own web views (the
terminal, the browser surface, the rendered-HTML preview, the offscreen Mermaid renderer) are
`isInspectable`, so Safari's Develop menu lists them. A release build's are not: the packaged
build (that marker), a production-trust bake or a distributed bundle (`--distribution`, what
`exact release` signs), as main #309 decides for Exact's own `iframe` web views
(`T3WebInspection.swift`; each web view writes a `t3.inspection:` line to stderr). There is no
View › Toggle Developer Tools: an inspector for the app's own UI stays out of exact2 (#101).

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
  `r11-upstream-sweep.ts`, reference 1826fb55cc). Draft rows have the reference's context
  menu (Copy ▸ Path / Branch, Project settings, Discard draft), and discarding a draft from
  the menu or an X goes behind the undo notice ("Discarded N draft(s), ⌘Z to undo"; Undo
  restores the text, images and worktree choice; `r11-upstream-drafts.ts`, 95edeb753b). The
  version pill shows only when it fits beside the brand (9efb016900).
- Conversation: bounded history with older-page loading, streaming assistant and
  tool rows, work-log inspector, checkpoints and changed files, fork dividers, answered
  questions, mention and file chips, Markdown with code blocks (wrap, copy) and Mermaid
  diagrams rendered by the connected server's own Mermaid build. A failed workspace
  preparation offers Retry (`prepared-run.retry`), and the failure row hides once a retry
  supersedes it (`r11-upstream-retry.ts`, 737993303d). Code colouring follows Shiki for
  every declarator of a `const`/`let`/`var` list, object keys versus type annotations,
  import lines and keyword-named members (`r11-misc-ts-decl.ts`).
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
  measured reservation at its end (`r4-composer-overlay.ts`); with Chat width Wide or Full
  the context strip's workspace control is the desktop Select.
- Requests: approvals, single/multiple-choice and free-text questions.
- Version control in the workspace card (`r4-git-*`): the branch picker (search, create
  a ref with Enter, check out, "Start from origin"), the git actions quick action and
  menu, the commit dialog (file selection, message, "Commit on new branch"), the
  default-branch confirm, progress with hook output, the 10 s inline success, pull,
  the Publish repository wizard, the "Run on" picker across environments that share a
  repository, and the new-worktree start flow. On a local draft a ref search that parses as a
  pull request reference (`#42`, `42`, a pull request URL, `gh pr checkout 42`) leads both
  pickers with "Checkout pull request", which opens PullRequestThreadDialog
  (`r9-connect-checkout.ts`, `r9-connect.contract`): `git.resolvePullRequest` per edit, then
  Local or Worktree through `git.preparePullRequestThread` (Worktree passes the draft's thread
  id), and the draft moves onto the checkout. As in the reference, the field's text is selected
  when the dialog opens, each edit's lookup starts at once, and "Resolving pull request..."
  stays until 450 ms after the last edit before the answer or error shows
  (`r10-connect-timing.ts`).
- Right side: the workspace card (docked beside the chat from 984 pt, a header popover
  below), Changes (branch) and Uncommitted diffs plus per-turn diffs, lineage with
  merge-back, the surface chooser and a tab bar per thread (`r4-surfaces-*`): Files
  (the workspace tree with search and expand-all, file previews with breadcrumbs,
  rendered Markdown/CSV and an editor that writes back with `projects.writeFile`; opened
  from Go to file, content search and file chips; its explorer and rendered/source
  choices persist; an `.html` file opens rendered in the sandboxed WebKit body (its sibling stylesheets, scripts and
  images load through the signed asset URL's token directory, and the page may load stylesheets, scripts,
  images, fonts and fetches from other hosts as the reference's sandboxed frame does (https, and `http` to an IP
  address or a named host: `app.json` allows arbitrary loads in web content, #106), `R6MediaPreview.swift`)
  from a signed asset URL, with the reference's Show HTML source / Show rendered page toggle,
  `r10-device-files-html.ts`; regex literals in its scripts are coloured as Shiki does,
  `r10-device-html-regex.ts`; every right-panel tab (Files, files, the
  pull request list, the Diff, the Device surface with its device, pull request details and
  attachments) is kept per thread across launches, `r10-device-panels.ts`, `r11-device-panels.ts`;
  the Diff's preview of a project outside the server's root is asked again at the server's cwd
  as DiffPanel does, `r11-device-diff.ts`), Linked pull requests (rows, Copy link, Open, Watch for changes / Stop
  watching, Unlink), the Pull request surface (`r5-panels-*`: the thread's or a link's
  pull request in the Pull Requests page's detail panel, opened from the chooser's P, the
  details card's `#N` rows with "Show N more", and pull request links; `r6-pr-*`: with the
  host's detail the row splits into its state glyph, the checks segment and its popover
  (attention and running checks, "Show all", Details links), the tooltip card (state,
  base ← head, checks, files and diff size) and the one action worth taking: Resolve and
  Fix check the pull request out into a worktree with `git.preparePullRequestThread` and
  leave the task unsent in the project's draft (`r7-handoff-thread.ts`: the draft's thread id
  is allocated first and passed along, so the server runs the project's worktree setup
  script for that thread, and the draft later launches under the same id, across a
  relaunch), Ready and Merge (after "Merge pull
  request?") run `pullRequests.runAction`; a pull request tab without a linked snapshot
  takes its state from the loaded detail; the composer's context strip shows the thread's
  pull request chip, or a hand-off draft's from its branch status, with its tooltip list,
  +N and stack forms, `r7-handoff-strip.ts`), sent attachment
  previews (Markdown, table or numbered source, images, Copy contents and Save file;
  opened from a sent file chip; `r6-media-*`: PDFs in PDFKit, HTML rendered in a sandboxed
  WebKit view that loads its token directory and external hosts as the reference's frame does, or as source coloured by Shiki's html grammar under the
  Pierre themes (`r7-polish-html-syntax.ts`), audio and video in AVKit players, "Unable to
  load audio/video." with Try again) and Device (the "Set up devices" wizard over
  `device.configure` / `subscribeDeviceState`; Escape closes it; after onboarding a row
  opens its simulator with `device.open`, then the workspace shows the hub's H.264 screen
  decoded with VideoToolbox (iOS: serve-sim's AVCC body, MJPEG when the description cannot be
  decoded or the decoder stays behind for a second, `R11DeviceBacklog.swift`; Android: serve-emu's SEMU-framed socket) as the 3D phone (SceneKit over the
  connected server's own device models, never bundled; a procedural body for other devices)
  or flat, with Home, Rotate (Android: Back, Recents and a Portrait / Landscape menu),
  appearance, text size, the Tools drawer (foreground app, open URL, launch, terminate,
  Simulator / Emulator settings, accessibility frames over the screen, location, permissions,
  push, the event log), Save screenshot, Float over chat, Close, Power off, 3D / Flat, the
  iPad's Magic Keyboard and Restore 3D view; the focused screen forwards keys (HID usages on
  iOS, key codes and text on Android), `r6-media-device.ts`, `r7-device-tools.ts`,
  `R7Device*.swift`; the iPhone Duo's hinged 3D viewer on the fixed cover / inner panel feeds with its Fold shape and
  Device stance stands, and a foldable emulator's Fold / Unfold over serve-emu's `/api/fold` with its procedural
  hinged body, `r9-device-duo.ts`, `R9Device*.swift`). Sent videos are 4:3 tiles in the message's media grid;
  the tile and the video chip open the media dialog, playing.
- Pages: Pull Requests (list, detail, checks, copy actions), Usage (cost, tokens,
  limits, share bars that follow the selected metric, model dialog, custom model prices and
  Map to), welcome wizard. The palette's "New thread without a project" and the new-thread
  heading's No project open the machine's folder for threads without a project the same
  way (`r11-upstream-scratch.ts`, 845ddd9354).
- Pull request detail: besides the Summary and Timeline, the Code tab (`pages-pr-code*`: the
  change or one commit's change a slice at a time, Viewed ticks, line comments into the review
  being written, the host's conversations on their lines), hand-offs and quick actions
  (`pages-pr-handoffs*`, `pages-pr-quick*`) and the thread links (`pages-pr-links*`).
- Terminal: the thread terminal drawer below the chat column (`terminal.contract`,
  `terminal-drawer*.ts`; T3 Code's ThreadTerminalDrawer in its drawer mode): sessions on the
  server, split and close, the drawer's state kept per thread. The terminal is T3 Code's own
  Ghostty emulator page (`terminal-host/`, built into `assets/`) in a web view
  (`T3TerminalView.swift`); the sessions are native (`T3TerminalSessions.swift`).
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
counts as focus (the window's first responder is followed, `R9Input.swift`), and ⌘B / ⌘I end
a composing syllable first and match by key code under a non-Latin source such as Korean 2-Set. Outside
the composer a ⌘ or ⌃ letter chord under a non-Latin source is re-issued with its key's Latin
character, as the reference's `resolveEventKeys` does (`R10Connect.swift`), so ⌘B, ⌘K and menu
equivalents still match. exact2 #168 matches declared chords by physical key itself, but not
the menu's standard items, the terminal's web view or the module's own key monitors, so the
re-issue stays. After the thread list re-renders or scrolls under a still pointer, the row that
slid under it is hovered, as a browser's synthetic mouse move does (the host's own, exact2 #139). The menu bar is the reference
desktop app's (`R8KeysMenus.swift`): File shows only Close Window, View starts with Reload
and Force Reload, and the host's Develop and Go menus are removed, so ⌘D (diff), ⌘O (open in
editor) and ⌘1–9 reach the window; every button chord stays a hidden File key equivalent,
one dispatch button per chord (⌘N and ⇧⌘O both start a thread with no text field focused).
Edit › Undo sends ⌘Z to thread.undo when no editable text has the focus, and ⌘W closes the
active right-panel surface tab (its neighbour becomes active; the last tab closes the panel;
on the Pull Requests page the open pull request) before it closes the window. The surface launcher takes the focus when it mounts (`autofocus`), and
its letters are its rows' `aria-keyshortcuts` (`shell-panels.contract`), which Exact presses before type-to-focus; a Markdown table's Copy
menu is a window-level popup, placed from the trigger's drawn frame (`R8KeysMeasure.swift`),
that Escape and an outside press close (`r8-keys-table-menu.*`). A global hotkey of another
app (a launcher holding ⌘1, say) never reaches the window.

Credentials stay in Keychain, scoped to the server origin and environment. The Swift
module atomically saves the versioned `t3-code.json` preference file (selections,
drafts, sidebar and page preferences, dismissed notices, pending operation identities)
under Exact's app data directory. **Disconnect** keeps the credential; **Forget**
removes it. The embedded server is the primary environment, "This machine"
(`local-primary.ts`): the client connects to it with the
bearer the server issued at start (memory only, never Keychain), never saves it and never
remembers its origin (its port can change every launch; the transport keeps a focus token
`primary` beside `t3.server.origin`). On launch the client reconnects by itself: to the primary
when it was the last focus, else to the last switched-on saved environment with its Keychain
credential, else to the primary while the Local environment switch is on (`r8-pointer-reconnect.ts`);
while the server starts, the window shows the connecting state (the host opens the first window
before the server is ready, exact2 #117). The window keeps its frame across launches
(the host's frame autosave, restored after the window's final style since exact2 #113). Settings ›
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
saved environment; both sections show from two (`r11-misc-connections.ts`). Every running thread
of every connected environment keeps its detail stream (`keep-alive.ts`), so opening one shows it
at once. A pairing that
fails saves nothing: no row, no catalog entry, no remembered origin (the transport remembers an
origin only once its socket opens), and a failed first connection falls back to the saved
environment it replaced. A successful Add environment with nothing connected stays on
Settings › Connections with the reference's "Backend added" toast, and a pasted pairing URL
fills both Host and Pairing code (`r10-connect-pairing.ts`); a failed pairing with nothing
connected leaves no client state naming its origin. A stored `onboardingCompletedAt` older than T3 Code itself (the 1970
values of older builds) reads as unset, so the real time is stored (`r9-connect-onboarding.ts`).
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
- Data modules cannot send on a WebSocket ([`issues/20261009-runner-owned-bidirectional-streams.md`](../../issues/20261009-runner-owned-bidirectional-streams.md),
  #126), so the RPC transport lives in the Swift module.
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
([`issues/closed/20261011-repeating-hatch-across-list-rows.md`](../../issues/closed/20261011-repeating-hatch-across-list-rows.md)).
These it does not use, because on macOS they do not do what the reference does:

- `showModal(id)` / `close(id)` ([`issues/closed/20261009-gui-dialog-action-commands.md`](../../issues/closed/20261009-gui-dialog-action-commands.md), #282).
  A modal `dialog` is an AppKit sheet and an `alertdialog` is an `NSAlert`, not the reference's
  in-page popup; with Keyboard Navigation off its Tab stops only at fields; and a dialog opened
  by a click gives the focus back to nothing. So each dialog keeps its own Tab trap
  (`dialog-trap.contract` says why), and closing a dialog whose opener the view cannot name
  still leaves the focus nowhere (Base UI gives it back).
- App-declared menu bar items ([`issues/closed/20261009-contract-menu-bar-extensions.md`](../../issues/closed/20261009-contract-menu-bar-extensions.md)).
  An `-exact-menu` item is enabled only while its window is key and has no sheet, app-menu items
  are filed after About's separator, and `-exact-menu-remove` names none of the items the app
  hides, so the menu bar stays the module's (`T3Menus.swift`, `R8KeysMenus.swift`, `R10Connect.swift`).
- The notification click ([`issues/closed/20261009-notification-default-click-design.md`](../../issues/closed/20261009-notification-default-click-design.md)).
  Main's click needs a Contract `showNotification(navigate=)`, which always plays the system
  sound (the reference's notification is silent), cannot be posted from the data module, and
  is delivered to a navigation root the app does not have. The click stays in `T3Notifications.swift`.
- An inline link run as a Tab stop on macOS
  ([`issues/closed/20261012-macos-inline-link-run-tab-stop.md`](../../issues/closed/20261012-macos-inline-link-run-tab-stop.md)). This fix is the host's, so the
  clone gets it on main with no change of its own. No drive has tabbed to a link in a Markdown
  table cell since.

Differences that belong to the app:

- The Browser surface is WebKit, not Chromium (#100 was closed as not planned). It has no
  "Open DevTools" item, and pages get no Notification API or geolocation.
- The host opens the first window before the embedded server is ready (#117 was closed as not
  planned), so the window shows the connecting state first.
- Code colours: eight languages are left out because of their licences (ada, gnuplot, nginx
  and racket are GPL-3.0; dax, dream-maker, glsl and tcl have no licence upstream), so they
  paint as plain text. Every other grammar is loaded at launch, so a fresh window colours
  an embedded language straight away, where the reference does so only after its first use.
- An ⌥-click on a reply's `#fragment` link opens nothing. Chromium downloads the page instead.
- The app menu's item reads "Settings…", as macOS names it (the host sets it); the reference's
  Electron menu reads "Settings...".
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
#655, #663, #664, #676, #679 and #681), a session with real keys on the terminal and the
real-input and VoiceOver sessions G to K. Each item says what is wrong or missing; the open
framework issues that someone using the app runs into are in the section above.

**Known differences:**

- VO-Space on a composer citation chip, from the end of the thread, scrolls the timeline to the
  cited text. VoiceOver then moves its cursor to the 'Scroll to end' pill that appears as the
  timeline leaves the end, and the keyboard focus follows it; the reference keeps the focus on
  the chip's link. Only VoiceOver's own press does this. A pointer press, or an accessibility
  press from another client, leaves the focus on the prompt with the chip selected, and a
  VO-Space while the pill is already showing keeps it there too. The app posts nothing that
  names the pill before the focus moves (a macOS host issue: VoiceOver's press moves its cursor
  to an element that appears during the press).
- Escape in a new binding's recorder (Settings › Keybindings) with nothing recorded blurs the
  field to the window on macOS, where the reference keeps it focused
  ([`issues/20261016-escape-in-recorder-blurs-on-macos.md`](../../issues/20261016-escape-in-recorder-blurs-on-macos.md);
  F-2). It is not VoiceOver's: after main's #681, real keys alone reproduce it, Tab into the field
  then Escape 3 of 3 and a click into it then Escape 1 of 1 (VoiceOver then says 'scroll area').
- A context-menu item that opens Rename project or Project grouping can still let VoiceOver
  re-read a long draft before the dialog, and the dialog's name is then not heard
  ([`issues/20261016-native-menu-pick-voiceover-old-focus.md`](../../issues/20261016-native-menu-pick-voiceover-old-focus.md);
  rows H-9, G-5 and FN6 pass with this difference). The landing (the field or Cancel) is right.

**Not failures:**

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
- ⌘Z after snoozing a background row from another environment once left it snoozed (FN29); it
  did not reproduce, and the input tool could not choose the submenu item again.
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
- Session K did not run some rows, for want of fixture data or because the task left them out;
  none is a failure: Usage's dialogs (H-2, FN8; no pooled limits), FN17 (no real-input path makes
  a focused control disabled), FN24 (a Screen Recording grant), G-5's Remove project (no empty
  project), the diff Turn submenu and the linked pull request, approval and device menus (FN5),
  the project scope search, Reply and a diff comment (FN7), and realinput-1015c row 24. J9, J11,
  J15, J16 and J17 need pre-fix builds or superseded framework test apps. J10 ran on the T3 lane
  instead of PR #676's test app (58 of 58 outside presses closed the popovers), and J19's
  NSPopover path does not exist in the clone.

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

**Fixed, not checked with real input or VoiceOver:**

- Each Settings page opens at its top: each page has its own scroller, keyed on the route, the
  scope and a restore, as `routes/settings.tsx` keys its Outlet. The search jump still lands on
  its target (tests and an agent drive).

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
  composition is no longer re-issued with its Latin letter (R10Connect). A composing syllable is
  not drawn, in the clone as in T3 Code, whose terminal input is a hidden textarea.

Real-input sessions should type IME rows with held keys: orca's zero-dwell press-key can make the
input method commit a jamo at once. They should switch input sources with the system shortcut or
menu, not TISSelectInputSource, before they judge IME rows.

**Not checked yet:** the device player (needs a device), Reduce motion and trackpad momentum,
the thread, pull request, review comment and annotation chips by pointer, the context popover
and progress bars under VoiceOver, the branch's PR glyph, bold and italic font faces, the
permission helper's real grant (the user's), and a tailnet. Also:

- The composer's file and image previews: Tab and Shift-Tab over their buttons with Keyboard
  Navigation off and on; in an HTML preview a `target="_blank"` link opens the default browser, a
  `_top` link does nothing and a link to the page's own section scrolls the frame; the image
  preview refitting after a window resize.
- Escape on "Remove from this device…" and on "Delete action?" by a person's keys (VoiceOver and
  pointer pass); Escape on a git dialog or the Merge confirmation over the thread-details popover
  in a narrow window.
- VO-Space on the chip kinds session K did not press (sample.py, an image, a video and doc.pdf
  pass; the citation chip is the known difference above).
- Kotoeri's ⌃K in the terminal has not been run: no Japanese input source is installed.

**Next real-input session:** each Settings page opening at its top after another page was
scrolled, and the search jump still landing on its target; the rows session K did not run (under
"Not failures") once the lane has pooled limits, an empty project, a linked pull request, an
approval, a device and turn diffs; and the list above.

**Fixtures.** `tools/second-environment`, `tools/machines` and `tools/acp-standin` give a second
environment, a machines list and an ACP agent stand-in (each has its README). Still missing: a
tailnet, and a server older than the client for the machines-update popover's busy states.

**Framework issues opened by this app:** open on main are
[`issues/20261016-escape-in-recorder-blurs-on-macos.md`](../../issues/20261016-escape-in-recorder-blurs-on-macos.md)
and [`issues/20261016-native-menu-pick-voiceover-old-focus.md`](../../issues/20261016-native-menu-pick-voiceover-old-focus.md)
(both under "Known differences" above); the VoiceOver press that moves its cursor to the 'Scroll
to end' pill has no record yet. The macOS issues opened from 2026-10-13 to 2026-10-16 are closed
on main, the light-dismiss and VoiceOver ones (exact2 PRs #663, #664, #676, #679 and #681)
included, and so is the pointer events one (`pointerdown` and `pointerup` now bubble to every
ancestor that hears them, and `stopPropagation()` stops them).

**Workarounds main has made unnecessary:** since pointer events bubble, the window's `main` hears
every press itself, so the app's forwards to it are redundant: the sidebar's `sidebarPressDown`
and an answer's `down` (markdown.contract) call `outsidePressDown`, the floating theme editor's
pickers and inspector take `outsideDown` and `outsideUp` as props, and the diff and Files gutters
report a `press` that closes a skill chip's details (`closeChipFromPress`). Each press now runs
the window's outside press twice, which is harmless, as each step checks what it already closed.
Remove them, with their comments, in a clone branch that drives each case.

## Source and checks

`app.contract` keeps the window's resources, mutations and tasks, the state they read and
the actions that write it or send (a child component may not own resources or assign root
state); its view is `T3Window` in `app-window.contract`, which owns the window's view-only
state (the sidebar's hover and drag, the title menu, the settings editors' menus, Add
environment's fields) and lays out the components of `app-main.contract`,
`app-settings.contract` and `app-overlays.contract`; each takes the root names it reads as
props of the same names, and an action that writes both sets the window's half and calls the
root's (`…Root`). Feature areas live in their own files
(`sidebar-*`, `timeline-*`, `composer-*`, `shell-*`, `pages-*`, `settings-*`,
`palette*`, `r3-*`), each `.ts` with its Contract view and tests. `client.ts`,
`protocol.ts`, `domain.ts` and `presentation.ts` own the data source, commands and
event projection. A labelled `button` or `link` that may mount no text says `title=""`: since
LLP 1115 the Mac host shows the `aria-label` of a button whose mounted, not `aria-hidden` children
give no text as its help tag, and the reference shows only its own tips (`help-tags.test.ts`
applies the host's rule to the view tree). `modules/apple/` is the native module; `macos/` holds the bake adapter crate (`t3-code-macos`)
and the AppKit tests; `tools/` holds the lanes (`github-lane/`, `network-lane/`), the generator
of the generated sources (`generate/`) and the Shiki comparison. T3's MIT notice is retained in `LICENSE-T3`.

Exact asks an answer again when a topic it watches changes. Since exact2 #183 a change
that arrives while the snapshot read is in flight lets that read's reply land and then asks
once more, so the topics go straight to Exact (the app's read gate is gone). An answer Exact
replaces for new arguments still never receives its pending native replies, so each
WebSocket RPC carries a trace id the transport lists while it is pending, and "Some requests
are slow" counts only requests the server has not answered.
The transport follows the reference's reconnect policy (jittered 1 s·2ⁿ⁺¹ ladder capped at
five minutes, reset after 30 s connected; Retry, returning to the app and an offline report
probe the live socket instead of replacing it) and resubscribes a failed stream on the same
session after 250 ms doubling to 30 s.

```sh
bun examples/t3-code/tools/generate/generate.mjs   # the generated sources, first
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o /tmp/t3-code.plan
cargo test -p contract --test it button_migration -- --ignored   # the async lane's sweep of this app: app.contract and its imports (minutes)
cargo test --manifest-path examples/t3-code/Cargo.toml --lib   # the app's workspace: the bake, then its tests (Hermes env of the root setup)
```

A Cargo build alone does not compile or launch the Apple crate's Swift module. Use the app
build above for an integrated check.

The module AppKit/XCTest binaries under `macos/tests/<name>/` build with Exact's
module facade, the app's generated data keys, every file in `modules/apple/` (the
`composer`, `menus` and `r5-panels` tests define their own `exactModule`, so they leave
out `T3Module.swift` and its `T3Module+<area>.swift` op files; an `extension T3Module` goes in one
of those op files, never beside other code, or these three binaries do not compile) and the test
directory's sources. Run from the repository root:

```sh
X=$(xcode-select -p); F="$X/Platforms/MacOSX.platform/Developer/Library/Frameworks"; L="$X/Platforms/MacOSX.platform/Developer/usr/lib"
R="$PWD/target/t3-tests"; mkdir -p "$R"; export T3_APP_DIR="$PWD/examples/t3-code"
T3_DK="$R" bun -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_DK + "/ExactDataKeys.swift");'
for d in examples/t3-code/macos/tests/*/; do
  n=$(basename "$d"); O="$R/$n"; mkdir -p "$O"
  [ "$n" = timeline-keyboard ] && continue # Actual host regression; separate recipe below.
  M=$(ls examples/t3-code/modules/apple/*.swift); case $n in composer|menus|r5-panels) M=$(echo "$M" | grep -v /T3Module);; esac
  xcrun swiftc -swift-version 5 -module-name "T3$(echo $n | tr -d -)Tests" -F "$F" -I "$L" -L "$L" \
    -Xlinker -rpath -Xlinker "$F" -Xlinker -rpath -Xlinker "$L" \
    host/apple/modules/ExactNativeModule.swift "$R/ExactDataKeys.swift" $M "$d"*.swift -o "$O/$n-tests" || continue
  [ $n = snapshot ] && { rm -rf "$O/fixture"; mkdir -p "$O/fixture"; export T3_SNAPSHOT_TEST_ROOT="$O/fixture"; }
  EXACT_ASSETS="$T3_APP_DIR" T3_BROWSER_TEST_DIR="$O" T3_COMPOSER_TEST_DIR="$O" T3_MENUS_TEST_DIR="$O" T3_MERMAID_TEST_DIR="$O" T3_PANELS_TEST_DIR="$O" T3_TERMINAL_TEST_DIR="$O" "$O/$n-tests" $([ $n = mermaid ] && echo "$T3_SERVER")
done
```

The `timeline-keyboard` regression uses the actual ExactKit key loop and scroll
views. It compiles ExactKit's sources into its own binary, so it passes
`-package-name apple`, the name SwiftPM gives `host/apple/Package.swift`'s
package (ExactKit's `package` declarations need one). After building the macOS
app (`bun host/apple/build.mjs t3-code-macos`, which writes
`examples/t3-code/target/aarch64-apple-darwin/host-dev/libt3_code_macos.a`), run it from the
repository root:

```sh
mkdir -p target/t3-tests
xcrun swiftc -swift-version 5 -package-name apple -module-name ExactKit -I host/apple/Sources/CExact \
  $(rg --files host/apple/Sources/ExactKit -g '*.swift') \
  examples/t3-code/macos/tests/timeline-keyboard/main.swift \
  -L examples/t3-code/target/aarch64-apple-darwin/host-dev -lt3_code_macos -lc++ \
  -o target/t3-tests/timeline-keyboard-tests
target/t3-tests/timeline-keyboard-tests
```

`terminal` needs `bun examples/t3-code/terminal-host/build.mjs` first (it loads the page from
`assets/`); `T3_TERMINAL_SCALE=1` adds the 1/4/11/44-view cost table, and
`bun examples/t3-code/terminal-host/verify-vendor.mjs` checks the vendored binaries.
`mermaid` needs `T3_SERVER` set to a running T3 server's origin (it loads that server's
Mermaid build into an offscreen web view). The live transport tests skip unless
`T3_TRANSPORT_PAIRING_FILE` points at a fresh disposable pairing JSON file and
`T3_TRANSPORT_ORIGIN` names its server; `r3.swift` serves its own loopback WebSocket peer
for the reconnect policy, stream retries and outdated-host updates. The SSH tests read
a temporary home, never `~/.ssh`; the live tunnel test skips unless `T3_SSH_COMMAND`
names an ssh test double, and agent runs of the app read hosts only from `T3_SSH_HOME`.
`macos/tests/ssh/fake-ssh.sh` is that double (its header lists the variables): the password
prompt tests run against it always, the live test when `T3_SSH_COMMAND` names it and
`FAKE_SSH_REMOTE_HOME` holds a running server's `.t3/userdata/server-runtime.json`.
`T3_MENUS_EVIDENCE` set to a directory also renders the quit pill in both appearances, and
`T3_PERMISSION_HELPER_EVIDENCE` the snapshot test's permission helper panel.
**Check for Updates...** (under About and in Help) reads the bundle's receipt: a build
without an update store (`deploy.store` is `"0"`) shows the reference's "Automatic
updates are not available right now." box; a build with one omits both items.
