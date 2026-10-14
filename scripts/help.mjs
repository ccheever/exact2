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
// (`runTests` reads all but those `test` lists as internal).
const DRIVE = [
  ['--storage <name>', 'a scratch store, kept between drives by name'],
  ['--size <w>x<h>', 'the viewport (default 420x900; a simulator\'s own)'],
  ['--scale <n>', 'shrink screenshots to n pixels per point; 1 captures points'],
  ['--chrome platform', 'Apple: UIKit\'s bars, the Mac\'s sidebar (default: painted)'],
  ['--timing platform', 'Apple: UIKit\'s transitions keep their own timing (default: frozen)'],
  ['--touch platform', 'iOS simulator: every tap a real touch, through the XCTest runner'],
  ['--epoch <ISO|ms|now>', 'the date at the clock\'s zero (default 2026-01-01)'],
  ['--locale <tag> · --time-zone <zone> · --seed <n>', 'default en-US, UTC, 1'],
  ['--fail-fetch <prefix>', 'each fetch whose URL starts with it fails, from the first data load'],
  ['--url <url>', 'web: drive a page already served (the dev loop\'s) instead of building'],
  ['--browser chrome|firefox|webkit', 'web (default chrome; bunx playwright@1.63.0 install firefox webkit)'],
  ['--sim <udid|name>', 'iOS: which simulator (default: a booted iPhone)'],
  ['--device · --phone <udid|name> · --reader', 'a connected iPhone (built with ios --device), and which · Android: the app on the Canvas reader (its APK)'],
  ['--plan <file>', 'a compiled plan (contract build -o) in place of the app\'s'],
  ['--world <file> · --open <file>', 'a game: a saved world to load · macOS: a document on its command line'],
  ['--json', 'one compact JSON object per op reply, one per line (default: short text)'],
];
const TEST_IGNORES = ['--timing <v>', '--world <file>', '--open <file>', '--json'];
// The drive flags a first build reaches for; `--help all` shows the rest.
const DRIVE_FIRST = ['--storage', '--size', '--chrome', '--epoch', '--sim', '--json'];
const naming = (names, keep) => ([row]) => specs(row).some((spec) => names.includes(name(spec))) === keep;
const DRIVE_INTERNAL = ['--app <name>', '--session <label>'];
const NATIVE_PORTS = 'iOS simulator: a Unix socket; --device: a free TCP port; macOS, Linux, Android: stdio.';

// host/apple/build.mjs's flags, which `ios` and `mac` both run: each documents its own, and takes the rest.
const APPLE = ['--ios', '--tvos', '--device', '--run', '--no-install', '--prebuild', '--host', '--embed', '--bundle', '--distribution', '--unsigned', '--test',
  '--archive <out.ipa>', '--sim <udid|name>', '--phone <udid|name>', '--url <url>', '--optimize <speed|size>', '--link <plan>'];
const appleRest = (documented) => APPLE.filter((spec) => !documented.some(([row]) => specs(row).some((d) => name(d) === name(spec))));
const OPTIMIZE = ['--optimize speed|size', 'the Rust archive\'s (default speed, or app.json host.ios|macos.optimize)'];
const PREBUILD = ['--prebuild', 'start the build apart from this shell and return; its log: .exact/<ios|macos>-build.log'];

