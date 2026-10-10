# T3 Code for Exact on macOS

A macOS client for an existing T3 Code server. Contract draws the interface;
TypeScript owns the client state and projections; an app-local Swift module
handles HTTP, WebSocket RPC, Keychain credentials, the composer's text view, menus,
notifications and window chrome. Provider execution, workspaces, Git and conversation
history remain on the T3 server. The app carries the official T3 server release (the CLI
archive of the release built from the reference pin, unmodified; `server-runtime/runtime-pin.json`)
and runs it as its own local server (20261005-embedded-server-runtime; "This machine" is built on it
by 20261005-local-primary-environment). It does not modify T3 Code.

## Reference and verification status

The reference is T3 Code `f870c419fc` (Nightly `0.0.46-nightly.20261005.1`). Parity is
judged against the web client that revision's own server serves, rebuilt from that
source into `target/t3-ref/runtime-f870c41` (ignored build output). Round 11 ported the
client-side commits `f90b77d809..f870c419fc` (1826fb55cc, 95edeb753b, 737993303d,
0c81120137, 9efb016900, 845ddd9354; the rest only move server test stubs). Earlier rounds
used `f90b77d809` and `8ed276c`.

Last integrated check (round 11, 2026-10-05, after the upstream, device and misc lanes; no
source changed in the integration pass apart from this README and `AGENT-HANDOFF.md`):

- `bun test examples/t3-code`: 991 pass, 0 fail (105 files). Strict `tsc` on
  `app.ts` is clean. `contract build` of `app.contract`: 1789 slots, 41 resources, 41305
  nodes. Every source file, `.contract` included, is at most 1,500 lines (`client.ts` 1455,
  `app.contract` 1327, `composer-controls.contract` 1010).
- `cargo test -p t3-code-macos --lib`: 7 pass (Markdown/transcript parsing,
  table blocks and their column alignment).
