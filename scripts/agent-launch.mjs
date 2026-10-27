import {gameNonInput, proofInputFiles, executionInputFiles} from '../game/inputs.mjs';
export {gameNonInput} from '../game/inputs.mjs';
import {readStatus, formatStatus} from '../game/status.mjs';
// Session setup shared by the agent CLI and its programmatic driver.
import { widgetArguments, validateWidget } from './agent-widget.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { accessSync, constants, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { basename, delimiter, dirname, isAbsolute, relative, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { types as utilTypes } from 'node:util';
import { filesystemLock } from './filesystem.mjs';
import { chooseSimulator, phone } from '../host/apple/devices.mjs';
import { androidDevice } from './agent-android.mjs';
import { bakeOutput, gpuModules, linuxBinary, moduleDirectory, pendingBuildInputs, resolveApp, windowsFile, shaderWatchRoots, webDist } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** Poll host I/O until ready; a bound only ends a hung load. Return its last observation for diagnostics. */
export async function pollUntil(read, ready, { bound = 60_000, interval = 50, now = Date.now, pause = ms => new Promise(r => setTimeout(r, ms)) } = {}) {
  const until = now() + bound;
  while (true) {
    const value = await read();
    if (ready(value)) return value;
    const remaining = until - now();
    if (remaining <= 0) return value;
    await pause(Math.min(interval, remaining));
  }
}

// The compiler's warnings in a build's output, and the lines a drive says for them: app-cli.mjs, which runs a
// drive's build first, has them; verify, the dev loop and agent-build.mjs read them from here.
export { compilerWarnings, warningLines } from './app-cli.mjs';

/** Preserve the original operation failure and any owned cleanup handle. */
export function retainCleanupError(error, failure) {
  error.message += `; cleanup: ${failure.message}`;
  error.cleanupError = failure;
}

/** Browser-process diagnostics that do not describe the page or Exact. Page
 * exceptions and console errors arrive over CDP separately and remain logs. */
export function browserDiagnosticNoise(line) {
  return /crashpad|updater|gcm|VERBOSE|DevTools listening/i.test(line)
    // Linux without a session bus or GSettings schemas: Chrome's dbus client and GLib report it on every launch.
    || /:ERROR:dbus\/(bus|object_proxy)\.cc:\d+\] (Failed to connect to the bus|Failed to call method: org\.freedesktop\.DBus)/.test(line) || /GLib-GIO-CRITICAL \*\*: [\d:.]+: g_settings_schema_source_lookup: assertion 'source != NULL' failed$/.test(line)
    || /CVDisplayLinkCreateWithCGDisplay failed|CVReturn:\s*-6670/i.test(line)
    // macOS Chrome's allocator shim, at every launch with no page loaded (Chrome 154; app farm round 1: eight builds
    // read it as an Exact or wasm fault), and its on-device model service starting. Neither is the page's.
    || /^Trying to load the allocator multiple times\. This is \*not\* supported\.$|^Created TensorFlow Lite XNNPACK delegate for CPU\.$/.test(line)
    // The browser process checking the renderer's paint-timing report
    // against itself (two paints in one frame, image before first): its
    // bookkeeping, not the page's. The page's own errors come over CDP.
    || /\bpage_load_metrics_update_dispatcher\.cc:\d+\] Invalid first_\w+ [\d.]+ s for \w+ [\d.]+ s$/.test(line);
}
/** Only the caller's throwaway browser profile. Bun 1.4.2 on Windows ignores
 * rmSync's maxRetries: a real sharing lock fails in <1 ms. Yield between bounded
 * attempts so browser shutdown can finish; a persistent lock still fails. */
export async function removeBrowserProfile(profile) {
  for (let attempt = 0; ; attempt++) {
    try { rmSync(profile, {recursive:true, force:true}); return; }
    catch (error) {
      if (attempt === 5 || !['EBUSY','ENOTEMPTY','EPERM'].includes(error.code)) throw error;
      await new Promise(resolve => setTimeout(resolve, 100 * (attempt + 1)));
    }
  }
}

/** The helper never establishes browser exit; both recorded children must exit.
 * `terminate` is the owned helper-spawn boundary used by the refusal fixtures. */
export async function closeWindowsBrowser(child, cdp, exited, profile, terminate = pid =>
  spawn('taskkill', ['/PID', String(pid), '/T', '/F'], {windowsHide:true, stdio:['ignore','pipe','pipe']})) {
  const start = performance.now(), elapsed = () => Math.round(performance.now() - start);
  const detail = {}, wait = async promise => {
    let timer;
    try { return await Promise.race([promise.then(() => true), new Promise(resolve => { timer = setTimeout(() => resolve(false), 2000); })]); }
    finally { clearTimeout(timer); }
  };
  const bounded = value => value == null ? null : String(value).slice(-2048);
  const didExit = process => process.exitCode !== null || process.signalCode !== null;
  try { await cdp.send('Browser.close', {}, undefined, 2000); detail.cdp = {outcome:'reply', ms:elapsed()}; }
  catch (error) { detail.cdp = {error:bounded(error.message), ms:elapsed()}; }
  await wait(exited);
  detail.graceMs = elapsed();
  let helper, helperExit = Promise.resolve();
  if (!didExit(child)) {
    const began = performance.now();
    const info = detail.taskkill = {pid:null, status:null, signal:null, error:null, stdout:'', stderr:'', deadline:false};
    try {
      helper = terminate(child.pid);
      info.pid = helper.pid ?? null;
      for (const name of ['stdout','stderr']) {
        helper[name]?.setEncoding('utf8');
        helper[name]?.on('data', data => { info[name] = bounded(info[name] + data); });
        helper[name]?.on('error', error => { info[`${name}Error`] = bounded(error.message); });
      }
      helperExit = new Promise(resolve => {
        helper.once('exit', () => { info.ms = Math.round(performance.now() - began); resolve(); });
        helper.on('error', error => {
          info.error = bounded(error.message);
          // Failed spawn owns no process. An error on a launched child is not exit.
          if (!helper.pid) resolve();
        });
      });
      if (!await wait(helperExit)) {
        info.deadline = true;
        try { info.killSent = helper.kill('SIGKILL'); }
        catch (error) { info.killError = bounded(error.message); }
      }
    } catch (error) { info.error = bounded(error.message); }
    info.waitMs = Math.round(performance.now() - began);
  }
  // Reap the helper and observe Chrome concurrently within the existing final
  // wait. Neither helper success nor closed output pipes certify either exit.
  await wait(Promise.all([exited, helperExit]));
  const helperLive = helper?.pid && !didExit(helper);
  if (helper) {
    Object.assign(detail.taskkill, {status:helper.exitCode, signal:helper.signalCode});
    helper.stdout?.destroy(); helper.stderr?.destroy();
  }
  if (!didExit(child) || helperLive) {
    Object.assign(detail, {totalMs:elapsed(), exitCode:child.exitCode, signalCode:child.signalCode});
    const reason = !didExit(child) ? `Chrome ${child.pid} did not exit` : `Chrome termination helper ${helper.pid} did not exit`;
    const error = new Error(`${reason}; owned profile retained at ${profile}; shutdown ${JSON.stringify(detail)}`);
    if (helperLive) error.ownedHelper = helper;
    throw error;
  }
}

/** Where a web drive's named scratch store lives (`--storage <name>`), kept between drives as a native one is (dash,
 * weather, kanban: a second drive opened an empty store): Chrome's profile for it, beside the native stores' cache
 * (agent-test.mjs `storeBase`), and the port its page is served on — an origin's storage is its host and port's, so
 * one name is one port, from its hash, below the ephemeral range. A store is its app's: its id's and its directory's
 * (LLP 1116.004 D8: a bake-off's contacts read an earlier round's `contacts.json`, written under the same default
 * id and store name), so two apps of one id never open each other's. */
/** The store a drive with `env` uses: the driver's environment under the drive's own, as `runTests` places and removes
 * it and a native host sees it (review b5-c 3). `app` is resolveApp's: its `id` and `dir`. */
export const driveStore = (app, storage, env, platform = process.platform) => {
  const launched = { ...process.env, ...(env ?? {}) };
  return webStore(app, storage, launched, launched.HOME || homedir(), platform);
};
/** An app directory's twelve hex digits, which name its development stores: a drive's profiles here, and the
 * page's own storage in a development web build (host/web-js/build.mjs `DEVELOPMENT_STORE`). `dir` is canonical
 * (resolveApp's, through symlinks), so one app has one digest however it was reached. */
export const appDigest = dir => createHash('sha256').update(dir).digest('hex').slice(0, 12);
/** A host file as a development web build ships it for the app at `dir`: its `DEVELOPMENT_STORE` names the app's
 * own store (storage-environment.js, rt.js `Kept`). A release ships the file as written, null: the id alone. */
export const developmentStore = (code, dir) => code.replaceAll('const DEVELOPMENT_STORE = null;', `const DEVELOPMENT_STORE = ${JSON.stringify(appDigest(dir))};`);
export function webStore({ id, dir }, storage, env = process.env, home = homedir(), platform = process.platform) {
  if (!id || !dir) throw new Error('webStore: an app\'s id and directory name its store');
  const cache = platform === 'darwin' ? resolve(home, 'Library/Caches') : env.XDG_CACHE_HOME?.startsWith('/') ? env.XDG_CACHE_HOME : resolve(home, '.cache');
  const digest = appDigest(dir), base = resolve(cache, 'exact', id, `agent-web-${digest}`);
  return { base, profile: resolve(base, storage), port: 20000 + createHash('sha256').update(`${id}/${digest}/${storage}`).digest().readUInt32BE(0) % 28000 };
}

/** Chrome's flags for conformance's network (host/web-js/conformance/net.mjs), when
 *  `EXACT_NET_PROXY` names its proxy: every outside request goes through it, loopback
 *  (the page's own server) excepted, and its run's certificate is trusted by its SPKI
 *  digest (`EXACT_NET_SPKI`). */
export function netFlags(environment = process.env) {
  if (!environment.EXACT_NET_PROXY) return [];
  return [`--proxy-server=http://${environment.EXACT_NET_PROXY}`, `--ignore-certificate-errors-spki-list=${environment.EXACT_NET_SPKI}`,
    // Chrome's implicit bypass covers link-local addresses too: `<-loopback>` drops it, and loopback is named instead
    '--proxy-bypass-list=<-loopback>;127.0.0.1;localhost;[::1]',
    // Chrome's own services (sign-in, updates, safe browsing), which would otherwise ask the proxy for Google's hosts
    '--disable-background-networking', '--disable-sync', '--no-pings', '--disable-domain-reliability', '--disable-client-side-phishing-detection',
    '--disable-features=AutofillServerCommunication,OptimizationHints,MediaRouter,CertificateTransparencyComponentUpdater,NetworkTimeServiceQuerying'];
}

