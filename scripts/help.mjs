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
//
//   bun scripts/help.mjs [<verb>]      a verb's help, or the index

/** A flag's spec is its name, then its argument if it takes one (`--port <n>`); one row may hold
 * several, joined by " · ". */
const specs = (row) => row.split(' · ');
const name = (spec) => spec.split(' ')[0];
const takesValue = (spec) => spec.includes(' ');

// The drive flags `scripts/agent-launch.mjs` `parseFlags` takes, shared by `agent` and `test`
// (`runTests` reads all but those `test` lists as internal).
const DRIVE = [
  ['--storage <name>', 'a scratch store for storage sources, kept between drives under that name'],
  ['--size <w>x<h>', 'the viewport (desktop default 420x900; a simulator is its device\'s size)'],
  ['--chrome platform', 'Apple: the real navigation and tab bars (default: the authored ones, painted)'],
  ['--timing platform', 'Apple: UIKit\'s transitions keep their own timing (default: frozen)'],
  ['--touch platform', 'iOS simulator: every tap a real touch, through the XCTest runner'],
  ['--epoch <ISO|ms|now>', 'the date at the agent clock\'s zero (default 2026-01-01T00:00:00Z)'],
  ['--locale <tag> · --time-zone <zone> · --seed <n>', 'default en-US, UTC, 1'],
  ['--fail-fetch <prefix>', 'each fetch whose URL starts with it fails, from the first data load'],
  ['--url <url>', 'web: drive a page already served (the dev loop\'s) instead of building'],
  ['--browser chrome|firefox|webkit', 'web (default chrome; bunx playwright@1.63.0 install firefox webkit)'],
  ['--device · --phone <udid|name>', 'a connected iPhone (built with ios --device), and which'],
  ['--plan <file>', 'a compiled plan (contract build -o) in place of the app\'s'],
  ['--world <file>', 'a game: a saved world to load'],
  ['--open <file> · --json', 'macOS: a document on its command line · replies as JSON lines'],
];
const TEST_IGNORES = ['--timing <v>', '--world <file>', '--open <file>', '--json'];
const DRIVE_INTERNAL = ['--app <name>', '--session <label>'];
const NATIVE_PORTS = 'iOS simulator: a Unix socket; --device: a free TCP port; macOS, Linux, Android: stdio.';

// host/apple/build.mjs's flags, which `ios` and `mac` both run: each documents its own, and takes the rest.
const APPLE = ['--ios', '--tvos', '--device', '--run', '--host', '--embed', '--bundle', '--distribution', '--unsigned', '--test',
  '--archive <out.ipa>', '--sim <udid|name>', '--phone <udid|name>', '--url <url>', '--optimize <speed|size>', '--link <plan>'];
const appleRest = (documented) => APPLE.filter((spec) => !documented.some(([row]) => specs(row).some((d) => name(d) === name(spec))));
const OPTIMIZE = ['--optimize speed|size', 'the Rust archive\'s (default speed, or app.json host.ios|macos.optimize)'];

