// Shared lifecycle for game proofs: operations and assertions stay in the game.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { open as openSession, render } from '../scripts/agent.mjs';
import { cdpFailureContext, gameNonInput } from '../scripts/agent-launch.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { buildBake, executableName, resolveApp } from '../scripts/app.mjs';
import { closeFilesystemReader } from '../scripts/filesystem.mjs';

/** Some runtime-created stacks omit the informative Error message. */
export function formatProofError(error) {
  const message = String(error), stack = error?.stack;
  const rendered = typeof stack === 'string' && stack.length
    ? stack.includes(message) ? stack : `${message}\n${stack}`
    : message;
  const context = cdpFailureContext(error);
  return context ? `${rendered}\nCDP ${JSON.stringify(context)}` : rendered;
}

/** The failed operation's report row; never serialize the Error or its handles. */
export function proofFailureRow(session, method, args, clock, error) {
  const context = cdpFailureContext(error);
  return {session, method, args, clock, error:error.message, steps:error.steps, ...(context ? {cdp:context} : {})};
}

// Offline diagnostics over existing state reads (LLP 1012; LLP 1046.001 D2/D5).
// These are inspection captures, not EXSIM saves or a second simulation codec.
export async function captureWorld(session, name = 'world') {
  const snapshot = await session.world(name).snapshot({all:true});
  const {world} = await session.op({op:'state', ...await session.target(name), world:true});
  if (!world || snapshot.tick !== world.tick || snapshot.hash !== world.hash)
    throw new Error('world changed during capture; capture on the agent clock with no concurrent drive');
  if (world.entities !== snapshot.entities?.length)
    throw new Error('incomplete world capture; entity count differs');
  const capture = {format:'exact-world-state-v1', name:world.name,
    tick:world.tick, hash:world.hash, truncated:snapshot.truncated,
    entities:snapshot.entities, resources:world.resources,
    simulation:{hz:world.hz, seed:world.seed, args:world.args,
      input:world.input, published:world.published}};
  validateWorldCapture(capture);
  return capture;
}

function validateWorldCapture(capture) {
  const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  if (capture?.format !== 'exact-world-state-v1' || typeof capture.name !== 'string'
      || !Number.isSafeInteger(capture.tick) || capture.tick < 0
      || typeof capture.hash !== 'string' || !capture.hash.length
      || capture.truncated !== false || !Array.isArray(capture.entities)
      || !record(capture.resources) || !record(capture.simulation))
    throw new Error('expected a complete exact-world-state-v1 capture (truncated captures are refused)');
  const names = new Set(), ids = new Set();
  for (const entity of capture.entities) {
    if (!record(entity) || !Number.isSafeInteger(entity.id) || entity.id < 0
        || !(entity.name === null || typeof entity.name === 'string') || !record(entity.components)
        || ids.has(entity.id) || (entity.name !== null && names.has(entity.name)))
      throw new Error('invalid or duplicate entity identity in world capture');
    ids.add(entity.id);
    if (entity.name !== null) names.add(entity.name);
  }
  const json = (value, depth = 0) => {
    if (depth > 128) throw new Error('world capture exceeds 128 levels');
    if (value === null || typeof value === 'string' || typeof value === 'boolean') return;
    if (typeof value === 'number' && Number.isFinite(value)) return;
    if (Array.isArray(value) || record(value)) {
      for (const child of Object.values(value)) json(child, depth + 1);
      return;
    }
    throw new Error('world capture must contain finite JSON values');
  };
  json(capture);
}

/** Exact JSON-value comparison, with stable entity names and positional array indices. */
/** The accessible names the platform exposes (LLP 1080.002 `tree --ax`), for
 * a proof's name checks: `{unavailable: true}` where the host exposes no
 * accessibility tree (Linux), else `name(testId)` (the view's own element first)
 * and `all`, every element's name. A check reads `ax.unavailable || ax.name(id) === X`. */
export async function axNames(session) {
  const {ax} = await session.tree(null, {ax: true});
  if (ax?.unavailable) return {unavailable: true, name: () => undefined, all: []};
  const elements = ax?.elements ?? [];
  const of = id => elements.find(e => e.testId === id && e.via === 'self') ?? elements.find(e => e.testId === id);
  return {unavailable: false, name: id => of(id)?.name, frame: id => of(id)?.frame, all: elements.map(e => e.name)};
}

