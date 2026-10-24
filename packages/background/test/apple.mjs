// @exact/background's conformance suite on Apple (LLP 1133.017 §14 stage 2):
// the fixture app (`test/fixture`, as `web.mjs` makes it) built for macOS or
// an iOS simulator and driven by the agent with the implementation bound
// (`--bind background`), so `NWPathMonitor`, `NSBackgroundActivityScheduler`
// and `BGTaskScheduler` answer and not the substitute.
//
// - The status: available, `granted` while background refresh is available,
//   `periodic` and `sync` true.
// - iOS's Info.plist carries `fetch` and the refresh identifier, from the
//   grant lines.
// - A tag registered while the path is satisfied fires at once as a wake of
//   the implementation's own; its handler's request lands, and the tag is
//   retired.
// - Refused, the handler registers the tag again: listed through its
//   backoff, not fired again, and kept across a relaunch (`UserDefaults`).
// - `registerPeriodicSync` registers (a scheduler on the Mac; on iOS a
//   refresh request, whose submit the simulator may refuse: the journal's
//   line is printed).
//
// What needs the system (the network going and coming back, a refresh task
// launched by iOS) is a person's (checklist.md).
//
// usage: bun packages/background/test/apple.mjs [macos] [ios --sim <udid>]
import { cpSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim');
const run = (cmd, args, cwd, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 26 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();

// The handler's request: answered, or refused while `refuse` is set.
let refuse = false, pulls = 0;
const puller = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch() {
  pulls++;
  return new Response(refuse ? 'no' : 'ok', { status: refuse ? 503 : 200 });
} });

function makeFixture() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-background-')), app = join(dir, 'bgfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id).replace('__PULL_ORIGIN__', `http://127.0.0.1:${puller.port}`));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/background': `file:${pkg}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id, remove: () => rmSync(dir, { recursive: true, force: true }) };
}

for (const host of hosts.length ? hosts : ['macos']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  if (host === 'ios' && !sim) throw new Error('ios: name a simulator you booted (--sim <udid>)');
  const fixture = makeFixture();
  Object.assign(process.env, { EXACT_APP_DIR: fixture.app, EXACT2: root, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
  if (sim) process.env.EXACT_SIM = sim;
  // Registrations live in the app's defaults, which no agent store holds: emptied before and after.
  const forget = () => run(...(host === 'ios' ? ['xcrun', ['simctl', 'spawn', sim, 'defaults', 'delete', fixture.id, 'exact.background']] : ['defaults', ['delete', fixture.id, 'exact.background']]), root, true);
  try {
    run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', '--sim', sim] : []), 'bgfixture-apple'], root);
    forget();
    if (host === 'ios') {
      const target = join(fixture.app, 'target'), at = readdirSync(target, { recursive: true }).find((f) => /ios-sim/.test(f) && f.endsWith('.app/Info.plist'));
      const plist = at ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', join(target, at)], root)) : {};
      check(where, 'Info.plist carries fetch and the refresh identifier', JSON.stringify(plist.UIBackgroundModes) === '["fetch"]'
        && JSON.stringify(plist.BGTaskSchedulerPermittedIdentifiers) === JSON.stringify([`${fixture.id}.refresh`]), JSON.stringify([plist.UIBackgroundModes, plist.BGTaskSchedulerPermittedIdentifiers]));
    }
    const { open } = await import(resolve(root, 'scripts/agent.mjs'));
    const session = () => open({ host, app: 'bgfixture', bind: 'background', storage: `bgfixture-${process.pid}` });
    let s = await session();
    try {
      const status = await until(s, 'status', (t) => /^true /.test(t ?? ''));
      check(where, 'the status reads true granted true true', status === 'true granted true true', status);

      refuse = false;
      pulls = 0;
      await s.tap('queue-t1');
      const pulled = await until(s, 'pulled', (t) => t === 't1 true false');
      check(where, 'registered online, the wake runs the handler, whose request lands', pulled === 't1 true false' && pulls === 1, `${pulled} pulls=${pulls}`);
      const retired = await until(s, 'tags', (t) => t === '');
      check(where, 'its invocation completed, the tag is retired', retired === '', retired);
      const wake = (await s.op({ op: 'state' })).wake;
      check(where, 'the wake was the implementation\'s, completed', wake?.outcome === 'completed' && wake?.invocations?.[0]?.tag === 't1', JSON.stringify(wake));

      refuse = true;
      await s.tap('queue-t2');
      const failedPull = await until(s, 'pulled', (t) => t === 't2 false false');
      await s.clock('+1000 real');
      check(where, 'refused, its handler registers it again', failedPull === 't2 false false' && pulls === 2, `${failedPull} pulls=${pulls}`);
      check(where, 'registered again, it stays listed through its backoff, not fired again', (await texts(s)).tags === 't2' && pulls === 2, `${(await texts(s)).tags} pulls=${pulls}`);

      await s.tap('try-periodic');
      const periodic = await until(s, 'periodic', (t) => t !== 'none');
      check(where, 'registerPeriodicSync registers', periodic === 'registered', periodic);
      const journal = (await s.logs({ all: true })).lines;
      const submit = journal.find((l) => l.includes('refresh not submitted'));
      if (host === 'ios') console.log(`note ${where} the refresh request: ${submit ?? 'submitted'}`);
      if (failed()) console.log(journal.filter((l) => /background|wake/.test(l)).join('\n'));
    } finally {
      await s.close();
    }
    s = await session();
    try {
      const kept = await until(s, 'tags', (t) => t === 't2');
      await s.clock('+1000 real');
      check(where, 'a relaunch keeps the tag and its backoff', kept === 't2' && pulls === 2, `${kept} pulls=${pulls}`);
    } finally {
      await s.close();
    }
  } finally {
    forget();
    if (host === 'ios') run('xcrun', ['simctl', 'uninstall', sim, fixture.id], root, true);
    fixture.remove();
  }
}
puller.stop();
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