/** Playwright's launch and context options for the same network: Firefox and WebKit through the
 *  proxy, loopback direct, and the run's certificate accepted (Playwright has no SPKI pin). */
export function netOptions(environment = process.env) {
  if (!environment.EXACT_NET_PROXY) return { launch: {}, context: {} };
  return { launch: { proxy: { server: `http://${environment.EXACT_NET_PROXY}`, bypass: '127.0.0.1,localhost' } }, context: { ignoreHTTPSErrors: true } };
}

/** The device a native drive will open, resolved as its launch resolves it, and pinned so the launch opens that one: a
 * simulator's UDID (`EXACT_SIM`), a phone's (`flags.phone`), an Android device's serial (`ANDROID_SERIAL`), else
 * this machine. The store lock's key (`exclusiveNativeStore`). */
export function resolveDevice(host, flags, appId) {
  if ((host === 'ios' || host === 'host-ios') && flags.device) { const ph = phone(flags.phone); flags.phone = ph.udid; return `phone ${ph.udid}`; }
  if (host === 'ios' || host === 'host-ios') {
    const sim = chooseSimulator(undefined, { app: host === 'host-ios' ? `${appId}.host` : appId, dir: resolveApp(flags.app).dir, hint: '--sim <udid|name> (or EXACT_SIM)' });
    process.env.EXACT_SIM = sim.udid;
    return `simulator ${sim.udid}`;
  }
  if (host === 'android') { const serial = androidDevice(process.env); process.env.ANDROID_SERIAL = serial; return `android ${serial}`; }
  return 'this machine';
}
/** The store a drive that names none uses: on a native host, `default`, so the app's files are kept from one drive
 * to the next as the app keeps them from one launch to the next (`--reset-storage` empties it, `--storage <name>`
 * names another). On the web a named store is a Chrome profile emptied each drive, so a web drive names its own. */
export function defaultStore(host, flags) {
  return flags.storage ?? (['ios', 'macos', 'mac', 'linux'].includes(host) && !flags.url ? 'default' : undefined);
}
/** Says, at the first drive from the app's directory `dir` on `device` that keeps `defaultStore`, that it is kept.
 * A drive that compares starts empty and a plain one does not, so a plain drive can show what an earlier one stored
 * (the euxy port's library); and another checkout of the same app id (a second port on this simulator) shares the store. */
export function noteDefaultStore({ host, device, appId, dir }, root = resolve(homedir(), '.cache/exact/agent-store-locks')) {
  const part = (s) => String(s).replace(/[^\w.-]/g, '_');
  let source = String(dir);
  try { source = realpathSync(source); } catch { /* the path as given */ }
  const store = resolve(root, ...[host === 'mac' ? 'macos' : host, device, appId, 'default'].map(part));
  const mark = resolve(store, `.noted-${createHash('sha256').update(source).digest('hex').slice(0, 16)}`);
  if (existsSync(mark)) return;
  try { mkdirSync(store, { recursive: true }); writeFileSync(mark, ''); } catch { /* noted again next time */ }
  console.error("note: no --storage named, so this drive keeps the store `default`, what earlier drives of this app on this device stored (one that compares with a reference starts empty); --reset-storage empties it, --storage <name> names another");
}
/** One drive at a time of a native named store (`--storage`), from before the launch to the close: a reset empties
 * the store at the app's boot, which would delete what another drive of it is using. Keyed by the host, the device as
 * the drive names it (`device`: a simulator, a phone, an Android device, or this machine), the app's id and the store,
 * so a second drive is refused while the first is open. The OS releases the lock if the driver dies. */
export async function exclusiveNativeStore({ host, device, appId, storage }, launch, root = resolve(homedir(), '.cache/exact/agent-store-locks')) {
  const part = (s) => String(s).replace(/[^\w.-]/g, '_');
  mkdirSync(root, { recursive: true });
  let release, acquired;
  const ready = new Promise((done) => { acquired = done; });
  const lifetime = new Promise((done) => { release = done; });
  const holding = filesystemLock(root, `${[host, device, appId, storage].map(part).join('/')}/.lock`, async () => { acquired(); await lifetime; });
  try { await Promise.race([ready, holding]); }
  catch (error) { throw new Error(`--storage ${storage}: another drive of this store is open on ${device} (${error.message}); close it, or name another store`); }
  const unlock = async () => { release(); await holding; };
  try {
    const carrier = await launch(), close = carrier.close.bind(carrier);
    let closed;
    carrier.close = () => closed ??= (async () => { try { await close(); } finally { await unlock(); } })();
    return carrier;
  } catch (error) { await unlock(); throw error; }
}

/** Own a named Chrome profile before resetting it, including when two drives
 * choose different page ports. The OS releases the lock if the driver dies. */
export async function exclusiveWebStore(options, launch) {
  if (options.storage === undefined || options.browser !== 'chrome' || options.reuse) return launch();
  const app = resolveApp(options.app), store = driveStore(app, options.storage, options.env);
  mkdirSync(store.base, {recursive: true});
  let release, acquired;
  const ready = new Promise(resolve => { acquired = resolve; });
  const lifetime = new Promise(resolve => { release = resolve; });
  const holding = filesystemLock(store.base + '-locks', `${options.storage}/.lock`, async () => { acquired(); await lifetime; });
  try { await Promise.race([ready, holding]); }
  catch (error) { throw new Error(`--storage ${options.storage}: profile lock failed (${error.message}); close any drive of this store or use another name`); }
  const unlock = async () => { release(); await holding; };
  try {
    const carrier = await launch(), close = carrier.close.bind(carrier);
    let closed;
    carrier.close = () => closed ??= (async () => { await close(); await unlock(); })();
    return carrier;
  } catch (error) { await unlock(); throw error; }
}

/** The one port a web drive's page is served on, `--port <n>` else `EXACT_PORT`, or null for the default (a free port;
 * a named store's from its hash). Builders on a shared machine must keep every listener in an assigned range (app farm
 * wave 1: 21 builds could not). A test run serves its tests on it one at a time. A named store's data is its origin's,
 * so drive one store on one port. */
export function webPort(flag, env = process.env) {
  const given = flag ?? env.EXACT_PORT;
  if (given === undefined || given === '') return null;
  const port = Number(given);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error(`${flag === undefined ? 'EXACT_PORT' : '--port'} ${given}: a port number, 1–65535`);
  return port;
}

/** Temporary tests own a fresh page origin; a persistent named drive keeps its port. */
export function webPortRange(env = process.env) {
  if (env.PORT_LO === undefined && env.PORT_HI === undefined) return null;
  const lo = Number(env.PORT_LO), hi = Number(env.PORT_HI);
  if (!Number.isInteger(lo) || !Number.isInteger(hi) || lo < 1 || hi > 65535 || lo > hi)
    throw new Error(`PORT_LO/PORT_HI must name an inclusive port range, 1–65535 (got ${env.PORT_LO}/${env.PORT_HI})`);
  return [lo, hi];
}
export function drivePagePort(fixed, kept, temporary = false, env = process.env) {
  if (fixed != null) return fixed;
  const range = webPortRange(env), port = !temporary ? kept?.port : null;
  return port != null && (!range || (port >= range[0] && port <= range[1])) ? port : 0;
}

/** Listen on `port` (0: any free one). A named port still held is asked again for `ms`: the run's last page, or the
 * last run's, can still be letting go of it (app farm wave 1: "port in use" between runs). `named` (`--port`/EXACT_PORT)
 * says so in the refusal, and when the port is in Linux's ephemeral range, which any outgoing connection can borrow. */
export async function listenOn(server, port, host = '127.0.0.1', ms = port ? 2000 : 0, named = false, env = process.env) {
  const range = port === 0 ? webPortRange(env) : null;
  if (range) {
    for (let candidate = range[0]; candidate <= range[1]; candidate++) {
      try { return await listenOn(server, candidate, host, 0, false, {}); }
      catch (error) { if (error.code !== 'EADDRINUSE') throw error; }
    }
    throw Object.assign(new Error(`ports ${range[0]}–${range[1]} (PORT_LO/PORT_HI) are all in use`), {code: 'EADDRINUSE'});
  }
  for (const end = Date.now() + ms; ; await new Promise((r) => setTimeout(r, 100))) {
    try {
      return await new Promise((ok, fail) => {
        const listening = () => { server.off('error', failed); ok(server.address().port); }, failed = (e) => { server.off('listening', listening); fail(e); };
        server.once('error', failed).once('listening', listening).listen(port, host);
      });
    }
    catch (e) {
      if (e.code === 'EADDRINUSE' && Date.now() < end) { server.close(() => {}); continue; }
      if (e.code !== 'EADDRINUSE' || !named) throw e;
      let range = null; try { range = readFileSync('/proc/sys/net/ipv4/ip_local_port_range', 'utf8').trim().split(/\s+/).map(Number); } catch {}
      const borrowed = range && port >= range[0] && port <= range[1] ? `; it is in this machine's ephemeral range (${range[0]}–${range[1]}), which outgoing connections borrow for a while, so a port outside it is steadier` : '';
      throw Object.assign(new Error(`port ${port} (--port or EXACT_PORT) is in use: another drive or program holds it${borrowed}`), { code: e.code });
    }
  }
}

/** A launching Chrome's stderr, unfiltered: the last lines, and the first fatal one, which the native stack trace
 * after it pushes out of any tail (app farm 082: "Socket path too long" under a long TMPDIR read as a bare stack). */
export function launchLog(limit = 8) {
  const log = { tail: [], fatal: null, add(line) {
    if (!log.fatal && /:FATAL:|\bCheck failed\b/.test(line)) log.fatal = line;
    log.tail.push(line); if (log.tail.length > limit) log.tail.shift();
  } };
  return log;
}

/** Why Chrome did not start or answer, from its `launchLog` (the pipe only closes). */
export function launchFailure(error, log, { alive, chrome, env = process.env }) {
  const tail = log.tail.slice(-6), lines = log.fatal && !tail.includes(log.fatal) ? [log.fatal, '…', ...tail] : tail;
  const temp = env.TMPDIR || tmpdir(), socket = /Socket path too long/i.test(log.fatal ?? '')
    ? `\nChrome keeps a Unix socket under TMPDIR (${temp}, ${temp.length} characters), and a socket's path must fit in 107 bytes: give the drive a shorter TMPDIR.` : '';
  return new Error(`Chrome ${alive ? 'did not answer' : 'did not start'} (${error.message}); CHROME=${chrome}\n${lines.map(l => '  chrome: ' + l).join('\n') || '  (it printed nothing)'}${socket || (alive ? '' : '\nSet CHROME to a browser that runs here.')}`);
}

