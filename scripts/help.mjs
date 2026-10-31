#!/usr/bin/env bun
// The app CLI's help (LLP 1116 D4): one text per verb of the `exact.mjs` that
// `exact new` writes (game/new.mjs `commandsFor`), with every flag the verb's
// script takes, the ports it opens, whether it stays in the foreground, and an
// example. The generated file forwards `--help`, `-h`, `help` and no verb here,
// so `exact.mjs update` never carries help text. The scripts a verb runs call
// `guardFlags` first: `--help`/`-h` prints the verb's help and exits 0, and a
// flag the script does not take is refused, naming the ones it does, before
// anything builds or starts. Builders replay these texts in context: keep each
// short, and never name a flag its script does not parse (help.test.mjs checks).
// A verb with `more` (LLP 1116.003 D5) shows what a first build needs; `--help all`
// adds its other flags and `bodyAll`'s lines (the ports, the rarer forms).
//
//   bun scripts/help.mjs [<verb>] [all]      a verb's help (all: every flag and the ports), or the index

/** A flag's spec is its name, then its argument if it takes one (`--port <n>`); one row may hold
 * several, joined by " · ". */
const specs = (row) => row.split(' · ');
const name = (spec) => spec.split(' ')[0];
const takesValue = (spec) => spec.includes(' ');

// The drive flags `scripts/agent-launch.mjs` `parseFlags` takes, shared by `agent` and `test`
// (`runTests` reads all but those `test` lists as internal). A default said as exact.mjs's is the app
// launcher's (app-cli.mjs): scripts/agent.mjs run alone paints the bars and keeps the device's pixels,
// which the repository's smokes and pixel fixtures are written against.
const DRIVE = [
  ['--port <n>', 'web: the page’s port (or EXACT_PORT); backend origins remain those app.ts grants'],
  ['--backend <url>', 'web: select the backend origin for this build'],
  ['--storage <name>', 'a scratch store, kept on native hosts; Chrome\'s starts empty'],
  ['--keep-storage · --reset-storage', 'reopen a named store (Chrome, or a compare) · empty a named store first, on any host'],
  ['--size <w>x<h>', 'the viewport (default 420x900; a simulator\'s own)'],
  ['--scale <n>', 'screenshots at n pixels per point (exact.mjs agent on ios, macos: 1; an iPhone\'s own: 3)'],
  ['--chrome platform|agent', 'UIKit\'s bars (exact.mjs on iOS); agent paints the header'],
  ['--timing platform · --bind <extension>', 'Apple: UIKit\'s transitions keep their own timing (default: frozen) · Apple, Android: a module answered by its implementation, not the substitute'],
  ['--touch platform', 'iOS simulator: every tap a real touch, through the XCTest runner'],
  ['--epoch <ISO|ms|now> · --login [hidden]', 'the date at the clock\'s zero (default 2026-01-01) · a launch the login item made, hidden: with no window'],
  ['--locale <tag> · --format-locale <tag> · --time-zone <zone> · --seed <n>', 'default en-US, the locale (the Region\'s format, exactTime().formatLocale), UTC, 1'],
  ['--fail-fetch <prefix> · --permission <name>=<state> · --notification <entry> · --wake <entry> · --call \'<tool> <json>\'', 'each fetch whose URL starts with it fails, from the first data load · a module\'s permission at the start (geolocation=granted) · a relaunch handed a listed entry, as state.notifications printed it · a cold wake for a registration, as state.backgroundSync printed it · a cold call of a tool, as a system\'s call that started the app'],
  ['--process <target>=<file>|script:<file.json>|real · --login-env NAME=value · --home <dir>', 'macOS, Linux: what a child process is (none runs unnamed; `all=real` runs every one), the login shell\'s variables, home:/ for the data code (LLP 1133.007 §7)'],
  ['--url <url> · --browser chrome|firefox|webkit', 'web: drive a page already served (the dev loop\'s) instead of building · web (default chrome; bunx playwright@1.64.1 install firefox webkit)'],
  ['--sim <udid|name>', 'iOS: which simulator (default: a booted one)'],
  ['--device · --phone <udid|name> · --reader · --headless', 'a connected iPhone (built with ios --device), and which · Android: the APK (an `exact new` app\'s default) · the Linux host under adb shell'],
  ['--plan <file>', 'a compiled plan (contract build -o) in place of the app\'s'],
  ['--world <file> · --open <file>', 'a game: a saved world to load · macOS: a document on its command line'],
  ['--family <name> · --mode fullColor|accented|vibrant · --scheme light|dark · --props <json> · --placement <name> · --state <json>', 'gallery: widget variant/props or activity placement/state; --size large|xxxl is widget text size'],
  ['--json', 'one compact JSON object per op reply, one per line (default: short text)'],
];
const TEST_IGNORES = ['--bind <extension>', '--timing <v>', '--world <file>', '--open <file>', '--json', '--family <name>', '--mode <name>', '--scheme <name>', '--props <json>', '--placement <name>', '--state <json>'];
// The drive flags a first build reaches for; `--help all` shows the rest.
const DRIVE_FIRST = ['--storage', '--size', '--chrome', '--epoch', '--sim', '--json'];
const naming = (names, keep) => ([row]) => specs(row).some((spec) => names.includes(name(spec))) === keep;
const DRIVE_INTERNAL = ['--app <name>', '--session <label>', '--pid <pid>', '--out <file>'];
const NATIVE_PORTS = 'iOS simulator: a Unix socket; --device: a free TCP port; macOS, Linux, Android: stdio.';

// host/apple/build.mjs's flags, which `ios` and `mac` both run: each documents its own, and takes the rest.
const APPLE = ['--painted-benchmark', '--watchos', '--watch-sim <udid>', '--ios', '--tvos', '--device', '--run', '--no-install', '--prebuild', '--release', '--host', '--embed', '--bundle', '--distribution', '--unsigned', '--test',
  '--archive <out.ipa>', '--sim <udid|name>', '--phone <udid|name>', '--url <url>', '--optimize <speed|size>', '--link <plan>', '--widget-gallery'];
