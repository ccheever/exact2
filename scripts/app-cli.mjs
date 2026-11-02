// An app's commands (LLP 1086; LLP 1116.006 D7): what `bun exact.mjs <verb>` runs in an app that
// `exact new` made. Its exact.mjs is a few lines that find this checkout (EXACT2) and call `run`, so
// a builder reading it learns where the verbs are (`bun exact.mjs --help`), not this code. An
// exact.mjs written before this file held all of it, and still runs; `bun exact.mjs update` thins it.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { constants, tmpdir } from 'node:os';
import { basename, dirname, isAbsolute, relative, resolve } from 'node:path';
import { noteBunLoaded } from '../js/bake/src/__exact_env.mjs';

/** The compiler's warnings in what a build said (LLP 1116.004 D7), each once as `contract build` prints
 * it, `file:line:col warning [id] text`: the web compiler's or a bake's own line, Cargo's for a build
 * script's (`warning: <crate>@<version>: …`) or a linked bake's (`warning: <app>: …`, linked-plan.mjs); its
 * file from `cwd` when it is below it. */
export function compilerWarnings(text, cwd = process.cwd()) {
  const lines = new Set();
  for (const line of String(text ?? '').split('\n')) {
    const m = /^(?:warning: [^\s:]+: )?(\S(?:.*?\S)?):(\d+:\d+ warning \[[\w-]+\] .*)$/.exec(line);
    if (!m) continue;
    const near = relative(cwd, m[1]);
    lines.add(`${isAbsolute(m[1]) && !near.startsWith('..') ? near : m[1]}:${m[2]}`);
  }
  return [...lines];
}

/** A drive's lines for the compiler's `warnings` (`compilerWarnings`) of the app in `dir` built for `host`: the
 * warnings no build of it said before, each in full, then one line counting them all and naming the file that lists
 * them, `.exact/warnings-<host>.txt`, which this replaces. A drive repeated over unchanged warnings says the count
 * alone; one with none says nothing. */
export function warningLines(dir, host, warnings) {
  const file = resolve(dir, '.exact', `warnings-${host}.txt`), shown = relative(process.cwd(), file);
  let before = null; try { before = new Set(readFileSync(file, 'utf8').split('\n').filter(Boolean)); } catch { /* none said yet */ }
  // Replaced whole, so a drive beside this one reads either list, never half of one.
  try { mkdirSync(dirname(file), { recursive: true }); writeFileSync(`${file}.${process.pid}.tmp`, warnings.map((w) => `${w}\n`).join('')); renameSync(`${file}.${process.pid}.tmp`, file); } catch { /* said in full below */ }
  if (!warnings.length) return [];
  const fresh = before ? warnings.filter((w) => !before.has(w)) : warnings;
  const where = shown.startsWith('..') ? file : shown;
  const noted = warnings.filter(information).length, counted = warnings.length - noted;
  const count = `compiler: ${[(counted || !noted) && `${counted} warning${counted === 1 ? '' : 's'}`, noted && `${noted} note${noted === 1 ? '' : 's'} (not counted)`].filter(Boolean).join(' and ')}`;
  if (!fresh.length) return [`${count}, as before (${where} lists them; bun exact.mjs contract build quotes each line)`];
  // In the compiler's order (`contract::warning_rank`), the symbols only the web draws blank one line: 21 of them hid
  // the euxy port's header warning.
  const web = fresh.filter(webOnly), one = web.length === 1;
  const group = web.length ? [`${web[0].split(' warning [')[0]} warning [lower-symbol-platform] ${web.length} SF Symbol use${one ? '' : 's'} draw${one ? 's' : ''} blank on the web, here first: an icon token or a role draws on every host (${where} lists each, and how)`] : [];
  return [...fresh.filter((w) => !webOnly(w)), ...group, `${count}${fresh.length < warnings.length ? `, ${fresh.length} new (above)` : ''}; ${where} lists them all`];
}

/** A compiler warning that is information, not a problem: `tamber-unreachable` names a control whose host draws the
 *  platform's own look, which the author should know of and cannot change (LLP 1121 D9, amended 2026-10-30). A drive
 *  and verify say it but do not count it. contract's `warning_rank::information` is the same test. */
export const information = (w) => / warning \[tamber-unreachable\] /.test(w);

/** A compiler warning only the web's picture shows: an SF Symbol drawn blank where the web is the one target besides
 *  Apple's (contract's `web_only`). Its first use names the targets, `ships web (…), where it is blank`; a later one,
 *  `blank on web too`. */