// Desktop input, matching Viewport::default() even on headless Linux without devices.
// Blink's ui::PointerType fine is 4; ui::HoverType hover is 2. CDP emulation can override them.
export const DESKTOP_INPUT = '--blink-settings=primaryPointerType=4,availablePointerTypes=4,primaryHoverType=2,availableHoverTypes=2';

/** The needs that take Playwright's full Chromium (Chrome for Testing, run with `--headless=new`) in place of its
 * headless shell: WebGPU and WebGL on the GPU (the shell has only SwiftShader, whose fallback adapter a GPU app takes
 * silently and then draws wrong), the PDF viewer, HEVC, the back/forward cache (the shell restores no page
 * from it), and timings that should match a person's browser. */
export const CHROMIUM_NEEDS = ['gpu', 'pdf', 'hevc', 'bfcache', 'perf'];
const SHELL = { 'darwin-arm64': 'chrome-headless-shell-mac-arm64', 'darwin-x64': 'chrome-headless-shell-mac-x64',
  'linux-x64': 'chrome-headless-shell-linux64', 'linux-arm64': 'chrome-headless-shell-linux-arm64', 'win32-x64': 'chrome-headless-shell-win64' };
/** Playwright's two Chromium builds, from its full build's path: the headless shell is the same revision, beside it in
 * the same browsers directory (Playwright has no public path for the shell). */
export function chromiumBuilds(full, platform = process.platform, arch = process.arch) {
  const at = /^(.*)([/\\])chromium-(\d+)\2/.exec(full), dir = SHELL[`${platform}-${arch}`];
  if (!at || !dir) return { full, shell: null };
  const [, browsers, separator, revision] = at, exe = platform === 'win32' ? '.exe' : '';
  return { full, shell: [browsers, `chromium_headless_shell-${revision}`, dir, `chrome-headless-shell${exe}`].join(separator) };
}
let pinned;
const playwrightFull = () => pinned ??= createRequire(import.meta.url)('playwright-core').chromium.executablePath();
const playwrightVersion = () => JSON.parse(readFileSync(resolve(ROOT, 'package.json'), 'utf8')).devDependencies['playwright-core'];
const executableAt = (path) => { try { accessSync(path, constants.X_OK); return true; } catch { return false; } };
/** What a web drive of `app` (resolveApp's) needs of Chromium: the GPU when the app has a GPU module. */
export const webNeeds = app => app?.hasGpu || (app?.manifest && gpuModules(app.manifest).length) ? ['gpu'] : [];
/** The Chromium a drive or test launches: CHROME when it is set (a bare name is resolved through PATH; one that names
 * no browser is `unavailable`), else the build Playwright pins (package.json): its headless shell, or its full build for a need in CHROMIUM_NEEDS. A browser
 * that is not there is no error: `unavailable` says why and how to install it, so a test can skip on it. `environment`,
 * `platform`, `arch` and `full` (the full build's path) are the machine's unless a test of this function names them. */
export function chromium({ needs = [], environment = process.env, platform = process.platform, arch = process.arch, full } = {}) {
  const unknown = needs.filter((need) => !CHROMIUM_NEEDS.includes(need));
  if (unknown.length) throw new Error(`chromium: unknown need ${unknown.join(', ')} (${CHROMIUM_NEEDS.join(', ')})`);
  if (environment.CHROME) {
    const name = environment.CHROME, candidates = /[/\\]/.test(name) ? [resolve(name)]
      : (environment.PATH ?? '').split(delimiter).filter(Boolean).map((dir) => resolve(dir, name));
    const executable = candidates.find(executableAt);
    return { executable: executable ?? name, unavailable: executable ? null : `CHROME names no browser: ${name}` };
  }
  const builds = chromiumBuilds(full ?? playwrightFull(), platform, arch), wantsFull = needs.length > 0;
  const executable = wantsFull ? builds.full : builds.shell;
  if (executable && executableAt(executable)) return { executable, unavailable: null };
  const [what, browser] = wantsFull ? ['Chromium', 'chromium'] : ['Chromium headless shell', 'chromium-headless-shell'];
  const where = executable ? `at ${executable}` : `for ${platform}-${arch}`;
  return { executable: executable ?? builds.full, unavailable: `${what} is missing ${where}; install it with bunx playwright@${playwrightVersion()} install ${browser}, or set CHROME to a browser` };
}

const sharedBrowsers = new Map();
/** A test suite's browser: the process's one Playwright browser of this engine (`chromium`, `firefox`, `webkit`), Chromium
 * build (`needs`, as `chromium` takes them) and launch options, shared by every suite that asks for the same, so only the
 * first suite to ask launches it. What a suite opens on it is its own: `newContext` and `newPage` each make a browser context, and `close`
 * closes those, never the browser, which Playwright closes when the process exits. */
export async function suiteBrowser(engine = 'chromium', { needs = [], timeout, ...launch } = {}) {
  const playwright = await import('playwright-core');
  const options = { headless: true, ...(engine === 'chromium' ? { executablePath: chromium({ needs }).executable } : {}), ...launch };
  // A launch timeout is not part of the key: it launches the same browser.
  const key = JSON.stringify([engine, options]);
  if (!sharedBrowsers.has(key)) {
    const launching = playwright[engine].launch({ ...options, timeout });
    sharedBrowsers.set(key, launching);
    const forget = () => { if (sharedBrowsers.get(key) === launching) sharedBrowsers.delete(key); };
    launching.then((browser) => browser.on('disconnected', forget), forget);
  }
  const browser = await sharedBrowsers.get(key), contexts = new Set();
  const own = (context) => { contexts.add(context); context.on('close', () => contexts.delete(context)); return context; };
  return {
    browser,
    newContext: async (options) => own(await browser.newContext(options)),
    newPage: async (options) => (own(await browser.newContext(options))).newPage(),
    close: async () => { await Promise.all([...contexts].map((context) => context.close().catch(() => {}))); },
  };
}

const sharedCdpBrowsers = new Map();
/** A test suite's Chromium over the DevTools pipe, for a test that speaks CDP itself (`Cdp`): the process's one browser
 * for this build (`needs`, as `chromium` takes them) and these flags (`args`, after the common ones), shared by every
 * suite that asks for the same, so only the first suite to ask launches it. The suite has a browser context of its
 * own: `page(url)` opens a target in it and attaches (flatten), and `close` disposes of the context, never the
 * browser, which ends with the process (its pipe closes) or when it exits (the next suite launches another).
 * `lines` holds the browser's stderr, past its diagnostic noise; a suite may empty it. */
export async function cdpBrowser({ needs = [], args = [] } = {}) {
  const { executable, unavailable } = chromium({ needs });
  if (unavailable) throw new Error(unavailable);
  const key = JSON.stringify([executable, args]);
  if (!sharedCdpBrowsers.has(key)) {
    const profile = mkdtempSync(resolve(tmpdir(), 'exact-cdp-'));
    const child = spawn(executable, ['--headless=new', '--remote-debugging-pipe', `--user-data-dir=${profile}`, '--no-sandbox',
      '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-background-networking', '--disable-component-update', ...args,
      'about:blank'],
    { stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] });
    const shared = { cdp: new Cdp(child.stdio[3], child.stdio[4]), lines: [] };
    child.stderr.on('data', (d) => { for (const l of String(d).split('\n')) if (l && !browserDiagnosticNoise(l)) shared.lines.push(l); });
    child.on('exit', () => {
      if (sharedCdpBrowsers.get(key) === shared) sharedCdpBrowsers.delete(key);
      shared.cdp.fail('Chrome exited');
      rmSync(profile, { recursive: true, force: true });
    });
    // Shared, it must not hold the process open: a pending call's deadline does that while one waits.
    for (const handle of [child, child.stderr, child.stdio[3], child.stdio[4]]) handle.unref?.();
    process.once('exit', () => child.kill('SIGKILL'));
    sharedCdpBrowsers.set(key, shared);
  }
  const { cdp, lines } = sharedCdpBrowsers.get(key);
  const { browserContextId } = await cdp.send('Target.createBrowserContext');
  return {
    cdp, lines, browserContextId,
    page: async (url = 'about:blank') => {
      const { targetId } = await cdp.send('Target.createTarget', { url, browserContextId });
      const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
      return { targetId, sessionId };
    },
    close: () => cdp.send('Target.disposeBrowserContext', { browserContextId }).catch(() => {}),
  };
}

