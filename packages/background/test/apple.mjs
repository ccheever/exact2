// @exact/background's conformance suite on Apple (LLP 1133.017 §14 stage 2):
// the fixture app (`test/fixture`, as `web.mjs` makes it) built for macOS or
// an iOS simulator and driven by the agent with the implementation bound
// (`--bind background`), so `NWPathMonitor`, `NSBackgroundActivityScheduler`
// and `BGTaskScheduler` answer and not the substitute.
//
// - The status: available, `granted` while background refresh is available,
//   `periodic` and `sync` true.
// - iOS's Info.plist carries `fetch`, `processing` and the three task
//   identifiers, from the grant lines.
// - A tag registered while the path is satisfied fires at once as a wake of
//   the implementation's own; its handler's request lands, and the tag is
//   retired.
// - Refused, the handler registers the tag again: listed through its
//   backoff, not fired again, and kept across a relaunch (`UserDefaults`).
// - `registerPeriodicSync` registers, and the host's store of wake requests
//   (ExactKit's `WakeStore`, in the app's defaults) holds a refresh request
//   for it 15 minutes on; on iOS, while a `sync` tag waits out its backoff,
//   a refresh request and a processing one for it too. The simulator
//   refuses every submit (`BGTaskSchedulerErrorDomain` code 1).
// - With `--long`, on the Mac: the periodic tag's scheduler fires within
//   30 minutes, its wake raising `periodicsync`, whose handler's request
//   lands.
//
// What needs the system (the network going and coming back, a task launched
// by iOS, the headless boot) is a person's (checklist.md).
//
// usage: bun packages/background/test/apple.mjs [macos] [ios --sim <udid>] [--long]
import { cpSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { checker } from '../../camera/test/fixture.mjs';
import { texts, until } from '../../photos/test/fixture.mjs';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const argv = process.argv.slice(2), sim = argv.includes('--sim') ? argv[argv.indexOf('--sim') + 1] : process.env.EXACT_SIM;
const hosts = argv.filter((a, i) => ['macos', 'ios'].includes(a) && argv[i - 1] !== '--sim'), long = argv.includes('--long');
const run = (cmd, args, cwd, ok = false) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 26 });
  if (r.status !== 0 && !ok) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};
const { check, failed } = checker();

// The handler's request: answered, or refused while `refuse` is set, except the periodic tag's.
let refuse = false, pulls = 0;
const puller = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
  pulls++;
  const no = refuse && new URL(req.url).searchParams.get('tag') !== 'p';
  return new Response(no ? 'no' : 'ok', { status: no ? 503 : 200 });
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