export const webOnly = (w) => /warning \[lower-symbol-platform\] /.test(w) && (/ blank on web too/.test(w) || /ships web \((?:(?!\) and ).)*?, where it is blank/.test(w));

/** The window a Windows app opens in, `[width, height]`: app.json's `host.windows.window`, else the host's default
 * (exact_windows::Window). A drive of an app on Windows is that size, as a person first sees it. */
export const windowsWindow = (manifest) => [manifest?.host?.windows?.window?.width ?? 1280, manifest?.host?.windows?.window?.height ?? 720];

/** exact2's verbs for the app `name` in `dir`: each the script it runs, from the checkout, and its
 * first arguments. `host` is a drive's (`test ios`); a game has its own and no `verify` or `pattern`. */
export function appVerbs(name, dir, { host = 'web', game = false } = {}) {
  return {
    status: ['scripts/exact.mjs', 'status', dir],
    web: ['host/web/dev.mjs', '--app', name],
    'web-build': ['host/web/build.mjs', name],
    serve: ['host/web/serve.mjs'],
    test: ['scripts/agent.mjs', host, '--app', name],
    agent: ['scripts/agent.mjs', host, '--app', name],
    // Builds itself, so not below (LLP 1116.004 D7): one report of the build, the tests and check.
    ...game ? {} : { verify: ['scripts/verify.mjs', host, '--app', name] },
    ios: ['host/apple/build.mjs', '--ios', `${name}-apple`],
    mac: ['host/apple/build.mjs', `${name}-apple`],
    linux: ['scripts/build-linux.mjs', name],
    windows: ['host/windows/build.mjs', name],
    // The APK on the Canvas reader, installed (LLP 1107 D1); `--headless` the Linux host for Android instead.
    android: ['scripts/agent-android.mjs', 'build', name, '--apk', '--install'],
    update: ['scripts/exact.mjs', 'new', dir, '--update'],
    lock: ['scripts/app-cli.mjs', '--lock', dir],
    contract: ['scripts/exact.mjs', 'contract'],
    tamber: ['scripts/exact.mjs', 'tamber'],
    hatch: ['scripts/exact.mjs', 'hatch'],
    snapback: ['scripts/exact.mjs', 'snapback', dir],
    backend: ['scripts/exact.mjs', 'backend', dir],
    ...game ? {} : { pattern: ['scripts/exact.mjs', 'pattern'] },
    docs: ['scripts/exact.mjs', 'docs'],
    feedback: ['scripts/feedback.mjs'],
    // A game's own: its hostless Rust tests and its proof's baseline (exact2's game/README.md).
    ...game ? {
      check: ['game/app/shells.mjs', dir, '--check'],
      game: ['scripts/exact.mjs', 'game'],
      'test-rust': ['game/app/shells.mjs', dir, '--test'],
      shells: ['game/app/shells.mjs', dir],
      lock: ['game/app/shells.mjs', dir, '--lock'],
      prove: ['game/prove.mjs', dir],
    } : {},
  };
}

