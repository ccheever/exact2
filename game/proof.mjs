import {sceneInputs} from './app/scenes.mjs';
// Shared lifecycle for game proofs: operations and assertions stay in the game.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, relative, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { open as openSession, render } from '../scripts/agent.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { buildBake, resolveApp } from '../scripts/app.mjs';
import { closeFilesystemReader } from '../scripts/filesystem.mjs';

export function parseInventoryLine(line) {
  const m = line.trim().match(/^(\d+)\s+(\d+)\s+(\S+)\s+(.{24})\s+(.+)$/);
  return m && !m[3].startsWith('Z') ? {pid:Number(m[1]), parent:Number(m[2]), stamp:m[4], command:m[5]} : null;
}

export function artifactInventory(host, dist, artifacts) {
  try {
    const manifest = [];
    const walk = (dir, prefix) => {
      for (const name of readdirSync(dir).sort()) {
        const path = resolve(dir, name), key = `${prefix}/${name}`;
        if (statSync(path).isDirectory()) walk(path, key);
        else manifest.push([key, createHash('sha256').update(readFileSync(path)).digest('hex')]);
      }
    };
    if (host === 'web') {
      readFileSync(resolve(dist, 'exact.json'));
      walk(dist, 'dist');
    } else if (host === 'linux') {
      for (const path of [artifacts.binary, artifacts.module]) manifest.push([basename(path), createHash('sha256').update(readFileSync(path)).digest('hex')]);
    } else {
      const executable = resolve(artifacts.bundle, host === 'macos' ? 'Contents/MacOS/ExactMac' : 'ExactIOS');
      readFileSync(executable);
      walk(artifacts.bundle, 'bundle');
      // The driver launches the standalone product on macOS, loading its adjacent dylibs/assets.
      if (artifacts.binary) {
        readFileSync(artifacts.binary); // A missing actual carrier always invalidates the receipt.
        walk(artifacts.products ?? resolve(artifacts.binary, '..'), 'product');
      }
    }
    return {digest:createHash('sha256').update(JSON.stringify(manifest)).digest('hex'), files:manifest};
  } catch { return null; }
}
// Shared by the proof and its artifact lifecycle regression: mode is baked only on web.
export function buildInputHash(host, target, mode = '0', profile = process.env.EXACT_GAME_PROOF_PROFILE ?? 'gpu-dev') {
  const hash = createHash('sha256').update(host).update(target);
  if (host === 'web') hash.update(mode);
  if (host === 'linux') hash.update(profile);
  return hash;
}
export async function ensureBuildReceipt({receipt, inputs, artifact, build}) {
  let digest = artifact();
  const stamp = () => JSON.stringify({inputs, artifact:digest});
  if (digest && existsSync(receipt) && readFileSync(receipt, 'utf8') === stamp()) return false;
  await build();
  digest = artifact();
  if (!digest) throw new Error('build produced no complete proof artifact');
  writeFileSync(receipt, stamp());
  return true;
}
export function artifactDigest(host, dist, artifacts) {
  return artifactInventory(host, dist, artifacts)?.digest ?? null;
}

