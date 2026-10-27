// Native desktop and phone carriers over the agent JSON-lines wire.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { appleRunBinary, appleArtifacts, assertAppleIdentity } from '../host/apple/build.mjs';
import { phone, phoneBridge, forwardedEnv, crashReports } from '../host/apple/devices.mjs';
import { resolveApp, executableName, linuxBinary, linuxBuild, bakeOutput, bakeTarget } from './app.mjs';
import { nativeBuild, planOverride, refuseBuild } from './agent-build.mjs';
import { refuseStale, proofProductChanges, unchecked, packagedBuildChanges, depInfoChanges } from './agent-launch.mjs';
import { androidBinary, androidDeploy } from './agent-android.mjs';
import { nativeKey, mouseContact } from './agent-keys.mjs';
import { jsonLines, hangup, waitAtMost, bootRefusal, REPLY_MS } from './agent-wire.mjs';
import { platformWheel } from './agent-wheel.mjs';
const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** One JSON-lines protocol over stdio on desktop hosts, or a phone's outbound socket. */
export async function openStdio({ host, plan, world, size, app, env: extra = {}, session, documents = [], device = false, phone: pick, onProcess }) {
  const a = resolveApp(app);
  const linux = host === 'linux', android = host === 'android'; // @ref LLP 1107 — Android: the Linux host on a phone, under adb
  const windows = host === 'windows', portable = linux || windows || android;
  const sample = host === 'host';
  const artifacts = portable ? null : appleArtifacts(a, { destination: device ? 'ios' : 'macos', host: sample });
  const deviceBundle = artifacts?.bundle;
  const bin = windows ? (process.env.EXACT_WINDOWS_BIN ?? resolve(a.dir, 'dist-windows', `${executableName(a)}.exe`))
    : android ? (process.env.EXACT_ANDROID_BIN ?? androidBinary(a)) : linux ? (process.env.EXACT_LINUX_BIN ?? linuxBinary(a)) : (process.env.EXACT_MAC_BIN ?? appleRunBinary(a, sample));
  // The macOS build by its own record (agent-build.mjs, which an app's exact.mjs runs first to build it, or bakes its plan-only edit).
  const macBuild = !portable && !device && !process.env.EXACT_MAC_BIN ? nativeBuild('macos', a, { sample, checked: process.env.EXACT_BUILD_CHECKED }) : null, macPlan = macBuild && !plan && !sample ? planOverride('macos', a, macBuild) : null;
  if (macPlan) plan = macPlan.plan; else if (macBuild) refuseBuild('macos', macBuild, a, { sample });
  if (!existsSync(device ? deviceBundle : bin)) throw new Error(device ? 'run bun host/apple/build.mjs --device first' : windows ? 'run bun host/windows/build.mjs first' : android ? `run bun scripts/agent-android.mjs build ${a.name}${process.env.EXACT_ANDROID_READER ? ' --apk' : ''} first` : linux ? `run ${linuxBuild(a).join(' ')} first` : `EXACT_MAC_BIN names no file: ${bin}`);
  if (!portable) assertAppleIdentity(a, device ? resolve(deviceBundle, artifacts.executable) : bin);
  if (portable && process.env.EXACT_PROOF_PRODUCTS) refuseStale(host, bin, proofProductChanges(process.env.EXACT_PROOF_PRODUCTS, bin, a), 'rerun the proof'); else if (windows && process.env.EXACT_WINDOWS_BIN) unchecked('windows', 'EXACT_WINDOWS_BIN');
  else if (windows) {
    const receipt = resolve(bakeOutput(a), `windows-${bakeTarget('windows')}.build.json`);
    refuseStale('windows', bin, packagedBuildChanges(receipt, dirname(bin), a), `bun host/windows/build.mjs ${a.crate('windows')}`);
  }
  else if ((linux && process.env.EXACT_LINUX_BIN) || (android && process.env.EXACT_ANDROID_BIN)) unchecked(host, android ? 'EXACT_ANDROID_BIN' : 'EXACT_LINUX_BIN');
  else if (linux || android) refuseStale(host, bin, depInfoChanges(bin), android ? `bun scripts/agent-android.mjs build ${a.name}` : linuxBuild(a).join(' '));
  else if (!device && process.env.EXACT_MAC_BIN) unchecked(host, 'EXACT_MAC_BIN');
  if (device && (plan || extra.EXACT_PLAN || extra.EXACT_ASSETS)) throw new Error('a phone cannot read host-local plan/assets paths; use --url or its embedded app');
  const ph = device ? phone(pick) : null;
  if (device) {
    const installed = spawnSync('xcrun', ['devicectl', 'device', 'install', 'app', '--device', ph.udid, deviceBundle], { encoding: 'utf8' });
    if (installed.status !== 0) throw new Error(`device install: ${installed.stderr || installed.stdout || installed.error?.message}`);
    if (world) {
      const copied = spawnSync('xcrun', ['devicectl', 'device', 'copy', 'to', '--quiet', '--device', ph.udid,
        '--domain-type', 'appDataContainer', '--domain-identifier', a.id, '--source', resolve(world), '--destination', 'tmp/exact-agent.world'], { encoding: 'utf8', timeout: 45000 });
      if (copied.status !== 0) throw new Error(`phone world copy: ${copied.stderr || copied.error || copied.stdout}`);
      extra = { ...extra, EXACT_WORLD: '~/tmp/exact-agent.world' };
    }
  }
  const env = { EXACT_ASSETS: windows ? dirname(bin) : linux || android ? a.dir : artifacts.capture, ...process.env, EXACT_AGENT: '1' };
  if (plan) env.EXACT_PLAN = plan;
  if (portable && size) env.EXACT_SIZE = `${size[0]}x${size[1]}`;
  // @ref LLP 1039 §5 — measure the requested Mac content viewport.
  if (!portable && size) { env.EXACT_WINDOW_WIDTH = String(size[0]); env.EXACT_WINDOW_HEIGHT = String(size[1]); }
  if (portable) {
    // The pinned font: DejaVu Sans from scripts/fixtures/fonts shapes and
    // paints the host's text on every machine, so a pixel fixture recorded
    // here matches on a builder (LLP 1015 §5). The environment still wins.
    env.EXACT_PAINTER ??= 'cpu';
    if (windows) env.EXACT_GPU_RENDER ??= '1';
    env.EXACT_FONTS ??= android ? '/system/fonts' : resolve(ROOT, 'scripts/fixtures/fonts/assets'); // a phone draws with its own faces
    if (!android) env.EXACT_FONT ??= 'DejaVu Sans';
  }
  Object.assign(env, extra);
  const bridge = device ? await phoneBridge() : null, droid = android ? androidDeploy(a, bin, { ...env, ...extra }) : null;
  const launched = Date.now(); let closing = false;
  const child = device
    ? spawn('xcrun', ['devicectl', 'device', 'process', 'launch', '--quiet', '--console', '--terminate-existing', '--device', ph.udid,
        '--environment-variables', JSON.stringify({ ...forwardedEnv(), ...(size ? { EXACT_WINDOW_WIDTH: env.EXACT_WINDOW_WIDTH, EXACT_WINDOW_HEIGHT: env.EXACT_WINDOW_HEIGHT } : {}), ...extra, EXACT_AGENT: '1', ...bridge.env }), a.id], { stdio: ['pipe', 'pipe', 'pipe'] })
    : droid ? droid.spawn(env.EXACT_LAUNCH_URL ? [env.EXACT_LAUNCH_URL] : []) : spawn(bin, portable && env.EXACT_LAUNCH_URL ? [env.EXACT_LAUNCH_URL] : documents, { env, stdio: ['pipe', 'pipe', 'pipe'], windowsHide:true });
  onProcess?.(child);
  const hostLines = [];
  child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  if (device) child.stdout.on('data', (d) => { for (const l of String(d).split('\n')) if (l) hostLines.push('app: ' + l); });
  let lines = device ? null : jsonLines(child.stdout, child.stdin, hostLines);
  const fail = (why) => { lines?.fail(why); bridge?.fail(new Error(why)); };
  child.on('error', (e) => fail(`launch failed: ${e.message}`));
  const exited = new Promise((r) => child.on('exit', (code, signal) => { r(code ?? signal); fail(closing ? 'the app was closed' : hangup({ what: 'the app exited', exit: { code, signal }, hostLines, reports: device || droid ? [] : crashReports(basename(bin), launched) })); }));
  const close = async () => { closing = true; bridge?.close(); try { child.stdin.end(); if (device) child.kill('SIGTERM'); } catch {} await waitAtMost(exited, 2000); if (child.exitCode === null && child.signalCode === null) { try { child.kill('SIGKILL'); } catch {} } await exited; };
  let readyTimeout;
  try {
    const readyLine = device ? bridge.ready.then(({ socket, announcement }) => {
      lines = jsonLines(socket, socket, hostLines);
      socket.on('close', () => lines.fail('the phone agent connection closed; ' + hostLines.slice(-20).join('\n')));
      socket.resume();
      return announcement;
    }) : lines.next();
    const ready = await Promise.race([readyLine, new Promise((_, reject) => {
      readyTimeout = setTimeout(() => reject(new Error('the app never became ready; ' + hostLines.join('\n'))), 20000);
    })]);
    clearTimeout(readyTimeout);
    if (!ready.ready) throw new Error('unexpected first line: ' + JSON.stringify(ready));
    if (ready.error) throw new Error(bootRefusal(ready.error));
    // The sample host routes by label: the session the caller named, and
    // `s.session = "b"` moves every later request to another.
    const state = { session: session ?? null };
    const heldButton = mouseContact();
    const heldWheel = { open: null }; // a `wheel … hold` not yet released (LLP 1142 D3)
    const ask = (req, session = state.session) => waitAtMost(lines.ask(session ? { ...req, session } : req), req.op === 'perf' && req.live > 0 ? req.live + (device ? 45000 : REPLY_MS) : device ? 45000 : REPLY_MS, () => {
      const why = device ? `phone ${req.op} did not answer within 45 s; check app health, foreground state and network\n` + hostLines.slice(-20).join('\n')
        : hangup({ what: `${req.op} did not answer within ${REPLY_MS / 1000} s`, pid: child.pid, hostLines });
      if (device) bridge.close(); else lines.fail(why);
      throw new Error(why);
    });
    return {
      host, boot: ready.boot, hostLines, gpuMs: () => null, sessions: ready.sessions ?? null, state, plan,
      ask, reveal: (id) => ask({ op: 'reveal', id }),
      async input(id, kind, opts) {
        if (kind === 'key') {
          const session = state.session;
          return nativeKey({id, opts: portable ? {...opts, ownedRelease:false} : opts, ask: req => ask(req, session)});
        }
        const guest = { selector: opts.selector, x: opts.x, y: opts.y, entity: opts.entity, world: opts.world, under: opts.under, phase: opts.phase };
        const phase = ['down', 'move', 'hold', 'up', 'cancel'].includes(kind);
        // A contact that went down with `mouse` (a drag's) holds the left button through its phases (review A1).
        // A button's point and the modifiers held, as the request says them (#107).
        const at = opts.at !== undefined ? { at: opts.at } : {}, held = opts.modifiers ? { modifiers: opts.modifiers } : {};
        const r = phase ? await heldButton.ask(kind, opts, (button) => ask({ op: 'tap', phase: kind, ...(id != null ? { id } : {}), x: opts.x, y: opts.y, dx: opts.dx, dy: opts.dy, ms: opts.ms, ...(opts.virtual ? { virtual: true } : {}), ...held, ...button })) : ['contextmenu', 'dblclick', 'mouse', 'auxclick'].includes(kind) ? await ask({ op: 'tap', id, [kind]: true, ...at, ...held }) : kind === 'clicks' ? await ask({ op: 'tap', id, clicks: opts.clicks, ...at, ...held }) : kind === 'pinch' ? await ask({ op: 'tap', id, pinch: opts.pinch, at: opts.at }) : kind === 'wheel' && (opts.fling || opts.hold || opts.release) ? await platformWheel(ask, id, opts, heldWheel) : kind === 'wheel' ? await ask({ op: 'tap', id, wheel: opts.wheel, ...(opts.gesture ? { gesture: true } : {}), ...at, ...held }) : kind === 'drop' ? await ask({ op: 'tap', id, drop: opts.drop }) : kind === 'dragenter' || kind === 'dragleave' ? await ask({ op: 'tap', id, [kind]: opts[kind] }) : kind === 'hover' ? await ask({ op: 'tap', id, hover: true }) : kind === 'press' ? await ask({ op: 'tap', id, ...guest, ...at, ...(opts.modifiers ? { modifiers: opts.modifiers } : {}) }) : kind === 'mediasession' ? await ask({ op: 'tap', id, mediaSession: opts.mediaSession, ...(opts.seconds != null ? { seconds: opts.seconds } : {}) }) : await ask({ op: 'type', id, text: opts.text, ...(opts.clipboard ? { clipboard: opts.clipboard } : {}), ...(opts.files ? { files: opts.files } : {}), ...(opts.image ? { image: opts.image } : {}), ...guest });
        if (r.error) throw new Error(r.error);
        return r;
      },
      async screenshot(path, window = false, film = false) {
        const remote = device ? `${ready.container}/tmp/exact-agent.png` : droid ? droid.shot : path;
        const r = await ask({ op: 'screenshot', path: remote, window, ...(film ? { film } : {}) });
        if (r.error) throw new Error(r.error);
        if (droid) { droid.pull(path); r.screenshot = path; }
        if (device) {
          const copied = spawnSync('xcrun', ['devicectl', 'device', 'copy', 'from', '--quiet', '--device', ph.udid,
            '--domain-type', 'appDataContainer', '--domain-identifier', a.id, '--source', 'tmp/exact-agent.png', '--destination', resolve(path)], { encoding: 'utf8', timeout: 20000 });
          if (copied.status !== 0) throw new Error(`phone screenshot copy: ${copied.stderr || copied.error || copied.stdout}`);
          r.screenshot = path;
        }
        return r;
      },
      // A drive that ends with a held wheel lifts it, as a finger leaves the trackpad.
      async close() { if (heldWheel.open) await platformWheel(ask, heldWheel.open.tapped, { wheel: [0, 0], release: true }, heldWheel).catch(() => {}); await close(); },
    };
  } catch (e) {
    await close();
    throw e;
  } finally {
    clearTimeout(readyTimeout);
  }
}