// Two system tasks can cover a sync already raised by the network callback.
// Check their lifetime directly; neither simulator nor OS scheduling is deterministic.
function overlappingWakes() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-background-wakes-'));
  try {
    const source = join(dir, 'Check.swift'), seam = join(dir, 'ExactPackage.o'), binary = join(dir, 'check');
    writeFileSync(source, String.raw`import ExactPackage
import Foundation

@main struct Check {
    static func main() {
        let defaults = UserDefaults.standard
        let previous = defaults.data(forKey: "exact.background")
        defaults.removeObject(forKey: "exact.background")
        defer { defaults.set(previous, forKey: "exact.background") }
        func wait(_ ready: () -> Bool) {
            let limit = Date().addingTimeInterval(5)
            while !ready() && Date() < limit { _ = RunLoop.main.run(mode: .default, before: limit) }
            precondition(ready(), "callback did not arrive")
        }
        let context = ExactPackageContext(agent: true, session: "test", changed: { _ in }, log: { print($0) }, now: { 0 })
        let module = BackgroundModule(context: context)
        var registered = false
        module.later("registerSync", ["tag": "outbox"], preferred: nil) { _ in registered = true }
        wait { registered }
        var ended: ((String) -> Void)?
        var raised = 0, done: [Bool] = []
        func wake(_ kind: String) -> ExactWake {
            ExactWake(kind: kind, keys: ["sync"], deadline: nil, platform: nil, raise: { _, _, callback in
                raised += 1
                ended = callback
            }, done: { done.append($0) })
        }
        module.woke(wake("processing"))
        wait { raised == 1 }
        module.woke(wake("refresh"))
        var drained = false
        DispatchQueue.main.async { drained = true }
        wait { drained }
        precondition(done.isEmpty, "a platform task finished while its sync was still running")
        precondition(raised == 1, "the same sync was raised twice")
        ended?("completed")
        wait { done.count == 2 }
        precondition(done.allSatisfy { $0 }, "completed sync failed a platform task")
        var tags: [[String: Any]]?
        module.later("syncTags", [:], preferred: nil) { tags = $0["exact:answer"] as? [[String: Any]] }
        wait { tags != nil }
        precondition(tags?.isEmpty == true, "completed sync was not retired")
        print("PASS overlapping platform wakes await one sync and retire it")
    }
}
`);
    run('xcrun', ['swiftc', '-parse-as-library', '-module-name', 'ExactPackage', '-emit-module',
      '-emit-module-path', join(dir, 'ExactPackage.swiftmodule'), '-c', resolve(root, 'host/apple/modules/ExactPackage.swift'), '-o', seam], root);
    run('xcrun', ['swiftc', '-parse-as-library', '-I', dir, resolve(pkg, 'apple/Background.swift'), source, seam, '-o', binary], root);
    console.log(run(binary, [], dir).trim());
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

overlappingWakes();

for (const host of hosts.length ? hosts : ['macos']) {
  const where = host === 'ios' ? 'ios-simulator' : 'macos';
  if (host === 'ios' && !sim) throw new Error('ios: name a simulator you booted (--sim <udid>)');
  const fixture = makeFixture();
  Object.assign(process.env, { EXACT_APP_DIR: fixture.app, EXACT2: root, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
  if (sim) process.env.EXACT_SIM = sim;
  // Registrations and wake requests live in the app's defaults, which no agent store holds: deleted before and after.
  // On the simulator, the app's defaults are a file in its data container.
  const container = () => {
    const dir = run('xcrun', ['simctl', 'get_app_container', sim, fixture.id, 'data'], root, true).trim();
    return dir && join(dir, 'Library/Preferences', `${fixture.id}.plist`);
  };
  const forget = () => host === 'ios' ? container() && rmSync(container(), { force: true }) : run('defaults', ['delete', fixture.id], root, true);
  // The host's wake requests, by `package key`.
  const stored = () => {
    let file = join(fixture.app, 'target', 'wakes.plist');
    if (host === 'ios') file = container() || file;
    else run('defaults', ['export', fixture.id, file], root, true);
    // `plutil -extract` reads a dot as a key path's separator.
    const raw = run('plutil', ['-convert', 'xml1', '-o', '-', file], root, true).match(/<key>exact\.wakes<\/key>\s*<data>([^<]*)<\/data>/)?.[1];
    return raw ? JSON.parse(Buffer.from(raw.replace(/\s/g, ''), 'base64').toString('utf8')) : {};
  };
  try {
    run('bun', [resolve(root, 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios', '--sim', sim] : []), 'bgfixture-apple'], root);
    forget();
    if (host === 'ios') {
      const target = join(fixture.app, 'target'), at = readdirSync(target, { recursive: true }).find((f) => /ios-sim/.test(f) && f.endsWith('.app/Info.plist'));
      const plist = at ? JSON.parse(run('plutil', ['-convert', 'json', '-o', '-', join(target, at)], root)) : {};
      const tasks = ['processing.network', 'processing.network-power', 'refresh'].map((t) => `${fixture.id}.${t}`);
      check(where, 'Info.plist carries fetch, processing and the three identifiers', JSON.stringify(plist.UIBackgroundModes) === '["fetch","processing"]'
        && JSON.stringify(plist.BGTaskSchedulerPermittedIdentifiers) === JSON.stringify(tasks), JSON.stringify([plist.UIBackgroundModes, plist.BGTaskSchedulerPermittedIdentifiers]));
    }
    const { open } = await import(resolve(root, 'scripts/agent.mjs'));
    const session = () => open({ host, app: 'bgfixture', bind: 'background', storage: `bgfixture-${process.pid}` });
    let s = await session();
    let periodicDeadline;
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
      const asked = Date.now();
      const periodic = await until(s, 'periodic', (t) => t !== 'none');
      check(where, 'registerPeriodicSync registers', periodic === 'registered', periodic);
      // The simulator's defaults reach the container's file a moment after they are set.
      let requests = {};
      for (const end = Date.now() + 15_000; Date.now() < end && !requests['background periodicsync p']; await Bun.sleep(500)) requests = stored();
      const p = requests['background periodicsync p'];
      periodicDeadline = p?.at;
      check(where, 'the host keeps a refresh request for the periodic tag, 15 minutes on', p?.kind === 'refresh' && Math.abs(p.at - asked - 900000) < 60000,
        JSON.stringify(requests));
      if (host === 'ios') {
        const sync = requests['background sync'], processing = requests['background sync processing'];
        check(where, 'a tag in its backoff asks a refresh and a processing wake with a network connection', sync?.kind === 'refresh'
          && processing?.kind === 'processing' && processing.network === true && processing.power === false && Math.abs(processing.at - sync.at) < 1000, JSON.stringify(requests));
      }
      if (long && host === 'macos') {
        const ticked = await until(s, 'pulled', (t) => t === 'p true false', 30 * 60_000);
        check(where, 'the periodic tag\'s scheduler fires, its handler\'s request lands', ticked === 'p true false', `${ticked} after ${Math.round((Date.now() - asked) / 1000)} s`);
        console.log(`note ${where} the scheduler fired ${Math.round((Date.now() - asked) / 1000)} s after the registration`);
      }
      const journal = (await s.logs({ all: true })).lines;
      if (failed()) console.log(journal.filter((l) => /background|wake/.test(l)).join('\n'));
    } finally {
      await s.close();
    }
    const before = pulls;
    s = await session();
    try {
      const kept = await until(s, 'tags', (t) => t === 't2');
      await s.clock('+1000 real');
      check(where, 'a relaunch keeps the tag and its backoff', kept === 't2' && pulls === before, `${kept} pulls=${pulls - before}`);
      const deadline = stored()['background periodicsync p']?.at;
      check(where, 'a relaunch preserves the periodic deadline', Math.abs(deadline - periodicDeadline) < 250,
        JSON.stringify({ before: periodicDeadline, after: deadline }));
    } finally {
      await s.close();
    }
  } finally {
    forget();
    if (host === 'ios') run('xcrun', ['simctl', 'uninstall', sim, fixture.id], root, true);
    else rmSync(join(homedir(), 'Library/Preferences', `${fixture.id}.plist`), { force: true });
    fixture.remove();
  }
}
puller.stop();
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
