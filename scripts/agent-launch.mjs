// Session setup shared by the agent CLI and its programmatic driver.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { accessSync, constants, existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, delimiter, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bakeOutput, linuxBinary, pendingBuildInputs, resolveApp, shaderWatchRoots, webDist } from './app.mjs';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));

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
    else if (argv[i] === '--phone') flags.phone = argv[++i];
    else if (argv[i] === '--storage') flags.storage = argv[++i];
    else rest.push(argv[i]);
  }
  return { flags, rest };
}

/** LLP 1027.000.000 D3: the date at the agent clock's zero, unless the drive names one. */
export const AGENT_EPOCH = '2026-01-01T00:00:00Z';

export function launchFacts({seed, locale, timeZone, epoch, env = {}}) {
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
  return {seed, locale, timeZone, epoch};
}

export function launchEnvironment(facts) {
  return {EXACT_AGENT_SEED: String(facts.seed), EXACT_AGENT_LOCALE: facts.locale, EXACT_AGENT_TIME_ZONE: facts.timeZone, EXACT_AGENT_EPOCH: String(facts.epoch)};
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
  if (changed.length) throw staleError(`${what} build is stale: ${listed(changed)} changed since ${shown(built)} was built; run ${command}`);
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
export function packagedBuildChanges(receipt, directory) {
  if (!existsSync(receipt)) return ['missing compiler build receipt'];
  const build = JSON.parse(readFileSync(receipt, 'utf8'));
  if (build.version !== 1 || !build.binary?.inputs || !build.products?.length) return ['invalid compiler build receipt'];
  const changed = pendingBuildInputs(build);
  const products = build.products.filter(product => /\.(exe|dll)$/.test(product.path));
  if (!products.some(product => product.path.endsWith('.exe'))) changed.push('receipt has no executable');
  for (const product of products) {
    const path = resolve(directory, basename(product.path));
    try {
      if (createHash('sha256').update(readFileSync(path)).digest('hex') !== product.sha256) changed.push(path);
    } catch { changed.push(path); }
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
// saved world — and the trees a build takes files from (the bake's assets and
// deck, a game's art and logic, a native module's scripts, the host crates,
// fonts and strings). A build reads more than the bake captures, so the rule
// names the outputs and leaves everything else an input.
const OUTPUT = /\.(png|jpe?g|gif|webp|apng|avif|bmp|log|txt|mov|mp4|webm|pdf|trace|world)$/i;
const INPUT_TREE = /^(assets|deck|gpu|art|modules|fonts|strings|logic|data|web|apple|linux)(\/|$)/;
/** A skip for an app's own files that no build reads. A file a build can
 * read always counts, ignored by Git or not: what the bake captures, anything
 * in an input tree, under `keep` (declared shader roots) or in one of the
 * app's Rust crates (which can `include_bytes!` any file beside them), and
 * what `app.json` names (an icon). Of the rest, a gitignored file or a
 * picture, log, note or saved world is not an input: a screenshot saved into
 * the app is not a change to it. */
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
    if (BUILD_SOURCE.test(rel) || INPUT_TREE.test(rel) || under(path, keep) || inCrate(path) || manifest.includes(rel)) return false;
    return OUTPUT.test(rel) || ignored(path);
  };
}

/** An Apple build's receipt: its Rust and Swift inputs by digest, then by mtime the app's own files the receipt leaves out on purpose (the root build script's watches: the contract, app.json, data, shaders, assets — what the baked plan and bundle are made from). */
export function receiptChanges(receipt, app) {
  if (!existsSync(receipt)) return [];
  const { build, target } = JSON.parse(readFileSync(receipt, 'utf8')), since = statSync(receipt).mtimeMs;
  const ignored = notBuildInput(app.dir, shaderWatchRoots(app));
  const own = newerThan(since, [app.dir], path => /\/(apple|linux|web)$/.test(path) && path.startsWith(app.dir + '/') || ignored(path));
  // The receipt names what the binary links, not what built it: the Rust
  // archive's own dep-info also names its build script's (the compiler, the bake).
  const archive = `lib${app.crate('apple').replace(/-/g, '_')}.d`;
  let infos = []; try { infos = readdirSync(resolve(app.target, target)).map(p => resolve(app.target, target, p, archive)).filter(existsSync); } catch {}
  const info = infos.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
  const tools = info ? depInfoNewer(since, null, info) : [];
  return [...new Set([...(build?.binary ? pendingBuildInputs(build) : []), ...own, ...tools])];
}

/** A web `dist/`: app and shared runtime sources newer than its build marker.
 * Both are build inputs and both refuse a drive; the split makes diagnostics
 * and tests able to say which side changed without weakening that rule. */
/** Whether a path inside a game (relative to its directory) is not a build input:
 * its proof, pins, documents, tests, and helper scripts outside the built trees.
 * The proof's input digest (game/proof.mjs) and the web staleness check share it. */
export function gameNonInput(path) {
  path = path.replaceAll('\\', '/');
  return /(^|\/)(pins\.json|proof\.mjs|[^/]*\.test\.mjs|[^/]*\.md)$/.test(path)
    || (/\.m?js$/.test(path) && !/^(logic|data|gpu|art|assets|deck)\//.test(path));
}
export function webChanges(dist, app) {
  const marker = resolve(dist, '.exact-build.json');
  if (!existsSync(marker)) return { app: [], shared: [], all: [] };
  const since = statSync(marker).mtimeMs, js = JSON.parse(readFileSync(marker, 'utf8')).target === 'js';
  const roots = js ? ['host/web-js', 'contract', 'plan', 'kernel/tables', { shallow: 'host/web' }] : ['host/web', 'runner', 'kernel', 'plan', 'motion', 'num', 'contract'];
  const ignored = notBuildInput(app.dir, shaderWatchRoots(app));
  const notInput = path => Boolean(app.manifest?.game) && gameNonInput(relative(app.dir, path));
  const appChanges = newerThan(since, [app.dir], path => /\/(apple|linux)$/.test(path) && path.startsWith(app.dir + '/') || ignored(path) || notInput(path));
  const shared = newerThan(since, roots.map(r => typeof r === 'string' ? resolve(ROOT, r) : { shallow: resolve(ROOT, r.shallow) }));
  return { app: appChanges, shared, all: [...new Set([...appChanges, ...shared])] };
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
          if (msg.error) p?.reject(new Error(`${msg.error.message} (${p.method})`));
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
    for (const [id, p] of this.pending) { this.pending.delete(id); p.reject(new Error(`${why} (${p.method})`)); }
  }
  send(method, params = {}, sessionId, timeoutMs = 15000) {
    if (this.closed) return Promise.reject(new Error(`${this.closed} (${method})`));
    const id = this.next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`${method} did not answer within ${timeoutMs} ms`)); }, timeoutMs);
      this.pending.set(id, { resolve: (v) => { clearTimeout(timer); resolve(v); }, reject: (e) => { clearTimeout(timer); reject(e); }, method });
      this.input.write(JSON.stringify({ id, method, params, sessionId }) + '\0');
    });
  }
}