- All 26 AppKit/XCTest binaries under `macos/tests/` pass with the recipe below:
  attach 3, composer 56, composer-files 4, contextmenu 6, fleet 8, intent 4, menus 10,
  notifications 4, r5-composer 3, r5-panels 5, r6-device 3 (loopback serve-sim peers),
  r6-media 5 (PDFKit, sandboxed WebKit; rendered HTML loads its siblings from the asset
  token's directory and, as the reference's frame, external hosts), r7-device 13 (H.264 over a loopback hub; with
  `T3_DEVICE_GLB_DIR` set to a T3 server's `client/assets` the served-model renders run
  too), r8-keys 6, r8-pointer 2 (the title row keeps the frame the host restored), r9-device 13 (the iPhone Duo viewer over loopback panel
  feeds and the served model, the foldable), r9-input 10, r10-connect 4 (select-on-open,
  Korean 2-Set chords re-issued by key code and reaching menu equivalents), r10-device 4 (a Duo panel feed reopens its stream after a stalled main thread,
  including a 10-run Closed loop; the physical hand-off's single elected feed), r11-device 3
  (the 3D phone keeps H.264 and 3D through main-thread stalls, ten first opens; the soft-queue
  window), r11-upstream 3 (the draft row's NSMenu), sidebar 5,
  ssh 4 (1 live test skipped), transport 31 (2 live tests skipped), snapshot 161 checks,
  mermaid 10 checks against a running HEAD server.
- The integrated app was built and driven against isolated HEAD-oracle (`f870c41`) backends
  with the deterministic fixture provider: the 34 round-3 states, 13 round-4 states and 9
  round-5 states at 1280×840 and 840×620, light and dark (224 pairs, equal to round 10);
  the pull request row against a local GitHub CLI stand-in (40 + 40 pairs); the attachment
  previews (17); the round-7 polish cells (16); the device workspace through a labelled
  fixture device hub (40; the 3D phone no longer falls back to flat on first open); the
  round-8 states (20); the round-9 states (42 pairs and 40 Duo / fold cells); the round-10
  states (Connections 5 pairs, device 20 pairs, 20 Duo Closed launches); and the round-11
  states: the Settle / Un-settle sweep (5 pairs, three threads settled and un-settled on the
  server), draft discard and Undo (6), workspace-preparation Retry (3, a real worktree
  created by the retry), the version pill, Load balancing / GitHub sharing with a switched-off
  loopback environment (8), rendered HTML with sibling assets and Shiki declarations (5), 15
  first opens of the 3D phone with the app's main thread frozen (SIGSTOP) three times each,
  and every right-panel tab after a relaunch (10, labelled synthetic seed transfer). Git,
  device and pull request effects were read back from disposable repositories, the
  stand-in's call log and the fixture hub's input log. The verdicts and the remaining
  differences are in `AGENT-HANDOFF.md`.

Live checks need a running T3 server; states the fixture cannot produce (subagents,
provider updates, usage data, linked pull requests with host state, terminal/element
chips) are covered by unit tests against the HEAD data shapes only. Real-keyboard and
real-pointer checks last ran in round 8 (chords, undo, menus, launcher, table menu,
multi-select, hovers, relaunch reconnect and frame, pairing, Return / Shift-Return,
right-click). The input fixes of rounds 9 to 11 (⌘B after a click, chords under Korean
2-Set inside and outside the composer, the first click after composing, the hover under a
still pointer after ⌘Z, a real wheel before a thread switch, the checkout field's
select-on-open, the row-action sweep with a real drag, the draft row's right-click menu) are
proven by AppKit binaries and agent drives only: the Mac's screen stayed locked through
every real-input window of rounds 9 to 11. The manual checklist is in `AGENT-HANDOFF.md`.

## Build and connect

Requires macOS 14 or later, Xcode, the repository's Rust toolchain, Bun version
from `package.json`, and the native TypeScript Hermes toolchain described in the
[root setup instructions](../../../README.md#1-install-the-tools). Run from the
Exact repository root:

```sh
bun install --frozen-lockfile
export EXACT_APP_DIR="$PWD/examples/t3-code"
bun examples/t3-code/terminal-host/build.mjs   # the terminal page into assets/ (ignored; EXACT2-GAPS X46)
bun examples/t3-code/stage-runtime.mjs         # the embedded T3 server into server-runtime/ (ignored; network once)
bun host/apple/build.mjs t3-code-macos --bundle --run
```

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

Safari's Web Inspector (exact2 #101): in a development build the app's own web views (the
terminal, the rendered-HTML preview, the offscreen Mermaid renderer) are `isInspectable`, so
Safari's Develop menu lists them. A release build's are not: the packaged build (that marker), a
production-trust bake or a distributed bundle (`--distribution`, what `exact release` signs), as
main #309 decides for Exact's own `iframe` web views (`T3WebInspection.swift`; each web view writes
a `t3.inspection:` line to stderr). There is no View › Toggle Developer Tools: an inspector for the
app's own UI stays out of exact2 (#101).

Start your existing T3 installation with `t3`, or use its normal source-checkout
startup command. Configure and authenticate at least one provider in T3 Code.
This client implements orchestration protocol 2 and requires the server's
`serverResolvedCommandContext` capability for writes. A server on another protocol
is listed as outdated or newer; one that can update itself offers **Update** in
Settings › Connections. A connected server older than this client shows "Server
update available" above the composer (and a warning in the thread details card);
its Update streams the update's progress, waits for the server to come back on the
new version and shares that state with Settings › Connections (`server-update.ts`).

1. In T3 Code, open **Settings → Connections** and create a fresh pairing link.
   A command-line installation can also use `t3 pair` for a running server.
2. In this app's welcome wizard (or **Add environment**), paste the pairing URL.
3. Wait for synchronization, then pick a project and thread, or add an existing
   workspace by its absolute path **on the server**. Adding a project does not create
   the directory.
4. Choose a model and send. New threads use the project override or the server
   default for permissions, workspace and interaction mode.

## Supported workflows

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
- Command palette (threads, projects, actions, files, content search, New thread in…,
  Link pull request), toasts, the Nightly mobile-app notice, macOS notifications (after
  the user grants them).
- Settings: all 14 routes with project/environment scope, search, theme editor and VS
  Code theme import, keybindings, providers, connections (pairing, SSH, outdated-host
  update), SnapShots, diagnostics and licenses.

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
on the Pull Requests page the open pull request) before it closes the window. The surface launcher takes the focus when it mounts and
answers its letters before type-to-focus (`R8KeysLauncher.swift`); a Markdown table's Copy
menu is a window-level popup, placed from the trigger's drawn frame (`R8KeysMeasure.swift`),
that Escape and an outside press close (`r8-keys-table-menu.*`). A global hotkey of another
app (on the round-8 Mac, Raycast holds ⌘1) never reaches the window.

Credentials stay in Keychain, scoped to the server origin and environment. The Swift
module atomically saves the versioned `t3-code.json` preference file (selections,
drafts, sidebar and page preferences, dismissed notices, pending operation identities)
under Exact's app data directory. **Disconnect** keeps the credential; **Forget**
removes it. The embedded server is the primary environment, "This machine"
(20261005-local-primary-environment, `local-primary.ts`): the client connects to it with the
bearer the server issued at start (memory only, never Keychain), never saves it and never
remembers its origin (its port can change every launch; the transport keeps a focus token
`primary` beside `t3.server.origin`). On launch the client reconnects by itself: to the primary
when it was the last focus, else to the last switched-on saved environment with its Keychain
credential, else to the primary while the Local environment switch is on (`r8-pointer-reconnect.ts`);
while the server starts, the window shows the connecting state (the host opens the first window
before the server is ready, exact2 #117). The window keeps its frame across launches
(the host's frame autosave, restored after the window's final style since exact2 #113). Settings ›
Connections shows "This machine" (`this-machine.ts`): the Local environment switch, which asks
first and then stops or starts the embedded server in place (the reference relaunches the app,
which exact2 cannot, #122), and the Version row. Under it (20261005-this-machine-network-access,
`connections-network.ts`) are Network access ("Reachable at <url>" and its endpoints; the server
binds 0.0.0.0 while it is on, loopback otherwise, and refuses when no LAN or Tailscale IPv4
exists), Tailscale HTTPS (the server runs `tailscale serve` on the chosen port; the app reads
`tailscale status --json` only while network access or Serve is on, cached 60 s,
`T3LocalNetwork.swift`), and the Authorized clients fold: pairing links with their permissions,
expiry, Share (endpoint choice, link, code, QR; HTTPS endpoints pair through the hosted app,
`pairing-urls.ts`) and Revoke, and the paired clients live from `subscribeAuthAccess`
(`auth-access.ts`). Each change restarts the embedded server in place with the new envelope.
The Local environment switch, Network access and Tailscale Serve (enabled, port) live where T3 Code
keeps them, `<T3 home>/userdata/desktop-settings.json` (`T3DesktopSettings.swift`: read once at the
first session, sparse compact writes through a temp file and a rename), so the clone and T3 Code
share them; the keys an older clone kept in `t3-code.json` are carried over once. The default
endpoint stays in `t3-code.json` (the reference's renderer localStorage); a created link's
credential stays in memory.
While the embedded server runs, `t3 app <dir>`
(the server's own CLI; T3 Code's `t3`, or `<T3 home>/runtime/versions/<version>/t3`) reaches the app
on `<$TMPDIR>/t3code-<uid>/<24 hex of sha256(<T3 home>/userdata)>.sock` (20261005-app-activation,
`T3AppControl.swift`, `desktop-activation.ts`): the app comes to the front, adds the folder as a
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

## Known limits and exclusions

The terminal renderer's [2026-10-06 verification](.exact/implementation/20261005-t3code-macos-parity/evidence/20261005-terminal-surface/20261006-theme-parity/attempt.md) includes light/dark,
preset/custom theme pairs against the original renderer, font updates, and integrated app
window captures with visible ANSI output and typed loopback text. Settings now reach the
terminal. This is renderer evidence; the drawer and PTY session integrations below remain excluded.

- Excluded or not built: the terminal drawer and Terminal surface (a hand-off's setup script
  runs on the server but its output is not shown; the `t3-terminal` view, T3 Code's own Ghostty
  emulator in a web view, exists with a development harness, ⌃⌥⇧T, `AGENT-HANDOFF.md` "Terminal
  spike"), the Browser surface,
  pinch zoom of the 3D phone (the reference's is a no-op too; the iPhone Duo's pinch moves its
  hinge, `R9DeviceDuoView.swift`), dragging and resizing the floating device player, web,
  iOS and Linux delivery.
- Known in-app differences (round 11): during a row-action sweep the hover card or tooltip
  that was open at the press stays until release, and Escape does not cancel the sweep (the
  reference closes the card and cancels); rendered HTML loads https and `http` hosts, named
  ones included (`app.json` allows arbitrary loads in web content, [#106](https://github.com/ccheever/exact2/issues/106)),
  and its token directory; the reference opens a thread's live device session as a
  floating player on load and this client does not; No project drafts cannot switch machine.
- Framework limits worked around in-app: host text truncates at word boundaries and
  draws no placeholder colour; negative-spread shadows draw faint; popovers open below, above or centred on
  their invoker (`position-area`) but never flip near a window edge (#112); SVG paths cannot morph (morph icons cross-fade); backdrop blur sees only
  its parent (the composer is opaque, where the reference's glass shows the transcript
  through it); a textarea sizes to its plain value (a prompt whose chip links are long
  can be a line taller than its chips draw at narrow widths); an Exact answer replaced for new arguments drops its native replies (LLP 1016 D5;
  a watched topic's re-ask no longer does since exact2 #183), so a cache that another answer could await
  shares resolved values only (`readDetail` in `r6-pr-actions.ts`); data sources have no clock; every
  editable textarea has `autocorrect="off"`, which keeps the typed bytes on macOS since
  exact2 #111 (`text-entry.test.ts` checks it). Workarounds
  for limits main has since fixed (hit testing, `pointer-events`, cursors, `position-area`,
  key modifiers, `title`) are gone; `EXACT2-GAPS.md` lists what was removed and each open
  item's state on the pin.
- Needs a person: physical modifier chords, right-click menus, real pointer drags and
  hovers, macOS notification and screen-capture grants, provider installs and logins,
  GitHub writes.

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
applies the host's rule to the view tree). `modules/apple/` is the native module; `apple/` holds the bake adapter
and native tests. T3's MIT notice is retained in `LICENSE-T3`.

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
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o /tmp/t3-code.plan
EXACT_APP_DIR="$PWD/examples/t3-code" cargo test -p t3-code-macos --lib   # with the Hermes env of the root setup
```

The Apple crate is a workspace member outside the default Cargo members; a Cargo
build alone does not compile or launch its Swift module. Use the app build above
for an integrated check.

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
  T3_BROWSER_TEST_DIR="$O" T3_COMPOSER_TEST_DIR="$O" T3_MENUS_TEST_DIR="$O" T3_MERMAID_TEST_DIR="$O" T3_PANELS_TEST_DIR="$O" T3_TERMINAL_TEST_DIR="$O" "$O/$n-tests" $([ $n = mermaid ] && echo "$T3_SERVER")
done
```

The `timeline-keyboard` regression uses the actual ExactKit key loop and scroll
views. It compiles ExactKit's sources into its own binary, so it passes
`-package-name apple`, the name SwiftPM gives `host/apple/Package.swift`'s
package (ExactKit's `package` declarations need one). After building the macOS
app (`bun host/apple/build.mjs t3-code-macos`, which writes
`target/aarch64-apple-darwin/host-dev/libt3_code_macos.a`), run it from the
repository root:

```sh
mkdir -p target/t3-tests
xcrun swiftc -swift-version 5 -package-name apple -module-name ExactKit -I host/apple/Sources/CExact \
  $(rg --files host/apple/Sources/ExactKit -g '*.swift') \
  examples/t3-code/macos/tests/timeline-keyboard/main.swift \
  -L target/aarch64-apple-darwin/host-dev -lt3_code_macos -lc++ \
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

To drive the app the way the parity rounds did (an isolated fixture backend serving the
HEAD oracle, the built app in agent mode, a headless Chrome reference), see the lane
tooling under `target/t3-ui-parity/` described in `AGENT-HANDOFF.md`.

## Pitfalls found here

These three were in the branch's copy of `docs/agent-pitfalls.md`; adopt-main-fixes-r7 moved
them here so the branch carries no framework-file edits. They go back to main's pitfalls file
with #99 if they still hold there.

- **A numeric `height` transition jumps on macOS.** Apple's automatic height ownership
  requires a border-box node with `interpolate-size="allow-keywords"`, including numeric
  endpoints. Use both on the clipping parent, for example `box-sizing="border-box"
  interpolate-size="allow-keywords" height=(open ? 280 : 0) transition="height 400ms
  ease-out"`. A fixed-height child then retains its grid while the parent animates. A
  standalone macOS probe measured 105.88 pt at 100 ms and continuous reversal. (Terminal
  parity, framework height code identical to `origin/main` `a72661fd4`, 2026-10-07;
  `host/apple/src/height.rs`.)
- **A one-edge border draws a 3 px frame, or a closed panel stays 3 px tall.**
  `border-style="solid"` enables every edge; unspecified widths retain CSS's `medium`
  default. Set `border-width=0` before the intended edge, for example `border-width=0
  border-top-width=1 border-style="solid"`. The terminal drawer measured 3 px when its
  declared height was zero, and its panes lost 6 px to unintended side borders. (Terminal
  parity, 2026-10-06.)
- **Physical IME input differs in a macOS agent window.** `EXACT_AGENT=1` launches a
  non-activating accessory app. Its window can be key and its textarea AX-focused while
  another process remains the foreground application. In that state, a terminal probe
  received the first Korean syllable as separate Jamo; a fresh terminal in a normally
  launched, active app composed it correctly. For physical keyboard acceptance, use the
  normal app and verify both `NSApp.isActive` and the foreground PID before asking someone
  to type. A key window or successful synthetic input alone is insufficient. Keep other test
  windows hidden and name the visible app. (Terminal parity, 2026-10-07.)
