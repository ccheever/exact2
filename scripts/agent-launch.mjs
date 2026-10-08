// Session setup shared by the agent CLI and its programmatic driver.
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { accessSync, constants, existsSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { basename, delimiter, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { types as utilTypes } from 'node:util';
import { filesystemLock } from './filesystem.mjs';
import { bakeOutput, linuxBinary, moduleDirectory, pendingBuildInputs, resolveApp, windowsFile, shaderWatchRoots, webDist } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

/** Preserve the original operation failure and any owned cleanup handle. */
export function retainCleanupError(error, failure) {
  error.message += `; cleanup: ${failure.message}`;
  error.cleanupError = failure;
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
 * one name is one port, from its hash, below the ephemeral range. */
/** The store a drive with `env` uses: the driver's environment under the drive's own, as `runTests` places and removes
 * it and a native host sees it (review b5-c 3). */
export const driveStore = (appId, storage, env, platform = process.platform) => {
  const launched = { ...process.env, ...(env ?? {}) };
  return webStore(appId, storage, launched, launched.HOME || homedir(), platform);
};
export function webStore(appId, storage, env = process.env, home = homedir(), platform = process.platform) {
  const cache = platform === 'darwin' ? resolve(home, 'Library/Caches') : env.XDG_CACHE_HOME?.startsWith('/') ? env.XDG_CACHE_HOME : resolve(home, '.cache');
  const base = resolve(cache, 'exact', appId, 'agent-web');
  return { base, profile: resolve(base, storage), port: 20000 + createHash('sha256').update(`${appId}/${storage}`).digest().readUInt32BE(0) % 28000 };
}

/** One browser lookup for the agent and its tests: an explicit override,
 * otherwise the platform's ordinary Chromium installation. A bare CHROME
 * name is resolved through PATH before a test decides whether to skip. */
export function chromium(environment = process.env, platform = process.platform) {
  const named = environment.CHROME ? [environment.CHROME] : platform === 'win32' ? [
    resolve(environment.ProgramFiles ?? 'C:\\Program Files', 'Google/Chrome/Application/chrome.exe'),
    resolve(environment['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)', 'Google/Chrome/Application/chrome.exe'),
    ...(environment.LOCALAPPDATA ? [resolve(environment.LOCALAPPDATA, 'Google/Chrome/Application/chrome.exe')] : []),
    resolve(environment['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)', 'Microsoft/Edge/Application/msedge.exe'),
  ] : [platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : '/usr/bin/chromium'];
  const candidates = named.flatMap(name => /[/\\]/.test(name) ? [resolve(name)]
    : (environment.PATH ?? '').split(delimiter).filter(Boolean).map(dir => resolve(dir, name)));
  const executable = candidates.find(path => { try { accessSync(path, constants.X_OK); return true; } catch { return false; } });
  return { executable: executable ?? named[0], unavailable: executable ? null : `Chromium is missing at ${named.join(', ')}; set CHROME to an installed browser` };
}

export function parseFlags(argv) {
  const flags = { json: false };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--json') flags.json = true;
    else if (argv[i] === '--world') flags.world = resolve(argv[++i]);
    else if (argv[i] === '--plan') flags.plan = resolve(argv[++i]);
    else if (argv[i] === '--app') flags.app = argv[++i];
    else if (argv[i] === '--browser') flags.browser = argv[++i];
    else if (argv[i] === '--size') flags.size = argv[++i].split('x').map(Number);
    else if (argv[i] === '--test') flags.test = argv[++i];
    else if (argv[i] === '--session') flags.session = argv[++i];
    else if (argv[i] === '--open') (flags.open ??= []).push(resolve(argv[++i]));
    else if (argv[i] === '--url') flags.url = argv[++i];
    else if (argv[i] === '--device') flags.device = true;
    else if (argv[i] === '--seed') flags.seed = Number(argv[++i]);
    else if (argv[i] === '--locale') flags.locale = argv[++i];
    else if (argv[i] === '--time-zone') flags.timeZone = argv[++i];
    else if (argv[i] === '--epoch') flags.epoch = argv[++i];
    else if (argv[i] === '--timing') flags.timing = argv[++i];
    else if (argv[i] === '--touch') flags.touch = argv[++i];
    else if (argv[i] === '--chrome') flags.chrome = argv[++i];
    else if (argv[i] === '--phone') flags.phone = argv[++i];
    else if (argv[i] === '--storage') flags.storage = argv[++i];
    // A driver fault armed before the app's first data load (LLP 1103 D3): every fetch whose URL starts with it fails.
    else if (argv[i] === '--fail-fetch') flags.failFetch = [flags.failFetch, argv[++i]].filter(Boolean).join('\n');
    else rest.push(argv[i]);
  }
  return { flags, rest };
}

/** LLP 1027.000.000 D3: the date at the agent clock's zero, unless the drive names one. */
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
  const line = lineHeight != null ? `{ const s = new CSSStyleSheet(); s.replaceSync('body{line-height:${lineHeight}}'); document.adoptedStyleSheets = [...document.adoptedStyleSheets, s]; }` : '';
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

/** Launch lines from `state.faults` (LLP 1103 D3): a reload relaunches with the table as it is now. */
export const faultSpecOf = faults => (faults ?? []).map(f => [f.prefix, f.times ?? '-', f.left ?? '-', f.hits, f.armed ? 1 : 0].join('\t')).join('\n');

export function launchFacts({seed, locale, timeZone, epoch, failFetch, env = {}}) {
  seed = Number(seed ?? env.EXACT_AGENT_SEED ?? 1);
  // An ISO date or Unix milliseconds; hosts are told milliseconds.
  epoch = String(epoch ?? env.EXACT_AGENT_EPOCH ?? AGENT_EPOCH);
  epoch = /^\d+$/.test(epoch) ? Number(epoch) : /^\d{4}-\d\d-\d\d(T|$)/.test(epoch) ? Date.parse(epoch) : NaN;
  if (!Number.isSafeInteger(epoch) || epoch < 0) throw new Error('epoch: an ISO date or Unix milliseconds at or after 1970');
  locale = locale ?? env.EXACT_AGENT_LOCALE ?? 'en-US';
  timeZone = timeZone ?? env.EXACT_AGENT_TIME_ZONE ?? 'UTC';
  if (!Number.isSafeInteger(seed) || seed < 0) throw new Error('seed: an integer from 0 through 2^53 - 1');
  // Refuse malformed drive input before starting a process on any host.
  locale = Intl.getCanonicalLocales(locale)[0];
  if (!locale) throw new Error('locale: a BCP 47 language tag');
  new Intl.DateTimeFormat(locale, {timeZone}).format(0);
  failFetch = faultSpec(failFetch ?? env.EXACT_AGENT_FAIL_FETCH);
  return {seed, locale, timeZone, epoch, ...(failFetch ? {failFetch} : {})};
}

export function launchEnvironment(facts) {
  return {EXACT_AGENT_SEED: String(facts.seed), EXACT_AGENT_LOCALE: facts.locale, EXACT_AGENT_TIME_ZONE: facts.timeZone, EXACT_AGENT_EPOCH: String(facts.epoch), ...(facts.failFetch ? {EXACT_AGENT_FAIL_FETCH: facts.failFetch} : {})};
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
  if (changed.length) throw staleError(`${what} build is stale: ${listed(changed)} changed since ${shown(built)} was built; run ${command}${hint}`);
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

/** Every input Cargo's dep-info beside a binary names ([] without one). */
export function depInfoInputs(bin, info = `${bin}.d`) {
  if (!existsSync(info)) return [];
  const line = readFileSync(info, 'utf8').split('\n')[0];
  return line.slice(line.indexOf(': ') + 2).split(/(?<!\\) /).filter(Boolean).map(p => p.replace(/\\ /g, ' '));
}

// A build script's output (`…/build/<pkg>/out/…`) is rewritten whenever another
// build reruns that script with other variables (the web build does, for its
// render pass) without its sources changing; those sources are in the dep-info
// themselves, so they, not the output, decide.
const GENERATED = /\/build\/[^/]+\/out\//;
// A build script watches directories an app may not have (`../assets`, `../data`,
// `../gpu/shaders`: apps/svg-gallery has no assets); Cargo's dep-info names the
// watch whether or not the directory exists, and a directory that never existed
// is not a change. A file that is gone is: it was read when the build ran.
const DIRECTORY_WATCH = /\/[^./]+$/;
/** Dep-info's inputs modified after `since`, or gone, generated outputs and never-present watched directories aside. */
export function depInfoNewer(since, bin, info) {
  return depInfoInputs(bin, info).filter(p => !GENERATED.test(p)).filter(p => { try { return statSync(p).mtimeMs > since; } catch { return !DIRECTORY_WATCH.test(p); } }).map(shown);
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
const NOT_INPUT = /^(target|dist|dist.previous|dist-windows|web-dist|artifacts|node_modules|corpus|tests|conformance|\..*)$|\.test\.m?js$|\.test\.contract$|\.md$/;
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
// rule names the outputs and leaves everything else an input.
export const OUTPUT = /\.(png|jpe?g|gif|webp|apng|avif|bmp|log|txt|mov|mp4|webm|pdf|trace|world|csv|tsv)$/i;
export const INPUT_TREE = /^(assets|deck|gpu|art|modules|fonts|strings|logic|data|web|apple|ios|macos|linux)(\/|$)/;
// An agent's own tree (notes: shots/, tools/; platformer: drive.sh, tools/*.ops)
// and a test file are never inputs, even when the name looks like a source.
const AGENT_TREE = /^(shots|tools|repros)(\/|$)/;
const TEST_FILE = /(^|\/)[^/]*\.test\.(?:m?js|ts|rs|contract)$/;
// A shell or op-list helper outside the trees a build actually reads. A `.js`
// or `.mjs` file counts: `app.ts` may import one (a game's own scripts are
// `gameNonInput`'s), and a stale build run as fresh is worse than a rebuild.
const HELPER = /\.(?:sh|ops)$/;
const SCRIPT_TREE = /^(logic|data|gpu|render|art|assets|deck|modules)(\/|$)/;
/** A skip for an app's own files that no build reads. A file a build can
 * read always counts, ignored by Git or not: what the bake captures, anything
 * in an input tree, under `keep` (declared shader roots) or in one of the
 * app's Rust crates (which can `include_bytes!` any file beside them), and
 * what `app.json` names (an icon). Shots, tools, repros and test files never
 * count (LLP 1012: a drive refuses a stale build, not an agent's notes). Of
 * the rest, a helper script outside a script tree, a gitignored file, or a
 * picture, log, note, export or saved world is not an input. */
export function notBuildInput(dir, keep = []) {
  const ignored = gitIgnored(dir, keep);
  const under = (path, roots) => roots.some(p => path === p || path.startsWith(p + '/'));
  let manifest = ''; try { manifest = readFileSync(resolve(dir, 'app.json'), 'utf8'); } catch {}
  const inCrate = path => {
    for (let at = dirname(path); at.startsWith(dir + '/'); at = dirname(at)) if (existsSync(resolve(at, 'Cargo.toml'))) return true;
    return false;
  };
  return path => {
    const rel = relative(dir, path);
    if ((AGENT_TREE.test(rel) && !under(path, keep)) || TEST_FILE.test(rel)) return true;
    if (BUILD_SOURCE.test(rel) || INPUT_TREE.test(rel) || under(path, keep) || inCrate(path) || manifest.includes(rel)) return false;
    if (HELPER.test(rel) && !SCRIPT_TREE.test(rel)) return true;
    return OUTPUT.test(rel) || ignored(path);
  };
}

/** An Apple build's receipt: its Rust and Swift inputs by digest, then by mtime the app's own files the receipt leaves out on purpose (the root build script's watches: the contract, app.json, data, shaders, assets — what the baked plan and bundle are made from). */
export function receiptChanges(receipt, app) {
  if (!existsSync(receipt)) return [];
  const { build, target } = JSON.parse(readFileSync(receipt, 'utf8')), since = statSync(receipt).mtimeMs;
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
  const tools = info ? depInfoNewer(since, null, info) : [];
  return [...new Set([...(build?.binary ? pendingBuildInputs(build) : []), ...own, ...tools])];
}

/** A web `dist/`: app and shared runtime sources newer than its build marker.
 * Both are build inputs and both refuse a drive; the split makes diagnostics
 * and tests able to say which side changed without weakening that rule. */
/** Whether a path inside a game (relative to its directory) is not a build input:
 * its proof, pins, documents, tests, shots, tools, repros, and helper scripts
 * outside the built trees. The proof's input digest (game/proof.mjs), the web
 * staleness check and a game's native receipt share it. */
export function gameNonInput(path) {
  path = path.replaceAll('\\', '/');
  return /^(shots|tools|repros)\//.test(path)
    || /(^|\/)(pins\.json|proof\.mjs|[^/]*\.test\.(?:mjs|js|ts|rs|contract)|[^/]*\.md)$/.test(path)
    || (/\.(?:m?js|sh|ops)$/.test(path) && !/^(logic|data|gpu|render|art|assets|deck)\//.test(path));
}
/** What a web build of `app` reads, modified after `since` (every file by default):
 * the app's own files and the shared host/runtime roots, as shown paths. */
export function webInputs(app, js, since = -Infinity) {
  const roots = js ? ['host/web-js', 'contract', 'plan', 'kernel/tables', { shallow: 'host/web' }] : ['host/web', 'runner', 'kernel', 'svg-filter', 'plan', 'motion', 'num', 'contract'];
  const ignored = notBuildInput(app.dir, shaderWatchRoots(app));
  const notInput = path => Boolean(app.manifest?.game) && gameNonInput(relative(app.dir, path));
  const appFiles = newerThan(since, [app.dir], path => /\/(apple|ios|macos|linux)$/.test(path) && path.startsWith(app.dir + '/') || ignored(path) || notInput(path));
  const shared = newerThan(since, roots.map(r => typeof r === 'string' ? resolve(ROOT, r) : { shallow: resolve(ROOT, r.shallow) }));
  return { app: appFiles, shared };
}
const contentDigest = path => { try { return createHash('sha1').update(readFileSync(resolve(ROOT, path))).digest('hex'); } catch { return null; } };
/** Content digests of every input of a web build, recorded in its marker. */
export function webInputDigests(app, js) {
  const { app: own, shared } = webInputs(app, js);
  return Object.fromEntries([...own, ...shared].map(path => [path, contentDigest(path)]));
}
/** Inputs whose content changed since the build in `dist`. Modification times only
 * nominate candidates; a checkout that rewrote a file with its own bytes is not a
 * change. A marker without digests (an older build) trusts the times. */
export function webChanges(dist, app) {
  const marker = resolve(dist, '.exact-build.json');
  if (!existsSync(marker)) return { app: [], shared: [], all: [] };
  const built = JSON.parse(readFileSync(marker, 'utf8'));
  const changed = path => !built.inputs || built.inputs[path] !== contentDigest(path);
  const { app: own, shared: roots } = webInputs(app, built.target === 'js', statSync(marker).mtimeMs);
  const appChanges = own.filter(changed), shared = roots.filter(changed);
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
  if (Object.keys(req).some(k => !['op', 'close'].includes(k)) || req.close !== true) return { error: 'tap close takes no other input fields' };
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