/** `bun exact.mjs <verb> [arguments]` for the app `name` in `dir`, with the checkout `exact2`; the exit code. */
export async function run({ dir, name, game = false, exact2, args }) {
  // Before any process starts: which settings Bun loaded from this directory's `.env` files (LLP 1153 D4).
  noteBunLoaded();
  // The design tool's tests, drives and verify run on this checkout's own port (port-lock.mjs `checkoutPort`, unless
  // EXACT_TOOL_PORT names one), from a copy staged with that origin and its stub (tamber-tool-port.mjs
  // `stageToolTests`), so its stub never binds the 7740 a person's tool holds; EXACT_TOOL_PORT=7740 runs them in place.
  if (['test', 'agent', 'verify'].includes(args[0]) && !args.includes('--help') && !args.includes('-h') && existsSync(resolve(exact2, 'apps/design-tool')) && realpathSync(dir) === realpathSync(resolve(exact2, 'apps/design-tool'))) {
    process.env.EXACT_TOOL_PORT ??= String((await import(resolve(exact2, 'scripts/port-lock.mjs'))).checkoutPort(resolve(exact2)));
    const tool = await import(resolve(exact2, 'scripts/tamber-tool-port.mjs')), rest = args.slice(1);
    const tested = tool.testFiles(args[0], rest, (await import(resolve(exact2, 'scripts/help.mjs'))).knownFlags(args[0]));
    const staged = tool.stageToolTests(dir, rest, i => tested.has(i));
    if (staged) { dir = staged.dir; args = [args[0], ...staged.args]; }
  }
  const [verb, ...rest] = args;
  const host = (verb === 'test' || verb === 'agent' || verb === 'verify') && rest[0] && !rest[0].startsWith('-') ? rest.shift() : 'web';
  const verbs = appVerbs(name, dir, { host, game });
  // The app's own verbs (paint, minesweeper, ledger: a regenerated file dropped the ones added to it): app.json's
  // `commands`, each a command as argv (`"seed": ["bun", "seed.mjs"]`), run in the app's directory with the
  // arguments after the verb. A game's app.json is optional.
  const manifest = resolve(dir, 'app.json');
  const own = (existsSync(manifest) ? JSON.parse(readFileSync(manifest, 'utf8')).commands : null) ?? {};
  const clash = Object.keys(own).find((name) => verbs[name]);
  if (clash) {
    console.error(`app.json commands.${clash}: ${clash} is one of exact.mjs's own verbs; give the app's another name`);
    return 2;
  }
  // Help (LLP 1116 D4): `--help` or `-h` among a verb's arguments, `help [<verb>]` or no verb prints exact2's own
  // text (its scripts/help.mjs) and starts nothing. An app's own verb gets its arguments as given. `all` anywhere
  // among them asks for a verb's whole text (`web --help all`, `help web all`, `help all web`); with no verb it is
  // the index (`--help all`, `help all`), never a verb (LLP 1116.010 D2).
  const known = (a) => verbs[a] || own[a];
  const asked = !verb || ['help', '--help', '-h'].includes(verb) ? rest.find(known) ?? rest.find((a) => !a.startsWith('-') && a !== 'all') ?? '' : verbs[verb] && (rest.includes('--help') || rest.includes('-h')) ? verb : null;
  if (asked !== null) {
    const help = await import(resolve(exact2, 'scripts/help.mjs')).catch(() => null);
    if (!help) { console.error(`no exact2 at ${exact2}: EXACT2 names the checkout`); return 1; }
    return help.printHelp(asked, { verbs: Object.keys(verbs), own, all: rest.includes('all') });
  }
  if (verb === 'status' || verb === 'web' && rest.includes('--ready')) {
    const {readStatus, formatStatus, waitReady} = await import(resolve(exact2, 'game/status.mjs'));
    if (verb === 'status') { console.log(formatStatus(readStatus(dir))); return 0; }
    try { const row = await waitReady(dir, {timeout:Number(rest.includes('--timeout') ? rest[rest.indexOf('--timeout') + 1] : 600)}); console.log(`READY ${row.url}`); return 0; }
    catch (e) { console.error(e.message); return 1; }
  }
  if (!verbs[verb] && !own[verb]) {
    console.error(`Usage: bun exact.mjs <${[...Object.keys(verbs), ...Object.keys(own)].join('|')}> [arguments for that script]\nbun exact.mjs help says what each does.`);
    return 2;
  }
  // A native verb names its platform in app.json the first time, after that platform's setup rows, and keeps the
  // generated workspace current unless the app authored one (LLP 1153 D1.2; scripts/app-shells.mjs). A drive's
  // host counts (`test ios`). Absent from a checkout that lacks the module (the scaffold's test SDK), it is skipped.
  const platform = game || rest.includes('--help') || rest.includes('-h') ? null : (await import(resolve(exact2, 'scripts/app-shells.mjs')).catch(() => null))?.platformOf(verb, host);
  if (platform) {
    const shells = await import(resolve(exact2, 'scripts/app-shells.mjs'));
    const named = shells.namesPlatform(existsSync(manifest) ? JSON.parse(readFileSync(manifest, 'utf8')) : {}, platform);
    const report = named ? null : await import(resolve(exact2, 'scripts/exact.mjs')).then(({ sdkReport, platformReport }) => platformReport(platform, sdkReport(process.env, { only: platform, device: rest.includes('--device') }), { device: rest.includes('--device') })).catch((error) => ({ error: String(error?.message ?? error) }));
    const brought = await shells.bringPlatform(dir, platform, { report });
    if (brought.refused) { console.error(brought.refused); return 1; }
    for (const line of brought.lines) console.error(line);
  }
  // The automatic build reports on stderr, so a drive's stdout stays its reply (`--json`).
  // The child ends with this process: a signal here is passed on, and a dev server
  // stops when this process is gone however it ended (`EXACT_LAUNCHER_PID`).
  const spawned = (command, args, stdio = 'inherit', cwd = undefined, piped = () => {}) => new Promise((done) => {
    const child = spawn(command, args, {
      stdio,
      cwd,
      env: { ...process.env, HERMES_LEAN_SYS_OFFLINE: '1', EXACT_APP_DIR: dir, EXACT2: exact2, EXACT_LAUNCHER_PID: String(process.pid) },
    });
    piped(child);
    const pass = (signal) => child.kill(signal), signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
    for (const signal of signals) process.on(signal, pass);
    child.on('error', () => done(1));
    child.on('exit', (code, signal) => {
      for (const s of signals) process.off(s, pass);
      done(code ?? 128 + (constants.signals[signal] ?? 0));
    });
  });
  const script = ([file, ...args], more = [], stdio = 'inherit') => spawned(process.execPath, [resolve(exact2, file), ...args, ...more], stdio);
  const started = Date.now();
  // The diary's command log (exact2's docs/diary.md): which exact2, what ran, how it ended. No arguments.
  const log = (exit) => { if (verb !== 'feedback') try {
    const commit = spawnSync('git', ['-C', exact2, 'rev-parse', '--short=9', 'HEAD'], { encoding: 'utf8' }).stdout?.trim() || null;
    mkdirSync(resolve(dir, '.exact'), { recursive: true });
    appendFileSync(resolve(dir, '.exact/commands.jsonl'), JSON.stringify({ at: new Date(started).toISOString(), exact2: commit, verb: ['test', 'agent', 'verify'].includes(verb) ? `${verb} ${host}` : verb, exit, ms: Date.now() - started }) + '\n');
  } catch {} return exit; };
// exact.mjs runs from any directory, and a file argument is found where you are, else in the app: `app.contract`
// from the app's parent, `app/app.contract` from inside it (app farm wave 1: 13 builds ran one or the other). Each
// file found in the app is said, with the app's directory; one found nowhere is named before the command runs.
const here = process.cwd(), appDir = dir, source = (arg) => /\.(contract|ts|json)$/.test(arg) && !arg.startsWith('-');
const inApp = (arg) => {
  if (!source(arg) || isAbsolute(arg) || existsSync(resolve(arg))) return null;
  const top = `${basename(appDir)}/`, found = [arg, ...(arg.startsWith(top) ? [arg.slice(top.length)] : [])].map((a) => resolve(appDir, a)).find((p) => existsSync(p));
  // `build --json` says nothing on stderr: its diagnostics carry the path taken, or the file it could not read.
  if (rest[0] === 'build' && rest.includes('--json')) return found ?? null;
  if (found) console.error(`exact.mjs: ${arg} is ${found} (the app is ${appDir}; this shell is in ${here})`);
  else console.error(`exact.mjs: no ${arg} in ${here} or in the app, ${appDir}`);
  return found ?? null;
};
if (verb === 'contract') {
  let moved = false;
  for (const [i, arg] of rest.entries()) {
    if (rest[i - 1] === '-o') continue;
    const found = inApp(arg);
    if (found) { rest[i] = found; moved = true; }
  }
  // What a moved source writes goes beside it, as it would have from the app (`types app.contract -o app.contract.d.ts`).
  const o = rest.indexOf('-o'), out = rest[o + 1];
  if (moved && o >= 0 && out && !isAbsolute(out)) rest[o + 1] = resolve(appDir, out.startsWith(`${basename(appDir)}/`) ? out.slice(basename(appDir).length + 1) : out);
}
  // The web build is about a second when nothing changed, so a web drive builds
  // first rather than refusing a stale build. An ios or macos drive builds first
  // when its build is missing or older than its sources, saying so on one line
  // (scripts/agent-build.mjs, LLP 1116.002 D4); a phone's stays explicit.
  const backendFlag = ['test', 'agent', 'verify'].includes(verb) && host === 'web' ? rest.indexOf('--backend') : -1;
  if (backendFlag >= 0) {
    const value = rest[backendFlag + 1];
    if (!value || value.startsWith('--')) throw new Error('--backend <url>: name the backend origin');
    process.env.EXACT_BACKEND_ORIGIN = value;
    rest.splice(backendFlag, 2);
  }
  // `--url` with a page served elsewhere drives that page; `--url /path` opens this app's build at a path, so it builds too.
  const drivesWeb = (verb === 'test' || verb === 'agent') && host === 'web'
    && !rest.some((a, i) => (a === '--url' && !rest[i + 1]?.startsWith('/')) || a === '--web-dist');
  // Xcode's builds are a Mac's: on Windows, the verbs that need one say what Windows has instead.
  if (process.platform === 'win32' && (['ios', 'mac'].includes(verb) || ['test', 'agent', 'verify'].includes(verb) && ['ios', 'macos', 'mac'].includes(host))) {
    console.error(`${verb}${verb === 'ios' || verb === 'mac' ? '' : ` ${host}`} needs a Mac (Xcode); on Windows: bun exact.mjs windows --run opens the app, test windows and verify windows check it`);
    return log(2);
  }
  const drivesNative = (verb === 'test' || verb === 'agent') && ['ios', 'macos', 'mac', 'windows'].includes(host) && !rest.includes('--device');
  // `test [host] [file|glob …]`: the test files named (each `.contract`, a glob or `--test <file>`, from the
  // current directory), else app.test.contract and every other `*.test.contract` beside it (an app's tests split
  // by area under the line cap); each runs in turn and any failure fails the command.
  const tests = [];
  for (let i = verb === 'test' ? 0 : rest.length; i < rest.length;) {
    const named = rest[i] === '--test' ? rest.splice(i, 2)[1] : /\.contract$|\*/.test(rest[i]) ? rest.splice(i, 1)[0] : (i++, null);
    if (named == null) continue;
    const found = named.includes('*') ? [...new Bun.Glob(named).scanSync({ absolute: true })].sort() : [inApp(named) ?? resolve(named)];
    if (!found.length) { console.error(`no test file matches ${named}`); return log(2); }
    tests.push(...found);
  }
  if (drivesWeb || drivesNative) {
    // A flag the drive does not take is refused before the build, not after it (scripts/help.mjs).
    (await import(resolve(exact2, 'scripts/help.mjs')).catch(() => null))?.guardFlags(verb, rest);
    // A server entry that does not read DB is refused before the build too, not after it (verify.mjs `withAppServer`, LLP 1116.008 D1).
    const refusal = verb === 'test' ? (await import(resolve(exact2, 'scripts/verify.mjs'))).appServer(dir)?.refusal : null;
    if (refusal) { console.error(`test: ${refusal}`); return log(1); }
    // An `agent` drive's check hashes the build's inputs, and the driver need not again (LLP 1012.001.000 D1: once a drive).
    const checked = verb === 'agent' && drivesNative ? resolve(tmpdir(), `exact-build-check-${process.pid}`) : null;
    if (checked) process.env.EXACT_BUILD_CHECK_OUT = checked;
    // A web build says every compiler warning each time; the drive says the new ones and a count (`warningLines`).
    const built = drivesWeb ? await webBuildQuiet(spawned, [resolve(exact2, verbs['web-build'][0]), ...verbs['web-build'].slice(1)], dir)
      : await script(['scripts/agent-build.mjs', host, '--app', name], [], [0, 2, 2]);
    if (checked) { delete process.env.EXACT_BUILD_CHECK_OUT; try { process.env.EXACT_BUILD_CHECKED = readFileSync(checked, 'utf8'); rmSync(checked); } catch {} }
    if (built) return log(built);
  }
  if (verb === 'contract' && rest[0] === 'build' && existsSync(manifest) && existsSync(resolve(exact2, 'snapback4/web/device-build.mjs'))) {
    const app = { dir, manifest: JSON.parse(readFileSync(manifest, 'utf8')) };
    const { devicePreflight } = await import(resolve(exact2, 'snapback4/web/device-build.mjs'));
    await devicePreflight(app);
  }
  if (own[verb]) { const [command, ...args] = own[verb]; return log(await spawned(command === 'bun' ? process.execPath : command, [...args, ...rest], 'inherit', dir)); }
  // A drive on iOS shows UIKit's bars, as the app and verify do (`--chrome agent` paints the authored header and
  // tablist instead), and an Apple drive's screenshots are a pixel a point: a third of an iPhone's pixels each
  // way, about a third of the tokens an agent reads a picture for (`--scale 3`: the device's). A phone's drive too,
  // which drivesNative leaves out only because its build is explicit.
  const drivesApple = (verb === 'test' || verb === 'agent') && ['ios', 'macos', 'mac'].includes(host);
  if (drivesApple && !rest.includes('--chrome') && host === 'ios') rest.push('--chrome', 'platform');
  if (drivesApple && verb === 'agent' && !rest.includes('--scale')) rest.push('--scale', '1');
  if ((verb === 'test' || verb === 'agent') && host === 'windows' && !rest.includes('--size') && existsSync(manifest)) rest.push('--size', windowsWindow(JSON.parse(readFileSync(manifest, 'utf8'))).join('x'));
  // An Android drive is the APK's (`bun exact.mjs android` installs it), unless it asks for the headless Linux host.
  if ((verb === 'test' || verb === 'agent') && host === 'android' && !rest.includes('--headless') && !rest.includes('--reader')) rest.push('--reader');
  // A finished site is served to this machine unless --lan asks for every interface (LLP 1153 D3.3); the dev loop
  // and serve.mjs run alone keep LAN (LLP 1023 D8).
  if (verb === 'serve' && !rest.includes('--lan')) rest.push('--loopback');
  if (verb !== 'test') return log(await script(verbs[verb], rest));
  // An app with a server of its own (the rest pattern's) has it run for the tests, where app.ts calls it, on an empty
  // database kept through them: as verify runs it, by the same code (LLP 1116.007 D2).
  const { withAppServer } = await import(resolve(exact2, 'scripts/verify.mjs'));
  return log(await withAppServer(dir, async () => {
    let failed = 0;
    // None authored: app.test.contract, whose absence the driver names.
    const files = tests.length ? tests : (await import(resolve(exact2, 'scripts/app.mjs'))).authoredTests(dir);
    for (const file of files.length ? files : [resolve(dir, 'app.test.contract')]) {
      if (files.length > 1) console.log(`# ${relative(process.cwd(), file)}`);
      failed = (await script(verbs.test, ['--test', file, ...rest])) || failed;
    }
    return failed;
  }, { verb: 'test', optional: true }));
}