/** Existing proof entry point: world-only data captures; no embedded source is executed. */
export function captureTools({open, identity, host}) {
  const current = () => {
    const receipt = identity();
    if (!receipt?.digest || !Array.isArray(receipt.files) || !receipt.files.length) throw new Error('capture: loaded artifact identity unavailable; use a local authenticated proof build');
    return receipt;
  };
  return {
    async capture(session, target = 'world', {script = '', failure = '', external = false, limits} = {}) {
      if (external) throw new Error('capture refused: unsupported external dependency');
      const receipt = current();
      if (!session.loadedArtifact || session.loadedArtifact !== receipt.digest) throw new Error('capture: loaded versus current artifact differs; start a new isolated proof session');
      await session.world(target).capture('start', {build:receipt.digest, ...(limits ? {limits} : {})});
      return {
        async finish(path) {
          if (current().digest !== receipt.digest) throw new Error('capture: artifacts changed during recording; exact capture refused');
          const reply = await session.world(target).capture('stop');
          const state = await session.state(target, {world:true}), logs = await session.logs();
          const bundle = {format:'exact-game-capture-1', scope:'world-only', reproduction:'simulation', target,
            artifacts:receipt, metadata:{host, input:session.input?.delivery('key') ?? 'unavailable', clock:session.controlled ? 'controlled' : 'live', uiState:'omitted', externalResults:'unsupported', scene:state.world?.resources?.SceneIdentity?.digest ?? 'unavailable'},
            script:String(script).slice(0,16384), operations:session.captureOperations?.() ?? {unavailable:true}, observedFailure:String(failure).slice(0,16384),
            capture:reply.capture, data:reply.data, expected:{tick:reply.capture?.lastReliableTick, hash:reply.capture?.hash},
            logs:JSON.stringify(logs).slice(0,16384), inventory:['world checkpoint','normalized input and live bindings','artifact digests and relative artifact names','bounded logs','author supplied script description and failure; never executed']};
          if (typeof bundle.data !== 'string' || bundle.data.length > 17 * 1024 * 1024) throw new Error('capture payload missing or over limit');
          writeFileSync(resolve(path), JSON.stringify(bundle, null, 2) + '\n');
          return {path:resolve(path), ...reply.capture};
        },
      };
    },
    async replay(path, {through} = {}) {
      if (statSync(path).size > 20 * 1024 * 1024) throw new Error('capture file exceeds 20 MiB');
      const bundle = JSON.parse(readFileSync(path, 'utf8')), receipt = current();
      if (bundle.format !== 'exact-game-capture-1' || bundle.scope !== 'world-only' || bundle.reproduction !== 'simulation') throw new Error('unsupported capture bundle');
      if (bundle.artifacts?.digest !== receipt.digest || !equal(bundle.artifacts.files, receipt.files)) throw new Error('capture build differs from actual local artifact inventory; exact replay refused');
      if (bundle.capture?.complete !== true || bundle.capture?.incomplete) throw new Error('capture incomplete; replay refused');
      if (typeof bundle.data !== 'string' || !/^[0-9a-f]+$/.test(bundle.data) || bundle.data.length > 17 * 1024 * 1024) throw new Error('invalid capture payload');
      if (typeof bundle.target !== 'string' || bundle.target.length > 128) throw new Error('invalid capture world target');
      if (through !== undefined && (!Number.isSafeInteger(through) || through < 0 || through > bundle.capture.records)) throw new Error('invalid seek record boundary');
      const scratch = mkdtempSync(resolve(tmpdir(), 'exact-game-replay-'));
      let session;
      try {
        session = await open({env:{EXACT_SURFACE_STORE:scratch, XDG_DATA_HOME:scratch, XDG_CACHE_HOME:scratch}});
        if (session.loadedArtifact !== receipt.digest || current().digest !== receipt.digest) throw new Error('replay artifacts changed while opening isolated session');
        const reply = await session.world(bundle.target).capture('replay', {build:receipt.digest, data:bundle.data, ...(through !== undefined ? {through} : {})});
        if (through === undefined && (reply.replay?.world?.hash !== bundle.expected?.hash || reply.replay?.world?.tick !== bundle.expected?.tick)) throw new Error('replay final observation differs from capture');
        return {...reply, storage:'isolated scratch', metadata:bundle.metadata};
      } finally { try { await session?.close(); } finally { rmSync(scratch, {recursive:true, force:true}); } }
    },
  };
}

export async function closeSessions(monitor, record, sessions, check) {
  clearInterval(monitor);
  try { try { record(); } catch { /* Inventory is best-effort. */ } }
  finally {
    for (const session of sessions) {
      try { await session.close(); } catch (e) { check('session cleanup', false, e.message); }
    }
  }
}
export function equal(a, b) {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') return false;
  if (Array.isArray(a) || Array.isArray(b)) {
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length
      && a.every((value, i) => equal(value, b[i]));
  }
  const keys = Object.keys(a).sort(), other = Object.keys(b).sort();
  return keys.length === other.length
    && keys.every((key, i) => key === other[i] && equal(a[key], b[key]));
}

export const webUnavailable = log => /web carrier unavailable:[^\n]*: ENOENT;/.test(log);