export function parseFlags(argv) {
  const selection = widgetArguments(argv);
  argv = selection.argv;
  const flags = { json: false, ...(selection.widget ? { widget: selection.widget } : {}), ...(selection.activity ? { activity: selection.activity } : {}) };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--json') flags.json = true;
    else if (argv[i] === '--world') flags.world = resolve(argv[++i]);
    else if (argv[i] === '--plan') flags.plan = resolve(argv[++i]);
    else if (argv[i] === '--backend') { const value = argv[++i]; if (!value || value.startsWith('--')) throw new Error('--backend <url>: name the backend origin'); process.env.EXACT_BACKEND_ORIGIN = value; }
    else if (argv[i] === '--app') flags.app = argv[++i];
    else if (argv[i] === '--browser') flags.browser = argv[++i];
    else if (argv[i] === '--size') {
      const size = argv[++i] ?? '';
      if (!/^\d+(?:\.\d+)?x\d+(?:\.\d+)?$/.test(size) || !size.split('x').every(v => Number(v) > 0 && Number.isFinite(Number(v)))) throw new Error('--size: expected positive <w>x<h>; large|xxxl needs widget <entry>');
      flags.size = size.split('x').map(Number);
    }
    // A number, or what was typed, which `open` refuses by name (`--scale 1x`, a missing value).
    else if (argv[i] === '--scale') { const v = argv[++i] ?? ''; flags.scale = v.trim() !== '' && Number.isFinite(Number(v)) ? Number(v) : v; }
    else if (argv[i] === '--test') flags.test = argv[++i];
    else if (argv[i] === '--session') flags.session = argv[++i];
    else if (argv[i] === '--open') (flags.open ??= []).push(resolve(argv[++i]));
    else if (argv[i] === '--url') flags.url = argv[++i];
    else if (argv[i] === '--device') flags.device = true;
    // Android: the app on the Canvas reader (its APK), not the Linux host headless. Said as the variable
    // `agent-android.mjs` reads, as `--sim` is said as EXACT_SIM: the driver's own file is at its line cap.
    else if (argv[i] === '--reader') { flags.reader = true; process.env.EXACT_ANDROID_READER = '1'; }
    else if (argv[i] === '--seed') flags.seed = Number(argv[++i]);
    else if (argv[i] === '--locale') flags.locale = argv[++i];
    else if (argv[i] === '--format-locale') flags.formatLocale = argv[++i];
    else if (argv[i] === '--time-zone') flags.timeZone = argv[++i];
    else if (argv[i] === '--epoch') flags.epoch = argv[++i];
    else if (argv[i] === '--timing') flags.timing = argv[++i];
    else if (argv[i] === '--touch') flags.touch = argv[++i];
    else if (argv[i] === '--chrome') flags.chrome = argv[++i];
    else if (argv[i] === '--phone') flags.phone = argv[++i];
    else if (argv[i] === '--sim') flags.sim = argv[++i];
    else if (argv[i] === '--storage') flags.storage = argv[++i];
    else if (argv[i] === '--keep-storage') flags.keepStorage = true;
    else if (argv[i] === '--reset-storage') flags.resetStorage = true;
    else if (argv[i] === '--port') flags.port = argv[++i];
    // `trace --pid <pid>` asks a running development app for its trace; `trace --out <file>` writes it with its map joined (LLP 1142 D2, D5).
    else if (argv[i] === '--pid') flags.pid = argv[++i];
    else if (argv[i] === '--out') flags.out = resolve(argv[++i]);
    // A driver fault armed before the app's first data load (LLP 1103 D3): every fetch whose URL starts with it fails.
    else if (argv[i] === '--fail-fetch') flags.failFetch = [flags.failFetch, argv[++i]].filter(Boolean).join('\n');
    // A module's permission as the drive starts (LLP 1133.002 §6): `--permission geolocation=granted`, a `prefer` before the first op.
    else if (argv[i] === '--permission') { const [name, state] = String(argv[++i]).split('='); (flags.permission ??= {})[`permission.${name}`] = state; }
    // A relaunch whose platform hands it a listed entry (LLP 1133.004 §7): `--notification '<entry>'`, the
    // entry as `state.notifications` printed it, delivered as its tap would be before the first op.
    else if (argv[i] === '--notification') (flags.launchEntries ??= []).push({ launch: 'notification', entry: argv[++i] });
    // A cold wake (LLP 1133.017 §10): `--wake '<entry>'`, a registration as `state` printed it, raised before the first op.
    else if (argv[i] === '--wake') (flags.launchEntries ??= []).push({ launch: 'wake', entry: argv[++i] });
    // A cold tool call (LLP 1133.023 §10): `--call '<tool> <json>'`, delivered once the data module is ready, as a system's call that started the app.
    else if (argv[i] === '--call') (flags.launchEntries ??= []).push({ launch: 'call', entry: argv[++i] });
    // A launch the login item made (LLP 1133.025 D2, D6): `--login`, or `--login hidden` for one registered to open with
    // no window. The runner answers `launchedAtLogin`; the macOS host leaves the window unshown for a hidden one.
    else if (argv[i] === '--login') {
      flags.login = argv[i + 1] === 'hidden' ? (i++, 'hidden') : 'shown';
      (flags.launchEntries ??= []).push({ launch: 'login', entry: flags.login });
    }
    // A module answered by its implementation, not the substitute (LLP 1133.001 D7's behaviour tests; Apple, Android).
    else if (argv[i] === '--bind') flags.bind = [flags.bind, argv[++i]].filter(Boolean).join(',');
    else rest.push(argv[i]);
  }
  if ((flags.widget || flags.activity) && !argv.some(arg => arg === '--help' || arg === '-h')) validateWidget(flags, rest);
  if (flags.keepStorage && flags.resetStorage) throw new Error('--keep-storage and --reset-storage are mutually exclusive');
  if (['ios', 'host-ios'].includes(rest[0]) && flags.phone !== undefined && !flags.device) throw new Error('--phone needs --device; for a simulator use --sim <udid|name> (or EXACT_SIM)');
  // A native drive's store is `default` when it names none (`defaultStore`); an authored test's is its own.
  if ((flags.keepStorage || flags.resetStorage) && (flags.test ? flags.storage : defaultStore(rest[0], flags)) === undefined) throw new Error('--keep-storage and --reset-storage need --storage <name>');
  return { flags, rest };
}

/** LLP 1027.000.000 D3: the date at the agent clock's zero, unless the drive names one.
 * `now` names the machine's clock, read once at launch: a drive against a live backend
 * (Snapback 4's dev server runs on real time) dates what it shows and writes as the server does. */
export const AGENT_EPOCH = '2026-01-01T00:00:00Z';

/** A page script that holds what a comparison of two pages must hold equal, on every carrier:
 * `mediaClock: 'frozen'` plays media at rate 0 from its first load, so both pages read one
 * position, not the wall clock's (play, pause and seeks still happen; an authored rate still
 * applies; time does not advance, so ending and looping are not exercised); `lineHeight`
 * (cross-browser conformance's) gives the body a fixed line height in place of `normal`, whose
 * value each engine takes from its own font metrics (plain HTML, 16px system-ui: Chrome and
 * WebKit 18 px, Firefox 20 px). It is an adopted sheet, in place before any page script, after
 * the shell's own `body { font }` in the cascade; authored line heights still apply. '' for neither. */
export function parityScript({ mediaClock = 'wall', lineHeight = null } = {}) {
  if (!['wall', 'frozen'].includes(mediaClock)) throw new Error(`mediaClock: ${mediaClock} (wall or frozen)`);
  if (lineHeight != null && !/^\d+(\.\d+)?$/.test(String(lineHeight))) throw new Error(`lineHeight: ${lineHeight} is a unitless number`);
  // The freeze is invisible to the page: each element's real rates are 0 (so time stands still), while
  // `playbackRate` and `defaultPlaybackRate` hold the page's own values. Writes are validated as the real
  // setters validate them (a detached element takes the value first); a change fires `ratechange`; a load
  // (`load()`, `src`, `srcObject`, the `src` attribute) resets the page's rate to its default at once, as
  // every engine does (plain HTML: rate 2, then a new src: Chrome, Firefox and WebKit read 1 at once and
  // fire one ratechange, and a load drops the rate events still pending). Every trusted `ratechange` on a
  // frozen element (made by createElement or `new Audio`, or seen at loadstart) is the freeze's own and is
  // swallowed before any listener (a capture listener on the element from its creation, and on the window),
  // so no count depends on when the page attached its handlers. The page's
  // events are dispatched by the freeze (untrusted).
  const media = mediaClock === 'frozen' ? `{ const P = HTMLMediaElement.prototype, d = k => Object.getOwnPropertyDescriptor(P, k), real = { rate: d('playbackRate'), def: d('defaultPlaybackRate'), src: d('src'), srcObject: d('srcObject') }, load = P.load, setAttribute = Element.prototype.setAttribute, page = new WeakMap();
    let check = null; const swallow = e => { if (e.isTrusted) e.stopImmediatePropagation(); }; const valid = (k, n) => { check ??= document.createElement('audio'); real[k].set.call(check, n); return real[k].get.call(check); };
    const of = m => { let v = page.get(m); if (!v) { v = { rate: real.rate.get.call(m), def: real.def.get.call(m) }; page.set(m, v); m.addEventListener('ratechange', swallow, true); real.def.set.call(m, 0); real.rate.set.call(m, 0); } return v; };
    const make = Document.prototype.createElement; Document.prototype.createElement = function (...a) { const e = make.apply(this, a); if (e instanceof HTMLMediaElement) of(e); return e; };
    const Audio0 = globalThis.Audio; if (Audio0) { globalThis.Audio = function Audio(...a) { const e = new Audio0(...a); of(e); return e; }; globalThis.Audio.prototype = Audio0.prototype; }
    const pending = new WeakMap(), told = m => { const t = setTimeout(() => { pending.get(m)?.delete(t); m.dispatchEvent(new Event('ratechange')); }, 0); (pending.get(m) ?? pending.set(m, new Set()).get(m)).add(t); };
    const reset = m => { const v = of(m); for (const t of pending.get(m) ?? []) clearTimeout(t); pending.get(m)?.clear(); if (v.rate !== v.def) { v.rate = v.def; told(m); } };
    for (const k of ['rate', 'def']) Object.defineProperty(P, k === 'rate' ? 'playbackRate' : 'defaultPlaybackRate', { configurable: true, enumerable: true, get() { return of(this)[k]; }, set(n) { const x = valid(k, n), v = of(this); if (v[k] !== x) { v[k] = x; told(this); } } });
    for (const k of ['src', 'srcObject']) if (real[k]) Object.defineProperty(P, k, { configurable: true, enumerable: true, get() { return real[k].get.call(this); }, set(x) { real[k].set.call(this, x); reset(this); } });
    P.load = function () { const r = load.call(this); reset(this); return r; };
    Element.prototype.setAttribute = function (name, value) { const r = setAttribute.call(this, name, value); if (this instanceof HTMLMediaElement && String(name).toLowerCase() === 'src') reset(this); return r; };
    addEventListener('ratechange', e => { if (page.has(e.target)) swallow(e); }, true);
    addEventListener('loadstart', e => { if (e.target instanceof HTMLMediaElement) { of(e.target); real.def.set.call(e.target, 0); real.rate.set.call(e.target, 0); } }, true); }` : '';
  // The cross-browser parity sheet (`lineHeight` set) also pins a native field's (`data-native`) line
  // height, `normal` in each engine's own metric. Its padding and border, like every native button's,
  // are the page's own since LLP 1116 D1 (host/web/index.html), the same in every engine. Over index.html's
  // field rule, under a field's authored (inline) rows.
  const controls = `#exact-root input[data-native],#exact-root textarea[data-native]{line-height:${lineHeight}}`;
  const line = lineHeight != null ? `{ const s = new CSSStyleSheet(); s.replaceSync('body{line-height:${lineHeight}} ${controls}'); document.adoptedStyleSheets = [...document.adoptedStyleSheets, s]; }` : '';
  return media + line;
}

/** The fault table's launch lines (LLP 1103 D3; the runner's `Faults::parse`): `<prefix>` or `<prefix>\t<times>`, or a reload's whole entry. Refused here, before any process starts, as the host would. */
export function faultSpec(spec) {
  const lines = String(spec ?? '').split('\n').filter(l => l.trim());
  for (const line of lines) {
    const [prefix, ...counts] = line.split('\t');
    if (!prefix) throw new Error('fail fetch: each fault names a non-empty URL prefix');
    if (counts.length > 4 || counts.some((c, i) => !(c === '-' || /^\d+$/.test(c)) && !(i === 3 && /^[01]$/.test(c)))) throw new Error(`fail fetch: an unreadable fault line: ${JSON.stringify(line)}`);
    if (counts[0] === '0') throw new Error('fail fetch: `times` is a positive integer');
  }
  return lines.join('\n');
}

/** The page address with the fault table a reload carries (LLP 1103 D3); `undefined` leaves the launch's. */
export function withFaults(href, failFetch) {
  if (failFetch === undefined) return href;
  const url = new URL(href);
  if (failFetch) url.searchParams.set('failFetch', failFetch); else url.searchParams.delete('failFetch');
  return url.href;
}