/** The app's web build, run as a drive's first step: what it says goes to stderr as it comes, except the compiler's
 * warnings, which are said after as `warningLines` gives them. Its exit code. */
async function webBuildQuiet(spawned, args, dir) {
  const warnings = [], streams = [];
  const code = await spawned(process.execPath, args, ['inherit', 'pipe', 'pipe'], undefined, (child) => {
    for (const stream of [child.stdout, child.stderr]) {
      let rest = '';
      const line = (text) => { const found = compilerWarnings(text); if (found.length) warnings.push(...found); else process.stderr.write(`${text}\n`); };
      stream.setEncoding('utf8');
      stream.on('data', (chunk) => { const lines = (rest + chunk).split('\n'); rest = lines.pop(); lines.forEach(line); });
      streams.push(new Promise((done) => stream.on('end', () => { if (rest) line(rest); done(); })));
    }
  });
  await Promise.all(streams);
  for (const said of warningLines(dir, 'web', [...new Set(warnings)])) console.error(said);
  return code;
}

/** Explicit app lock repair: preserve selected versions and resolve only manifest drift. */
export async function lockApp(dir) {
  const { cargoEnvironment } = await import('./app.mjs');
  const { authoredWorkspace, shellsDir } = await import('./cargo-check.mjs');
  const options = { cwd: dir, env: cargoEnvironment(), encoding: 'utf8', maxBuffer: 1 << 26 };
  // An app that authored no workspace has the generated one once a native verb wrote it (LLP 1153 D1.3), else no Rust.
  const workspace = authoredWorkspace(dir) ?? (existsSync(resolve(shellsDir(dir), 'Cargo.toml')) ? shellsDir(dir) : null);
  if (!workspace) return `${dir}: no Rust here yet; a native verb (bun exact.mjs ios, mac, linux, android, windows) brings it`;
  const lock = resolve(workspace, 'Cargo.lock');
  const before = existsSync(lock) ? readFileSync(lock, 'utf8') : '';
  let result = spawnSync('cargo', ['metadata', '--offline', '--format-version', '1'], { ...options, cwd: workspace });
  if (result.status !== 0 && /no matching package named|--offline was specified|attempting to make an HTTP request|in the offline mode|offline mode \(via `--offline`\)/.test(result.stderr ?? '')) {
    console.error(`${workspace}: fetching missing Cargo sources to resolve the app lock`);
    result = spawnSync('cargo', ['metadata', '--format-version', '1'], { ...options, cwd: workspace });
  }
  if (result.status !== 0) throw new Error(`could not resolve ${lock}: ${result.stderr || result.error?.message}`);
  const { lockChanges } = await import('../game/app/shells.mjs');
  const changes = lockChanges(before, readFileSync(lock, 'utf8'));
  return `${lock}: ${changes.length ? `resolved\n${changes.join('\n')}` : 'current; no changes'}`;
}
if (import.meta.main && process.argv[2] === '--lock') {
  try { console.log(await lockApp(process.argv[3])); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