export function diffWorlds(before, after, {limit = 100} = {}) {
  validateWorldCapture(before); validateWorldCapture(after);
  if (before.name !== after.name) throw new Error('cannot compare captures of different games');
  if (!Number.isSafeInteger(limit) || limit < 1) throw new Error('diff limit must be a positive integer');
  const changes = [];
  let total = 0;
  const missing = Symbol('missing');
  const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  const pathKey = key => /^[A-Za-z_][A-Za-z_0-9]*$/.test(key) ? `.${key}` : `[${JSON.stringify(key)}]`;
  const walk = (a, b, path) => {
    if (Object.is(a, b)) return;
    if (Array.isArray(a) && Array.isArray(b)) {
      for (let i = 0; i < Math.max(a.length, b.length); i++)
        walk(i < a.length ? a[i] : missing, i < b.length ? b[i] : missing, `${path}[${i}]`);
      return;
    }
    if (record(a) && record(b)) {
      for (const key of [...new Set([...Object.keys(a), ...Object.keys(b)])].sort())
        walk(Object.hasOwn(a, key) ? a[key] : missing, Object.hasOwn(b, key) ? b[key] : missing, path + pathKey(key));
      return;
    }
    total++;
    if (changes.length < limit) changes.push({path,
      kind:a === missing ? 'added' : b === missing ? 'removed' : 'changed',
      ...(a === missing ? {} : {before:a}), ...(b === missing ? {} : {after:b})});
  };
  const entities = capture => new Map(capture.entities.map(e => [
    e.name === null ? `entities[#${e.id}]` : `entities[${JSON.stringify(e.name)}]`, e.components]));
  const a = entities(before), b = entities(after);
  for (const key of [...new Set([...a.keys(), ...b.keys()])].sort())
    walk(a.has(key) ? a.get(key) : missing, b.has(key) ? b.get(key) : missing, key);
  walk(before.resources, after.resources, 'resources');
  walk(before.simulation, after.simulation, 'simulation');
  return {name:before.name, before:{tick:before.tick, hash:before.hash},
    after:{tick:after.tick, hash:after.hash}, hashChanged:before.hash !== after.hash,
    total, omitted:total - changes.length, changes};
}

export function formatWorldDiff(diff) {
  const lines = [`${diff.name}: tick ${diff.before.tick} → ${diff.after.tick}; hash ${diff.before.hash} → ${diff.after.hash}`];
  for (const change of diff.changes) {
    const value = key => Object.hasOwn(change, key) ? JSON.stringify(change[key]) : '<absent>';
    lines.push(`${change.kind} ${change.path}: ${value('before')} → ${value('after')}`);
  }
  lines.push(`${diff.total} inspected difference(s)${diff.omitted ? `; ${diff.omitted} omitted` : ''}`);
  if (diff.hashChanged && !diff.total)
    lines.push('World hashes differ despite equal inspected values; inspection is not a complete binary save comparison.');
  return lines.join('\n');
}

if (import.meta.main) {
  try {
    const [command, before, after, ...extra] = process.argv.slice(2);
    if (command !== 'diff' || !before || !after || extra.length)
      throw new Error('Usage: bun game/proof.mjs diff before.json after.json');
    const diff = diffWorlds(JSON.parse(readFileSync(before, 'utf8')), JSON.parse(readFileSync(after, 'utf8')));
    console.log(formatWorldDiff(diff));
    process.exitCode = diff.total || diff.hashChanged ? 1 : 0;
  } catch (error) {
    console.error(`world diff refused: ${error.message}`);
    process.exitCode = 2;
  }
}

export function parseInventoryLine(line) {
  const m = line.trim().match(/^(\d+)\s+(\d+)\s+(\S+)\s+(.{24})\s+(.+)$/);
  return m && !m[3].startsWith('Z') ? {pid:Number(m[1]), parent:Number(m[2]), stamp:m[4], command:m[5]} : null;
}