/** The page address at another date at the clock's zero (a test's `reload`, LLP 1116.009 D1): `epoch` Unix ms, else the address as it is. */
export function withEpoch(href, epoch) {
  if (epoch === undefined) return href;
  const url = new URL(href);
  url.searchParams.set('epoch', String(epoch));
  return url.href;
}

/** Launch lines from `state.faults` (LLP 1103 D3): a reload relaunches with the table as it is now. */
export const faultSpecOf = faults => (faults ?? []).map(f => [f.prefix, f.times ?? '-', f.left ?? '-', f.hits, f.armed ? 1 : 0].join('\t')).join('\n');

export function launchFacts({seed, locale, formatLocale, timeZone, epoch, failFetch, env = {}}) {
  seed = Number(seed ?? env.EXACT_AGENT_SEED ?? 1);
  // An ISO date, Unix milliseconds or `now` (the machine's clock, read here, once); hosts are told milliseconds.
  epoch = String(epoch ?? env.EXACT_AGENT_EPOCH ?? AGENT_EPOCH);
  epoch = epoch === 'now' ? Date.now() : /^\d+$/.test(epoch) ? Number(epoch) : /^\d{4}-\d\d-\d\d(T|$)/.test(epoch) ? Date.parse(epoch) : NaN;
  if (!Number.isSafeInteger(epoch) || epoch < 0) throw new Error('epoch: an ISO date, Unix milliseconds at or after 1970, or now');
  locale = locale ?? env.EXACT_AGENT_LOCALE ?? 'en-US';
  timeZone = timeZone ?? env.EXACT_AGENT_TIME_ZONE ?? 'UTC';
  // Below 2^32, as every host draws it (LLP 1116.005 D5): refused, not wrapped.
  if (!Number.isSafeInteger(seed) || seed < 0 || seed >= 2 ** 32) throw new Error('seed: a whole number from 0 through 2^32 - 1 (4294967295)');
  // Refuse malformed drive input before starting a process on any host.
  locale = Intl.getCanonicalLocales(locale)[0];
  if (!locale) throw new Error('locale: a BCP 47 language tag');
  new Intl.DateTimeFormat(locale, {timeZone}).format(0);
  // The Region's format apart from the language (`exactTime().formatLocale`); unsaid, the hosts answer the locale.
  formatLocale = formatLocale ?? env.EXACT_AGENT_FORMAT_LOCALE;
  if (formatLocale !== undefined) formatLocale = Intl.getCanonicalLocales(formatLocale)[0];
  failFetch = faultSpec(failFetch ?? env.EXACT_AGENT_FAIL_FETCH);
  return {seed, locale, timeZone, epoch, ...(formatLocale ? {formatLocale} : {}), ...(failFetch ? {failFetch} : {})};
}

export function launchEnvironment(facts) {
  return {EXACT_AGENT_SEED: String(facts.seed), EXACT_AGENT_LOCALE: facts.locale, EXACT_AGENT_TIME_ZONE: facts.timeZone, EXACT_AGENT_EPOCH: String(facts.epoch), ...(facts.formatLocale ? {EXACT_AGENT_FORMAT_LOCALE: facts.formatLocale} : {}), ...(facts.failFetch ? {EXACT_AGENT_FAIL_FETCH: facts.failFetch} : {})};
}

// A build older than what it was made from is refused before launch
// (LLP 1012.001.000 D1), so a drive never verifies yesterday's app. Each build
// answers from its own record of its inputs: Cargo's dep-info, the Apple
// receipt, and for the web's `dist/` (which names none) the sources its build
// reads. These detect staleness; passing them is not proof of freshness. A
// binary the caller names (EXACT_LINUX_BIN, EXACT_MAC_BIN) is the caller's
// own: the driver says it did not check. A served page (--url) is its server's.
const shown = path => path.startsWith(ROOT + '/') ? relative(ROOT, path) : path;
const listed = changed => changed.slice(0, 3).join(', ') + (changed.length > 3 ? ` and ${changed.length - 3} more` : '');

/** Throws when `changed` names anything: what, since which build, and the command that rebuilds it. */
export function refuseStale(what, built, changed, command) {
  // A file no build reads belongs in the app's `.exact/` (Depot: evidence JSON beside the app refused every drive).
  const hint = changed.some(p => /\.json$/.test(p) && !/(^|\/)(app|package|tsconfig)\.json$/.test(p)) ? '; a file no build reads (evidence, logs, runtime state) belongs in the app\'s `.exact/`, which no build, watcher or freshness check reads' : '';
  const progress = process.env.EXACT_APP_DIR ? formatStatus(readStatus(process.env.EXACT_APP_DIR).filter(r => r.state === 'running')) : '';
  if (changed.length) throw staleError(`${what} build is stale: ${listed(changed)} changed since ${shown(built)} was built; run ${command}${hint}${progress && !progress.startsWith('No ') ? `; ${progress}` : ''}`);
}

/** A refusal that a rebuild answers: the driver exits 3 for it, so an app's
 * `exact.mjs` can build and drive again (LLP 1012.001.000: the driver itself
 * never builds). */
export const staleError = message => Object.assign(new Error(message), { stale: true });

/** Says, without refusing, what a coarse rule found or what was not checked. */
export function warnStale(what, built, changed, command) {
  if (changed.length) console.error(`${what} build may be stale (unverified): ${listed(changed)} changed since ${shown(built)} was built; ${command}`);
}
export const unchecked = (what, variable) => console.error(`${what} build freshness unchecked: ${variable} names the binary`);

/** Every input Cargo's dep-info beside a binary names ([] without one), as absolute paths. Under Cargo's
 * `build.dep-info-basedir` it writes them relative; `base` is what they are relative to, the workspace Cargo ran in. */
export function depInfoInputs(bin, info = `${bin}.d`, base = process.cwd()) {
  if (!existsSync(info)) return [];
  const line = readFileSync(info, 'utf8').split('\n')[0];
  return line.slice(line.indexOf(': ') + 2).split(/(?<!\\) /).filter(Boolean).map(p => resolve(base, p.replace(/\\ /g, ' ')));
}

// A build script's output (`…/build/<pkg>/out/…`) is rewritten whenever another
// build reruns that script with other variables (the web build does, for its
// render pass) without its sources changing; those sources are in the dep-info
// themselves, so they, not the output, decide. A bake made before Cargo for a
// linked plan (`….baked/`, scripts/linked-plan.mjs) names its own in its record.
const GENERATED = /\/build\/[^/]+\/out\/|\.baked\//;
// A build script watches directories an app may not have (`../assets`, `../data`,
// `../gpu/shaders`: apps/svg-gallery has no assets); Cargo's dep-info names the
// watch whether or not the directory exists, and a directory that never existed
// is not a change. A file that is gone is: it was read when the build ran.
const DIRECTORY_WATCH = /\/[^./]+$/;
/** Dep-info's inputs modified after `since`, or gone, generated outputs and never-present watched directories aside. */
export function depInfoNewer(since, bin, info, base) {
  return watchedNewer(since, depInfoInputs(bin, info, base).filter(p => !GENERATED.test(p)));
}

/** Of `paths`, files and the directories a build script watches whole, those modified after `since`, or gone. */
export function watchedNewer(since, paths) {
  const skips = new Map();
  // What no build reads: a build's output, installed packages (their declarations are inputs of their own) and a
  // hidden tree; within an app, also what its other freshness checks pass over (`notBuildInput`, and the names
  // `newerThan` skips, a README or a test), each outside the trees a build takes files from.
  const skipFor = dir => {
    let app = dir;
    while (!existsSync(resolve(app, 'app.json')) && dirname(app) !== app) app = dirname(app);
    if (!existsSync(resolve(app, 'app.json'))) return (path, isDir) => isDir && UNWATCHED_TREE.test(basename(path));
    if (!skips.has(app)) {
      const ignored = notBuildInput(app);
      skips.set(app, (path, isDir) => !INPUT_TREE.test(relative(app, path))
        && (isDir ? UNWATCHED_TREE.test(basename(path)) || ignored(path) : ignored(path) || NOT_INPUT.test(basename(path))));
    }
    return skips.get(app);
  };
  return [...new Set(paths.flatMap(p => {
    let stat;
    try { stat = statSync(p); } catch { return DIRECTORY_WATCH.test(p) ? [] : [shown(p)]; }
    if (!stat.isDirectory()) return stat.mtimeMs > since ? [shown(p)] : [];
    const dir = resolve(p);
    return watchedChanges(since, dir, skipFor(dir));
  }))];
}

const UNWATCHED_TREE = /^(target|node_modules|\..*)$/;
/** A directory in dep-info is a build script's watch, which Cargo scans recursively. It changed when a file under it
 * that a build reads was modified after `since`, or when a directory in it was (an entry created, removed or renamed)
 * after `since` and not by the creation of an entry no build reads: the JS build creates `app.contract.d.ts` in the
 * app directory after a native build, which moves the directory's mtime to that file's birth and is no change to
 * that build. A later write to such an entry explains nothing; a filesystem without birth times excuses nothing. */
function watchedChanges(since, dir, skip) {
  const out = [];
  const walk = at => {
    let entries; try { entries = readdirSync(at, { withFileTypes: true }); } catch { return; }
    let created = -Infinity;
    for (const e of entries) {
      const path = resolve(at, e.name);
      let stat; try { stat = statSync(path); } catch { continue; }
      const isDir = stat.isDirectory();
      if (skip(path, isDir)) created = Math.max(created, stat.birthtimeMs || -Infinity);
      else if (isDir) walk(path);
      else if (stat.mtimeMs > since) out.push(shown(path));
    }
    // The directory's mtime lands a few microseconds after the entry's birth.
    const mtime = statSync(at).mtimeMs;
    if (mtime > since && mtime > created + 1) out.push(shown(at));
  };
  walk(dir);
  return out;
}

/** Cargo's dep-info beside a binary: every source input newer than the binary, or gone. */
export const depInfoChanges = bin => existsSync(bin) ? depInfoNewer(statSync(bin).mtimeMs, bin) : [];

/** A packaged Windows game keeps compiler provenance in the private bake cache.
 * Require both unchanged source inputs and the exact copied executable/DLLs. */
export function packagedBuildChanges(receipt, directory, app) {
  if (!existsSync(receipt)) return ['missing compiler build receipt'];
  const build = JSON.parse(readFileSync(receipt, 'utf8'));
  if (build.version !== 1 || !build.binary?.inputs || !build.products?.length) return ['invalid compiler build receipt'];
  const changed = pendingBuildInputs(build);
  const products = build.products.filter(product => /\.(exe|dll)$/.test(product.path));
  if (!products.some(product => product.path.endsWith('.exe'))) changed.push('receipt has no executable');
  for (const product of products) {
    const path = resolve(directory, windowsFile(app, product.path));
    try {
      if (createHash('sha256').update(readFileSync(path)).digest('hex') !== product.sha256) changed.push(path);
    } catch { changed.push(path); }
  }
  const compatibility = resolve(directory, 'compat.json');
  if (existsSync(compatibility)) {
    const assets = JSON.parse(readFileSync(compatibility, 'utf8')).embedded?.assets ?? [];
    for (const asset of assets.filter(asset => /^shaders\/[A-Za-z_][A-Za-z0-9_]*\.wgsl$/.test(asset.name))) {
      const path = resolve(directory, asset.name);
      try {
        const bytes = readFileSync(path);
        if (bytes.length !== asset.bytes || createHash('sha256').update(bytes).digest('hex') !== asset.sha256) changed.push(path);
      } catch { changed.push(path); }
    }
  }
  return changed;
}

