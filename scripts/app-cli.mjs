// An app's commands (LLP 1086; LLP 1116.006 D7): what `bun exact.mjs <verb>` runs in an app that
// `exact new` made. Its exact.mjs is a few lines that find this checkout (EXACT2) and call `run`, so
// a builder reading it learns where the verbs are (`bun exact.mjs --help`), not this code. An
// exact.mjs written before this file held all of it, and still runs; `bun exact.mjs update` thins it.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { constants } from 'node:os';
import { basename, isAbsolute, relative, resolve } from 'node:path';

/** exact2's verbs for the app `name` in `dir`: each the script it runs, from the checkout, and its
 * first arguments. `host` is a drive's (`test ios`); a game has its own and no `verify` or `pattern`. */
export function appVerbs(name, dir, { host = 'web', game = false } = {}) {
  return {
    web: ['host/web/dev.mjs', '--app', name],
    'web-build': ['host/web/build.mjs', name],
    test: ['scripts/agent.mjs', host, '--app', name],
    agent: ['scripts/agent.mjs', host, '--app', name],
    // Builds itself, so not below (LLP 1116.004 D7): one report of the build, the tests and check.
    ...game ? {} : { verify: ['scripts/verify.mjs', host, '--app', name] },
    ios: ['host/apple/build.mjs', '--ios', `${name}-apple`],
    mac: ['host/apple/build.mjs', `${name}-apple`],
    linux: ['scripts/build-linux.mjs', name],
    android: ['scripts/agent-android.mjs', 'build', name],
    update: ['scripts/exact.mjs', 'new', dir, '--update'],
    contract: ['scripts/exact.mjs', 'contract'],
    tamber: ['scripts/exact.mjs', 'tamber'],
    hatch: ['scripts/exact.mjs', 'hatch'],
    ...game ? {} : { pattern: ['scripts/exact.mjs', 'pattern'] },
    feedback: ['scripts/feedback.mjs'],
    // A game's own: its hostless Rust tests and its proof's baseline (exact2's game/README.md).
    ...game ? {
      'test-rust': ['game/app/shells.mjs', dir, '--test'],
      lock: ['game/app/shells.mjs', dir, '--lock'],
      windows: ['host/windows/build.mjs', name],
      prove: ['game/prove.mjs', dir],
    } : {},
  };
}

/** `bun exact.mjs <verb> [arguments]` for the app `name` in `dir`, with the checkout `exact2`; the exit code. */
export async function run({ dir, name, game = false, exact2, args }) {
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
  if (!verbs[verb] && !own[verb]) {
    console.error(`Usage: bun exact.mjs <${[...Object.keys(verbs), ...Object.keys(own)].join('|')}> [arguments for that script]\nbun exact.mjs help says what each does.`);
    return 2;
  }
  // The automatic build reports on stderr, so a drive's stdout stays its reply (`--json`).
  // The child ends with this process: a signal here is passed on, and a dev server
  // stops when this process is gone however it ended (`EXACT_LAUNCHER_PID`).
  const spawned = (command, args, stdio = 'inherit', cwd = undefined) => new Promise((done) => {
    const child = spawn(command, args, {
      stdio,
      cwd,
      env: { ...process.env, HERMES_LEAN_SYS_OFFLINE: '1', EXACT_APP_DIR: dir, EXACT2: exact2, EXACT_LAUNCHER_PID: String(process.pid) },
    });
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
  const drivesWeb = (verb === 'test' || verb === 'agent') && host === 'web' && !rest.some((a) => a === '--url' || a === '--web-dist');
  const drivesNative = (verb === 'test' || verb === 'agent') && ['ios', 'macos', 'mac'].includes(host) && !rest.includes('--device');
  // `test [host] [file|glob …]`: the test files named (each `.contract`, a glob or `--test <file>`, from the
  // current directory), else app.test.contract; each runs in turn and any failure fails the command.
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
    const sim = rest.includes('--sim') ? ['--sim', rest[rest.indexOf('--sim') + 1]] : [];
    const built = await script(drivesWeb ? verbs['web-build'] : ['scripts/agent-build.mjs', host, '--app', name, ...sim], [], [0, 2, 2]);
    if (built) return log(built);
  }
  if (own[verb]) { const [command, ...args] = own[verb]; return log(await spawned(command === 'bun' ? process.execPath : command, [...args, ...rest], 'inherit', dir)); }
  if (verb !== 'test') return log(await script(verbs[verb], rest));
  // An app with a server of its own (the rest pattern's) has it run for the tests, where app.ts calls it, on an empty
  // database kept through them: as verify runs it, by the same code (LLP 1116.007 D2).
  const { withAppServer } = await import(resolve(exact2, 'scripts/verify.mjs'));
  return log(await withAppServer(dir, async () => {
    let failed = 0;
    for (const file of tests.length ? tests : [resolve(dir, 'app.test.contract')]) {
      if (tests.length > 1) console.log(`# ${relative(process.cwd(), file)}`);
      failed = (await script(verbs.test, ['--test', file, ...rest])) || failed;
    }
    return failed;
  }, { verb: 'test', optional: true }));
}