// Compare observed pins, never the old expected values, before touching pins.json.
export function agreePins(rows, previous, hosts) {
  const modes = ['0', '1', 'fresh-game'];
  if (!hosts.includes('linux')) throw new Error('repin refused: linux continuous / Save / FreshGame are required');
  let reference;
  for (const host of hosts) for (const mode of modes) {
    const matches = rows.filter(row => row.host === host && row.mode === mode && row.profile !== 'release');
    if (matches.length !== 1 || matches[0].failures?.length) throw new Error(`repin refused: ${host} ${mode} missing or failed; inspect artifacts/prove and rerun --paranoid`);
    const row = matches[0], pins = row.pins;
    if (typeof row.game !== 'string' || !row.game.length) throw new Error(`repin refused: ${host} ${mode} has no game identity`);
    if ((previous.game && row.game !== previous.game) || (reference && row.game !== reference.game))
      throw new Error(`repin refused: ${host} ${mode} game identity disagrees`);
    if (!pins || !Object.keys(pins.ticks ?? {}).length || !Object.keys(pins.saves ?? {}).length)
      throw new Error(`repin refused: ${host} ${mode} has no tick/save observations`);
    for (const [section, pattern] of [['ticks', /^0x[0-9a-f]{16}$/], ['saves', /^[0-9a-f]{64}$/]]) {
      for (const key of Object.keys(previous[section] ?? {})) if (!(key in pins[section]))
        throw new Error(`repin refused: ${host} ${mode} did not observe ${section} ${key}`);
      for (const [key, value] of Object.entries(pins[section])) {
        if (!pattern.test(value)) throw new Error(`repin refused: ${host} ${mode} invalid ${section} ${key}: ${value}`);
        if (reference && reference.pins[section][key] !== value)
          throw new Error(`repin refused: ${host} ${mode} ${section} ${key}=${value} disagrees with ${reference.host} ${reference.mode}=${reference.pins[section][key]}; bun game/games/${row.name}/proof.mjs ${host} --paranoid`);
      }
      if (reference && !equal(Object.keys(pins[section]).sort(), Object.keys(reference.pins[section]).sort()))
        throw new Error(`repin refused: ${host} ${mode} ${section} inventory disagrees with ${reference.host} ${reference.mode}`);
    }
    reference ??= row;
  }
  const release = rows.filter(row => row.host === 'linux' && row.mode === '0' && row.profile === 'release');
  if (release.length !== 1 || release[0].failures?.length || release[0].game !== reference.game
      || !equal(release[0].pins, reference.pins))
    throw new Error('repin refused: linux release proof missing, failed, or disagrees with gpu-dev; pins.json unchanged');
  return {...reference.pins, game:reference.game, hosts};
}
export function pinRecorder(previous, name, check, collecting = false) {
  const pins = {ticks:{}, saves:{}};
  const record = (section, key, got) => {
    const expected = previous[section]?.[key];
    if (key in pins[section]) check(`pin ${key} repeated consistently`, pins[section][key] === got);
    pins[section][key] = got;
    if (!collecting && (expected !== undefined || Object.keys(previous.ticks ?? {}).length || Object.keys(previous.saves ?? {}).length)) check(
      expected === got ? `pin ${key}=${got}` : `pin ${key} differs (expected ${expected}, got ${got}); if the change is intended: bun game/prove.mjs ${name} --repin`, expected === got);
  };
  return {pins,
    pin(tick, state, key = String(tick)) {
      if (typeof key !== "string" || key.length < 1 || key.length > 256 || ["__proto__","constructor","prototype"].includes(key)) throw new Error("invalid pin key");
      check(`pin ${tick} sampled at expected tick (got ${state?.tick})`, state?.tick === tick && /^0x[0-9a-f]{16}$/.test(state?.hash));
      record('ticks', key, state?.hash);
    },
    pinSave(key, path) { record('saves', key, createHash('sha256').update(readFileSync(path)).digest('hex')); },
  };
}
// Gameplay assertions can succeed before a game's first baseline exists.
export function proofStatus({failures, expected, pins, collecting = false, partial = false}) {
  if (failures.length) return 'FAIL';
  if (collecting || partial || ['ticks', 'saves'].some(section =>
    !Object.keys(expected[section] ?? {}).length || !equal(expected[section], pins[section]))) return 'UNVERIFIED';
  return 'PASS';
}
// A report describes only recorded failures/stalls, never guesses from successful calls.
export function facilityReport(replies) {
  const success = (r, method) => r.method === method && r.reply != null && !r.error && !r.reply.error;
  const stalls = replies.filter(r => r.method === 'clock' && r.reply?.settled === false);
  const failures = replies.filter(r => r.error || r.reply?.error);
  const hints = [];
  const sameSession = (a,b) => a.session === b.session && replies.indexOf(a) >= replies.indexOf(b) && (a.clock == null || b.clock == null || a.clock >= b.clock);
  const stateUsed = failure => replies.some(r => sameSession(r,failure) && success(r, 'state') && !r.args?.[0]);
  const busyUsed = stalls.every(stall => replies.some(r => sameSession(r,stall) && success(r, 'state') && r.args?.[0]?.endsWith(':*') && r.args?.[3] === true));
  if (stalls.length) hints.push(`${stalls.length} stalls; state world:* busy exposes moving values and busy reasons${busyUsed ? '' : '; state unused'}`);
  const geometry = failures.filter(r => /\bis hidden\b|\bhidden (?:behind|\()|\bbehind (?:the )?camera\b|\boff screen\b|\bcovered or not hit\b|\bno screen box\b/.test(r.error ?? r.reply.error));
  if (geometry.some(f => !replies.some(r => sameSession(r,f) && success(r, 'layout') && f.args?.[0] && (r.args?.[0] === f.args[0] || r.args?.[0]?.endsWith(`:${f.args[0]}`))))) hints.push('layout unused; layout <id> (with the driver --json flag) shows visibility and available screen boxes');
  if (failures.some(r => /asset|save|restor/.test(r.error ?? r.reply.error) && !stateUsed(r))) hints.push('state unused; untargeted state exposes pending assets and restore errors');
  if (failures.some(f => !replies.some(r => sameSession(r,f) && success(r, 'logs')))) hints.push('logs unused; logs includes reload/carry refusals');
  if (!stalls.length && !failures.length) hints.push('no recorded stalls or refusals');
  return hints;
}

/// Whether a repository file is outside a game's deterministic build inputs:
/// other games, the bench and its probes, the twins, diaries, LLPs, apps, build
/// outputs. Remaining files are inputs, regardless of their name or extension.
export function proofInputExcluded(file, name, appPrefix = `game/games/${name}/`) {
  const gamePath = file.startsWith(appPrefix) ? `game/${file.slice(appPrefix.length)}` : file;
  return (/^(game\/(bench|twins|diaries|artifacts)\/|llp\/)/.test(file) && !file.startsWith(appPrefix))
    || (file.startsWith('game/games/') && !file.startsWith(appPrefix))
    || ['node_modules/', 'target/', '.shells/', '.scene/', 'dist/', 'dist.previous/', 'artifacts/'].some(output => file.startsWith(output) || file.startsWith('game/' + output) || file.startsWith(appPrefix + output))
    || /^(host\/web\/dist(?:\.previous)?|host\/apple\/\.build|game\/render\/target)\//.test(file)
    || (file.startsWith('apps/') && !file.startsWith(appPrefix))
    // Under the add-on, tests, examples, proof scripts and pins describe proofs, not bakes: editing them rebakes
    // nothing. Outside `game/` every tracked file under a source root counts (a Swift package reads anything).
    || (file.startsWith('game/') && (/\/(tests|examples)\//.test(gamePath) || /(^|\/)(pins\.json|proof\.mjs|.*\.test\.mjs)$/.test(gamePath)));
}
export function proofInputFiles(root, app) {
  const top = spawnSync('git', ['rev-parse','--show-toplevel'], {cwd:root, encoding:'utf8'});
  const repository = top.status === 0 ? top.stdout.trim() : root;
  const files = spawnSync('git', ['ls-files','-z','--cached','--others','--exclude-standard'], {cwd:repository, encoding:'utf8'});
  const walk = (dir, prefix = '') => readdirSync(dir, {withFileTypes:true}).flatMap(entry => {
    if (['.git','node_modules'].includes(entry.name) || (!prefix && ['target','.build','.shells','.scene','dist','dist.previous','artifacts'].includes(entry.name))) return [];
    const path = prefix + entry.name;
    return entry.isDirectory() ? walk(resolve(dir,entry.name), path + '/') : entry.isFile() ? [path] : [];
  });
  // Fleet exports have no Git index; external games are outside exact2's index.
  // Always include the app's own inputs, even when its defaults are gitignored.
  const sources = files.status === 0 ? files.stdout.split('\0').filter(Boolean) : walk(repository);
  sources.push(...walk(app).map(file => relative(repository, resolve(app, file))));
  const prefix = relative(repository, app) + '/';
  const contentInputs = new Set(sceneInputs(app).map(file => relative(repository, file)));
  sources.push(...contentInputs);
  return [...new Set(sources)].sort().filter(file =>
    (!proofInputExcluded(file, basename(app), prefix) || contentInputs.has(file)) && existsSync(resolve(repository, file))).map(file => relative(root, resolve(repository, file))).sort();
}
export async function paranoidRuns(run, restore = async () => 0, host = 'web') {
  let failed = false;
  for (const mode of ['0', '1', 'fresh-game']) {
    try { failed = (await run(mode)) !== 0 || failed; }
    catch (error) { console.error(error); failed = true; }
  }
  try { if (host === 'web') failed = (await restore()) !== 0 || failed; }
  catch (error) { console.error(error); failed = true; }
  return failed;
}

// Both whole-app state and targeted world snapshots are observations.
export function captureCommand(path) {
  return `bun ${/^[a-zA-Z0-9_./-]+$/.test(path) ? path : "'"+path.replaceAll("'", "'\\''")+"'"} web`;
}
export function worldObservations(observations, session, identities = new Set()) {
  return reply => {
    const worlds = Array.isArray(reply?.world) ? reply.world : [reply?.world ?? reply];
    for (const world of worlds) if (world?.hash && Number.isSafeInteger(world.tick)) {
      observations.set(session, {session, tick:world.tick, hash:world.hash});
      if (typeof world.game === 'string' && world.game.length) identities.add(world.game);
    }
  };
}

export async function proof(meta, script) {
  const app = fileURLToPath(new URL('.', meta.url)), name = basename(app);
  const root = fileURLToPath(new URL('..', import.meta.url));
  const host = process.argv.slice(2).find(arg => !arg.startsWith('--')) ?? 'linux', out = resolve(process.env.EXACT_PROOF_OUT ?? resolve(app, 'artifacts', host));
  const buildOut = resolve(app, 'artifacts');
  mkdirSync(buildOut, {recursive:true});
  const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(app, 'dist'));
  mkdirSync(out, {recursive:true});
  // Re-execute the actual proof, comparing every session's final simulation state.
  if (process.argv.includes('--paranoid')) {
    if (!['web', 'linux'].includes(host)) throw new Error('--paranoid supports web and linux');
    const failed = await paranoidRuns(async mode => {
      const started = performance.now();
      const child = spawn(process.execPath, [fileURLToPath(meta.url), host], {
        env:{...process.env, EXACT_GAME_PARANOID:mode, EXACT_GAME_PARANOID_COMPARE:'1'}, stdio:'inherit',
      });
      const code = await new Promise((ok, reject) => { child.on('exit', ok); child.on('error', reject); });
      console.log(`PARANOID ${name} ${host} ${mode}: ${((performance.now()-started)/1000).toFixed(3)} s (including build)`);
      return code;
    }, async () => {
      const child = spawn(process.execPath, [fileURLToPath(meta.url), host, '--build-only'], {
        env:{...process.env, EXACT_GAME_PARANOID:'0'}, stdio:'inherit',
      });
      return await new Promise((ok, reject) => { child.on('exit', ok); child.on('error', reject); });
    }, host);
    process.exit(failed ? 1 : 0);
  }
  const finalWorlds = [], observations = new Map(), gameIdentities = new Set();
  const previousPins = JSON.parse(readFileSync(resolve(app, 'pins.json'), 'utf8'));
  const collecting = process.env.EXACT_PROOF_REPIN === '1';
  const compareParanoid = process.env.EXACT_GAME_PARANOID_COMPARE === '1';
  Object.assign(process.env, {EXACT_APP_DIR:app, EXACT_WEB_DIST:dist,
    EXACT_UPDATE_TRUST:'development'});
  const started = performance.now(), failures = [], transcript = [], replies = [], sessions = new Set();
  const say = line => { transcript.push(line); console.log(line); };
  const check = (label, ok, value) => {
    say(`${ok ? 'PASS' : 'FAIL'} ${label}${value === undefined ? '' : ': ' + JSON.stringify(value)}`);
    if (!ok) failures.push(label);
    return ok;
  };
  const {pins, pin, pinSave} = pinRecorder(previousPins, app.startsWith(resolve(root,"game/games") + "/") ? name : app, check, collecting);
  // The GPU-less host has no process tree to discover: retain the process
  // handles from the carrier and await them. Global ps can block indefinitely
  // on this Mac; an optional web descendant audit is bounded and never delays
  // headless gameplay verification.
  const children = [], recorded = new Map();
  const released = new Set(), parents = new Map();
  const onProcess = child => {
    children.push(child); recorded.set(child.pid, 'carrier');
    return () => {
      // Called only after the complete native detach ACK, including all sessions.
      released.add(child.pid);
      for (let changed = true; changed;) {
        changed = false;
        for (const [pid,parent] of parents) if (released.has(parent) && !released.has(pid)) { released.add(pid); changed = true; }
      }
      for (const pid of released) recorded.delete(pid);
      const index = children.indexOf(child); if (index >= 0) children.splice(index, 1);
      say(`CARRIER detached after acknowledgement: ${child.pid}`);
    };
  };
  let auditUnavailable = false, inventoryPending;
  const inventory = () => new Promise(resolve => {
    const child = spawn('ps', ['-axo', 'pid=,ppid=,stat=,lstart=,comm='], {stdio:['ignore','pipe','ignore']});
    let output = '', done = false;
    const finish = rows => { if (done) return; done = true; clearTimeout(timer); resolve(rows); };
    const timer = setTimeout(() => {
      auditUnavailable = true;
      child.kill('SIGKILL'); // This invocation's recorded ps, never a name/pattern.
      child.stdout.destroy(); child.unref(); finish(null);
    }, 200);
    child.stdout.on('data', data => output += data);
    child.on('error', () => { auditUnavailable = true; finish(null); });
    // A zombie is dead: killed with its group, not yet reaped by launchd.
    child.on('exit', code => finish(code === 0 ? output.trim().split('\n').map(parseInventoryLine).filter(Boolean) : null));
  });
  const sample = () => {
    if (host === 'linux' || host === 'ios' || auditUnavailable || inventoryPending) return;
    inventoryPending = inventory().then(rows => {
      const owned = new Set([process.pid, ...children.map(child => child.pid)]);
      for (let changed = true; changed;) {
        changed = false;
        for (const row of rows ?? []) if (owned.has(row.parent) && !owned.has(row.pid) && !released.has(row.pid)) {
          parents.set(row.pid, row.parent);
          owned.add(row.pid); recorded.set(row.pid, row.stamp); changed = true;
        }
      }
    }).finally(() => { inventoryPending = null; });
  };
  const monitor = setInterval(sample, 100);
  let loadedIdentity = () => null;
  let reusableWeb, reusableOptions;
  const open = async (options = {}) => {
    const launchReceipt = options.url ? null : loadedIdentity();
    const signature = JSON.stringify(options);
    if ((options.fresh || options.world || options.plan || signature !== reusableOptions) && reusableWeb) { await reusableWeb.close(); reusableWeb = null; }
    const reuse = host === 'web' && !options.world && !options.plan ? reusableWeb : null;
    reusableWeb = null;
    if (reuse) say('CARRIER reused web process; fresh document');
    const raw = await openSession({host, app, size:[1280,720], webDist:dist, onProcess, reuse, ...options,
      env:{EXACT_GAME_PARANOID:process.env.EXACT_GAME_PARANOID ?? '0', ...options.env}});
    sample();
    if (launchReceipt && loadedIdentity()?.digest !== launchReceipt.digest) { await raw.close(); throw new Error('artifacts changed during proof session launch'); }
    raw.loadedArtifact = launchReceipt?.digest ?? null;
    let closed = false;
    const close = async () => { if (!closed) {
      try {
        if (compareParanoid || process.env.EXACT_PROOF_COMPARE === '1') {
          const state = await raw.op({op:'state', ...await raw.target('world'), world:true});
          const world = state.world;
          const logs = (await raw.logs()).world ?? [];
          finalWorlds.push({session:id, tick:world?.tick, hash:world?.hash,
            published:world?.published,
            // Native hosts own incremental journal cursors, even for since:0.
            // Include the chunks this script already read as well as the tail.
            journal:[...replies.filter(r => r.session === id && r.method === 'logs')
              .flatMap(r => r.reply?.world ?? []), ...logs]
              .map(({from, next, lines, tick}) => Object.fromEntries(
                Object.entries({from, next, lines, tick}).filter(([,value]) => value !== undefined)))});
          if (!world?.hash) throw new Error('paranoid comparison: final world hash missing');
        }
        sample();
      } finally {
        if (host === 'web' && !options.fresh && !options.world && !options.plan && !reusableWeb) { reusableWeb = raw.carrier; reusableOptions = signature; }
        else await raw.close();
        closed = true;
      }
    } };
    sessions.add({close});
    const id = sessions.size;
    raw.captureOperations = () => {
      const steps = replies.filter(r => r.session === id && ['tap','type','clock'].includes(r.method)).slice(-1024);
      const json = JSON.stringify(steps);
      return json.length <= 65536 ? steps : {unavailable:'script transcript exceeds 64 KiB; normalized engine records remain authoritative'};
    };
    return new Proxy(raw, {get(target, method) {
      if (method === 'close') return close;
      // world(name) runs with the proxy as its receiver: its operations stay recorded.
      if (!['tap','type','clock','state','tree','layout','logs','screenshot'].includes(method)) return target[method];
      return async (...args) => {
        try {
          const reply = await target[method](...args);
          if (method === 'state') worldObservations(observations, id, gameIdentities)(reply);
          replies.push({session:id, method, args, reply, clock:target.now});
          say(`${method} ${args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ')}\n${render(method,reply)}`);
          return reply;
        } catch (error) {
          replies.push({session:id, method, args, clock:target.now, error:error.message, steps:error.steps}); if (error.steps) say(render('type', {steps:error.steps})); throw error;
        }
      };
    }});
  };
  try {
    if (!['web','macos','ios','linux'].includes(host)) throw new Error(`proof host unavailable: ${host}`);
    const hash = buildInputHash(host, resolveApp(app).target, process.env.EXACT_GAME_PARANOID ?? '0');
    for (const file of proofInputFiles(root, app)) {
      hash.update(file).update(readFileSync(resolve(root,file)));
    }
    const digest = hash.digest('hex'), receipt = resolve(buildOut, `build-${host}.sha256`);
    const appInfo = resolveApp(app);
    const linuxTarget = host === 'linux' ? spawnSync('rustc', ['-vV'], {encoding:'utf8'}).stdout.match(/^host: (.+)$/m)?.[1] : null;
    const profile = process.env.EXACT_GAME_PROOF_PROFILE ?? 'gpu-dev';
    if (!['gpu-dev', 'release'].includes(profile)) throw new Error('EXACT_GAME_PROOF_PROFILE must be gpu-dev or release');
    const artifacts = host === 'linux' ? {binary:resolve(appInfo.target, linuxTarget, `${profile}/${appInfo.crate('linux')}`), module:resolve(appInfo.target, linuxTarget, `${profile}/lib${appInfo.crate('gpu').replaceAll('-','_')}.${process.platform === 'darwin' ? 'dylib' : 'so'}`)} : host === 'web' ? null : appleArtifacts(appInfo, {destination:host === 'macos' ? 'macos' : 'ios-simulator'});
    if (host === 'linux') process.env.EXACT_LINUX_BIN = artifacts.binary;
    const built = await ensureBuildReceipt({receipt, inputs:digest,
      artifact:() => artifactDigest(host, dist, artifacts), build:async () => {
      const disk = spawnSync('df', ['-Pk', root], {encoding:'utf8'});
      const availableKiB = Number(disk.stdout.trim().split('\n').at(-1).trim().split(/\s+/)[3]);
      if (disk.status !== 0 || !Number.isFinite(availableKiB) || availableKiB < 25 * 1024 * 1024) throw new Error('build refused: less than 25 GiB available');
      say(`BUILD stale or missing receipt ${receipt}; rebuilding: bun ${fileURLToPath(meta.url)} ${host} --build-only`);
      if (host === 'linux') {
        if (!linuxTarget) throw new Error('rustc did not report its target');
        // Native mode is read at launch; keep compile-time environment stable.
        buildBake(appInfo, 'linux', linuxTarget, {profile, env:{EXACT_GAME_PARANOID:'0'}});
      } else {
        const child = spawn('bun', [resolve(root,host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios'] : host === 'macos' ? ['--bundle'] : [])], {cwd:root, env:process.env, stdio:'inherit'});
        sample();
        const code = await new Promise((ok, reject) => {child.on('exit',ok); child.on('error',reject);});
        if (code !== 0) throw new Error(`app build exited ${code}`);
      }
    }});
    if (!built) say(`BUILD cached ${name} ${host}`);
    loadedIdentity = () => artifactInventory(host, dist, artifacts);
    if (!process.argv.includes('--build-only')) {
      await script({open, check, equal, out, host, say, pin, pinSave, ...captureTools({open, identity:loadedIdentity, host})});
      if (!process.argv.some(arg => ['--screenshot-only','--capture40'].includes(arg)))
        for (const section of ['ticks','saves']) for (const key of Object.keys(previousPins[section] ?? {}))
          check(`pin ${key} observed; if intentionally removed, update the proof and pins.json together`, key in pins[section]);
    }
  } catch (error) { check('proof interrupted',false,error.stack ?? String(error)); }
  finally {
    await closeSessions(monitor, sample, sessions, check);
    if (reusableWeb) await reusableWeb.close();
    closeFilesystemReader(); // The static server's resident reader is this process's child.
    await inventoryPending;
    let remaining = children.filter(child => child.exitCode === null && child.signalCode === null)
      .map(child => ({pid:child.pid}));
    if (host !== 'linux' && host !== 'ios' && !auditUnavailable) {
      // A killed process group reaps its helpers a few milliseconds after the
      // carrier's exit event: recorded descendants (pid and start stamp) get a
      // short grace, then any survivor is a leak and fails the proof by name.
      // Nothing here signals a discovered pid — only handles owned at spawn are killed.
      const stale = async () => (await inventory())?.filter(row => recorded.get(row.pid) === row.stamp) ?? [];
      let rows = await stale();
      for (const deadline = Date.now() + 2000; rows.length && Date.now() < deadline; rows = await stale()) await new Promise(r => setTimeout(r, 100));
      remaining.push(...rows);
    }
    if (auditUnavailable) say('SKIP descendant process audit: ps stalled; carrier close still awaited every recorded host process.');
    check('all recorded children exited', remaining.length === 0, remaining);
    if (compareParanoid) {
      finalWorlds.sort((a,b) => a.session - b.session);
      const baseline = resolve(out, `paranoid-${host}-normal.json`);
      writeFileSync(resolve(out, `paranoid-${host}-${process.env.EXACT_GAME_PARANOID}.json`), JSON.stringify(finalWorlds));
      if (process.env.EXACT_GAME_PARANOID === '0') writeFileSync(baseline, JSON.stringify(finalWorlds));
      else {
        const matches = equal(finalWorlds, JSON.parse(readFileSync(baseline, 'utf8')));
        const mode = process.env.EXACT_GAME_PARANOID === '1' ? 'Save' : 'FreshGame';
        check(matches ? `paranoid ${mode} matches continuous state` : `paranoid ${mode} differs at ${finalWorlds.map(w => `session ${w.session} tick ${w.tick}`).join(', ')}; bun game/games/${name}/proof.mjs ${host} --paranoid`,
          matches, finalWorlds.map(({session, tick, hash}) => ({session, tick, hash})));
      }
    }
    writeFileSync(resolve(out,'process-cleanup.json'),JSON.stringify({recorded:[...recorded],remaining,auditUnavailable},null,2)+'\n');
    const saves = [...new Set(replies.filter(r => r.method === 'screenshot' && r.args[2] === 'save' && !r.error).map(r => r.args[0]))].sort().map(path => {
      const name = basename(path), bytes = readFileSync(path);
      return {name, bytes:bytes.length, sha256:createHash('sha256').update(bytes).digest('hex')};
    });
    const partial = process.argv.some(arg => ['--build-only','--screenshot-only','--capture40'].includes(arg));
    const status = proofStatus({failures, expected:previousPins, pins, collecting, partial});
    if (!compareParanoid && process.env.EXACT_PROOF_COMPARE !== '1') finalWorlds.push(...observations.values());
    writeFileSync(resolve(out,'summary.json'), JSON.stringify({name, game:gameIdentities.size === 1 ? [...gameIdentities][0] : null, host, status, mode:process.env.EXACT_GAME_PARANOID ?? '0', pins, facilities:facilityReport(replies), failures, seconds:(performance.now()-started)/1000, worlds:finalWorlds, saves, auditUnavailable}, null, 2)+'\n');
    if (process.argv.includes('--report')) for (const hint of facilityReport(replies)) say(`REPORT ${hint}`);
    say(`PROOF ${status} ${name} ${host}: ${failures.length} failures; ${((performance.now()-started)/1000).toFixed(3)} s`);
    if (status === 'PASS' && host === 'linux') say(`Capture the PNG: ${captureCommand(relative(root, fileURLToPath(meta.url)))}`);
    if (status === 'UNVERIFIED' && !collecting && !partial)
      say(`UNVERIFIED: no pins — run bun game/prove.mjs '${app.replaceAll("'", "'\\''")}'`);
    writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
    writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
  }
  process.exit(failures.length || (!collecting && !process.argv.some(arg => ['--build-only','--screenshot-only','--capture40'].includes(arg)) && proofStatus({failures, expected:previousPins, pins}) !== 'PASS') ? 1 : 0);
}