/** The plans a development bake of this app left a source map beside (LLP 1012.001.000 D6), as `sourceMapReader` locators: the Linux binary's own (its dep-info names the plan in OUT_DIR) and each platform's in the bake output (Apple, web). Which one describes the running plan is the reply's digest's to say. */
export function bakedPlans(linuxBin, bakeDir) {
  const out = depInfoInputs(linuxBin).filter(p => p.endsWith('/out/app.plan'));
  try { for (const f of readdirSync(bakeDir)) if (f.endsWith('.plan.map.json')) out.push(resolve(bakeDir, f.slice(0, -'.map.json'.length))); } catch {}
  return out.filter(p => existsSync(p + '.map.json'));
}

/** Where a trace's source map may be (LLP 1079 D5), when it carries none:
 * the development plan, the web build's, and the maps the named app's bake
 * left — where the live driver looks. */
export function traceLocators(appName) {
  const out = [process.env.EXACT_DEV_PLAN, resolve(webDist(), 'app.plan')].filter(Boolean);
  try { const a = resolveApp(appName); out.push(...bakedPlans(process.env.EXACT_LINUX_BIN ?? linuxBinary(a), bakeOutput(a))); } catch {}
  return out;
}

// Outputs, fixtures and prose are not what a build is made from.
// A dist's swap siblings (`dist.gen`, `dist.gen.reaped`, `dist.leases`, `dist.lock`, `dist.previous`,
// host/web/generations.mjs) are written by builds, never read by one: a later build's reap is no change.
const NOT_INPUT = /^(target|(web-)?dist(\.(gen|gen\.reaped|leases|lock|previous|stage-[^/]*))?|dist-windows|artifacts|node_modules|corpus|tests|conformance|\..*)$|\.test\.m?js$|\.test\.contract$|\.md$/;
/** Files under `roots` modified after `since`; `{shallow}` roots contribute only their own files. */
export function newerThan(since, roots, skip = () => false) {
  const out = [];
  const walk = (dir, deep) => {
    let entries; try { entries = readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const e of entries) {
      if (NOT_INPUT.test(e.name)) continue;
      const path = resolve(dir, e.name);
      if (skip(path)) continue;
      if (e.isDirectory()) { if (deep) walk(path, true); }
      else if (e.isFile() && statSync(path).mtimeMs > since) out.push(shown(path));
    }
  };
  for (const root of roots) typeof root === 'string' ? walk(root, true) : walk(root.shallow, false);
  return out;
}

// What a build can read from an app: the bake's capture (`js/bake/src/lib.rs`,
// `sources`), and Rust and WGSL sources and manifests.
const BUILD_SOURCE = /\.(ts|json|contract|ttf|otf|rs|toml|wgsl)$|(^|\/)Cargo\.lock$|^(assets|deck|gpu\/shaders)\//;
/** The app's gitignored paths, as a skip for its own files: an ignored file
 * no build reads is not an input. One a build can read still counts (a
 * generated asset or source, a local key), as does anything under `keep`
 * (declared shader roots). Outside Git, nothing. */
export function gitIgnored(dir, keep = []) {
  const listed = spawnSync('git', ['ls-files', '--others', '--ignored', '--exclude-standard', '--directory', '-z'], { cwd: dir, encoding: 'utf8' });
  const paths = listed.status === 0 ? listed.stdout.split('\0').filter(Boolean).map(p => resolve(dir, p)) : [];
  if (!paths.length) return () => false;
  // Files, not directories: an ignored directory is still walked for what the bake captures in it.
  const under = (path, roots) => roots.some(p => path === p || path.startsWith(p + '/'));
  return path => under(path, paths) && !under(path, keep) &&
    !statSync(path, { throwIfNoEntry: false })?.isDirectory() && !BUILD_SOURCE.test(relative(dir, path));
}