const appleRest = (documented) => APPLE.filter((spec) => !documented.some(([row]) => specs(row).some((d) => name(d) === name(spec))));
const OPTIMIZE = ['--optimize speed|size', 'the Rust archive\'s (default speed, or app.json host.ios|macos.optimize)'];
const PREBUILD = ['--prebuild', 'start the build apart from this shell and return; its log: .exact/<ios|macos>-build.log'];
const RELEASE = ['--release', 'the build that ships, as EXACT_UPDATE_TRUST=production: it ignores EXACT_AGENT and EXACT_MODULES, so drive the development build; a first is minutes: --prebuild --release early, then --release waits for it and takes seconds; an iOS agent drive installs over it, saying how to put it back'];

export const HELP = {
  backend: {
    line: 'run the Snapback backend',
    usage: 'bun exact.mjs backend [dev|serve <release>] [flags]',
    flags: [],
    body: 'Runs the backend package installed above the app, on app.ts’s loopback origin. Use snapback --port <n> to move it.',
  },
  snapback: {
    line: 'wire Snapback',
    usage: 'bun exact.mjs snapback [--port <n>] [--rebuild]',
    flags: [['--port <n>', 'select a local backend origin for the bake without source edits'],
      ['--online', 'use the server without a local device'],
      ['--artifact-only', 'with --rebuild: repair the device without changing wiring'],
      ['--rebuild', 'rebuild a missing or stale device after updating Snapback4']],
    body: 'Mounts the driver and generated operations, builds assets/snapback4.wasm, and names the server to start.',
  },
  status: {line:'web/prove status', usage:'bun exact.mjs status', flags:[], body:'Lists web/prove invocations with their phase, elapsed time, previous duration and exit status.', example:'bun exact.mjs status'},
  web: {
    line: 'the web dev loop: serves the app, reloading the page on every save (foreground)',
    usage: 'bun exact.mjs web [--port <n>] [--lan] [--allow-host <name>]… [--wasm]',
    common: ['web [--port <n>]    for a person watching: its URL, rebuilt on save'],
    flags: [
      ['--port <n>', 'listen on <n>, refused if taken (default 8765, else the first free port to 8864)'],
      ['--ready', 'wait for the current game web invocation to serve its current build; start web separately'],
      ['--timeout <s>', 'limit the --ready wait (default 600 seconds)'],
      ['--lan', 'bind every interface, so a phone on this network opens the printed LAN URL'],
    ],
    more: [
      ['--allow-host <name>', 'also answer to this host name (a tunnel\'s); repeatable'],
      ['--remote <host:port>', 'with a design: a TLS-only listener for paired remote clients (a certificate made for this run; its fingerprint printed)'],
      ['--trust-tunnel <port>', 'with a design: a loopback port for a tunnel you run; only paired tokens work through it'],
      ['--allow-plugin-origin', 'with a design: a sandboxed page (a Figma plugin) may call the wire with a paired token, here and through the tunnel'],
      ['--relay <host[:port]> · --relay-account <label> · --relay-cert <file> · --relay-key <file> · --relay-name <tool> · --relay-dns-hook <command> · --relay-acme <url> · --require-caa', 'with a design: dial out to a hosted relay that hands clients down unopened; its names\' certificate got by ACME (or yours, --relay-cert/--relay-key); --relay-name <tool> <canvas>, names of your own, their ACME record put up by your --relay-dns-hook, run in its own process group and stopped with it on a timeout or close (a command that leaves the group, by setsid, is yours to stop) (--require-caa: refuse until your CAA records pin it)'],
      ['--wasm', 'the resident wasm loop (a game\'s always): a plan restarts in place, state carried'],
      ['--pair-ttl <duration>', 'a paired design client\'s token lifetime (90m, 12h, 2d; default 12h)'],
    ],
    internal: ['--app <name>', '--serve-as <port>'],
    body: `Runs     in the foreground until ^C, printing its URL. The dev loop is for a person watching
         the app change: verify, test and agent build and serve their own copy, so checking
         the app needs no dev server.`,
    bodyAll: `Ports    one HTTP port on 127.0.0.1 (--lan: every interface), printed with the URL. A native
         client opening <url>__dev/open starts the producers on a second, loopback-only port.
         The server stops with its process however it ends.`,
    example: 'bun exact.mjs web --port 9000      # then open http://127.0.0.1:9000/',
  },
  'web-build': {
    line: 'build the web app once (the page test web and agent web drive)',
    usage: 'bun exact.mjs web-build [--out <dir>] [--render rust|js|none]',
    flags: [
      ['--out <dir>', 'a release build there (inside the app: dist), as deploy publishes, and how a static host serves it'],
      ['--render rust|js|none', 'pre-rendered pages: rust (the app\'s <app>-render entry; the default where it has one), js (Bun; the default otherwise), none'],
    ],
    internal: ['--js', '--wasm', '--bake'],
    body: `Builds the JS target (a game: wasm) into the app's target/web-dist; about a second when
nothing changed. A capability the JS runtime lacks fails the build, named. Beside the pages
are shell.html and routes.json, the route table a static host serves them by (serve).
Ports    none. Runs to completion: exit 0 when it built. test web and agent web run it first.`,
    example: 'bun exact.mjs web-build --out dist && bun exact.mjs serve dist',
  },
  serve: {
    line: 'serve a built site, as a static host would (npm start)',
    usage: 'bun exact.mjs serve [<dir>] [--port <n>] [--lan]',
    flags: [
      ['--port <n>', 'listen on <n> (default 8765)'],
      ['--lan', 'bind every interface, so another device on this network opens it'],
      ['--origin <dir>', 'a directory deploy published to, in place of a build'],
    ],
    internal: ['--loopback'],
    body: `Serves <dir> (default target/web-dist, web-build's) and prints its URL. For a request: the
file at its path; else, for a browser's navigation to a path a route in routes.json matches,
shell.html with 200, or for a render=request or cached route its page rendered under Bun;
else 404.html with 404. A static host serves the directory so, with those routes' shell.
Ports    one HTTP port, on 127.0.0.1 (--lan: every interface). Runs in the foreground until ^C.`,
    example: 'bun exact.mjs serve dist --port 3000',
  },
  test: {
    line: 'run *.test.contract (or the files named) on a host: web by default',
    usage: 'bun exact.mjs test [web|ios|macos|linux|windows|android] [<file.test.contract>|<glob>]… [flags]',
    common: ['test [web|ios] [<file>…] [--size <w>x<h>] [--sim <name>]'],
    flags: DRIVE.filter(naming(['--size', '--sim'], true)),
    more: [
      ['--test <file>', 'a test file, as a bare path or glob is (from the current directory)'],
      ['--storage <name>', 'the base of each test\'s own store name (default test)'],
      ...DRIVE.filter(naming(['--storage', '--size', '--sim', ...TEST_IGNORES.map(name)], false)),
    ],
    internal: [...TEST_IGNORES, ...DRIVE_INTERNAL],
    body: `Compiles first, a compile error said as contract build says it, so no contract build before
it: web builds the app (web-build); ios, macos and windows build when the build is missing or
older than its sources, saying so on stderr; windows drives at app.json's window size. Each test launches the app fresh with its own
store, after its first data has landed; its leading lines (size, epoch, fail fetch …)
override flags.
Server   runs server.ts on DB=.exact/test/db.sqlite; refuses a live server or an entry ignoring DB.
Runs     to completion, exit 1 when a test fails; nothing is left running.
Backend  test web never starts a Snapback backend: start snapback4 dev on app.ts’s selected origin’s port
         first; checked before launch. The app's own server.ts is managed as above. EXACT_PORT is the page’s.
Lines    tap, type, expect text|tree|state|disabled|refusal, clock data|settle|+<ms>|until state …, reload, back, fail|pass
         fetch, request <METHOD> "/path" [json] [as <persona>] [expect <status>]: docs/recipes/tests.md;
         all: docs/contract-grammar.md "Authored tests".`,
    bodyAll: `A phone's build is ios --device's.
Ports    --port <n> or EXACT_PORT chooses the page’s port; otherwise web: headless Chrome over a pipe; the page on 127.0.0.1, each test at a
         port fixed by its own store's name (20000-47999).
         ${NATIVE_PORTS}`,
    example: 'bun exact.mjs test ios tests/*.test.contract --size 390x844',
  },
  agent: {
    line: 'drive an app: tree, tap, type, state, screenshot, clock …',
    usage: 'bun exact.mjs agent [web|ios|macos|linux|windows|android] [flags] <op> [<op> …]',
    common: ['agent [web|ios] [--storage <name>] [--size <w>x<h>] [--sim <name>] <op> …',
      '  each op one quoted argument: "tap <target>" "type <target> <text>" "clock data|settle|+<ms>" "state [<name>]" tree "check [both]" "screenshot <png>" "resize <w>x<h>" "prefer <feature> <value>" back'],
    flags: DRIVE.filter(naming(DRIVE_FIRST, true)),
    more: DRIVE.filter(naming(DRIVE_FIRST, false)),
    internal: ['--test <file>', ...DRIVE_INTERNAL],
    body: `Gallery: ios|macos widget|activity <entry> tree|state|screenshot|clock · drag: tap <target> drag dx dy [hold ms during "read"] [then dx dy …] (one iOS touch).
Ops      a target is a testId, else a view's exact text or label
  tree [<target>|--ax] · logs · layout|perf [<target>] · resize <w>x<h> · close · back
  state [<name>] [--full]  a state, resource or field, dotted: groceries.0.done; >8 KB: sizes
  tap <target> [contextmenu|hover|drag <dx> <dy>|down|…] · tap up
  type <target> <text>|key <Name>   (a select, date or checkbox: its value)
  clock data|settle|held|+<ms> [real]    replies · transitions · push
  check [dark|both]  what is wrong · screenshot <png> [over <ms> every <ms>] [window]  window: bars
  screenshot <png> compare <ref> · prefer <feature> <value> · fail|pass fetch <prefix>`,
    bodyAll: `Builds if stale, launches, runs each quoted op on the agent's clock and closes. A game's world: post [<surface>] <text> is a message its next tick reads (type <surface> message <text>). No --storage, no storage; --reset-storage empties a native store, --keep-storage keeps a web or compare's. A deep link is typed on the navigate= root: "type <root> euxy://p/1" (macOS, iOS: through protocol_handlers, no alert). Held contact: "tap <t> down" "screenshot <png>" "tap up". clock held [<target>…] [<n>s] waits for a held reply with the clock frozen, then runs clock data (default bound: 20 s). clock until state <path> == <JSON literal> within <ms> waits, the clock frozen, until state reads it (a stream's state after its first message); a miss names the last value.
Ports    web: --port/EXACT_PORT, else a named store’s port (20000-47999), else free. ${NATIVE_PORTS}`,
    example: 'bun exact.mjs agent web --storage s1 tree "tap add" "screenshot .exact/a.png"',
  },
  verify: {
    line: 'build if stale, run *.test.contract, check each screen it reaches light and dark, relaunch once',
    usage: 'bun exact.mjs verify [web|ios|windows|android] [--sim <udid|name>] [--shots <dir>] [--rescroll] [--json]',
    common: ['verify [ios] [--shots <dir>] [--sim <name>]    done when it says "verify: clean"'],
    flags: [
      ['--sim <udid|name>', 'iOS: which simulator (else EXACT_SIM, a booted iPhone, or the newest Pro)'],
      ['--shots <dir>', 'also copy every screenshot it captured there, named as in .exact/verify/ (a test\'s own `screenshot` steps too)'],
      ['--rescroll', 'ios: measure scrolling again though nothing it depends on changed'],
      ['--json', 'the report as one JSON object (ios: `scrolling`, each finding\'s metric, value, UIKit\'s and remedy)'],
    ],
    internal: ['--app <name>'],
    body: `web launches at 390x844 and 1280x900, ios at the simulator's own size with UIKit's bars (an edit to only
app.contract or app.ts runs its new plan, no native rebuild), each on a fresh scratch store, and runs check
light and dark on the screen it opens to. The tests are app.test.contract's, then each other *.test.contract's
beside it, as test runs them. Each test's last screen is checked the same way in the test's own session (web: at
both widths; ios runs the tests with UIKit's bars). Then the app relaunches once, on real time, on the store a
passing test left, and says whether its first screen shows what that test stored. One short report: the build if
it built, the compiler's warnings (as contract build says them, built or not), the tests, check's findings (each
once, naming the screens it was on), the relaunch, then "verify: clean" or "verify: N problems". A clean verify
is done. One screenshot of each screen goes to .exact/verify/ (the first screen's per width and scheme, each
test's named by the test, the relaunch's): look at them when the features work.
Large    each of those screens again at the largest text size (ios: the simulator's, put back
         after; web: a 53 px root, iOS's body there, at 390x844, and the tests run again there
         to read up to 12 screens they pass through): what spills (past the edge, squeezed,
         overlapping) in the app's own layout, each with a remedy that gives the text room; a
         smaller or fixed font is the tell, and so is a line-clamp in a list's row or hyphens="auto"
         on a text of several words (a UILabel hyphenates only a word longer than its line; a
         grouped list's rows hyphenate as cells do). Counted ("3 at the largest text size"), never failing.
Native   windows: headless at app.json's window size, then the desktop's window (windows-window.png); android: the APK, light and dark. Neither checks, reads the largest text or scrolls yet.
Scroll   ios: ~15 s of real touches on a long first-screen list, once per build (host/apple/feel): "like UIKit", or
         a counted finding per metric the app can fix; Exact's own one uncounted line; load > cores: physics only.
Runs     to completion: exit 1 when the build, a test, a launch or the relaunch fails; the
         findings (check's, the relaunch's, the largest text size's, scrolling's) and the compiler's
         warnings are counted, never fail it. A screen no test ends on is not checked: end a test there.
         Its first line says about how long it takes (its last run's time): wait that once.
Server   an app with its own (server.ts, the rest pattern's): verify runs it where app.ts calls
         it, on an empty database (DB=.exact/verify/db.sqlite), through the relaunch, then stops
         it; one already answering there (a dev server) is refused, named, never written to; so is
         a server entry that does not read DB (process.env.DB), with the fix.
Ports    none to choose: test's and agent's (headless Chrome over a pipe; a simulator's socket).`,
    example: 'bun exact.mjs verify            # bun exact.mjs verify ios once at the end, for an iPhone app',
  },
  ios: {
    line: 'build for an iOS simulator and install it (--run launches it)',
    usage: 'bun exact.mjs ios [--run|--prebuild] [--release] [--sim <udid|name>] [--device [--phone <udid|name>]] [--url <url>]',
    common: ['ios [--run] [--sim <name>]    build for a simulator; --run launches it'],
    flags: [
      ['--run', 'also launch it (in Simulator.app, or on the phone); returns once launched'],
      PREBUILD,
      RELEASE,
      ['--sim <udid|name>', 'which simulator (else EXACT_SIM, a booted iPhone, or the newest Pro)'],
      ['--device', 'a connected iPhone, signed with a development profile on this Mac'],
      ['--phone <udid|name>', 'which phone, with --device'],
      ['--archive <out.ipa>', 'with --device: an .ipa to distribute (EXACT_IDENTITY, EXACT_PROFILE), no phone; production trust, so with an update store EXACT_UPDATE_RECEIPT or EXACT_UPDATE_GENESIS=1'],
      ['--unsigned', 'with --archive: ad-hoc signed, for a service that re-signs it'],
      ['--tvos', 'an Apple TV simulator instead (with --device, an Apple TV)'],
      ['--url <url>', 'connect the client to that dev server (web\'s URL) for live reload'],
      OPTIMIZE,
    ],
    get internal() { return appleRest(this.flags); },
    body: `The first build takes minutes, later ones a minute or two. Of several booted simulators it
takes the one the app is installed on, else refuses to guess (--sim or EXACT_SIM). test ios
and agent ios run it first themselves when the build is missing or older than its sources;
an edit to only app.contract or what app.ts imports runs its new plan on the existing build
instead, in about a second.
Ports    none. Runs to completion (--run too); --prebuild returns at once, the build going on
         apart from this shell (its pid printed; test, agent and ios without it wait for it).`,
    example: 'bun exact.mjs ios --run',
  },
  mac: {
    line: 'build the macOS app (--run launches it here)',
    usage: 'bun exact.mjs mac [--run|--prebuild] [--release] [--bundle] [--url <url>] [--optimize speed|size]',
    flags: [
      ['--run', 'assemble the .app and launch it when built, in the foreground (its log here) until it quits'],
      PREBUILD,
      RELEASE,
      ['--bundle', 'assemble the .app bundle (the macOS default)'],
      ['--painted-benchmark', 'build the app’s <macos-crate>-painted bin with ink; benchmark carrier only (LLP 1149 S1.1)'],
      ['--url <url>', 'connect the app to that dev server (web\'s URL) for live reload'],
      OPTIMIZE,
    ],
    get internal() { return appleRest(this.flags); },
    body: `test macos and agent macos run it first themselves when the build is missing or stale.
Ports    none. Runs to completion; with --run, until the app quits (^C ends it).`,
    example: 'bun exact.mjs mac --run',
  },
  linux: {
    line: 'build the Linux host (test and agent linux drive it headless)',
    usage: 'bun exact.mjs linux',
    flags: [],
    body: `A development build (cargo profile host-dev); prints the binary's path. It runs headless
anywhere, macOS included, with no display.
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs linux && bun exact.mjs test linux',
  },
  android: {
    line: 'build the APK and install it (test, agent and verify android drive it)',
    usage: 'bun exact.mjs android [--run] [--headless]',
    flags: [
      ['--run', 'launch it once installed'],
      ['--headless', 'build the Linux host for Android instead (agent android --headless runs it under adb shell)'],
    ],
    // `--apk --install` are the verb's own; --release builds the release profile, for measuring; --write-locks writes
    // the Gradle lock and verification files of an app whose modules name Maven libraries.
    internal: ['--apk', '--install', '--release', '--write-locks'],
    body: `Builds android/ for aarch64-linux-android, packages it with exact2's Canvas reader
(host/android) into an APK under target/android/, prints its path and installs it on the
device: ANDROID_SERIAL, else the one adb lists (with several and none named, it refuses).
With no device it builds and says so. test android and agent android drive the installed
APK and refuse one older than its sources, naming this command.
Needs    the SDK (ANDROID_HOME) with platform 37.2 and an NDK, a JDK 17+ (JAVA_HOME), once in
         this directory rustup target add aarch64-linux-android, and the Android Hermes
         bundle (~/.cache/exact/hermes-android; bun <exact2>/scripts/hermes-android.mjs).
Device   an emulator elsewhere: adb connect 127.0.0.1:<port> through an ssh tunnel to its
         adb port, then ANDROID_SERIAL=127.0.0.1:<port>.
Ports    none. Runs to completion.`,
    example: 'ANDROID_SERIAL=emulator-5554 bun exact.mjs android && bun exact.mjs test android',
  },
  game: {
    line: 'game vocab: fields, defaults and recipes',
    usage: 'bun exact.mjs game vocab [<name>] [--json]',
    flags: [['--json', 'structured declarations, fields, defaults and recipe links']],
    body: `No name lists components, resources and World verbs. A name also accepts Type::method.
Requires the SDK Rust toolchain; Cargo reuses its compiled inputs. Writes generated
artifacts under game/target only. Ports none. Runs to completion.`,
    example: 'bun exact.mjs game vocab Follow',
  },
  contract: {
    line: 'the Contract compiler: build, types, vocab, fmt, symbols …',
    usage: 'bun exact.mjs contract <subcommand> …      (paths from where you run it)',
    common: ['contract build <file> --json · contract vocab <name>…    diagnostics · any tag, property, colour role or recipe (no name: the card)'],
    flags: [],
    body: `  build <file.contract> [-o <file.plan>] [--json] [--map]   --json: [] or each diagnostic with its range;
                                          app.contract: app.ts type-checked too, as every host's build does
  types <file.contract> [-o <app.d.ts>]   the types app.ts imports (build app.contract, test, web refresh them)
  vocab [--json] [--all] [<name> …]       no name: the card, what an app writes, in one read (--all: every
                                          tag, attribute, CSS property and function); each name's entry in
                                          turn, a recipe whole, a pattern's summary; exit 1 if one is unknown
  fmt [--check | --stdout] <file.contract> · fmt --uses <file.contract>
  symbols <file.contract> [--name <name>] · sources <file.contract>
  test <file.test.contract>               the compiled tests, as JSON (bun exact.mjs test runs them)
  verify <file.contract> [--types] [--prove <Module>] · rust <file.contract> [-o <shapes.rs>]
  lean <file.contract> [--components] [--name <ident>] [-o <file.lean>]
  tamber check <app> | eval <app> [--mode <axis>=<value>] | ir <app> [-o <tamber.json>] | fmt <app> [--check]   (a flag before or after <app>)
                                          the app's design (LLP 1121): checked, its tokens in one mode, its IR
The first call compiles the compiler (minutes on a cold checkout).
Ports    none. Runs to completion: exit 0 when it compiled.`,
    example: 'bun exact.mjs contract build app.contract --json',
  },
  docs: {
    line: 'a guide section by heading',
    usage: 'bun exact.mjs docs <heading words>',
    flags: [],
    body: `Prints the section of start-here, contract-for-agents, contract-grammar, agent-pitfalls,
game-pitfalls, reference or a recipe that holds every word, as a whole word, most densely (in code and in
a recipe they weigh more), after its path and line, then the next matches. A section whose
heading is on the words prints from its heading, any other from the paragraph that
mentions them; up to 120 lines. A word may be a CSS name (-exact-exit-animation).
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs docs tabular numbers',
  },
  pattern: {
    line: 'start from a working, tested app: one-screen, list-detail, tabs, dashboard, rest',
    usage: 'bun exact.mjs pattern [<name>] [--force]',
    common: ['pattern [<name>]    a working, tested starter'],
    flags: [['--force', 'replace a file you edited (without it, it is refused, named)']],
    body: `With no name, lists the patterns. With one, writes its app.contract, app.ts and
app.test.contract here (and what else it needs: rest's server.ts) with this
app's id and name, then the types app.ts imports. A file still as exact new or a pattern
wrote it is replaced; one you edited is refused unless --force. Each pattern passes its own
tests and no-tells; rest's run with test or verify, which run its server.
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs pattern list-detail && bun exact.mjs test web',
  },
  tamber: {
    line: 'the app\'s design (app.tamber): checked, its tokens in one mode, its IR',
    usage: 'bun exact.mjs tamber check <app> | eval <app> [--mode <axis>=<value>] | ir <app> [-o <tamber.json>] | fmt <app> [--check] | gen ts|rust|swift <app> [-o <dir>] | serve <app> [--port <n>] [--lan] [--remote <host:port>] [--trust-tunnel <port>] [--allow-host <name>] [--allow-plugin-origin] [--relay <host> --relay-account <label> …] [--pair-ttl <duration>] | pair <url> --scope read|propose|write [--name <name>] | op <url> <request JSON>|subscribe | import css <stylesheet> [--app <app>] | import shadcn|tailwind|dtcg|pen|astryx <source> <url> [--app <app>] [--prefix <p>] | tool [<app>] [--via <dev url>] [--no-open]   (<app>: its directory, app.contract or a .tamber file; a flag before or after it)',
    flags: [
      ['--mode <axis>=<value>[,…]', 'eval: the mode to print the tokens in; the rest at their defaults'],
      ['-o <file>', 'ir: write the IR (tamber.json, generated, never committed) there; gen: the directory (else the app\'s .exact/tamber/)'],
      ['--check', 'fmt: write nothing; exit 1 where the layout would change'],
      ['--port <n>', 'serve: the port (else any free one)'],
      ['--lan', 'serve: bind every interface; a client there still cannot have the token'],
      ['--remote <host:port>', 'serve: a TLS-only listener for paired remote clients (a certificate made for this run; its fingerprint printed)'],
      ['--trust-tunnel <port>', 'serve: a loopback port for a tunnel you run; only paired tokens work through it'],
      ['--allow-host <name>', 'serve: the public name of a tunnel to --trust-tunnel with its own certificate; repeatable'],
      ['--allow-plugin-origin', 'serve: a sandboxed page (a Figma plugin) may call the wire with a paired token'],
      ['--relay <host[:port]> · --relay-account <label> · --relay-cert <file> · --relay-key <file> · --relay-name <tool> · --relay-dns-hook <command> · --relay-acme <url> · --require-caa', 'serve: dial out to a hosted relay that hands clients down unopened; its names\' certificate got by ACME (or yours, --relay-cert/--relay-key); --relay-name <tool> <canvas>, names of your own, their ACME record put up by your --relay-dns-hook, run in its own process group and stopped with it on a timeout or close (a command that leaves the group, by setsid, is yours to stop) (--require-caa: refuse until your CAA records pin it)'],
      ['--pair-ttl <duration>', 'serve: a paired client\'s token lifetime (90m, 12h, 2d; default 12h)'],
      ['--scope <s>', 'pair: the code\'s scope, read, propose or write'],
      ['--name <name>', 'pair: the name the paired client is listed by'],
      ['--via <url>', 'tool: the app\'s dev loop, which holds its design; the tool reaches it through its own port, waiting (up to 20 min) while the loop starts'],
      ['--no-open', 'tool: print the URL; open no browser'],
    ],
    body: `The design is the file app.json's "tamber" names, else app.tamber beside app.contract
(LLP 1121). check prints each refusal at its line; eval prints "name = value" a token.
serve is the design server (the dev loops mount it too, at /__tamber): reads, edits,
undo and a change stream, with a token only this machine's loopback can have. gen writes
bindings from the IR, generated and never committed (LLP 1121 D12): ts, tamber.ts (a
ColorToken union, each token's CSS per mode) and tamber.css; rust, tamber.rs, a
Token<Color> constant a token (SURFACE_CARD), which session.tamber().get resolves; swift,
tamber.swift, ExactTamberToken.surfaceCard for a native view's ExactTamber(view). op takes
that token and sends one request, or prints the change stream (subscribe). import css
writes a stylesheet's :root custom properties into the design, between markers a later
import of it rewrites, naming each it refuses (LLP 1153 D6). import <kind> holds another
system's theme on the server as proposals: the tokens as one, each role override apart.
tool is the design tool (LLP 1122), built if stale, served by the design server at
http://127.0.0.1:7740/__tamber/tool/ and opened. docs/recipes/tamber.md.
Ports    serve: a loopback port (printed), until ctrl-c; tool: 127.0.0.1:7740, the one its
grants name, until ctrl-c (EXACT_TOOL_PORT names another: the tool is built for it, and a dev
loop must be started with the same to let it frame its pages). The rest: none; they run to
completion.`,
    example: 'bun exact.mjs tamber eval . --mode dark-style=dim',
  },
  add: {
    line: 'a standard module (geolocation): its dependency in package.json, installed',
    usage: 'exact add <module> <app>',
    flags: [],
    body: `<module> is one of exact2's (geolocation, or @exact/geolocation): a path into this checkout,
so it compiles against the SDK the app uses. Then name what you use, and its grant line:
docs/recipes/permissions.md.
Ports    none. Runs to completion.`,
    example: 'bun <exact2>/scripts/exact.mjs add geolocation .',
  },
  hatch: {
    line: 'an access hatch: a stub per target the app builds, and its app.json entry',
    usage: 'bun exact.mjs hatch <word> | --app | --window',
    flags: [
      ['--app', 'the app-scope hatch, in place of a word'],
      ['--window', 'the window-scope hatch, in place of a word'],
    ],
    body: `<word> is lowercase letters and digits joined by "-" (avatar); mark a node with
hatch="<word>". Writes only missing files, which are yours from then on: docs/reference.md
"Access hatches".
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs hatch avatar',
  },
  update: {
    line: 'after exact2 moves: rewrite exact.mjs, patches, toolchain and exact2 paths',
    usage: 'bun exact.mjs update [--with <platforms>] [--without <platform>]',
    flags: [
      ['--with <platforms>', 'name native platforms in app.json (ios, mac, linux, android, windows), as their verbs do on first use'],
      ['--without <platform>', 'stop shipping a platform: its app.json keys go; in an app that authored its crates, linux or android also lose their crate, workspace member and lock entries'],
    ],
    internal: ['--update'],
    body: `Also rewrites AGENTS.md's generated block and adds the use lines the app's files lack;
app.json commands and your files are kept. EXACT2=<path> names another exact2 checkout.
An app that authored no Cargo workspace has none to rewrite: its native workspace is
generated under .exact/ by the verb that builds it.
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs update',
  },
  feedback: {
    line: 'the authoring diary: preview, send, or set this project\'s standing answer',
    usage: 'bun exact.mjs feedback [preview|send [--yes] [--readme]|delete <id>|status|always|never|ask|local]',
    flags: [['--yes', 'with send: do not ask first on a terminal'], ['--readme', 'with send: attach the README (and AGENTS.md\'s own notes) too']],
    body: `  preview (the default)    exactly what send would send, redacted; the README offered beside it
  send · delete <id>       send it · delete a sent diary by its receipt
  status                   the standing answer, and what is unsent
  always|never|ask|local   set the answer (local: keep the diary, never ask or send)
Ports    none; send makes one HTTPS request. Runs to completion.`,
    example: 'bun exact.mjs feedback status',
  },
  // A game's own verbs (exact2's game/README.md).
  check: {
    line: 'check game logic types and determinism without linking',
    usage: 'bun exact.mjs check',
    flags: [], internal: ['--check'],
    body: 'Checks the logic library only. Host, linker and target errors belong to the build.\nPorts    none. Runs to completion; exits 1 on failure.',
    example: 'bun exact.mjs check',
  },
  'test-rust': {
    line: 'the game\'s hostless Rust tests (logic/tests) and determinism lints',
    usage: 'bun exact.mjs test-rust',
    flags: [],
    internal: ['--test'],
    body: 'Ports    none. Runs to completion: exit 1 when a test or lint fails.',
    example: 'bun exact.mjs test-rust',
  },
  shells: {
    line: 'write .shells/ without a build',
    usage: 'bun exact.mjs shells',
    flags: [],
    body: 'For Cargo commands in render/ or another crate that names ../.shells.\nPorts    none. Runs to completion.',
    example: 'bun exact.mjs shells',
  },
  lock: {
    line: 'refresh the app or game Cargo.lock against this Exact checkout',
    usage: 'bun exact.mjs lock',
    flags: [],
    internal: ['--lock'],
    body: 'Use after exact update reports app lock drift, after this checkout moves or after adding a dependency.\nApps preserve locked versions where possible, resolve offline first and fetch missing dependencies when needed; games fetch and recapture their lock. Builds never change the lock.\nPorts    none. Runs to completion.',
    example: 'bun exact.mjs lock',
  },
  windows: {
    line: 'build for Windows into dist-windows/ (--run opens it)',
    usage: 'bun exact.mjs windows [--run] [--release] [--out <dir>]',
    flags: [['--run', 'open it when built: here, until it quits; from SSH, on the signed-in desktop, returning once its window is up'], ['--release', 'the release profile (default gpu-dev)'], ['--out <dir>', 'package into <dir> instead of dist-windows/, e.g. beside a copy still running']],
    body: 'Needs Windows and the MSVC Rust target; test windows and agent windows build first when the build is stale, and\ndrive it headless. A build closes the copy the last --run opened from the same directory.\nPorts    none. Runs to completion; with --run here, until the app quits.',
    example: 'bun exact.mjs windows --run',
  },
  prove: {
    line: 'the game\'s proof: a first baseline in pins.json, then checks against it',
    usage: 'bun exact.mjs prove [--hosts <list>] [--repeat <n>] [--compare-saves] [--repin] [--release]',
    flags: [
      ['--hosts <list>', 'comma-separated, of web,linux,windows,macos,ios (default this machine\'s native host)'],
      ['--repeat <n>', 'runs per host (default 1)'],
      ['--compare-saves', 'also require identical saves across hosts'],
      ['--repin', 'accept new pins once every host and mode agrees'],
      ['--release', 'also check native release and shipping web; --repin publishes coverage'],
      ['--continue <run>', 'reuse the snapshot and completed legs of artifacts/prove/<run>'],
      ['--device', 'ios on a connected phone'],
      ['--phone <udid|name>', 'which phone, with --device'],
      ['--report', 'print the facilities a failed host lacks'],
    ],
    body: 'Ports    none. Runs to completion.',
    example: 'bun exact.mjs prove --hosts linux,web --compare-saves',
  },
  // exact2's own verb that makes an app (scripts/exact.mjs new), not one of an app's.
  new: {
    line: 'a new app outside this repo, using this checkout',
    usage: 'bun <exact2>/scripts/exact.mjs new <path> [--name <name>] [--bundle-id <id>] [--with ios,mac,linux,android,windows] [--game [--assets]]',
    flags: [
      ['--name <name>', 'the name people see (app.json app.name); default: the path\'s last part, capitalised'],
      ['--bundle-id <id>', 'the reverse-DNS id (app.json app.id, app.ts appId); default: com.example.<name>-<hex>'],
      ['--with <platforms>', 'name native platforms in app.json now (a native verb names its own the first time it runs)'],
      ['--online', 'use the server without a local device'],
      ['--snapback', 'write a guestbook wired to the Snapback backend above the app'],
      ['--port <n>', 'the Snapback origin’s port with --snapback (default 4400)'],
      ['--game', 'a game (game/README.md): a Rust world under Contract\'s menus'],
      ['--assets', 'with --game: declare game.assets'],
      ['--update', 'rewrite an existing app\'s generated files (its exact.mjs update runs this)'],
      ['--without <platform>', 'with --update: stop shipping a platform'],
    ],
    body: `The app is eight files and no Rust: app.contract, app.ts, app.test.contract, app.json, exact.mjs,
AGENTS.md, package.json and .gitignore. A native verb (ios, mac, linux, android, windows) names its platform
in app.json the first time it runs and builds in a workspace generated under .exact/ (LLP 1153 D1). Give
--name and --bundle-id here, before the first build: a later change to the id makes the next build cold
and leaves the old app installed. Then cd there: its exact.mjs runs everything, and bun exact.mjs help
lists its verbs. Read docs/start-here.md first.
Ports    none. Runs to completion.`,
    example: 'bun scripts/exact.mjs new ~/notes --name Notes --bundle-id com.yourcompany.notes && cd ~/notes && bun exact.mjs test web',
  },
};

/** A verb's help text: what a first build needs, or with `all` every flag and form. */
export function helpText(verb, { all = false } = {}) {
  const entry = HELP[verb];
  if (!entry) return null;
  const rows = all ? [...entry.flags, ...(entry.more ?? [])] : entry.flags;
  const width = Math.min(22, Math.max(0, ...rows.map(([row]) => row.length)));
  const flags = rows.map(([row, what]) => `  ${row.padEnd(width)}  ${what}`).join('\n');
  const body = all && entry.bodyAll ? `${entry.body}\n${entry.bodyAll}` : entry.body;
  const hidden = (entry.more ?? []).flatMap(([row]) => specs(row).map(name));
  const more = !all && (hidden.length || entry.bodyAll)
    ? [`More     bun exact.mjs ${verb} --help all: every flag (${hidden.slice(0, 4).join(', ')}${hidden.length > 4 ? ' …' : ''}), the ports`]
    : [];
  return [entry.usage, `  ${entry.line}`, '', ...(flags ? [flags, ''] : []), body, ...(verb === 'agent' && all ? [driverCapabilities()] : []), `Example  ${entry.example}`, ...more].filter((line) => !all || line !== '').join('\n');
}

/** The index: each verb of an app's exact.mjs, then the app's own (app.json `commands`), then the forms a build uses
 * (LLP 1116.009 D6: builders read `help` and then four or five verbs' `--help` for the same flags, each a step carried in
 * context to the end; every flag a form names is one its script parses, help.test.mjs). */
export function indexText({ verbs = Object.keys(HELP).filter((v) => !['new', 'add'].includes(v)), own = {} } = {}) {
  const names = [...verbs, ...Object.keys(own)], width = Math.max(...names.map((v) => v.length));
  const line = (verb) => HELP[verb]?.line ?? (own[verb] ? `app.json commands: ${own[verb].join(' ')}` : '');
  const forms = verbs.flatMap((verb) => HELP[verb]?.common ?? []);
  return [
    'bun exact.mjs <verb> [arguments]    in the app directory (EXACT2=<path> selects the SDK)',
    ...names.map((verb) => `  ${verb.padEnd(width)}  ${line(verb)}`),
    ...forms.map((form) => `  ${form}`),
    'bun exact.mjs <verb> --help: flags and example; bun exact.mjs <verb> --help all: every flag and port.',
  ].join('\n');
}

/** What the generated exact.mjs prints for `help [verb]`, no verb, or a verb's `--help`; the exit code.
 * `all` among the command's arguments (`agent --help all`, `help agent all`) asks for the whole text. */
export function printHelp(verb, { verbs, own = {}, all = process.argv.slice(2).includes('all') } = {}) {
  if (!verb) return console.log(indexText({ verbs, own })), 0;
  if (own[verb] && !HELP[verb]) return console.log(`${verb}: this app's own command (app.json commands): ${own[verb].join(' ')}\nIts arguments follow as given; run in the app's directory.`), 0;
  const text = (!verbs || verbs.includes(verb)) && helpText(verb, { all });
  if (text) return console.log(text), 0;
  console.error(`no verb ${verb}\n\n${indexText({ verbs, own })}`);
  return 2;
}

/** The flags a verb's help documents, `--help all`'s included. */
const documented = (verb) => [...HELP[verb].flags, ...(HELP[verb].more ?? [])];

/** Every flag a verb's script takes, documented or not, by name: whether it takes a value. */
export const knownFlags = (verb) => new Map([...documented(verb).flatMap(([row]) => specs(row)), ...(HELP[verb].internal ?? [])].map((spec) => [name(spec), takesValue(spec)]));

/** A dispatched script's first call. `--help`/`-h` prints `verb`'s help and exits 0; a flag the
 * verb's script does not take is refused with the ones it does, exit 2. With `parsed`, `argv` is
 * what the script's own parser left, so every flag still in it is unknown (agent.mjs). */
export function guardFlags(verb, argv, { parsed = false, say = console.log, fail = console.error, exit = process.exit } = {}) {
  if (argv.includes('--help') || argv.includes('-h')) { say(helpText(verb, { all: argv.includes('all') })); return exit(0); }
  const entry = HELP[verb], known = knownFlags(verb);
  const unknown = [];
  for (let i = 0; i < argv.length; i++) {
    if (!/^-[^\d.]/.test(argv[i])) continue; // a positional, or a negative number
    if (!parsed && known.has(argv[i])) { if (known.get(argv[i])) i++; continue; }
    unknown.push(argv[i]);
  }
  if (!unknown.length) return argv;
  const takes = documented(verb).flatMap(([row]) => specs(row).map(name)).join(', ');
  fail(`${verb}: unknown flag ${unknown.join(', ')}. ${takes ? `It takes ${takes} (--help says what each takes and does).` : 'It takes no flags.'}`);
  return exit(2);
}

if (import.meta.main) { const args = process.argv.slice(2); process.exit(printHelp(args.find((a) => a !== 'all') ?? '', { all: args.includes('all') })); }

/** Host differences for the shared driver operations. */
export function driverCapabilities() {
  return `Capabilities: tree/layout/state/logs, tap/type/pick/clock/prefer/faults and screenshots: all hosts.
reload (interactive): web; reload (authored tests): all. Resize: web/macOS/Linux. Close: web/macOS. iOS touch: --touch platform.`;
}