export const HELP = {
  web: {
    line: 'the web dev loop: serves the app, rebuilds and reloads the page on every save (foreground)',
    usage: 'bun exact.mjs web [--port <n>] [--lan] [--allow-host <name>]… [--wasm]',
    flags: [
      ['--port <n>', 'listen on <n>, refused if taken (default 8765, else the first free port to 8864)'],
      ['--lan', 'bind every interface, so a phone on this network opens the printed LAN URL'],
      ['--allow-host <name>', 'also answer to this host name (a tunnel\'s); repeatable'],
      ['--wasm', 'the resident wasm loop (a game\'s always): a plan restarts in place, state carried'],
    ],
    internal: ['--app <name>', '--serve-as <port>'],
    body: `Ports    one HTTP port on 127.0.0.1 (--lan: every interface), printed with the URL. A native
         client opening <url>__dev/open starts the producers on a second, loopback-only port.
Runs     in the foreground until ^C. test web and agent web do not need it: they build and
         serve their own copy. To keep it while you work:
           bun exact.mjs web > .exact/web.log 2>&1 &      # $! is its PID; kill <PID> stops it
         The server stops with that process however it ends. If your tool's shell ends
         background jobs when the command returns, use the tool's own background mode.`,
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
    flags: [
      ['--test <file>', 'a test file, as a bare path or glob is (from the current directory)'],
      ['--storage <name>', 'the base of each test\'s own store name (default test)'],
      ...DRIVE.filter(([row]) => !specs(row).some((spec) => ['--storage', ...TEST_IGNORES.map(name)].includes(name(spec)))),
    ],
    internal: [...TEST_IGNORES, ...DRIVE_INTERNAL],
    body: `web builds the app first (web-build); a native host drives the build you made and refuses a
stale one, naming the rebuild. Each test launches the app fresh with its own store, after
its first data has landed; its leading lines (size, epoch, fail fetch …) override flags.
Ports    none to choose. web: headless Chrome over a pipe; the page on 127.0.0.1, each test at a
         port fixed by its own store's name (20000-47999).
         ${NATIVE_PORTS}
Runs     to completion, exit 1 when a test fails; nothing is left running.
Lines    tap, type, expect text|tree|state, clock data|settle|+<ms>, reload, fail|pass fetch:
         docs/start-here.md "Tests"; all: docs/contract-for-agents.md "Inspection and testing".`,
    example: 'bun exact.mjs test ios tests/*.test.contract --size 390x844',
  },
  agent: {
    line: 'drive the app as a person would: tree, tap, type, state, screenshot, clock … on a host',
    usage: 'bun exact.mjs agent [web|ios|macos|linux|android] [flags] <op> [<op> …]',
    flags: DRIVE,
    internal: ['--test <file>', ...DRIVE_INTERNAL],
    body: `Launches the app (web builds it first), runs each op, one quoted argument each, on the
agent's clock, prints each reply and closes it. Without --storage, storage is refused.
Ops      a target is a testId, else a view's exact text or label
  tree [<target>] · tree --ax · state [<path>] · logs · layout [<target>]
  tap <target> [contextmenu|dblclick|hover|drag <dx> <dy>|…]
  type <target> <text> · type <target> key <Name>   (a select, date or checkbox: its value)
  clock data · clock settle · clock +<ms> [real]    land what is in flight · end transitions
  screenshot <file.png> [window]                    window: with the platform's bars
  prefer <feature> <value> · perf [<target>] · resize <w>x<h> · close · fail|pass fetch <prefix>
  trace <file>      a saved development trace; every form: docs/contract-for-agents.md
                    "Inspection and testing", or agent web with no op
Ports    none to choose. web: headless Chrome over a pipe; the page on 127.0.0.1 at a free port, or
         with --storage one fixed by app and store (20000-47999; "in use": another drive has it).
         ${NATIVE_PORTS}
Runs     to completion; each drive launches the app anew (--storage keeps its data).`,
    example: 'bun exact.mjs agent web --storage s1 "clock data" tree "tap add" "screenshot .exact/a.png"',
  },
  ios: {
    line: 'build for an iOS simulator and install it (--run launches it)',
    usage: 'bun exact.mjs ios [--run] [--sim <udid|name>] [--device [--phone <udid|name>]] [--url <url>]',
    flags: [
      ['--run', 'also launch it (in Simulator.app, or on the phone); returns once launched'],
      ['--sim <udid|name>', 'which simulator (else EXACT_SIM, a booted iPhone, or the newest iPhone Pro)'],
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
refused, not guessed (set EXACT_SIM). test ios and agent ios drive what it installed.
Ports    none. Runs to completion (--run too). Start a long build in the background with &
         and keep editing; its PID ($!) stops it.`,
    example: 'bun exact.mjs ios --run',
  },
  mac: {
    line: 'build the macOS app (--run launches it here)',
    usage: 'bun exact.mjs mac [--run] [--bundle] [--url <url>] [--optimize speed|size]',
    flags: [
      ['--run', 'launch it when built, in the foreground (its log here) until it quits'],
      ['--bundle', 'also assemble the .app bundle'],
      ['--url <url>', 'with --run or --bundle: connect it to that dev server (web\'s URL) for live reload'],
      OPTIMIZE,
    ],
    get internal() { return appleRest(this.flags); },
    body: `test macos and agent macos drive what it built, and refuse a stale build.
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
    flags: [],
    body: `  build <file.contract> [-o <file.plan>] [--json] [--map]   --json: [] or each diagnostic with its range
  types <file.contract> [-o <app.d.ts>]   the types app.ts imports
  vocab [--json] [<name>]                 the tags, attributes and CSS properties Contract takes
  fmt [--check | --stdout] <file.contract> · fmt --uses <file.contract>
  symbols <file.contract> [--name <name>] · sources <file.contract>
  test <file.test.contract>               the compiled tests, as JSON (bun exact.mjs test runs them)
  verify <file.contract> [--types] [--prove <Module>] · rust <file.contract> [-o <shapes.rs>]
  lean <file.contract> [--components] [--name <ident>] [-o <file.lean>]
The first call compiles the compiler (minutes on a cold checkout).
Ports    none. Runs to completion: exit 0 when it compiled.`,
    example: 'bun exact.mjs contract build app.contract --json',
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

/** A verb's whole help text. */
export function helpText(verb) {
  const entry = HELP[verb];
  if (!entry) return null;
  const width = Math.min(22, Math.max(0, ...entry.flags.map(([row]) => row.length)));
  const flags = entry.flags.map(([row, what]) => `  ${row.padEnd(width)}  ${what}`).join('\n');
  return [entry.usage, `  ${entry.line}`, '', ...(flags ? [flags, ''] : []), entry.body, `Example  ${entry.example}`].join('\n');
}

/** The index: each verb of an app's exact.mjs, then the app's own (app.json `commands`). */
export function indexText({ verbs = Object.keys(HELP).filter((v) => v !== 'new'), own = {} } = {}) {
  const names = [...verbs, ...Object.keys(own)], width = Math.max(...names.map((v) => v.length));
  const line = (verb) => HELP[verb]?.line ?? (own[verb] ? `app.json commands: ${own[verb].join(' ')}` : '');
  return [
    'bun exact.mjs <verb> [arguments]    from the app\'s directory (EXACT2=<path> picks another exact2)',
    '',
    ...names.map((verb) => `  ${verb.padEnd(width)}  ${line(verb)}`),
    '',
    'bun exact.mjs <verb> --help: its flags, the ports it opens, whether it stays in the foreground,',
    'and an example. Read that, not exact2\'s scripts.',
  ].join('\n');
}

/** What the generated exact.mjs prints for `help [verb]`, no verb, or a verb's `--help`; the exit code. */
export function printHelp(verb, { verbs, own = {} } = {}) {
  if (!verb) return console.log(indexText({ verbs, own })), 0;
  if (own[verb] && !HELP[verb]) return console.log(`${verb}: this app's own command (app.json commands): ${own[verb].join(' ')}\nIts arguments follow as given; run in the app's directory.`), 0;
  const text = (!verbs || verbs.includes(verb)) && helpText(verb);
  if (text) return console.log(text), 0;
  console.error(`no verb ${verb}\n\n${indexText({ verbs, own })}`);
  return 2;
}

/** Every flag a verb's script takes, documented or not, by name: whether it takes a value. */
export const knownFlags = (verb) => new Map([...HELP[verb].flags.flatMap(([row]) => specs(row)), ...(HELP[verb].internal ?? [])].map((spec) => [name(spec), takesValue(spec)]));

/** A dispatched script's first call. `--help`/`-h` prints `verb`'s help and exits 0; a flag the
 * verb's script does not take is refused with the ones it does, exit 2. With `parsed`, `argv` is
 * what the script's own parser left, so every flag still in it is unknown (agent.mjs). */
export function guardFlags(verb, argv, { parsed = false, say = console.log, fail = console.error, exit = process.exit } = {}) {
  if (argv.includes('--help') || argv.includes('-h')) { say(helpText(verb)); return exit(0); }
  const entry = HELP[verb], known = knownFlags(verb);
  const unknown = [];
  for (let i = 0; i < argv.length; i++) {
    if (!/^-[^\d.]/.test(argv[i])) continue; // a positional, or a negative number
    if (!parsed && known.has(argv[i])) { if (known.get(argv[i])) i++; continue; }
    unknown.push(argv[i]);
  }
  if (!unknown.length) return argv;
  const takes = entry.flags.flatMap(([row]) => specs(row).map(name)).join(', ');
  fail(`${verb}: unknown flag ${unknown.join(', ')}. ${takes ? `It takes ${takes} (--help says what each takes and does).` : 'It takes no flags.'}`);
  return exit(2);
}

if (import.meta.main) process.exit(printHelp(process.argv[2] ?? ''));