// What an agent leaves in an app as it works — a screenshot, a log, notes, a
// saved world, an export — and the trees a build takes files from (the bake's
// assets and deck, a game's art and logic, a native module's scripts, the host
// crates, fonts and strings). A build reads more than the bake captures, so the
// rule names the outputs and leaves everything else an input. A local server's
// database and its journal are written as the app runs (round 4's book-club: a
// write-ahead log made every native drive stale; LLP 1116.002 D4).
export const OUTPUT = /\.(png|jpe?g|gif|webp|apng|avif|bmp|log|txt|mov|mp4|webm|pdf|trace|world|csv|tsv|db|sqlite3?)(-(wal|shm|journal))?$/i;
export const INPUT_TREE = /^(assets|deck|gpu|art|modules|fonts|strings|logic|data|web|apple|ios|macos|linux)(\/|$)/;
// An agent's own tree (notes: shots/, tools/; platformer: drive.sh, tools/*.ops)
// and a test file are never inputs, even when the name looks like a source.
const AGENT_TREE = /^(shots|tools|repros)(\/|$)/;
const TEST_FILE = /(^|\/)[^/]*\.test\.(?:m?js|ts|rs|contract)$/;
// A shell or op-list helper outside the trees a build actually reads. A `.js`
// or `.mjs` file is read through `moduleGraph` like a `.ts`: one app.ts
// imports counts, one it never imports (an export script beside the app) is
// no input; without a graph every one counts, since a stale build run as
// fresh is worse than a rebuild.
const HELPER = /\.(?:sh|ops)$/;
const SCRIPT_TREE = /^(logic|data|gpu|render|art|assets|deck|modules)(\/|$)/;
// What app.ts reaches (LLP 1116.003 D1.1). The bake captures every TypeScript
// and JSON file but type-checks and bundles from the entry alone (its tsconfig's
// `files` is `__exact_entry.ts`, which imports app.ts: js/bake/src/typescript.mjs),
// so a file app.ts never imports — a backend the app reaches only over HTTP
// (round 5's book-club `server/index.ts`), a script — is no build's input. Read
// by scanning each source's module specifiers (type-only imports and references
// included) and taking every file a relative one can resolve to as the bake's
// resolution would, a `typescript.sources` mount as if beside app.ts under its
// name: over-inclusion costs only a rebuild. Null (every TypeScript file counts)
// without an app.ts, or when tsconfig maps bare specifiers (`paths`, `baseUrl`).
const SPECIFIER = /(?:\bfrom\s*|\bimport\s*\(?\s*|\brequire\s*\(\s*|<reference\s+path\s*=\s*)(['"])([^'"\n]+)\1/g;
export const MODULE_FILE = /\.(tsx?|[cm]?js|json)$/;
export const CONFIG_JSON = /^(app|package|jsconfig|tsconfig[^/]*)\.json$/;
/** The declarations the bake writes beside app.ts: its output, never an input. */
export const DECLARATIONS = 'app.contract.d.ts';
/** The files app.ts imports, transitively (and itself), by path; or null. */
export function moduleGraph(dir, manifest) {
  const entry = resolve(dir, 'app.ts');
  if (!existsSync(entry)) return null;
  try { if (/"(paths|baseUrl)"\s*:/.test(readFileSync(resolve(dir, 'tsconfig.json'), 'utf8'))) return null; } catch {}
  const mounts = Object.entries(manifest?.typescript?.sources ?? {}).map(([name, path]) => [resolve(dir, name), resolve(dir, String(path))]);
  const disk = path => { for (const [at, real] of mounts) if (path === at || path.startsWith(at + '/')) return real + path.slice(at.length); return path; };
  const graph = new Set(), seen = new Set(), queue = [entry];
  while (queue.length) {
    const at = queue.pop(), file = disk(at);
    let isFile = false; try { isFile = statSync(file).isFile(); } catch {}
    if (seen.has(at) || !isFile) continue;
    seen.add(at); graph.add(file);
    if (!/\.(tsx?|[cm]?js)$/.test(file)) continue;
    for (const [, , spec] of readFileSync(file, 'utf8').matchAll(SPECIFIER)) {
      if (!spec.startsWith('.')) continue;
      const base = resolve(dirname(at), spec), stem = base.replace(/\.(m?js|jsx)$/, '');
      queue.push(base, ...['.ts', '.tsx', '.d.ts', '.json', '/index.ts', '/index.tsx', '/index.d.ts'].map(end => base + end), ...['.ts', '.tsx', '.mts', '.d.ts'].map(end => stem + end));
    }
  }
  return graph;
}

/** A skip for an app's own files that no build reads. A file a build can
 * read always counts, ignored by Git or not: what the bake captures, anything
 * in an input tree, under `keep` (declared shader roots) or in one of the
 * app's Rust crates (which can `include_bytes!` any file beside them), and
 * what `app.json` names (an icon). Shots, tools, repros and test files never
 * count (LLP 1012: a drive refuses a stale build, not an agent's notes), nor
 * TypeScript or JSON app.ts never imports (`moduleGraph`). Of the rest, a
 * helper script outside a script tree, a gitignored file, or a picture, log,
 * note, export or saved world is not an input. */
export function notBuildInput(dir, keep = []) {
  const ignored = gitIgnored(dir, keep);
  const under = (path, roots) => roots.some(p => path === p || path.startsWith(p + '/'));
  let manifest = ''; try { manifest = readFileSync(resolve(dir, 'app.json'), 'utf8'); } catch {}
  let parsed = {}; try { parsed = JSON.parse(manifest); } catch {}
  const graph = moduleGraph(dir, parsed);
  const inCrate = path => {
    for (let at = dirname(path); at.startsWith(dir + '/'); at = dirname(at)) if (existsSync(resolve(at, 'Cargo.toml'))) return true;
    return false;
  };
  return path => {
    const rel = relative(dir, path);
    if ((AGENT_TREE.test(rel) && !under(path, keep)) || TEST_FILE.test(rel) || rel === DECLARATIONS) return true;
    const native = INPUT_TREE.test(rel) || under(path, keep) || inCrate(path) || manifest.includes(rel);
    if (graph && MODULE_FILE.test(rel) && !CONFIG_JSON.test(rel) && !native && !graph.has(path)) return true;
    if (BUILD_SOURCE.test(rel) || native) return false;
    if (HELPER.test(rel) && !SCRIPT_TREE.test(rel)) return true;
    return OUTPUT.test(rel) || ignored(path);
  };
}

const fileDigest = path => { try { return createHash('sha256').update(readFileSync(path)).digest('hex'); } catch { return null; } };
/** An Apple build's receipt: its Rust and Swift inputs by digest, then by mtime the app's own files the receipt leaves out on purpose (the root build script's watches: the contract, app.json, data, shaders, assets — what the baked plan and bundle are made from). */
export function receiptChanges(receipt, app, { hashed = false } = {}) { // `hashed`: this drive compared its inputs' bytes already
  if (!existsSync(receipt)) return [];
  const { build, target, started } = JSON.parse(readFileSync(receipt, 'utf8'));
  // A file edited while the build ran is newer than what it read, though older than the receipt written at its end
  // (LLP 1116.008 D2): the build's own start bounds it, else (an older receipt) the receipt's time.
  const since = Math.min(statSync(receipt).mtimeMs, Date.parse(started) || Infinity);
  const ignored = notBuildInput(app.dir, shaderWatchRoots(app));
  // A game's proof, pins and helper scripts are not native inputs either
  // (game/proof.mjs shares gameNonInput). A non-game keeps every real source.
  const game = path => Boolean(app.manifest?.game) && gameNonInput(relative(app.dir, path));
  const skip = path => ignored(path) || game(path);
  const own = newerThan(since, [app.dir], path => /\/(apple|ios|macos|linux|web)$/.test(path) && path.startsWith(app.dir + '/') || skip(path));
  // Platform-local modules live under the host crate directory skipped above,
  // but their separate dylib's sources are absent from the binary receipt.
  const platform = target.includes('-ios') ? 'ios' : 'macos';
  own.push(...newerThan(since, [moduleDirectory(app.dir, platform)], skip));
  // The receipt names what the binary links, not what built it: the Rust
  // archive's own dep-info also names its build script's (the compiler, the bake).
  const archive = `lib${app.crate(platform).replace(/-/g, '_')}.d`;
  let infos = []; try { infos = readdirSync(resolve(app.target, target)).map(p => resolve(app.target, target, p, archive)).filter(existsSync); } catch {}
  const info = infos.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
  // It names every source the bake captured (its rerun-if-changed): of the app's own, those the
  // walk above skips are no input here either (a backend app.ts never imports); one gone still counts.
  const skipped = path => { const at = resolve(ROOT, path); return at.startsWith(app.dir + '/') && existsSync(at) && skip(at); };
  // A product the build wrote and then read (the GPU dylib it signs beside Cargo's, whose digest the bake binds) is
  // newer than the build's start by construction. The receipt records its bytes, which are what the bundle captured
  // (captureAppleProduct): those decide, not its time. Every other input keeps the build's start as its bound.
  const products = new Map((build?.products ?? []).map(p => [resolve(p.path), p.sha256]));
  const inputs = info ? depInfoInputs(null, info, app.workspace) : [];
  const replaced = inputs.filter(path => products.has(path) && fileDigest(path) !== products.get(path)).map(shown);
  // A linked plan's bake ran before Cargo: the dep-info names what it wrote, its record what it read.
  // A record gone or unreadable says nothing of what it read: the build is stale.
  let baked = [], unread = [];
  if (build?.linkedPlan) { const at = resolve(build.linkedPlan, 'inputs.json'); try { baked = JSON.parse(readFileSync(at, 'utf8')).paths; if (!Array.isArray(baked)) throw new Error(); } catch { baked = []; unread = [shown(at)]; } }
  const tools = info ? [...depInfoNewer(since, null, info, app.workspace), ...watchedNewer(since, baked)].filter(path => !skipped(path) && !products.has(resolve(ROOT, path))) : [];
  return [...new Set([...(build?.binary && !hashed ? pendingBuildInputs(build) : []), ...own, ...tools, ...unread, ...replaced])];
}

/** A web `dist/`: app and shared runtime sources newer than its build marker.
 * Both are build inputs and both refuse a drive; the split makes diagnostics
 * and tests able to say which side changed without weakening that rule. */
/** Whether a path inside a game (relative to its directory) is not a build input:
 * its proof, pins, documents, tests, shots, tools, repros, and helper scripts
 * outside the built trees. The proof's input digest (game/proof.mjs), the web
 * staleness check and a game's native receipt share it. */
/** What a web build of `app` reads, modified after `since` (every file by default):
 * the app's own files and the shared host/runtime roots, as shown paths. */
export function webInputs(app, js, since = -Infinity) {
  if (app.manifest?.game) {
    const files = proofInputFiles(ROOT, app.dir).map(file => resolve(ROOT, file));
    const admitted = since === -Infinity ? files : files.filter(file => statSync(file).mtimeMs > since);
    return {app:admitted.filter(file => file.startsWith(app.dir + '/')), shared:admitted.filter(file => !file.startsWith(app.dir + '/'))};
  }
  const roots = js ? ['host/web-js', 'contract', 'plan', 'kernel/tables', { shallow: 'host/web' }] : ['host/web', 'runner', 'kernel', 'svg-filter', 'plan', 'motion', 'num', 'contract'];
  const ignored = notBuildInput(app.dir, shaderWatchRoots(app));
  const notInput = path => Boolean(app.manifest?.game) && gameNonInput(relative(app.dir, path));
  const backend = resolve(app.dir, '.exact/backend-origin');
  const appFiles = newerThan(since, [app.dir], path => /\/(apple|ios|macos|linux)$/.test(path) && path.startsWith(app.dir + '/') || ignored(path) || notInput(path));
  const shared = newerThan(since, roots.map(r => typeof r === 'string' ? resolve(ROOT, r) : { shallow: resolve(ROOT, r.shallow) }));
  // Absence is an input too: removing a selection must invalidate its bake.
  appFiles.push(backend);
  return { app: appFiles, shared };
}
const contentDigest = path => { try { return createHash('sha1').update(readFileSync(resolve(ROOT, path))).digest('hex'); } catch { return null; } };
/** Content digests of every input of a web build, recorded in its marker. */
export function webInputDigests(app, js) {
  const { app: own, shared } = webInputs(app, js);
  return {...Object.fromEntries([...own, ...shared].map(path => [path, contentDigest(path)])), EXACT_BACKEND_ORIGIN: process.env.EXACT_BACKEND_ORIGIN ?? null, PORT: process.env.PORT ?? null};
}
/** Inputs whose content changed since the build in `dist`. Modification times only
 * nominate candidates; a checkout that rewrote a file with its own bytes is not a
 * change. A marker without digests (an older build) trusts the times. */
export function webChanges(dist, app) {
  const marker = resolve(dist, '.exact-build.json');
  if (!existsSync(marker)) return { app: [], shared: [], all: [] };
  const built = JSON.parse(readFileSync(marker, 'utf8'));
  const changed = path => !built.inputs || built.inputs[path] !== contentDigest(path);
  // Candidates: what changed since the build read its inputs (`inputsAt`, as it started, less the file
  // system's clock grain), else since its marker.
  const since = typeof built.inputsAt === 'number' ? built.inputsAt - 2000 : statSync(marker).mtimeMs;
  const { app: own, shared: roots } = webInputs(app, built.target === 'js', since);
  const appChanges = own.filter(changed), shared = roots.filter(changed);
  // A file the build read that is gone since (a source deleted) is a change too: absence is an input.
  for (const path of Object.keys(built.inputs ?? {})) if (!['EXACT_BACKEND_ORIGIN', 'PORT'].includes(path) && built.inputs[path] !== null && !appChanges.includes(path) && !existsSync(resolve(ROOT, path))) (resolve(ROOT, path).startsWith(`${app.dir}/`) ? appChanges : shared).push(path);
  if ((built.inputs?.EXACT_BACKEND_ORIGIN ?? null) !== (process.env.EXACT_BACKEND_ORIGIN ?? null)) appChanges.push('EXACT_BACKEND_ORIGIN');
  if ((built.inputs?.PORT ?? null) !== (process.env.PORT ?? null)) appChanges.push('PORT');
  return { app: appChanges, shared, all: [...new Set([...appChanges, ...shared])] };
}

// LLP 1015.000: capture bounded primitive params, never claim they are wire bytes.
const cdpFailures = new WeakMap(), CDP_STRING_LIMIT = 256 * 1024;
const cdpIdentity = value => typeof value === 'string' && value.length <= 256 ? value : null;
export const cdpFailureContext = error => cdpFailures.get(error);
function associateCdpFailure(error, context) {
  try {
    const prior = cdpFailures.get(error);
    cdpFailures.set(error, prior && prior !== context
      ? Object.freeze({schema:1, omitted:'shared-error', ambiguous:true}) : context);
  } catch { /* A primitive throw remains a primitive throw. */ }
  return error;
}
export function copyCdpFailureContext(from, to) {
  const context = cdpFailureContext(from);
  return context ? associateCdpFailure(to, context) : to;
}

/** Also the offline test seam: no getter, Proxy trap, coercion or hashing. */
export function captureCdpRequest(method, params, sessionId, requestId, timeoutMs) {
  const snapshot = {method:cdpIdentity(method), requestId, cdpSessionId:cdpIdentity(sessionId),
    timeoutMs:typeof timeoutMs === 'number' && Number.isFinite(timeoutMs) ? timeoutMs : null};
  const field = method === 'Runtime.evaluate' ? 'expression' : method === 'Page.navigate' ? 'url' : null;
  if (!field) return snapshot;
  const unavailable = omitted => { snapshot.parameter = {field, omitted}; return snapshot; };
  try {
    if (typeof utilTypes.isProxy !== 'function') return unavailable('proxy-check-unavailable');
    if (!params || typeof params !== 'object' || utilTypes.isProxy(params)) return unavailable('not-plain-data');
    const prototype = Object.getPrototypeOf(params);
    if (prototype !== null && (utilTypes.isProxy(prototype) || prototype !== Object.prototype)) return unavailable('not-plain-data');
    if (Object.getOwnPropertyDescriptor(params, 'toJSON') || prototype && Object.getOwnPropertyDescriptor(prototype, 'toJSON'))
      return unavailable('serialization-hook');
    const property = Object.getOwnPropertyDescriptor(params, field);
    if (!property || !Object.hasOwn(property, 'value') || typeof property.value !== 'string') return unavailable('not-string-data');
    const characters = property.value.length;
    snapshot.parameter = characters > CDP_STRING_LIMIT ? {field, characters, omitted:'length-limit'}
      : {field, characters, value:property.value};
  } catch { return unavailable('capture-unavailable'); }
  return snapshot;
}
function releaseCdpRequest(snapshot) { if (snapshot.parameter) delete snapshot.parameter.value; }
function finishCdpFailure(error, snapshot, category, input) {
  try {
    const {parameter, ...identity} = snapshot;
    const context = {schema:1, ...identity, category, source:'captured-primitive-params'};
    if (parameter) {
      const {value, ...metadata} = parameter;
      if (typeof value === 'string') {
        metadata.utf8Bytes = Buffer.byteLength(value, 'utf8');
        metadata.sha256 = createHash('sha256').update(value, 'utf8').digest('hex');
      }
      context.parameter = Object.freeze(metadata);
    }
    const pipe = {};
    try {
      for (const key of ['writableLength','writableNeedDrain','destroyed']) {
        const value = input?.[key];
        if (typeof value === 'boolean' || typeof value === 'number' && Number.isFinite(value)) pipe[key] = value;
      }
    } catch { pipe.omitted = 'unavailable'; }
    context.input = Object.freeze(pipe);
    associateCdpFailure(error, Object.freeze(Buffer.byteLength(JSON.stringify(context), 'utf8') <= 8192
      ? context : {schema:1, omitted:'context-limit'}));
  } catch { associateCdpFailure(error, Object.freeze({schema:1, omitted:'metadata-unavailable'})); }
  finally { releaseCdpRequest(snapshot); }
  return error;
}

/** The DevTools protocol over Chrome's --remote-debugging-pipe (fd 3 in, fd 4 out; NUL-delimited JSON). A closed pipe or a dead Chrome fails every pending call; every call has a deadline. */
export class Cdp {
  constructor(input, output) {
    this.input = input;
    this.next = 1;
    this.pending = new Map();
    this.listeners = [];
    this.closeListeners = new Set();
    this.closed = null;
    let buf = '';
    output.setEncoding('utf8');
    output.on('data', (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf('\0')) >= 0) {
        const msg = JSON.parse(buf.slice(0, i));
        buf = buf.slice(i + 1);
        if (msg.id) {
          const p = this.pending.get(msg.id);
          this.pending.delete(msg.id);
          if (msg.error) p?.reject(new Error(`${msg.error.message} (${p.method})`), 'protocol');
          else p?.resolve(msg.result);
        } else for (const l of this.listeners) l(msg);
      }
    });
    output.on('end', () => this.fail('the DevTools pipe closed'));
    output.on('error', (e) => this.fail(`the DevTools pipe failed: ${e.message}`));
    input.on('error', (e) => this.fail(`the DevTools pipe failed: ${e.message}`));
  }
  fail(why) {
    this.closed ??= why;
    for (const [id, p] of this.pending) { this.pending.delete(id); p.reject(new Error(`${why} (${p.method})`), 'transport'); }
    for (const listener of this.closeListeners) listener(this.closed);
    this.closeListeners.clear();
  }
  send(method, params = {}, sessionId, timeoutMs = 15000) {
    if (this.closed) return Promise.reject(finishCdpFailure(new Error(`${this.closed} (${method})`),
      captureCdpRequest(method, params, sessionId, null, timeoutMs), 'closed', this.input));
    const id = this.next++;
    const snapshot = captureCdpRequest(method, params, sessionId, id, timeoutMs);
    return new Promise((resolve, reject) => {
      const fail = (error, category) => reject(finishCdpFailure(error, snapshot, category, this.input));
      const timer = setTimeout(() => { this.pending.delete(id); fail(new Error(`${method} did not answer within ${timeoutMs} ms`), 'timeout'); }, timeoutMs);
      this.pending.set(id, { resolve: (v) => { clearTimeout(timer); releaseCdpRequest(snapshot); resolve(v); },
        reject: (e, category = 'transport') => { clearTimeout(timer); fail(e, category); }, method });
      try { this.input.write(JSON.stringify({ id, method, params, sessionId }) + '\0'); }
      catch (error) {
        // Keep the existing executor rejection and pending timer; only metadata is retired.
        finishCdpFailure(error, snapshot, 'send-refusal', this.input);
        throw error;
      }
    });
  }
}

/** The tab closed as a person closes it (studio diary R17), the Chrome carrier's `tap {close:true}`: `Page.close` runs
 * the page's `beforeunload`; a handler that prevented it opens Chrome's "Leave site?" (given the page's sticky
 * activation, as for a person), answered "Stay". Either the dialog or the target's detach comes; after the second the
 * page is gone. */
export async function closePage(req, { cdp, sessionId, call, frame }) {
  if (Object.keys(req).some(k => !['op', 'close'].includes(k)) || req.close !== true) return { malformed: true, error: 'tap close takes no other input fields' };
  let listener, timer;
  const outcome = new Promise((ok) => {
    listener = (msg) => {
      if (msg.sessionId === sessionId && msg.method === 'Page.javascriptDialogOpening' && msg.params.type === 'beforeunload') ok('dialog');
      else if (msg.method === 'Target.detachedFromTarget' && msg.params.sessionId === sessionId) ok('closed');
    };
    cdp.listeners.push(listener);
    timer = setTimeout(() => ok('timeout'), 5000);
  });
  try {
    call('Page.close').catch(() => {});
    const how = await outcome;
    if (how === 'timeout') return { error: 'the page neither closed nor asked to stay within 5 s of Page.close' };
    if (how === 'closed') return { closed: true, delivery: 'browser-window', native: 'Page.close' };
    await call('Page.handleJavaScriptDialog', { accept: false });
    await frame();
    return { closed: false, kept: 'a `beforeunload` called `preventDefault()`: the browser asked "Leave site?", answered "Stay"', delivery: 'browser-window', native: 'Page.close' };
  } finally { clearTimeout(timer); cdp.listeners.splice(cdp.listeners.indexOf(listener), 1); }
}

/** The agent's `back` on the web (LLP 1116.006 D4), the carrier's half: an open modal `dialog` or `role="alertdialog"`
 * popover takes the Escape a person presses, a real key the browser makes the dialog's close request (`closedby="none"`
 * keeps it open, as a `cancel` handler that prevents it does). `escape` presses it (CDP's, or Playwright's keyboard).
 * Null with none open: the page's agent then takes the browser's Back from the selected route (navigation.js `back`). */
export async function backLayer(req, { evaluate, escape, frame }) {
  if (Object.keys(req).some(k => !['op', 'back'].includes(k)) || req.back !== true) return { malformed: true, error: 'tap back takes no other input fields' };
  const open = `document.querySelectorAll('dialog:modal, [role="alertdialog"]:popover-open').length`, before = await evaluate(open);
  if (!before) return null;
  await escape();
  await frame();
  return { back: 'dialog', delivery: 'platform', ...(await evaluate(open) >= before ? { refused: 'the dialog stayed open: its closedby is "none", or a `cancel` handler prevented the close' } : {}) };
}

/** Escape pressed through CDP, down and up, as a keyboard's (`backLayer`). */
export const cdpEscape = (call) => async () => { for (const type of ['rawKeyDown', 'keyUp']) await call('Input.dispatchKeyEvent', { type, key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }); };

/** One simulator's display, across every drive and checkout: a capture that reads it (`withDisplay`) holds it, and so
 *  does a launch, which brings its app to the front over another drive's. Runs `fn` holding it. */
export async function simulatorDisplay(udid, fn, { directory = resolve(tmpdir(), 'exact-ios-drives'), timeout = 60000 } = {}) {
  const path = createHash('sha256').update(JSON.stringify(['display', udid])).digest('hex') + '/.lock';
  const started = Date.now();
  for (;;) {
    let entered = false;
    try { return await filesystemLock(directory, path, () => { entered = true; return fn(); }); }
    catch (error) {
      if (entered || !error.message.includes('stream is locked by another publisher')) throw error;
      if (Date.now() - started >= timeout) throw new Error(`simulator ${udid}'s display busy: another drive still held it after ${timeout / 1000}s`);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
}

/** The simulator has one running process per bundle id, across every checkout.
 * Hold an OS lock through launch and close; a crashed driver releases it through
 * the helper's stdin. Permanent lock files avoid unlink/reacquire races. */
export async function exclusiveIOS(udid, bundle, launch, { directory = resolve(tmpdir(), 'exact-ios-drives'), timeout = 60000 } = {}) {
  const path = createHash('sha256').update(JSON.stringify([udid, bundle])).digest('hex') + '/.lock';
  const started = Date.now();
  let release, holding;
  for (;;) {
    let acquired;
    const ready = new Promise(resolve => { acquired = resolve; });
    const done = new Promise(resolve => { release = resolve; });
    holding = filesystemLock(directory, path, async () => { acquired(); await done; });
    try { await Promise.race([ready, holding]); break; }
    catch (error) {
      if (!error.message.includes('stream is locked by another publisher')) throw error;
      if (Date.now() - started >= timeout) throw new Error(`iOS drive busy for ${bundle} on ${udid}: another drive still owns this simulator app after ${timeout / 1000}s`);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
  const unlock = async () => { release(); await holding; };
  try {
    const carrier = await launch(), close = carrier.close.bind(carrier);
    let closed;
    carrier.close = () => closed ??= (async () => { try { await close(); } finally { await unlock(); } })();
    return carrier;
  } catch (error) { await unlock(); throw error; }
}

/** A proof executes private products and checks those copies against their inputs. */
export function proofProductChanges(receipt, bin, app, root = ROOT) {
  const record = JSON.parse(readFileSync(receipt,'utf8')), changed = [];
  if (record.binary !== bin) return ['proof receipt names another binary'];
  const files = executionInputFiles(root,app.dir);
  for (const file of new Set([...files,...Object.keys(record.inputs)])) {
    const path = resolve(root,file);
    if (!files.includes(file) || !existsSync(path) || createHash('sha256').update(readFileSync(path)).digest('hex') !== record.inputs[file]) changed.push(file);
  }
  for (const [path,hash] of Object.entries(record.products))
    if (!existsSync(path) || createHash('sha256').update(readFileSync(path)).digest('hex') !== hash) changed.push(path);
  return changed;
}

/** Each `--notification '<entry>'` (LLP 1133.004 §7): the entry handed to the relaunched app, as the platform hands a
 * closed app its click, before the drive's first op; each `--wake '<entry>'` (LLP 1133.017 §10) likewise, raised as a
 * cold wake. */
export async function launchEntries(s, flags) {
  for (const entry of flags.launchEntries ?? []) {
    const r = await s.op({ op: 'launchEntry', ...entry });
    if (r?.error) throw new Error(r.error);
  }
}

if (import.meta.main) {
  const helperArgs = process.argv.slice(2);
  if (helperArgs.includes('--fixture-port')) {
    const at = helperArgs.indexOf('--app');
    const { backendOriginSelection } = await import('../js/bake/src/typescript.mjs');
    const selected = backendOriginSelection(resolveApp(at < 0 ? undefined : helperArgs[at + 1]).dir);
    if (!selected) throw new Error('this app has no selected fixture origin; pass --app <name>');
    console.log(new URL(selected.origin).port || (selected.origin.startsWith('https:') ? '443' : '80'));
  }
}