export const HELP = {
  web: {
    line: 'the web dev loop: serves the app, rebuilds and reloads the page on every save (foreground)',
    usage: 'bun exact.mjs web [--port <n>] [--lan] [--allow-host <name>]… [--wasm]',
    common: ['web [--port <n>]    for a person watching: its URL, rebuilt on save'],
    flags: [
      ['--port <n>', 'listen on <n>, refused if taken (default 8765, else the first free port to 8864)'],
      ['--lan', 'bind every interface, so a phone on this network opens the printed LAN URL'],
    ],
    more: [
      ['--allow-host <name>', 'also answer to this host name (a tunnel\'s); repeatable'],
      ['--wasm', 'the resident wasm loop (a game\'s always): a plan restarts in place, state carried'],
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
    usage: 'bun exact.mjs web-build [--render rust|js|none]',
    flags: [
      ['--render rust|js|none', 'pre-rendered pages: rust (default, the app\'s <app>-render entry), js (Bun), none'],
    ],
    internal: ['--js', '--wasm', '--bake'],
    body: `Builds the JS target (a game: wasm) into the app's target/web-dist; about a second when
nothing changed. A capability the JS runtime lacks fails the build, named.
Ports    none. Runs to completion: exit 0 when it built. test web and agent web run it first.`,
    example: 'bun exact.mjs web-build',
  },
  test: {
    line: 'run app.test.contract (or the files named) on a host: web by default',
    usage: 'bun exact.mjs test [web|ios|macos|linux|android] [<file.test.contract>|<glob>]… [flags]',
    common: ['test [web|ios] [<file>…] [--size <w>x<h>] [--sim <name>]'],
    flags: DRIVE.filter(naming(['--size', '--sim'], true)),
    more: [
      ['--test <file>', 'a test file, as a bare path or glob is (from the current directory)'],
      ['--storage <name>', 'the base of each test\'s own store name (default test)'],
      ...DRIVE.filter(naming(['--storage', '--size', '--sim', ...TEST_IGNORES.map(name)], false)),
    ],
    internal: [...TEST_IGNORES, ...DRIVE_INTERNAL],
    body: `web builds the app first (web-build); ios and macos build first when the build is missing or
older than its sources, saying so on stderr. Each test launches the app fresh with its own
store, after its first data has landed; its leading lines (size, epoch, fail fetch …)
override flags.
Server   an app with its own (server.ts, the rest pattern's): test runs it where app.ts calls it,
         on an empty database (DB=.exact/test/db.sqlite), then stops it, as verify does; one
         already answering there (a dev server) is refused, named, never written to; so is a
         server entry that does not read DB (process.env.DB), with the fix.
Runs     to completion, exit 1 when a test fails; nothing is left running.
Lines    tap, type, expect text|tree|state|disabled, clock data|settle|+<ms>, reload, back, fail|pass
         fetch, request <METHOD> "/path" [json] [as <persona>] [expect <status>]: docs/recipes/tests.md;
         all: docs/contract-grammar.md "Authored tests".`,
    bodyAll: `A phone's build is ios --device's.
Ports    none to choose. web: headless Chrome over a pipe; the page on 127.0.0.1, each test at a
         port fixed by its own store's name (20000-47999).
         ${NATIVE_PORTS}`,
    example: 'bun exact.mjs test ios tests/*.test.contract --size 390x844',
  },
  agent: {
    line: 'drive the app as a person would: tree, tap, type, state, screenshot, clock … on a host',
    usage: 'bun exact.mjs agent [web|ios|macos|linux|android] [flags] <op> [<op> …]',
    common: ['agent [web|ios] [--storage <name>] [--size <w>x<h>] [--sim <name>] <op> …',
      '  each op one quoted argument: "tap <target>" "type <target> <text>" "clock data|settle|+<ms>"',
      '  "state [<name>]" tree "check [both]" "screenshot <png>" "resize <w>x<h>" "prefer <feature> <value>" back'],
    flags: DRIVE.filter(naming(DRIVE_FIRST, true)),
    more: DRIVE.filter(naming(DRIVE_FIRST, false)),
    internal: ['--test <file>', ...DRIVE_INTERNAL],
    body: `Launches the app anew each drive (building first if stale, said on stderr), runs each op (one
quoted argument each) on the agent's clock, prints its reply. Without --storage, storage is
refused. An app's server.ts runs for the drive, its database kept per --storage. Runs to completion.
Ops      a target is a testId, else a view's exact text or label
  tree [<target>] · tree --ax · logs · layout [<target>]
  state [<name>]  a view's testId, else a state or resource, dotted: groceries.0.done
  tap <target> [contextmenu|dblclick|hover|drag <dx> <dy>|…]
  type <target> <text> · type <target> key <Name>   (a select, date or checkbox: its value)
  clock data · clock settle · clock +<ms> [real]    land what is in flight · end transitions
  check [dark|both] · screenshot <file.png> [window]  what is wrong, in text, before a picture · window: bars
  prefer <feature> <value> · perf [<target>] · resize <w>x<h> · close · back · fail|pass fetch <prefix>`,
    bodyAll: `Trace    trace <file> reads a saved trace. Op forms: docs/contract-for-agents.md, "Inspection and testing".
Ports    none to choose. web: headless Chrome over a pipe; the page on 127.0.0.1 at a free port, or
         with --storage one fixed by app and store (20000-47999; "in use": another drive has it).
         ${NATIVE_PORTS}`,
    example: 'bun exact.mjs agent web --storage s1 "clock data" tree "tap add" "screenshot .exact/a.png"',
  },
  verify: {
    line: 'build if stale, run app.test.contract, check each screen it reaches light and dark, relaunch once',
    usage: 'bun exact.mjs verify [web|ios] [--sim <udid|name>] [--shots <dir>] [--json]',
    common: ['verify [ios] [--shots <dir>] [--sim <name>]    done when it says "verify: clean"'],
    flags: [
      ['--sim <udid|name>', 'iOS: which simulator (else EXACT_SIM, a booted iPhone, or the newest Pro)'],
      ['--shots <dir>', 'also copy every screenshot it captured there, named as in .exact/verify/ (a test\'s own `screenshot` steps too)'],
      ['--json', 'the report as one JSON object'],
    ],
    internal: ['--app <name>'],
    body: `web launches at 390x844 and 1280x900, ios at the simulator's own size with UIKit's bars
(an edit to only app.contract or app.ts runs its new plan, no native rebuild), each on a
fresh scratch store, and runs check light and dark on the screen it opens to. Each test's
last screen is checked the same way in the test's own session (web: at both widths; ios
runs the tests with UIKit's bars). Then the app relaunches once, on real time, on the store
a passing test left, and says whether its first screen shows what that test stored. One
short report: the build if it built, the compiler's warnings (as contract build says them,
built or not), the tests, check's findings (each once, naming the screens it was on), the
relaunch, then "verify: clean" or "verify: N problems". A clean verify is done. One
screenshot of each screen goes to .exact/verify/ (the first screen's per width and scheme,
each test's named by the test, the relaunch's): look at them when the features work.
Runs     to completion: exit 1 when the build, a test, a launch or the relaunch fails; the
         findings (check's, the relaunch's) and the compiler's warnings are counted, never
         fail it. A screen no test ends on is not checked: end a test there.
Server   an app with its own (server.ts, the rest pattern's): verify runs it where app.ts calls
         it, on an empty database (DB=.exact/verify/db.sqlite), through the relaunch, then stops
         it; one already answering there (a dev server) is refused, named, never written to; so is
         a server entry that does not read DB (process.env.DB), with the fix.
Ports    none to choose: test's and agent's (headless Chrome over a pipe; a simulator's socket).`,
    example: 'bun exact.mjs verify            # bun exact.mjs verify ios once at the end, for an iPhone app',
  },
  ios: {
    line: 'build for an iOS simulator and install it (--run launches it)',
    usage: 'bun exact.mjs ios [--run|--prebuild] [--sim <udid|name>] [--device [--phone <udid|name>]] [--url <url>]',
    common: ['ios [--run] [--sim <name>]    build for a simulator; --run launches it'],
    flags: [
      ['--run', 'also launch it (in Simulator.app, or on the phone); returns once launched'],
      PREBUILD,
      ['--sim <udid|name>', 'which simulator (else EXACT_SIM, a booted iPhone, or the newest Pro)'],
      ['--device', 'a connected iPhone, signed with a development profile on this Mac'],
      ['--phone <udid|name>', 'which phone, with --device'],
      ['--archive <out.ipa>', 'with --device: an .ipa to distribute (EXACT_IDENTITY, EXACT_PROFILE), no phone'],
      ['--unsigned', 'with --archive: ad-hoc signed, for a service that re-signs it'],
      ['--tvos', 'an Apple TV simulator instead (with --device, an Apple TV)'],
      ['--url <url>', 'connect the client to that dev server (web\'s URL) for live reload'],
      OPTIMIZE,
    ],
    get internal() { return appleRest(this.flags); },
    body: `The first build takes minutes, later ones a minute or two; several booted simulators are
refused, not guessed (set EXACT_SIM). test ios and agent ios run it first themselves when
the build is missing or older than its sources; an edit to only app.contract or what app.ts
imports runs its new plan on the existing build instead, in about a second.
Ports    none. Runs to completion (--run too); --prebuild returns at once, the build going on
         apart from this shell (its pid printed; test and agent wait for it).`,
    example: 'bun exact.mjs ios --run',
  },
  mac: {
    line: 'build the macOS app (--run launches it here)',
    usage: 'bun exact.mjs mac [--run|--prebuild] [--bundle] [--url <url>] [--optimize speed|size]',
    flags: [
      ['--run', 'launch it when built, in the foreground (its log here) until it quits'],
      PREBUILD,
      ['--bundle', 'also assemble the .app bundle'],
      ['--url <url>', 'with --run or --bundle: connect it to that dev server (web\'s URL) for live reload'],
      OPTIMIZE,
    ],
    get internal() { return appleRest(this.flags); },
    body: `test macos and agent macos run it first themselves when the build is missing or stale.
Ports    none. Runs to completion; with --run, until the app quits (^C ends it).`,
    example: 'bun exact.mjs mac --run',
  },
  linux: {
    line: 'build the Linux host, which test linux and agent linux drive headless',
    usage: 'bun exact.mjs linux',
    flags: [],
    body: `A development build (cargo profile host-dev); prints the binary's path. It runs headless
anywhere, macOS included, with no display.
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs linux && bun exact.mjs test linux',
  },
  android: {
    line: 'build the Linux host for Android, which test android and agent android drive over adb',
    usage: 'bun exact.mjs android',
    flags: [],
    // The app on the Canvas reader, as an APK (host/android/README.md): for an app with an `android/` crate,
    // which `exact new` does not write yet.
    internal: ['--apk'],
    body: `Builds for aarch64-linux-android and prints the binary's path; the drive runs it under
adb shell on the first device or emulator (ANDROID_SERIAL picks one). Needs the NDK
(ANDROID_NDK_HOME, else the newest under the SDK's ndk/) and, once in this directory,
rustup target add aarch64-linux-android. A TypeScript data module also needs the
Android Hermes bundle (bun <exact2>/scripts/hermes-android.mjs build, once).
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs android && bun exact.mjs agent android tree',
  },
  contract: {
    line: 'the Contract compiler: build, types, vocab, fmt, symbols …',
    usage: 'bun exact.mjs contract <subcommand> …      (paths from where you run it)',
    common: ['contract build <file> --json · contract vocab <name>…    diagnostics · any tag, property or colour role (no name: the card)'],
    flags: [],
    body: `  build <file.contract> [-o <file.plan>] [--json] [--map]   --json: [] or each diagnostic with its range
  types <file.contract> [-o <app.d.ts>]   the types app.ts imports (build app.contract, test, web refresh them)
  vocab [--json] [--all] [<name> …]       no name: the card, what an app writes, in one read (--all: every
                                          tag, attribute, CSS property and function); each name's entry in
                                          turn, exit 1 if one is unknown
  fmt [--check | --stdout] <file.contract> · fmt --uses <file.contract>
  symbols <file.contract> [--name <name>] · sources <file.contract>
  test <file.test.contract>               the compiled tests, as JSON (bun exact.mjs test runs them)
  verify <file.contract> [--types] [--prove <Module>] · rust <file.contract> [-o <shapes.rs>]
  lean <file.contract> [--components] [--name <ident>] [-o <file.lean>]
  tamber check | eval [--mode <axis>=<value>] | ir [-o <tamber.json>] <app dir | app.contract | file.tamber>
                                          the app's design (LLP 1121): checked, its tokens in one mode, its IR
The first call compiles the compiler (minutes on a cold checkout).
Ports    none. Runs to completion: exit 0 when it compiled.`,
    example: 'bun exact.mjs contract build app.contract --json',
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
    usage: 'bun exact.mjs tamber check | eval [--mode <axis>=<value>] | ir [-o <tamber.json>] <app dir | app.contract | file.tamber>',
    flags: [
      ['--mode <axis>=<value>[,…]', 'eval: the mode to print the tokens in; the rest at their defaults'],
      ['-o <file>', 'ir: write the IR (tamber.json, generated, never committed) there'],
    ],
    body: `The design is the file app.json's "tamber" names, else app.tamber beside app.contract
(LLP 1121). check prints each refusal at its line; eval prints "name = value" a token.
docs/recipes/tamber.md.
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs tamber eval . --mode dark-style=dim',
  },
  hatch: {
    line: 'an access hatch: a stub for each target the app builds, and its app.json entry',
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
    line: 'after exact2 moves or changes: rewrite exact.mjs, patches, toolchain and exact2 paths',
    usage: 'bun exact.mjs update',
    flags: [],
    internal: ['--update'],
    body: `Also rewrites AGENTS.md's generated block and adds the use lines the app's files lack;
app.json commands and your files are kept. EXACT2=<path> names another exact2 checkout.
Ports    none. Runs to completion.`,
    example: 'bun exact.mjs update',
  },
  feedback: {
    line: 'the authoring diary: preview, send, or set this project\'s standing answer',
    usage: 'bun exact.mjs feedback [preview|send [--yes]|delete <id>|status|always|never|ask|local]',
    flags: [['--yes', 'with send: do not ask first on a terminal']],
    body: `  preview (the default)    exactly what send would send, redacted
  send · delete <id>       send it · delete a sent diary by its receipt
  status                   the standing answer, and what is unsent
  always|never|ask|local   set the answer (local: keep the diary, never ask or send)
Ports    none; send makes one HTTPS request. Runs to completion.`,
    example: 'bun exact.mjs feedback status',
  },
  // A game's own verbs (exact2's game/README.md).
  'test-rust': {
    line: 'the game\'s hostless Rust tests (logic/tests) and determinism lints',
    usage: 'bun exact.mjs test-rust',
    flags: [],
    internal: ['--test'],
    body: 'Ports    none. Runs to completion: exit 1 when a test or lint fails.',
    example: 'bun exact.mjs test-rust',
  },
  lock: {
    line: 'fetch (the network), resolve Cargo.lock again and print what changed',
    usage: 'bun exact.mjs lock',
    flags: [],
    internal: ['--lock'],
    body: 'When a build says exact2 moved since the lock was captured, or after adding a dependency;\nbuilds never change the lock.\nPorts    none. Runs to completion.',
    example: 'bun exact.mjs lock',
  },
  windows: {
    line: 'build the standalone Windows game into dist-windows/ (on Windows)',
    usage: 'bun exact.mjs windows [--run] [--release]',
    flags: [['--run', 'launch it when built'], ['--release', 'the release profile (default gpu-dev)']],
    body: 'Needs Windows and the MSVC Rust target.\nPorts    none. Runs to completion; with --run, until the game quits.',
    example: 'bun exact.mjs windows --run',
  },
  prove: {
    line: 'the game\'s proof: a first baseline in pins.json, then checks against it',
    usage: 'bun exact.mjs prove [--hosts <list>] [--repeat <n>] [--compare-saves] [--repin]',
    flags: [
      ['--hosts <list>', 'comma-separated, of web,linux,windows,macos,ios (default this machine\'s native host)'],
      ['--repeat <n>', 'runs per host (default 1)'],
      ['--compare-saves', 'also require identical saves across hosts'],
      ['--repin', 'accept new pins once every host and mode agrees'],
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
    usage: 'bun <exact2>/scripts/exact.mjs new <path> [--game [--assets]]',
    flags: [
      ['--game', 'a game (game/README.md): a Rust world under Contract\'s menus'],
      ['--assets', 'with --game: declare game.assets'],
      ['--update', 'rewrite an existing app\'s generated files (its exact.mjs update runs this)'],
    ],
    body: `The path's last part names the app. Then cd there: its exact.mjs runs everything, and
bun exact.mjs help lists its verbs. Read docs/start-here.md first.
Ports    none. Runs to completion.`,
    example: 'bun scripts/exact.mjs new ~/notes && cd ~/notes && bun exact.mjs test web',
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
  return [entry.usage, `  ${entry.line}`, '', ...(flags ? [flags, ''] : []), body, `Example  ${entry.example}`, ...more].join('\n');
}

/** The index: each verb of an app's exact.mjs, then the app's own (app.json `commands`), then the forms a build uses
 * (LLP 1116.009 D6: builders read `help` and then four or five verbs' `--help` for the same flags, each a step carried in
 * context to the end; every flag a form names is one its script parses, help.test.mjs). */
export function indexText({ verbs = Object.keys(HELP).filter((v) => v !== 'new'), own = {} } = {}) {
  const names = [...verbs, ...Object.keys(own)], width = Math.max(...names.map((v) => v.length));
  const line = (verb) => HELP[verb]?.line ?? (own[verb] ? `app.json commands: ${own[verb].join(' ')}` : '');
  const forms = verbs.flatMap((verb) => HELP[verb]?.common ?? []);
  return [
    'bun exact.mjs <verb> [arguments]    from the app\'s directory (EXACT2=<path> picks another exact2)',
    '',
    ...names.map((verb) => `  ${verb.padEnd(width)}  ${line(verb)}`),
    ...(forms.length ? ['', 'The forms a build uses:', ...forms.map((form) => `  ${form}`)] : []),
    '',
    'bun exact.mjs <verb> --help: what each flag does, whether it stays in the foreground, an example;',
    'bun exact.mjs <verb> --help all: every other flag and the ports it opens. Read it only for a flag not above.',
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