export function artifactDigest(host, dist, artifacts) {
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
    } else if (host === 'windows') {
      readFileSync(artifacts.binary); readFileSync(artifacts.module);
      walk(dirname(artifacts.binary), 'product');
    } else if (host === 'linux') {
      for (const path of [artifacts.binary, artifacts.module]) manifest.push([basename(path), createHash('sha256').update(readFileSync(path)).digest('hex')]);
    } else {
      const executable = resolve(artifacts.bundle, host === 'macos' ? 'Contents/MacOS' : '.', artifacts.executable);
      readFileSync(executable);
      walk(artifacts.bundle, 'bundle');
      // The driver launches the standalone product on macOS, loading its adjacent dylibs/assets.
      if (artifacts.binary) {
        readFileSync(artifacts.binary); // A missing actual carrier always invalidates the receipt.
        walk(artifacts.products ?? resolve(artifacts.binary, '..'), 'product');
      }
    }
    return createHash('sha256').update(JSON.stringify(manifest)).digest('hex');
  } catch { return null; }
}
// Shared by the proof and its artifact lifecycle regression: mode is baked only on web.
export function buildInputHash(host, target, mode = '0', profile = process.env.EXACT_GAME_PROOF_PROFILE ?? 'gpu-dev') {
  const hash = createHash('sha256').update(host).update(target);
  if (host === 'web') hash.update(mode);
  if (host === 'linux' || host === 'windows') hash.update(profile);
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

// Both native carriers exercise the same engine Save/FreshGame protocol. Keep
// Linux as the canonical lane whenever requested, including mixed host runs.
export const nativeProofHost = hosts => hosts.includes('linux') ? 'linux' : hosts.includes('windows') ? 'windows' : null;

// Compare observed pins, never the old expected values, before touching pins.json.
export function agreePins(rows, previous, hosts, app) {
  const modes = ['0', '1', 'fresh-game'];
  const native = nativeProofHost(hosts);
  if (!native) throw new Error('repin refused: linux or windows continuous / Save / FreshGame are required');
  let reference;
  for (const host of hosts) for (const mode of modes) {
    const matches = rows.filter(row => row.host === host && row.mode === mode && row.profile !== 'release');
    if (matches.length !== 1 || matches[0].failures?.length) throw new Error(`repin refused: ${host} ${mode} missing or failed; inspect artifacts/prove and rerun --paranoid`);
    const row = matches[0], pins = row.pins;
    if (!pins || !Object.keys(pins.ticks ?? {}).length || !Object.keys(pins.saves ?? {}).length)
      throw new Error(`repin refused: ${host} ${mode} has no tick/save observations`);
    for (const [section, pattern] of [['ticks', /^0x[0-9a-f]{16}$/], ['saves', /^[0-9a-f]{64}$/]]) {
      for (const key of Object.keys(previous[section] ?? {})) if (!(key in pins[section]))
        throw new Error(`repin refused: ${host} ${mode} did not observe ${section} ${key}`);
      for (const [key, value] of Object.entries(pins[section])) {
        if (!pattern.test(value)) throw new Error(`repin refused: ${host} ${mode} invalid ${section} ${key}: ${value}`);
        if (reference && reference.pins[section][key] !== value)
          throw new Error(`repin refused: ${host} ${mode} ${section} ${key}=${value} disagrees with ${reference.host} ${reference.mode}=${reference.pins[section][key]}; ${proofCommand(resolve(app, 'proof.mjs'), host, '--paranoid')}`);
      }
      if (reference && !equal(Object.keys(pins[section]).sort(), Object.keys(reference.pins[section]).sort()))
        throw new Error(`repin refused: ${host} ${mode} ${section} inventory disagrees with ${reference.host} ${reference.mode}`);
    }
    reference ??= row;
  }
  const release = rows.filter(row => row.host === native && row.mode === '0' && row.profile === 'release');
  if (release.length !== 1 || release[0].failures?.length || !equal(release[0].pins, reference.pins))
    throw new Error(`repin refused: ${native} release proof missing, failed, or disagrees with gpu-dev; pins.json unchanged`);
  return {...reference.pins, hosts};
}
export function pinInputs(rows) {
  const inputs=rows[0]?.inputs;
  if(!/^[a-f0-9]{64}$/.test(inputs ?? '') || rows.some(row=>row.inputs!==inputs))
    throw new Error('repin refused: proof inputs missing or changed between runs; pins.json unchanged');
  return inputs;
}
/** Provenance without a commit is valid only for an unborn or non-Git game. */
export function pinRevision(app, inputs) {
  const git = args => spawnSync('git', args, {cwd:app, encoding:'utf8', env:{...process.env, LC_ALL:'C'}});
  const refuse = result => { throw new Error(`repin refused: git provenance failed: ${result.error?.message ?? result.stderr?.trim() ?? result.status}; pins.json unchanged`); };
  const inside = git(['rev-parse', '--is-inside-work-tree']);
  if (inside.status !== 0) {
    let repository = false;
    for (let dir = resolve(app);; dir = dirname(dir)) {
      if (existsSync(resolve(dir, '.git'))) { repository = true; break; }
      if (dirname(dir) === dir) break;
    }
    if (!repository && inside.status === 128 && /not a git repository/.test(inside.stderr)) return `inputs:${inputs}`;
    refuse(inside);
  }
  const revision = git(['rev-parse', '--verify', '--quiet', 'HEAD^{commit}']);
  if (revision.status === 0) return revision.stdout.trim();
  const branch = git(['symbolic-ref', '-q', 'HEAD']);
  if (revision.status === 1 && branch.status === 0) {
    const missing = git(['show-ref', '--exists', branch.stdout.trim()]);
    if (missing.status === 2) return `inputs:${inputs}`;
  }
  refuse(revision);
}
export function pinRecorder(previous, app, check, collecting = false) {
  const pins = {ticks:{}, saves:{}};
  const record = (section, key, got) => {
    const expected = previous[section]?.[key];
    if (key in pins[section]) check(`pin ${key} repeated consistently`, pins[section][key] === got);
    pins[section][key] = got;
    if (!collecting && (expected !== undefined || Object.keys(previous.ticks ?? {}).length || Object.keys(previous.saves ?? {}).length)) check(
      expected === got ? `pin ${key}=${got}` : `pin ${key} differs (expected ${expected}, got ${got}); if the change is intended: ${proofCommand(resolve(import.meta.dir, 'prove.mjs'), app, '--repin')}`, expected === got);
  };
  return {pins,
    pin(tick, state) {
      check(`pin ${tick} sampled at expected tick (got ${state?.tick})`, state?.tick === tick && /^0x[0-9a-f]{16}$/.test(state?.hash));
      record('ticks', String(tick), state?.hash);
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
/// outputs. Source extensions, app assets, and Apple module inputs are admitted.
/// A game's own scripts (bench.mjs, a proxy, a probe) are tools, not bake inputs:
/// no bake reads a .mjs/.js outside its logic, data, gpu, render and asset folders.
export function proofInputExcluded(file, name, appPrefix = `game/games/${name}/`) {
  return /^(issues|\.claude)\//.test(file)
    || /(^|\/)(pins\.json|proof\.mjs|.*\.test\.mjs|.*\.md)$/.test(file)
    || (file.startsWith(appPrefix) && gameNonInput(file.slice(appPrefix.length)))
    || (!/\.(rs|toml|lock|contract|ts|js|mjs|wgsl|json|swift|h|c|html|css|modulemap)$/.test(file)
      && !(file.startsWith('host/apple/') && !basename(file).includes('.'))
      && !['art/', 'assets/', 'deck/'].some(dir => file.startsWith(appPrefix + dir)))
    || (/^(game\/(bench|twins|diaries|artifacts)\/|llp\/)/.test(file) && !file.startsWith(appPrefix))
    || (file.startsWith('game/games/') && !file.startsWith(appPrefix))
    || ['node_modules/', 'target/', '.shells/', 'dist/', 'dist.previous/', 'dist-windows/', 'artifacts/'].some(output => file.startsWith(output) || file.startsWith('game/' + output) || file.startsWith(appPrefix + output))
    || /^(host\/web\/dist(?:\.previous)?|host\/apple\/\.build|game\/render\/target)\//.test(file)
    || (file.startsWith('apps/') && !file.startsWith(appPrefix))
    || (file.startsWith('game/') && /\/(tests|examples)\//.test(file));
}
export function proofInputFiles(root, app, inventory) {
  const top = inventory ? {status:0, stdout:root} : spawnSync('git', ['rev-parse','--show-toplevel'], {cwd:root, encoding:'utf8'});
  const repository = top.status === 0 ? top.stdout.trim() : root;
  const files = inventory ?? spawnSync('git', ['ls-files','-z','--cached','--others','--exclude-standard'], {cwd:repository, encoding:'utf8'});
  const walk = (dir, prefix = '') => readdirSync(dir, {withFileTypes:true}).flatMap(entry => {
    if (['.git','node_modules'].includes(entry.name) || (!prefix && ['target','.build','.shells','dist','dist.previous','dist-windows','artifacts'].includes(entry.name))) return [];
    const path = prefix + entry.name;
    return entry.isDirectory() ? walk(resolve(dir,entry.name), path + '/') : entry.isFile() ? [path] : [];
  });
  // Fleet exports have no Git index; external games are outside exact2's index.
  // Always include the app's own inputs, even when its defaults are gitignored.
  const sources = files.status === 0 ? files.stdout.split('\0').filter(Boolean) : walk(repository);
  sources.push(...walk(app).map(file => relative(repository, resolve(app, file)).replaceAll('\\', '/')));
  const prefix = relative(repository, app).replaceAll('\\', '/') + '/';
  return [...new Set(sources)].sort().filter(file =>
    !proofInputExcluded(file, basename(app), prefix) && existsSync(resolve(repository, file))).map(file => relative(root, resolve(repository, file)).replaceAll('\\', '/')).sort();
}
// Clean tracked bytes are identified by Git blobs. Dirty/ignored app inputs and
// products reuse their digest only while their full stat tuple is unchanged.
const statKey = path => { const s=statSync(path); return [s.dev,s.ino,s.size,s.mtimeMs,s.ctimeMs].join(':'); };
export function proofInputs(root, app, cachePath) {
  const git = args => spawnSync('git',args,{cwd:root,encoding:'utf8',maxBuffer:32*1024*1024});
  const listed=git(['ls-files','-s','-z','--cached','--others','--exclude-standard']);
  const status=git(['status','--porcelain=v1','-z','--untracked-files=no','--no-renames']);
  const blobs=new Map(), paths=[];
  for(const row of listed.stdout?.split('\0').filter(Boolean) ?? []) {
    const match=/^\d+ ([a-f0-9]+) 0\t(.*)$/s.exec(row);
    const path=match ? match[2] : row; paths.push(path);
    if(match) blobs.set(path,match[1]);
  }
  const dirty=new Set((status.stdout??'').split('\0').filter(row=>row && row[1]!==' ').map(row=>row.slice(3)));
  let cache={}; try {cache=JSON.parse(readFileSync(cachePath,'utf8'));} catch { /* First proof. */ }
  const next={}, rows=[], prefix=relative(root,app).replaceAll('\\','/')+'/', groups={all:[],gpu:[],host:[]}; let reads=0;
  const files=proofInputFiles(root,app,listed.status===0 ? {status:0,stdout:paths.join('\0')} : undefined);
  for(const file of files) {
    const path=resolve(root,file), key=statKey(path); let digest=blobs.get(file);
    if(!digest || status.status!==0 || dirty.has(file)) {
      digest=cache[file]?.key===key ? cache[file].digest : null;
      if(!digest) {const bytes=readFileSync(path); reads++; digest=createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');}
      next[file]={key,digest};
    }
    rows.push([file,digest]);
  }
  for(const row of rows) {
    const [file]=row; groups.all.push(row);
    // Host bake depends on the declaration, not the game's implementation.
    // Shared compiler crates remain conservative inputs of both graphs.
    const own=file.startsWith(prefix) ? file.slice(prefix.length) : null;
    const gpuOnly=own!==null ? /^(logic|gpu|render|art|assets|deck)\//.test(own) : /^game\/(engine|render|physics|audio|bake)\//.test(file);
    const hostOnly=own!==null ? /\.contract$/.test(own) : /^host\//.test(file);
    if(!hostOnly) groups.gpu.push(row);
    if(!gpuOnly) groups.host.push(row);
  }
  if(cachePath) {mkdirSync(resolve(cachePath,'..'),{recursive:true});writeFileSync(cachePath,JSON.stringify(next));}
  return {...Object.fromEntries(Object.entries(groups).map(([kind,rows])=>[kind,createHash('sha256').update(JSON.stringify(rows)).digest('hex')])),reads};
}
export async function ensureLinuxReceipts({directory,gpuInputs,hostInputs,gpuArtifact,hostArtifact,build,completeGpu = () => {}}) {
  const gpu=await ensureBuildReceipt({receipt:resolve(directory,'build-linux-gpu.sha256'),inputs:gpuInputs,artifact:gpuArtifact,build:()=>build('gpu')});
  completeGpu();
  // The GPU build may have changed surfaces.json; observe it only afterwards.
  const host=await ensureBuildReceipt({receipt:resolve(directory,'build-linux-host.sha256'),inputs:hostInputs(),artifact:hostArtifact,build:()=>build('host')});
  return gpu || host;
}
export function productDigest(path, cachePath) {
  try {
    const key=statKey(path); let cached;
    try {cached=JSON.parse(readFileSync(cachePath,'utf8'));} catch { /* First product. */ }
    if(cached?.key===key) return cached.digest;
    const digest=createHash('sha256').update(readFileSync(path)).digest('hex');
    writeFileSync(cachePath,JSON.stringify({key,digest})); return digest;
  } catch {return null;}
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

export function proofCommand(script, ...args) {
  return ['bun', relative(process.cwd(), script), ...args]
    .map(value => /^[a-zA-Z0-9_./-]+$/.test(value) ? value : "'"+value.replaceAll("'", "'\\''")+"'").join(' ');
}
// Both whole-app state and targeted world snapshots are observations.
export function worldObservations(observations, session) {
  return reply => {
    const worlds = Array.isArray(reply?.world) ? reply.world : [reply?.world ?? reply];
    for (const world of worlds) if (world?.hash && Number.isSafeInteger(world.tick))
      observations.set(session, {session, tick:world.tick, hash:world.hash});
  };
}

export async function proof(meta, script) {
  const app = fileURLToPath(new URL('.', meta.url)), name = basename(app);
  const root = fileURLToPath(new URL('..', import.meta.url));
  const args = process.argv.slice(2), device = args.includes('--device');
  const phone = args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined;
  const host = args.find(arg => !arg.startsWith('--') && arg !== phone) ?? 'linux';
  if (device && host !== 'ios') throw new Error('--device requires the ios proof host');
  if (args.includes('--phone') && (!device || !phone || phone.startsWith('--'))) throw new Error('--phone requires --device and a device name or identifier');
  const destination = device ? 'ios-device' : host;
  const out = resolve(process.env.EXACT_PROOF_OUT ?? resolve(app, 'artifacts', destination));
  const buildOut = resolve(app, 'artifacts');
  mkdirSync(buildOut, {recursive:true});
  const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(app, 'dist'));
  mkdirSync(out, {recursive:true});
  // Re-execute the actual proof, comparing every session's final simulation state.
  if (process.argv.includes('--paranoid')) {
    if (!['web', 'linux', 'windows'].includes(host)) throw new Error('--paranoid supports web, linux and windows');
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
  const finalWorlds = [], paranoidSamples = [], observations = new Map();
  const previousPins = JSON.parse(readFileSync(resolve(app, 'pins.json'), 'utf8'));
  const collecting = process.env.EXACT_PROOF_REPIN === '1';
  const compareParanoid = process.env.EXACT_GAME_PARANOID_COMPARE === '1';
  Object.assign(process.env, {EXACT_APP_DIR:app, EXACT_WEB_DIST:dist,
    EXACT_UPDATE_TRUST:'development'});
  const started = performance.now(), failures = [], transcript = [], replies = [], sessions = new Set();
  const say = line => { transcript.push(line); console.log(line); };
  const check = (label, ok, value) => {
    say(`${ok ? 'PASS' : 'FAIL'} ${label}${ok || value === undefined ? '' : ': ' + JSON.stringify(value)}`);
    if (!ok) failures.push(label);
    return ok;
  };
  const {pins, pin, pinSave} = pinRecorder(previousPins, app, check, collecting);
  // The GPU-less host has no process tree to discover: retain the process
  // handles from the carrier and await them. Global ps can block indefinitely
  // on this Mac; an optional web descendant audit is bounded and never delays
  // headless gameplay verification.
  const children = [], recorded = new Map();
  const onProcess = child => { children.push(child); recorded.set(child.pid, 'carrier'); };
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
    if (host === 'linux' || host === 'windows' || host === 'ios' || auditUnavailable || inventoryPending) return;
    inventoryPending = inventory().then(rows => {
      const owned = new Set([process.pid, ...children.map(child => child.pid)]);
      for (let changed = true; changed;) {
        changed = false;
        for (const row of rows ?? []) if (owned.has(row.parent) && !owned.has(row.pid)) {
          owned.add(row.pid); recorded.set(row.pid, row.stamp); changed = true;
        }
      }
    }).finally(() => { inventoryPending = null; });
  };
  const monitor = setInterval(sample, 100);
  let reusableWeb, reusableOptions;
  const open = async (options = {}) => {
    const signature = JSON.stringify(options);
    if ((options.fresh || options.world || options.plan || signature !== reusableOptions) && reusableWeb) { await reusableWeb.close(); reusableWeb = null; }
    const reuse = host === 'web' && !options.world && !options.plan ? reusableWeb : null;
    reusableWeb = null;
    if (reuse) say('CARRIER reused web process; fresh document');
    const raw = await openSession({host, device, phone, app:name, size:[1280,720], webDist:dist, onProcess, reuse, ...options,
      env:{EXACT_GAME_PARANOID:process.env.EXACT_GAME_PARANOID ?? '0', ...options.env}});
    sample();
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
          if (world.paranoid) paranoidSamples.push({session:id, ...world.paranoid});
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
    return new Proxy(raw, {get(target, method) {
      if (method === 'close') return close;
      // world(name) runs with the proxy as its receiver: its operations stay recorded.
      if (!['tap','type','clock','state','tree','layout','logs','screenshot'].includes(method)) return target[method];
      return async (...args) => {
        try {
          const reply = await target[method](...args);
          if (method === 'state') worldObservations(observations, id)(reply);
          replies.push({session:id, method, args, reply, clock:target.now});
          return reply;
        } catch (error) {
          replies.push(proofFailureRow(id, method, args, target.now, error)); if (error.steps) say(render('type', {steps:error.steps})); throw error;
        }
      };
    }});
  };
  let inputDigest;
  try {
    if (!['web','macos','ios','linux','windows'].includes(host)) throw new Error(`proof host unavailable: ${host}`);
    const appInfo = resolveApp(name);
    const inputs = proofInputs(root, app, resolve(buildOut, 'input-digests.json'));
    inputDigest = inputs.all;
    const inputHash = value => buildInputHash(destination, appInfo.target, process.env.EXACT_GAME_PARANOID ?? '0').update(value).digest('hex');
    const digest = inputHash(inputs.all), receipt = resolve(buildOut, `build-${destination}.sha256`);
    const linuxTarget = host === 'linux' ? spawnSync('rustc', ['-vV'], {encoding:'utf8'}).stdout.match(/^host: (.+)$/m)?.[1] : null;
    const profile = process.env.EXACT_GAME_PROOF_PROFILE ?? 'gpu-dev';
    if (!['gpu-dev', 'release'].includes(profile)) throw new Error('EXACT_GAME_PROOF_PROFILE must be gpu-dev or release');
    const artifacts = host === 'windows' ? {binary:resolve(appInfo.dir, 'dist-windows', `${executableName(appInfo)}.exe`), module:resolve(appInfo.dir, `dist-windows/${appInfo.crate('gpu').replaceAll('-','_')}.dll`)}
      : host === 'linux' ? {binary:resolve(appInfo.target, linuxTarget, `${profile}/${appInfo.crate('linux')}`), module:resolve(appInfo.target, linuxTarget, `${profile}/lib${appInfo.crate('gpu').replaceAll('-','_')}.${process.platform === 'darwin' ? 'dylib' : 'so'}`)} : host === 'web' ? null : appleArtifacts(appInfo, {destination:host === 'macos' ? 'macos' : device ? 'ios' : 'ios-simulator'});
    if (host === 'linux') process.env.EXACT_LINUX_BIN = artifacts.binary;
    if (host === 'windows') process.env.EXACT_WINDOWS_BIN = artifacts.binary;
    const built = host === 'linux' && profile === 'gpu-dev' ? await ensureLinuxReceipts({
      directory:buildOut, gpuInputs:inputHash(inputs.gpu),
      completeGpu:() => writeFileSync(artifacts.module + '.proof.json',readFileSync(resolve(buildOut,'build-linux-gpu.sha256'))),
      hostInputs:() => inputHash(inputs.host + readFileSync(resolve(appInfo.workspace,'surfaces.json'),'utf8')),
      gpuArtifact:() => productDigest(artifacts.module,resolve(buildOut,'product-linux-gpu.json')),
      hostArtifact:() => productDigest(artifacts.binary,resolve(buildOut,'product-linux-host.json')),
      build:async part => {
        say(`BUILD ${part} stale or missing receipt`);
        buildBake(appInfo,'linux',linuxTarget,{profile,part,env:{EXACT_GAME_PARANOID:'0'}});
      },
    }) : await ensureBuildReceipt({receipt, inputs:digest,
      artifact:() => artifactDigest(host, dist, artifacts), build:async () => {
      say(`BUILD stale or missing receipt ${receipt}; rebuilding: bun ${fileURLToPath(meta.url)} ${host}${device ? ' --device' : ''} --build-only`);
      if (host === 'linux') {
        if (!linuxTarget) throw new Error('rustc did not report its target');
        // Native mode is read at launch; keep compile-time environment stable.
        buildBake(appInfo, 'linux', linuxTarget, {profile, env:{EXACT_GAME_PARANOID:'0'}});
      } else {
        const script = host === 'windows' ? 'host/windows/build.mjs' : host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs';
        const flags = host === 'windows' ? (profile === 'release' ? ['--release'] : []) : host === 'web' ? ['--wasm'] : device ? ['--device', ...(phone ? ['--phone', phone] : [])] : host === 'ios' ? ['--ios'] : host === 'macos' ? ['--bundle'] : [];
        const child = spawn('bun', [resolve(root,script), ...flags], {cwd:root, env:process.env, stdio:'inherit', windowsHide:true});
        sample();
        const code = await new Promise((ok, reject) => {child.on('exit',ok); child.on('error',reject);});
        if (code !== 0) throw new Error(`app build exited ${code}`);
      }
    }});
    if (!built) say(`BUILD cached ${name} ${destination}`);
    if (!process.argv.includes('--build-only')) {
      await script({open, check, equal, out, host, say, pin, pinSave});
      if (!process.argv.some(arg => ['--screenshot-only','--capture40'].includes(arg)))
        for (const section of ['ticks','saves']) for (const key of Object.keys(previousPins[section] ?? {}))
          check(`pin ${key} observed; if intentionally removed, update the proof and pins.json together`, key in pins[section]);
    }
  } catch (error) { check('proof interrupted',false,formatProofError(error)); }
  finally {
    await closeSessions(monitor, sample, sessions, check);
    if (reusableWeb) await reusableWeb.close();
    closeFilesystemReader(); // The static server's resident reader is this process's child.
    await inventoryPending;
    let remaining = children.filter(child => child.exitCode === null && child.signalCode === null)
      .map(child => ({pid:child.pid}));
    if (host !== 'linux' && host !== 'windows' && host !== 'ios' && !auditUnavailable) {
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
      // A sample owed at the end is an asset that never landed: its save was never checked.
      const skipped = paranoidSamples.reduce((n, p) => n + p.skipped, 0);
      if (skipped) say(`PARANOID ${skipped} samples deferred while shown assets were in flight`);
      check('no paranoid sample is still owed to an undelivered asset', paranoidSamples.every(p => !p.owed), paranoidSamples);
      finalWorlds.sort((a,b) => a.session - b.session);
      const baseline = resolve(out, `paranoid-${host}-normal.json`);
      writeFileSync(resolve(out, `paranoid-${host}-${process.env.EXACT_GAME_PARANOID}.json`), JSON.stringify(finalWorlds));
      if (process.env.EXACT_GAME_PARANOID === '0') writeFileSync(baseline, JSON.stringify(finalWorlds));
      else {
        const matches = equal(finalWorlds, JSON.parse(readFileSync(baseline, 'utf8')));
        const mode = process.env.EXACT_GAME_PARANOID === '1' ? 'Save' : 'FreshGame';
        check(matches ? `paranoid ${mode} matches continuous state` : `paranoid ${mode} differs at ${finalWorlds.map(w => `session ${w.session} tick ${w.tick}`).join(', ')}; ${proofCommand(fileURLToPath(meta.url), host, '--paranoid')}`,
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
    writeFileSync(resolve(out,'summary.json'), JSON.stringify({name, host, device, ...(phone ? {phone} : {}), status, inputs:inputDigest, mode:process.env.EXACT_GAME_PARANOID ?? '0', pins, facilities:facilityReport(replies), failures, seconds:(performance.now()-started)/1000, worlds:finalWorlds, saves, auditUnavailable}, null, 2)+'\n');
    if (process.argv.includes('--report')) for (const hint of facilityReport(replies)) say(`REPORT ${hint}`);
    say(`PROOF ${status} ${name} ${destination}: ${failures.length} failures; ${((performance.now()-started)/1000).toFixed(3)} s`);
    if (status === 'PASS' && host === 'linux') say(`Capture the PNG: ${proofCommand(fileURLToPath(meta.url), 'web')}`);
    if (status === 'UNVERIFIED' && !collecting && !partial)
      say(`UNVERIFIED: no pins — run ${proofCommand(resolve(import.meta.dir, 'prove.mjs'), app)}`);
    writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
    writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
  }
  process.exit(failures.length || (!collecting && !process.argv.some(arg => ['--build-only','--screenshot-only','--capture40'].includes(arg)) && proofStatus({failures, expected:previousPins, pins}) !== 'PASS') ? 1 : 0);
}
