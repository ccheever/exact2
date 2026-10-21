export const proofSDK = import.meta.dir + "/..";
import {refusalMessage} from '../scripts/agent-keys.mjs';
import {releasePinGaps, statKey, proofInputs, proofInputReader, proofInputFiles, proofInputExcluded, executionInputFiles} from './inputs.mjs';
export {proofInputs, proofInputReader, proofInputFiles, proofInputExcluded} from './inputs.mjs';
// Shared lifecycle for game proofs: operations and assertions stay in the game.
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { open as openSession, render, lookPicture } from '../scripts/agent.mjs';
import { cdpFailureContext } from '../scripts/agent-launch.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { buildBake, executableName, resolveApp } from '../scripts/app.mjs';
import { prepareProofTree } from './app/shells.mjs';
import { closeFilesystemReader, filesystemLock } from '../scripts/filesystem.mjs';
import { publicFileCards } from '../host/web/serve.mjs';
import { takeWarmGame } from '../host/web-js/module.mjs';
import { webInputDigests } from '../scripts/agent-launch.mjs';
import { decodePng } from '../scripts/png.mjs';

/** Some runtime-created stacks omit the informative Error message. */
export function formatProofError(error) {
  const refusal = refusalMessage(error?.reply);
  if (refusal && !process.env.EXACT_AGENT_DEBUG && !process.argv.includes('--json')) return refusal;
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
  // Receipts made before complete directory checks may describe stale artifacts.
  const hash = createHash('sha256').update('exact game proof receipt v4\0').update(host).update(target);
  if (host === 'web') hash.update(mode);
  if (host === 'linux' || host === 'windows' || host === 'web') hash.update(profile);
  return hash;
}
export async function ensureBuildReceipt({receipt, inputs, artifact, build}) {
  const readInputs = typeof inputs === 'function' ? inputs : () => inputs;
  let currentInputs = readInputs(), digest = artifact();
  const stamp = () => JSON.stringify({inputs:currentInputs, artifact:digest});
  if (digest && existsSync(receipt) && readFileSync(receipt, 'utf8') === stamp()) return false;
  await build();
  // A bake may rewrite generated inputs; bind them to the product from that same build.
  try { currentInputs = readInputs(); }
  catch (error) { rmSync(receipt, {force:true}); throw error; }
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
export function agreePins(rows, previous, hosts, app, {release: checkRelease = true} = {}) {
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
  for (const host of checkRelease ? [native, ...hosts.includes('web') ? ['web'] : []] : []) {
    const release = rows.filter(row => row.host === host && row.mode === '0' && row.profile === 'release');
    if (release.length !== 1 || release[0].failures?.length || !equal(release[0].pins, reference.pins))
      throw new Error(`repin refused: ${host} release proof missing, failed, or disagrees with gpu-dev; pins.json unchanged`);
  }
  return {...reference.pins, hosts, modes, profiles:checkRelease ? ['gpu-dev', 'release'] : ['gpu-dev']};
}
export const pinCoverageGaps = releasePinGaps;
export function pinInputs(rows) {
  const inputs=rows[0]?.inputs;
  if(!/^[a-f0-9]{64}$/.test(inputs ?? '') || rows.some(row=>row.inputs!==inputs))
    throw new Error('repin refused: proof inputs missing or changed between runs; pins.json unchanged');
  return inputs;
}
export function pinSourceInputs(rows) {
  const inputs=rows[0]?.sourceInputs;
  if(!/^[a-f0-9]{64}$/.test(inputs ?? '') || rows.some(row=>row.sourceInputs!==inputs))
    throw new Error('repin refused: source inputs missing or changed between runs; pins.json unchanged');
  return inputs;
}
/** Provenance without a commit is valid only for an unborn or non-Git game. */
export function pinRevision(app, sourceInputs) {
  const git = args => spawnSync('git', args, {cwd:app, encoding:'utf8', env:{...process.env, LC_ALL:'C'}});
  const refuse = result => { throw new Error(`repin refused: git provenance failed: ${result.error?.message ?? result.stderr?.trim() ?? result.status}; pins.json unchanged`); };
  const inside = git(['rev-parse', '--is-inside-work-tree']);
  if (inside.status !== 0) {
    let repository = false;
    for (let dir = resolve(app);; dir = dirname(dir)) {
      if (existsSync(resolve(dir, '.git'))) { repository = true; break; }
      if (dirname(dir) === dir) break;
    }
    if (!repository && inside.status === 128 && /not a git repository/.test(inside.stderr)) return `inputs:${sourceInputs}`;
    refuse(inside);
  }
  const revision = git(['rev-parse', '--verify', '--quiet', 'HEAD^{commit}']);
  if (revision.status === 0) return revision.stdout.trim();
  const branch = git(['symbolic-ref', '-q', 'HEAD']);
  if (revision.status === 1 && branch.status === 0) {
    const missing = git(['show-ref', '--exists', branch.stdout.trim()]);
    if (missing.status === 2) return `inputs:${sourceInputs}`;
  }
  refuse(revision);
}
/** Admission markers decide pin provenance; gameplay can log arbitrary text. */
export function assertNoLiveEdits(worlds, journals = []) {
  const rows = Array.isArray(worlds) ? worlds : [worlds, ...(worlds?.world ?? [])];
  const edited = rows.some(w => w?.developmentEdits || w?.sessionDevelopmentEdits);

  const lines = journals.flatMap(j => j.lines ?? []);
  const line = lines.find(l => /(?:^| tick=\d+ )set\s+\S+\s+[^.\s]+\.[^.\s]+\s+\S/.test(l));
  if (edited) throw new Error(`pin refused: run contains a set development edit${line ? ': ' + line : ''}`);
}

export function editProvenance() {
  let refusal;
  const pending = new Map();
  return {
    observe(worlds, journals, session = 'default') {
      const rows = Array.isArray(worlds) ? worlds : [worlds, ...(worlds?.world ?? [])];
      pending.set(session, rows.some(w => w?.queuedSets > 0));
      try { assertNoLiveEdits(worlds, journals); } catch (error) { refusal ??= error; }
    },
    assert() {
      if (refusal) throw refusal;
      if ([...pending.values()].some(Boolean)) throw new Error('pin refused: queued set development edit awaits admission');
    },
  };
}
// Read only provenance fields; walk skipped values to preserve the binary name table.
// Tags and bounds follow engine/src/data/bin.rs. This does not restore gameplay.
function savedFields(bytes, magic, fields) {
  const fail = reason => { throw new Error(`pinSave refused: invalid ${magic.startsWith('EXSIM') ? 'EXSIM v8' : 'EXGAME v6'} save: ${reason}`); };
  if (!bytes.subarray(0, magic.length).equals(Buffer.from(magic, 'binary'))) fail('unsupported format');
  if (bytes.length > 2 * 1024 ** 3) fail('load size limit');
  let pos = magic.length;
  const names = [], utf8 = new TextDecoder('utf-8', {fatal:true, ignoreBOM:true});
  const take = n => {
    if (!Number.isSafeInteger(n) || n < 0 || n > bytes.length - pos) fail('truncated value');
    const part = bytes.subarray(pos, pos += n); return part;
  };
  const byte = () => take(1)[0];
  const variable = () => {
    let n = 0n;
    for (let shift = 0n; shift < 70n; shift += 7n) {
      const b = byte();
      if (shift === 63n && b > 1) fail('varint overflow');
      n |= BigInt(b & 127) << shift;
      if (b < 128) return n;
    }
    fail('varint overflow');
  };
  const text = () => {
    const n = Number(variable());
    if (n > 64 * 1024 ** 2) fail('string size limit');
    try { return utf8.decode(take(n)); } catch { fail('invalid string'); }
  };
  const name = () => {
    const n = variable();
    if (n === 0n) { const s = text(); names.push(s); return s; }
    if (n > BigInt(names.length)) fail('unknown name index');
    return names[Number(n - 1n)];
  };
  const value = (keep, depth = 0) => {
    if (depth >= 256) fail('nesting limit');
    const tag = byte();
    switch (tag) {
      case 0: case 1: return Boolean(tag);
      case 2: case 3: return variable();
      case 4: take(4); return;
      case 5: take(8); return;
      case 6: return text();
      case 7: {
        const n = Number(variable()), out = keep ? [] : null;
        if (!Number.isSafeInteger(n) || n > bytes.length - pos) fail('sequence size limit');
        for (let i = 0; i < n; i++) { const item = value(keep, depth + 1); if (keep) out.push(item); }
        return out;
      }
      case 8: {
        const out = Object.create(null), seen = new Set();
        for (let field = byte(); field !== 0; field = byte()) {
          if (field !== 1) fail('expected field');
          const key = name();
          if (seen.has(key)) fail('duplicate field');
          seen.add(key);
          const retain = keep === true || keep instanceof Set && keep.has(key);
          const item = value(retain, depth + 1);
          if (retain) out[key] = item;
        }
        return out;
      }
      case 9: {
        variable(); const variant = name(), item = value(keep, depth + 1);
        return keep ? {variant, value:item} : null;
      }
      case 10: return null;
      case 11: return value(keep, depth + 1);
      case 12: case 13: case 14: case 15: {
        const part = take(Number(variable())); return keep ? part : null;
      }
      default: fail('unknown value tag');
    }
  };
  if (bytes[pos] !== 8) fail('expected record');
  const out = value(new Set(fields));
  if (pos !== bytes.length) fail('trailing data');
  return out;
}
function assertSaveEdits(bytes) {
  const saved = savedFields(bytes, 'EXSIM\0\x08', ['world', 'queue']);
  if (!Buffer.isBuffer(saved.world) || !Array.isArray(saved.queue))
    throw new Error('pinSave refused: invalid EXSIM v8 world or queue');
  const world = savedFields(saved.world, 'EXGAME\0\x06', ['developmentEdits']);
  if (world.developmentEdits !== undefined && typeof world.developmentEdits !== 'boolean')
    throw new Error('pinSave refused: invalid developmentEdits marker');
  if (world.developmentEdits) throw new Error('pinSave refused: save contains a set development edit');
  // Rust char::is_whitespace uses Unicode White_Space, unlike JS trim/\s.
  const completeSet = text => {
    const words = text.split(/\p{White_Space}+/u).filter(Boolean), path = words[2]?.split('.') ?? [];
    return words[0] === 'set' && words.length >= 4 && path.length === 2 && path.every(Boolean);
  };
  if (saved.queue.some(q => q?.event?.variant === 'Message'
      && typeof q.event.value?.text === 'string' && completeSet(q.event.value.text)))
    throw new Error('pinSave refused: save contains a queued set development edit awaiting admission');
}
export function pinRecorder(previous, app, check, collecting = false, provenance = editProvenance()) {
  const pins = {ticks:{}, saves:{}};
  const record = (section, key, got) => {
    provenance.assert();
    const expected = previous[section]?.[key];
    if (key in pins[section]) check(`pin ${key} repeated consistently`, pins[section][key] === got);
    pins[section][key] = got;
    if (!collecting && (expected !== undefined || Object.keys(previous.ticks ?? {}).length || Object.keys(previous.saves ?? {}).length)) check(
      expected === got ? `pin ${key}=${got}` : `pin ${key} differs (expected ${expected}, got ${got}); if the change is intended: ${proofCommand(resolve(import.meta.dir, 'prove.mjs'), app, '--repin')}`, expected === got);
  };
  return {pins,
    pin(tick, state) {
      if (state?.tableEdits) check(`pin ${tick} refused: development table edits are in the journal`, false);
      provenance.assert();
      provenance.observe([state]);
      provenance.assert();
      check(`pin ${tick} sampled at expected tick (got ${state?.tick})`, state?.tick === tick && /^0x[0-9a-f]{16}$/.test(state?.hash));
      record('ticks', String(tick), state?.hash);
    },
    pinSave(key, path) {
      provenance.assert();
      const bytes = readFileSync(path);
      assertSaveEdits(bytes);
      record('saves', key, createHash('sha256').update(bytes).digest('hex'));
    },
  };
}
// Gameplay assertions can succeed before a game's first baseline exists.
export function proofStatus({failures, expected, pins, collecting = false, partial = false}) {
  if (failures.length) return 'FAIL';
  if (collecting || partial || ['ticks', 'saves'].some(section =>
    !Object.keys(expected[section] ?? {}).length || !equal(expected[section], pins[section]))) return 'UNVERIFIED';
  return 'PASS';
}
export function proofVerdict(options, args = []) {
  const status = proofStatus({...options, partial:args.some(a => ['--build-only','--screenshot-only','--capture40'].includes(a))});
  const reason = status === 'FAIL' ? 'assertions failed' : status === 'PASS' ? 'complete baseline checked'
    : args.includes('--build-only') ? 'build only' : args.includes('--screenshot-only') || args.includes('--capture40') ? 'pixels only'
    : options.collecting ? 'collecting baseline' : !Object.keys(options.expected.ticks ?? {}).length || !Object.keys(options.expected.saves ?? {}).length
      ? 'no baseline yet (expected before the first prove)' : 'incomplete baseline observations';
  return {status, reason, code:status === 'PASS' ? 0 : status === 'FAIL' ? 1 : 3};
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

export async function ensureLinuxReceipts({directory,app,surface,gpuInputs,hostInputs,gpuArtifact,hostArtifact,build,completeGpu = () => {}}) {
  if (surface) mkdirSync(dirname(surface),{recursive:true});
  const declaration = resolve(directory,'build-linux-surfaces.json'), assets = resolve(directory,'build-linux-assets');
  // The keys `Actions` bind travel with the declaration (the bake's shortcut warning reads them).
  const keys = surface && resolve(dirname(surface),'keys.json'), keysCache = resolve(directory,'build-linux-keys.json');
  let cachedAssets = !app;
  // No cache is an ordinary miss; a cached file that would overwrite an authored one is a refusal.
  if (app && existsSync(resolve(assets,'files.json'))) cachedAssets = restoreBakedAssets(assets, app);
  const artifact = () => {
    const product = gpuArtifact();
    if (!surface) return product;
    if (!product || !existsSync(declaration) || !cachedAssets) return null;
    return createHash('sha256').update(product).update(readFileSync(declaration)).update(app ? bakedAssetDigest(assets) : '').digest('hex');
  };
  const gpu=await ensureBuildReceipt({receipt:resolve(directory,'build-linux-gpu.sha256'),inputs:gpuInputs,artifact,build:async () => {
    await build('gpu');
    if (surface) copyFileSync(surface,declaration);
    if (keys && existsSync(keys)) copyFileSync(keys,keysCache);
    if (app) { cacheBakedAssets(app,assets); cachedAssets = true; }
  }});
  // A fresh snapshot needs the declaration even when the GPU product is reused.
  if (surface) copyFileSync(declaration,surface);
  if (keys && existsSync(keysCache)) copyFileSync(keysCache,keys);
  completeGpu();
  // The GPU build may have changed surfaces.json; observe it only afterwards.
  const host=await ensureBuildReceipt({receipt:resolve(directory,'build-linux-host.sha256'),inputs:hostInputs,artifact:hostArtifact,build:()=>build('host')});
  return gpu || host;
}
function bakedAssetDigest(cache) {
  const hash = createHash('sha256'), files = JSON.parse(readFileSync(resolve(cache,'files.json')));
  hash.update(JSON.stringify(files));
  for (const name of files) hash.update(name).update(readFileSync(resolve(cache,name)));
  return hash.digest('hex');
}
function cacheBakedAssets(app, cache) {
  const root = resolve(import.meta.dir,'..'), authored = new Set(proofInputFiles(root,app));
  const manifest = resolve(app,'.baked-assets.json');
  const owned = new Set(existsSync(manifest) ? Object.keys(JSON.parse(readFileSync(manifest))).map(name=>'assets/'+name) : []);
  const files = executionInputFiles(root,app,undefined,undefined,authored).filter(file => {
    const name = relative(app,resolve(root,file));
    return name === '.baked-assets.json' || name.startsWith('assets/') && (!authored.has(file) || owned.has(name));
  }).map(file=>relative(app,resolve(root,file))).sort();
  rmSync(cache,{recursive:true,force:true}); mkdirSync(cache,{recursive:true});
  for (const name of files) { mkdirSync(dirname(resolve(cache,name)),{recursive:true}); copyFileSync(resolve(app,name),resolve(cache,name)); }
  writeFileSync(resolve(cache,'files.json'),JSON.stringify(files));
}
// A cached file lands only where the app's own bake manifest says a generated file of that
// digest is (or where nothing is): a file the author put there is refused by name, as the baker
// refuses the same collision (game/bake, "collides with an authored asset"). Every destination is
// checked before any is written.
function restoreBakedAssets(cache, app) {
  const files = JSON.parse(readFileSync(resolve(cache,'files.json')));
  const manifest = resolve(app,'.baked-assets.json');
  // Art was baked before the input scan. An older manifest must not remove ownership
  // of a new model, restore a removed one, or replace its current bytes.
  const cachedManifest = resolve(cache,'.baked-assets.json');
  if (existsSync(cachedManifest) !== existsSync(manifest)
      || existsSync(manifest) && !readFileSync(manifest).equals(readFileSync(cachedManifest))) return false;
  const generated = existsSync(manifest) ? JSON.parse(readFileSync(manifest)) : {};
  const writes = [];
  for (const name of files) {
    if (name !== '.baked-assets.json' && (!name.startsWith('assets/') || !resolve(app,name).startsWith(resolve(app,'assets')+'/'))) throw new Error(`invalid baked asset path: ${name}`);
    const destination = resolve(app,name), bytes = readFileSync(resolve(cache,name));
    if (!existsSync(destination)) { writes.push([destination,bytes]); continue; }
    const current = readFileSync(destination);
    if (current.equals(bytes)) continue;
    const recorded = name === '.baked-assets.json' ? undefined : generated[name.slice('assets/'.length)];
    const ownedDigest = recorded && (typeof recorded === 'string' ? recorded : recorded.digest ?? recorded.sha256);
    const currentDigest = createHash('sha256').update(current).digest('hex');
    if (name !== '.baked-assets.json' && ownedDigest !== currentDigest)
      throw new Error(`cached asset \`${name}\` would overwrite a file the bake does not own; move the authored file or delete the cached build (artifacts/build)`);
    writes.push([destination,bytes]);
  }
  for (const [destination,bytes] of writes) { mkdirSync(dirname(destination),{recursive:true}); writeFileSync(destination,bytes); }
  return true;
}
export async function withProofBuild(target, build) {
  for (;;) {
    let entered = false;
    try { return await filesystemLock(target, 'proof-products/.lock', async () => { entered = true; return await build(); }); }
    catch (error) { if (entered || !error.message.includes('stream is locked by another publisher')) throw error; }
    await new Promise(ok => setTimeout(ok,100));
  }
}
export function copyProofProducts(root, app, artifacts, destination, expected = {}) {
  mkdirSync(destination,{recursive:true});
  const products = {}, copied = {};
  for (const [kind,path] of Object.entries(artifacts)) {
    const to = resolve(destination,basename(path)), bytes = readFileSync(path);
    copyFileSync(path,to);
    const hash = createHash('sha256').update(bytes).digest('hex');
    if (createHash('sha256').update(readFileSync(to)).digest('hex') !== hash) throw new Error(`proof product changed while copying: ${path}`);
    if (expected[kind] && expected[kind] !== hash) throw new Error(`proof product changed after validation: ${path}`);
    copied[kind] = to; products[to] = hash;
    if (existsSync(path+'.proof.json')) { copyFileSync(path+'.proof.json',to+'.proof.json'); products[to+'.proof.json'] = createHash('sha256').update(readFileSync(to+'.proof.json')).digest('hex'); }
  }
  const inputs = Object.fromEntries(executionInputFiles(root,app).map(file => [file,createHash('sha256').update(readFileSync(resolve(root,file))).digest('hex')]));
  const receipt = resolve(destination,'products.json');
  writeFileSync(receipt,JSON.stringify({binary:copied.binary,inputs,products}));
  return {...copied,receipt};
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

// The look oracle's tells (LLP 1127.000 D1; game/render/src/tells.rs), by the
// words of their `look:` lines, and the ones a game's app.json silences.
export const TELLS = {'near-black':/near black$/, 'near-white':/near white$/, 'clipped':/% of pixels clipped$/,
  'particles-uncovered':/particles live, 0 pixels covered \(.*\)$/, 'no-shadow-casters':/sun shadow maps hold no texel nearer than their clear depth$/};
export function tellOf(line) {
  const at = typeof line === 'string' ? line.indexOf('look: ') : -1;
  if (at < 0) return null;
  const text = line.slice(at), name = Object.keys(TELLS).find(n => TELLS[n].test(text));
  return name ? {name, line:text} : null;
}
export function lookSilence(app) {
  const path = resolve(app, 'app.json');
  const silence = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')).game?.look?.silence ?? [] : [];
  const unknown = silence.filter(name => !(name in TELLS));
  if (unknown.length) throw new Error(`${path}: game.look.silence names ${unknown.join(', ')}; the tells are ${Object.keys(TELLS).join(', ')}`);
  return new Set(silence);
}
// A world's own `logs` line that a look frame added (`tick=N look: …`,
// surface/look.rs): presentation, which no save or fresh game reproduces.
export const isTellLine = line => typeof line === 'string' && /^tick=\d+ look: /.test(line);
// A world's journal chunks as the paranoid comparison reads them: without tell lines.
export function worldJournal(chunks) {
  return chunks.map(({from, next, lines, tick}) => Object.fromEntries(
    Object.entries({from, next, lines:lines?.filter(line => !isTellLine(line)), tick}).filter(([,value]) => value !== undefined)));
}
// The frames one reply judged and their tells: a settle's or a screenshot's
// lines, a film's frames (`lookFrames`), or a look frame's own. A reduced
// (contact-sheet) frame and a look frame from another view than the game's
// camera are pictures, and judge nothing. With them, every gap and failed look
// frame the reply names.
export function frameTells(reply) {
  const look = reply?.look, own = look && !Array.isArray(look) && !look.reduced && !reply.view ? look : null;
  const frames = reply?.lookFrames ?? (Array.isArray(look) ? [look] : own ? [(own.tells ?? []).map(t => t.line)] : []);
  return {frames:frames.map(lines => lines.map(tellOf).filter(Boolean)),
    gaps:[...own?.gaps ?? [], ...reply?.lookGaps ?? [], ...reply?.lookFailed ?? []]};
}
// The look result beside the simulation result (LLP 1127.000 D5). `worlds` is
// each world every session showed at its close, with its picture or why it has
// none; `tells` one entry a judged frame told; `gaps` what a judged frame could
// not measure. A tell no silence names fails it; it passes only when every
// world was seen and every measurement taken, and is otherwise unverified.
export function lookResult({worlds, tells, gaps = []}, silence) {
  // One entry a tell: its first line and how many distinct frames told it.
  const byName = list => [...Map.groupBy(list, t => t.name).values()]
    .map(all => ({...all[0], times:new Set(all.map((t, i) => t.frame ?? `entry ${i}`)).size}));
  const loud = byName(tells.filter(t => !silence.has(t.name))), quiet = byName(tells.filter(t => silence.has(t.name)));
  // Worlds without a picture, one line a canvas and reason: `sessions 1, 2 world: no picture on this host: …`.
  const unseen = Map.groupBy(worlds.filter(w => !w.picture), w => `${w.canvas}: ${w.unavailable ?? w.error ?? 'no picture'}`);
  const missing = [...[...unseen].map(([why, all]) => `session${all.length > 1 ? 's' : ''} ${all.map(w => w.session).join(', ')} ${why}`), ...new Set(gaps)];
  const status = loud.length ? 'FAIL' : worlds.length && !missing.length ? 'PASS' : 'UNVERIFIED';
  const detail = loud.length ? [...new Set(loud.map(t => t.name))].join(', ')
    : !worlds.length ? 'no world was shown' : missing.length ? missing.join('; ')
    : `${quiet.length} tells${quiet.length ? ' silenced' : ''}`;
  return {status, detail, loud, quiet, missing};
}
// One proof's look accounting (LLP 1127.000 D5): the pictures and judged
// frames, every world a session showed (by session and canvas id), and the
// ones a closing look frame, a judged settle or screenshot, or an explicit
// look(…) looked at. `idOf(session)` names a session; `out` the artifacts.
export function lookAccount(out, idOf) {
  const looks = {closing:new Map(), pictures:[], pages:[], sheets:[], tells:[], gaps:[], frames:0};
  const shown = new Map(), lookedAt = new Set();
  // Each judged frame's tells, marked with the frame (a tell several frames told counts each), and its gaps.
  const noteFrame = ({frames, gaps}) => {
    for (const tells of frames) { looks.frames += 1; looks.tells.push(...tells.map(t => ({...t, frame:looks.frames}))); }
    looks.gaps.push(...gaps);
  };
  const show = (session, id, canvas = id) => { if (!shown.has(`${session}:${id}`)) shown.set(`${session}:${id}`, {session, id, canvas}); };
  return {
    looks, show,
    // One look frame of a session's world, from `view` ({}, {top}, {yaw, pitch, distance}), into the artifacts. Only
    // the game's own camera is judged, and a judged one looked at its canvas; another view is a picture.
    async look(session, view = {}, label = 'look', canvas = 'world') {
      const frame = await lookPicture(session, resolve(out, `${label}.png`), canvas, view);
      looks.pictures.push(frame.screenshot);
      const judged = frameTells(frame);
      noteFrame(judged);
      const sid = idOf(session);
      if (sid != null && frame.canvas != null) {
        show(sid, frame.canvas, canvas);
        if (judged.frames.length) lookedAt.add(`${sid}:${frame.canvas}`);
      }
      return {picture:frame.screenshot, look:frame.look};
    },
    // A recorded reply ({session, method, reply}): the frames it judged, the worlds it showed (a settle's or a
    // state's world list, a tree's canvases) and the canvases its tells looked at.
    replied({session, method, reply}) {
      if (['clock', 'screenshot', 'state'].includes(method)) noteFrame(frameTells(reply));
      for (const w of Array.isArray(reply?.world) ? reply.world : []) if (w?.canvas != null) show(session, w.canvas);
      if (method === 'tree') for (const n of reply?.nodes ?? []) if (n.world) show(session, n.id, n.props?.testId ?? n.id);
      for (const id of reply?.lookSeen ?? []) lookedAt.add(`${session}:${id}`);
    },
    // Every world shown: its closing look frame's record, or seen by a judged frame, or a gap.
    worlds() {
      return [...shown].map(([key, w]) => looks.closing.get(key)
        ?? (lookedAt.has(key) ? {...w, picture:'a judged frame'} : {...w, error:'shown, and never looked at'}));
    },
  };
}
// The look result across hosts and runs (prove.mjs): a FAIL anywhere fails
// it; it passes only when every expected run ({host, repeat}) has a summary
// whose look passed, and a run with none, or none with a look, is a named gap.
export function lookAcrossRuns(rows, expected = rows.map(r => ({host:r.host, repeat:r.repeat}))) {
  const gaps = expected.filter(e => !rows.some(r => r.host === e.host && r.repeat === e.repeat && r.look))
    .map(e => `${e.host} run ${e.repeat}: no look result`);
  const looks = rows.map(r => r.look).filter(Boolean);
  const status = looks.some(l => l.status === 'FAIL') ? 'FAIL'
    : looks.length && !gaps.length && looks.every(l => l.status === 'PASS') ? 'PASS' : 'UNVERIFIED';
  return {status, gaps};
}
// A run that failed by its look alone: no simulation assertion failed, whether
// its simulation passed or was collecting a baseline (UNVERIFIED).
export const lookOnlyFailure = summary => summary?.look?.status === 'FAIL'
  && summary.simulation !== 'FAIL' && !(summary.failures ?? []).length;

export function proofCommand(script, ...args) {
  // From the filesystem root a relative path would read as a bare name (`Users/…`).
  const cwd = process.cwd(), path = dirname(cwd) === cwd ? script : relative(cwd, script);
  return ['bun', path, ...args]
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

// Read every world's executor, including worlds removed before the session closes.
export function executedModes(mode) {
  const records = new Map();
  return {
    observe(state, session) {
      for (const world of Array.isArray(state?.world) ? state.world : []) {
        if (!world?.hash) continue;
        const p = world.paranoid;
        if (p?.mode !== mode) throw new Error(`executed mode ${p?.mode ?? 'missing'} differs from requested ${mode} (session ${session}, canvas ${world.canvas})`);
        if (!Number.isSafeInteger(p.checked) || p.checked < 0 || typeof p.owed !== 'boolean')
          throw new Error('paranoid reconstruction evidence missing');
        const key = JSON.stringify([session, world.canvas, world.restarted]);
        const previous = records.get(key);
        records.set(key, {session, canvas:world.canvas, restarted:world.restarted, firstTick:previous?.firstTick ?? world.tick,
          firstChecked:previous?.firstChecked ?? p.checked, tick:world.tick, ...p});
      }
    },
    rows:() => [...records.values()],
    assert() {
      const rows = [...records.values()];
      if (!rows.length) throw new Error('executed mode evidence missing: no worlds observed');
      if (mode !== '0') {
        if (!rows.some(p => p.checked > p.firstChecked)) throw new Error(`paranoid ${mode}: no completed reconstruction samples`);
        for (const p of rows) {
          if (p.owed) throw new Error(`paranoid ${mode}: sample still owed (session ${p.session}, canvas ${p.canvas})`);
          if (p.tick > p.firstTick && p.checked <= p.firstChecked)
            throw new Error(`paranoid ${mode}: advanced world has no reconstruction samples (session ${p.session}, canvas ${p.canvas})`);
        }
      }
    },
  };
}

/** What a screenshot shows of a world canvas: the canvas's box from `layout()`, less
 * the boxes of every node inside the canvas's subtree in `tree()` (its HUD, masked
 * whatever its size or overflow; a sibling or ancestor is not masked), and the share of
 * the rest further than `tolerance` (per channel, 0–255) from its most common colour,
 * which a cleared canvas is everywhere. `shot` is the screenshot's reply, whose `w`
 * scales points to pixels. `pixels` is 0 when the HUD covers the whole canvas. */
export function worldPixels(png, shot, layout, tree, canvas = 'world', tolerance = 8) {
  const {width, height, data} = decodePng(png), scale = width / (shot?.w || layout.viewport.w);
  const owner = tree.nodes.find(n => n.props?.testId === canvas);
  const world = owner && layout.nodes.find(n => n.id === owner.id);
  if (!world) throw new Error(`no canvas ${canvas} in tree and layout`);
  const parents = new Map(tree.nodes.map(n => [n.id, n.parent]));
  const inside = id => { for (let p = parents.get(id); p != null; p = parents.get(p)) if (p === owner.id) return true; return false; };
  const hud = layout.nodes.filter(n => inside(n.id) && n.w > 0 && n.h > 0);
  const covered = (b, x, y) => x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h;
  const pixels = [];
  for (let py = Math.max(0, Math.floor(world.y * scale)); py < Math.min(height, Math.ceil((world.y + world.h) * scale)); py++) {
    for (let px = Math.max(0, Math.floor(world.x * scale)); px < Math.min(width, Math.ceil((world.x + world.w) * scale)); px++) {
      const [x, y] = [(px + 0.5) / scale, (py + 0.5) / scale];
      if (hud.some(b => covered(b, x, y))) continue;
      pixels.push((py * width + px) * 4);
    }
  }
  const counts = new Map();
  for (const i of pixels) {
    const key = (data[i] << 16) | (data[i + 1] << 8) | data[i + 2];
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  let clear = 0, most = -1;
  for (const [key, n] of counts) if (n > most) { most = n; clear = key; }
  const [r, g, b] = [clear >> 16, (clear >> 8) & 255, clear & 255];
  const drawn = pixels.filter(i => Math.max(Math.abs(data[i] - r), Math.abs(data[i + 1] - g), Math.abs(data[i + 2] - b)) > tolerance).length;
  return {pixels:pixels.length, drawn, share:pixels.length ? drawn / pixels.length : 0, hud:hud.length};
}

/** The engine fact a proof checks on a GPU host: the renderer reports itself `ready`
 * and the canvas, outside its HUD, is not one cleared colour. When the HUD covers the
 * whole canvas no pixel can speak, so it rests on `ready` and the host's first-draw
 * report (`perf.firstFrameMs`, where the host gives one) and says so. */
export function completedWorldFrame(shown, world) {
  const ready = world?.ready === true, firstDraw = world?.perf?.firstFrameMs;
  if (shown.pixels > 0) return {ok:ready && shown.drawn > 0, fallback:false, ready, ...shown};
  return {ok:ready && (firstDraw === undefined || Number.isFinite(firstDraw)), fallback:true, ready, firstDraw, ...shown};
}

/** The game a proof script checks: `--app <dir>`, else the nearest directory at or
 * above the script's own that holds `app.json` or `logic/src/lib.rs`, so a script
 * under `tools/` proves the game around it. */
export function proofGame(scriptDir, args) {
  const isGame = dir => existsSync(resolve(dir, 'app.json')) || existsSync(resolve(dir, 'logic/src/lib.rs'));
  const at = args.indexOf('--app');
  if (at >= 0) {
    const named = args[at + 1];
    if (!named || named.startsWith('--')) throw new Error('--app requires the game\'s directory');
    if (!isGame(resolve(named))) throw new Error(`--app ${named}: not a game (no app.json or logic/src/lib.rs there)`);
    return resolve(named);
  }
  for (let dir = resolve(scriptDir);; dir = dirname(dir)) {
    if (isGame(dir)) return dir;
    if (dirname(dir) === dir) throw new Error(`${resolve(scriptDir)}: no game at or above this script (no app.json or logic/src/lib.rs); pass --app <game directory>`);
  }
}

export async function proof(meta, script) {
  try { await runProof(meta, script); }
  catch (error) {
    console.error(`PROOF FAIL: ${formatProofError(error)}`);
    process.exitCode = 1;
  }
}
async function runProof(meta, script) {
  const args = process.argv.slice(2), device = args.includes('--device');
  const named = args.includes('--app') ? args[args.indexOf('--app') + 1] : undefined;
  const app = proofGame(fileURLToPath(new URL('.', meta.url)), args) + sep, name = process.env.EXACT_PROOF_NAME ?? basename(app);
  const root = fileURLToPath(new URL('..', import.meta.url));
  const phone = args.includes('--phone') ? args[args.indexOf('--phone') + 1] : undefined;
  const host = args.find(arg => !arg.startsWith('--') && arg !== phone && arg !== named) ?? 'linux';
  const appFlag = named === undefined ? [] : ['--app', app];
  if (device && host !== 'ios') throw new Error('--device requires the ios proof host');
  if (args.includes('--phone') && (!device || !phone || phone.startsWith('--'))) throw new Error('--phone requires --device and a device name or identifier');
  const destination = device ? 'ios-device' : host;
  const out = resolve(process.env.EXACT_PROOF_OUT ?? resolve(app, 'artifacts', destination));
  const buildOut = resolve(process.env.EXACT_PROOF_BUILD_OUT ?? resolve(app, 'artifacts'));
  mkdirSync(buildOut, {recursive:true});
  // Web builds replace dist atomically on every invocation. Keep that churn
  // inside the excluded artifact tree, away from monitored source directories.
  const dist = resolve(process.env.EXACT_WEB_DIST ?? resolve(buildOut, 'web-dist'));
  mkdirSync(out, {recursive:true});
  // Re-execute the actual proof, comparing every session's final simulation state.
  if (process.argv.includes('--paranoid')) {
    if (!['web', 'linux', 'windows'].includes(host)) throw new Error('--paranoid supports web, linux and windows');
    let unverified = false;
    const failed = await paranoidRuns(async mode => {
      const started = performance.now();
      const child = spawn(process.execPath, [fileURLToPath(meta.url), host, ...appFlag], {
        env:{...process.env, EXACT_GAME_PARANOID:mode, EXACT_GAME_PARANOID_COMPARE:'1'}, stdio:'inherit',
      });
      const code = await new Promise((ok, reject) => { child.on('exit', ok); child.on('error', reject); });
      unverified ||= code === 3;
      console.log(`PARANOID ${name} ${host} ${mode}: ${((performance.now()-started)/1000).toFixed(3)} s (including build)`);
      return code === 3 ? 0 : code;
    }, async () => {
      const child = spawn(process.execPath, [fileURLToPath(meta.url), host, ...appFlag, '--build-only'], {
        env:{...process.env, EXACT_GAME_PARANOID:'0'}, stdio:'inherit',
      });
      const code = await new Promise((ok, reject) => { child.on('exit', ok); child.on('error', reject); });
      return code === 3 ? 0 : code;
    }, host);
    console.log(`PROOF ${failed ? 'FAIL' : unverified ? 'UNVERIFIED' : 'PASS'} ${name}: paranoid modes ${failed ? 'failed' : unverified ? 'completed without a complete baseline' : 'checked the baseline'}`);
    process.exit(failed ? 1 : unverified ? 3 : 0);
  }
  const finalWorlds = [], observations = new Map();
  const modes = executedModes(process.env.EXACT_GAME_PARANOID ?? '0');
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
  const provenance = editProvenance();
  const {pins, pin, pinSave} = pinRecorder(previousPins, app, check, collecting, provenance);
  // The look step runs once per proof mode 0: the paranoid modes check the simulation.
  const silence = lookSilence(app), looking = (process.env.EXACT_GAME_PARANOID ?? '0') === '0';
  const sessionIds = new WeakMap(), account = lookAccount(out, session => sessionIds.get(session));
  const {looks, show} = account, look = (...args) => account.look(...args);
  // At a session's close: each world's look frame from its own camera at the held tick, which is judged; the page
  // as composited where the canvas draws; and a contact sheet over 600 ms at half resolution, which is a picture.
  const lookStep = async (raw, id) => {
    const nodes = ((await raw.tree()).nodes ?? []).filter(n => n.world);
    for (const node of nodes) {
      const canvas = node.props?.testId ?? node.id;
      const label = `look-${id}${nodes.length > 1 ? `-${canvas}` : ''}`, world = {session:id, id:node.id, canvas};
      show(id, node.id, canvas);
      looks.closing.set(`${id}:${node.id}`, world);
      try {
        world.picture = (await look(raw, {}, label, canvas)).picture;
        if (!['linux', 'windows'].includes(host)) looks.pages.push((await raw.screenshot(resolve(out, `${label}-page.png`))).screenshot);
        looks.sheets.push((await raw.screenshot(resolve(out, `${label}-sheet.png`), {over:600, every:50, window:file => lookPicture(raw, file, canvas, {resolution:0.5})})).screenshot);
      } catch (error) {
        if (error.unavailable) world.unavailable = error.unavailable;
        else if (!world.picture) world.error = `look frame failed: ${error.message}`;
        else looks.gaps.push(`session ${id} ${canvas}: ${error.message}`);
      }
    }
  };
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
        const journal = (await raw.logs({all:true})).world ?? [];
        const state = await raw.state();
        modes.observe(state, id);
        try { provenance.observe(state, journal, id); provenance.assert(); } catch (error) { check(error.message, false); }
        if (compareParanoid || process.env.EXACT_PROOF_COMPARE === '1') {
          const state = await raw.op({op:'state', ...await raw.target('world'), world:true});
          const world = state.world;
          const logs = journal;
          finalWorlds.push({session:id, tick:world?.tick, hash:world?.hash,
            published:world?.published,
            // Native hosts own incremental journal cursors, even for since:0.
            // Include the chunks this script already read as well as the tail.
            // A look frame's tell lines are presentation: out of the comparison.
            journal:worldJournal([...replies.filter(r => r.session === id && r.method === 'logs')
              .flatMap(r => r.reply?.world ?? []), ...logs])});
          if (!world?.hash) throw new Error('paranoid comparison: final world hash missing');
        }
        if (looking) await lookStep(raw, id);
        sample();
      } finally {
        if (host === 'web' && !options.fresh && !options.world && !options.plan && !reusableWeb) { reusableWeb = raw.carrier; reusableOptions = signature; }
        else await raw.close();
        closed = true;
      }
    } };
    sessions.add({close});
    const id = sessions.size;
    sessionIds.set(raw, id);
    const session = new Proxy(raw, {get(target, method) {
      if (method === 'close') return close;
      // world(name) runs with the proxy as its receiver: its operations stay recorded.
      if (!['op','tap','type','clock','state','tree','layout','logs','screenshot'].includes(method)) return target[method];
      return async (...args) => {
        try {
          const before = await raw.state();
          modes.observe(before, id);
          provenance.observe(before, [], id);
          const reply = await target[method](...args);
          const after = await raw.state();
          modes.observe(after, id);
          provenance.observe(after, method === 'logs' ? reply.world : [], id);
          if (method === 'state') worldObservations(observations, id)(reply);
          replies.push({session:id, method, args, reply, clock:target.now});
          return reply;
        } catch (error) {
          replies.push(proofFailureRow(id, method, args, target.now, error)); if (error.steps) say(render('type', {steps:error.steps})); throw error;
        }
      };
    }});
    sessionIds.set(session, id);
    return session;
  };
  let inputDigest, sourceInputDigest, lookFailed = false;
  try {
    if (!['web','macos','ios','linux','windows'].includes(host)) throw new Error(`proof host unavailable: ${host}`);
    const appInfo = resolveApp(name);
    // Make what the build would create in the game's directory before the first input
    // inventory (its Cargo target, .shells/, the baked assets/): the build then rewrites
    // the same files and creates no entry, so a fresh game's first proof completes.
    mkdirSync(appInfo.target, {recursive:true});
    if (prepareProofTree(app, {...process.env, CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR ?? resolve(app, 'target')})) say(`BAKE ${name} art/ into assets/`);
    let inputs;
    const scanInputs = proofInputReader(root, app, resolve(buildOut, 'input-digests.json'));
    const readInputs = () => inputs = scanInputs();
    const inputHash = value => buildInputHash(destination, appInfo.target, process.env.EXACT_GAME_PARANOID ?? '0').update(value).digest('hex');
    const receipt = resolve(buildOut, `build-${destination}.sha256`);
    const linuxTarget = host === 'linux' ? spawnSync('rustc', ['-vV'], {encoding:'utf8'}).stdout.match(/^host: (.+)$/m)?.[1] : null;
    const profile = process.env.EXACT_GAME_PROOF_PROFILE ?? 'gpu-dev';
    if (!['gpu-dev', 'release'].includes(profile)) throw new Error('EXACT_GAME_PROOF_PROFILE must be gpu-dev or release');
    const artifacts = host === 'windows' ? {binary:resolve(appInfo.dir, 'dist-windows', `${executableName(appInfo)}.exe`), module:resolve(appInfo.dir, `dist-windows/${appInfo.crate('gpu').replaceAll('-','_')}.dll`)}
      : host === 'linux' ? {binary:resolve(appInfo.target, linuxTarget, `${profile}/${appInfo.crate('linux')}`), module:resolve(appInfo.target, linuxTarget, `${profile}/lib${appInfo.crate('gpu').replaceAll('-','_')}.${process.platform === 'darwin' ? 'dylib' : 'so'}`)} : host === 'web' ? null : appleArtifacts(appInfo, {destination:host === 'macos' ? 'macos' : device ? 'ios' : 'ios-simulator'});
    if (host === 'linux') process.env.EXACT_LINUX_BIN = artifacts.binary;
    if (host === 'windows') process.env.EXACT_WINDOWS_BIN = artifacts.binary;
    const validatedProducts = {};
    const validatedProduct = kind => validatedProducts[kind] = productDigest(artifacts[kind],resolve(buildOut,`product-${host}-${kind}.json`));
    const built = await withProofBuild(appInfo.target, async () => {
    const rebuilt = host === 'linux' && profile === 'gpu-dev' ? await ensureLinuxReceipts({
      directory:buildOut, app, surface:resolve(appInfo.workspace,'surfaces.json'), gpuInputs:() => inputHash(readInputs().gpu),
      completeGpu:() => {
        const receipt = JSON.parse(readFileSync(resolve(buildOut,'build-linux-gpu.sha256')));
        writeFileSync(artifacts.module + '.proof.json',JSON.stringify({...receipt,artifact:productDigest(artifacts.module,resolve(buildOut,'product-linux-gpu.json'))}));
      },
      hostInputs:() => inputHash(readInputs().host + readFileSync(resolve(appInfo.workspace,'surfaces.json'),'utf8') + (existsSync(resolve(appInfo.workspace,'keys.json')) ? readFileSync(resolve(appInfo.workspace,'keys.json'),'utf8') : '')),
      gpuArtifact:() => validatedProduct('module'),
      hostArtifact:() => validatedProduct('binary'),
      build:async part => {
        say(`BUILD ${part} stale or missing receipt`);
        buildBake(appInfo,'linux',linuxTarget,{profile,part,env:{EXACT_GAME_PARANOID:'0'}});
      },
    }) : await ensureBuildReceipt({receipt, inputs:() => inputHash(readInputs().all),
      artifact:() => { const digest = artifactDigest(host, dist, artifacts); if (host === 'linux' || host === 'windows') { validatedProduct('binary'); validatedProduct('module'); } return digest; }, build:async () => {
      if (host === 'web' && takeWarmGame(appInfo, dist, {...process.env, EXACT_GAME_PROOF_PROFILE:profile})) {
        const marker = resolve(dist, '.exact-build.json');
        const built = JSON.parse(readFileSync(marker, 'utf8'));
        built.inputs = webInputDigests(appInfo, false);
        built.files = await publicFileCards(dist).finally(closeFilesystemReader);
        writeFileSync(marker, JSON.stringify(built) + '\n');
        return;
      }
      say(`BUILD stale or missing receipt ${receipt}; rebuilding: bun ${fileURLToPath(meta.url)} ${host}${device ? ' --device' : ''} --build-only`);
      if (host === 'linux') {
        if (!linuxTarget) throw new Error('rustc did not report its target');
        // Native mode is read at launch; keep compile-time environment stable.
        buildBake(appInfo, 'linux', linuxTarget, {profile, env:{EXACT_GAME_PARANOID:'0'}});
      } else {
        const script = host === 'windows' ? 'host/windows/build.mjs' : host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs';
        const flags = host === 'windows' ? (profile === 'release' ? ['--release'] : []) : host === 'web' ? ['--wasm'] : device ? ['--device', ...(phone ? ['--phone', phone] : [])] : host === 'ios' ? ['--ios'] : host === 'macos' ? ['--bundle'] : [];
        const child = spawn('bun', [resolve(root,script), ...flags], {cwd:root, env:{...process.env, EXACT_GAME_PROOF_PROFILE:profile}, stdio:'inherit', windowsHide:true});
        sample();
        const code = await new Promise((ok, reject) => {child.on('exit',ok); child.on('error',reject);});
        if (code !== 0) throw new Error(`app build exited ${code}`);
      }
    }});
    if (host === 'linux') {
      const privateProducts = copyProofProducts(root, app, artifacts, resolve(app,'artifacts/build',`${host}-${profile}`), validatedProducts);
      process.env.EXACT_LINUX_BIN = privateProducts.binary;
      process.env.EXACT_PROOF_PRODUCTS = privateProducts.receipt;
    }
    return rebuilt;
    });
    // The proof reports the same post-build input tree that its receipts bind.
    inputDigest = inputs.all;
    sourceInputDigest = inputs.authored;
    if (!built) say(`BUILD cached ${name} ${destination}`);
    if (!process.argv.includes('--build-only')) {
      await script({open, check, equal, out, host, say, pin, pinSave, look});
      if (!process.argv.some(arg => ['--screenshot-only','--capture40'].includes(arg)))
        for (const section of ['ticks','saves']) for (const key of Object.keys(previousPins[section] ?? {}))
          check(`pin ${key} observed; if intentionally removed, update the proof and pins.json together`, key in pins[section]);
    }
  // A host's typed refusal (a browser without WebGPU: `host web refused (gpu): …`) is the leg's result, by name.
  } catch (error) { check(refusalMessage(error?.reply) ?? 'proof interrupted',false,formatProofError(error)); }
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
    if (!process.argv.includes('--build-only')) {
      try { modes.assert(); check('executed mode and reconstruction evidence', true); }
      catch (error) { check(error.message, false); }
    }
    if (compareParanoid) {
      const paranoidSamples = modes.rows();
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
    const verdict = proofVerdict({failures, expected:previousPins, pins, collecting}, process.argv);
    const simulation = verdict.status;
    // The frames the script's settles, screenshots and look requests judged (agent-inspect.mjs worldTells), and the
    // worlds every reply showed; a world shown and never looked at is a gap.
    for (const r of replies) account.replied(r);
    const worlds = account.worlds();
    const seen = lookResult({...looks, worlds}, silence), lookFailures = [];
    // A tell fails the look, never the simulation: its own list, its own line.
    for (const tell of seen.loud) {
      const label = `${tell.line}${tell.times > 1 ? ` (and ${tell.times - 1} more frames)` : ''}; silence ${tell.name} in app.json's game.look.silence only if the look is intended`;
      if (looking) { say(`FAIL ${label}`); lookFailures.push(label); }
    }
    lookFailed = looking && seen.status === 'FAIL';
    const status = lookFailed ? 'FAIL' : simulation;
    const reason = lookFailed ? `${verdict.reason}; the look failed` : verdict.reason;
    if (!compareParanoid && process.env.EXACT_PROOF_COMPARE !== '1') finalWorlds.push(...observations.values());
    writeFileSync(resolve(out,'summary.json'), JSON.stringify({name, host, device, ...(phone ? {phone} : {}), status, inputs:inputDigest, sourceInputs:sourceInputDigest, mode:process.env.EXACT_GAME_PARANOID ?? '0', pins, paranoid:modes.rows(), facilities:facilityReport(replies), failures, seconds:(performance.now()-started)/1000, worlds:finalWorlds, saves, auditUnavailable, simulation, ...(looking ? {look:{status:seen.status, detail:seen.detail, failures:lookFailures, tells:looks.tells, silenced:seen.quiet.map(t => t.name), missing:seen.missing, frames:looks.frames, worlds, pictures:looks.pictures, pages:looks.pages, sheets:looks.sheets}} : {})}, null, 2)+'\n');
    if (process.argv.includes('--report')) for (const hint of facilityReport(replies)) say(`REPORT ${hint}`);
    if (looking) {
      say(`simulation: ${simulation} · look: ${seen.status}, ${seen.detail}`);
      for (const tell of seen.quiet) say(`look: silenced ${tell.name} by app.json (${tell.line}${tell.times > 1 ? `, and ${tell.times - 1} more frames` : ''})`);
      for (const missing of seen.missing) say(`look: unverified, ${missing}`);
      for (const picture of [...looks.pages, ...looks.pictures]) say(`LOOK ${picture}`);
      for (const sheet of looks.sheets) say(`LOOK sheet ${sheet}`);
      if (looks.pictures.length) say('LOOK: open one of these once and check it reads as designed before you report; the tells see brightness and coverage, not colour, facing or layout.');
    }
    say(`PROOF ${status} ${name} ${destination}: ${failures.length} failures; ${reason}; ${((performance.now()-started)/1000).toFixed(3)} s`);
    if (status === 'PASS' && host === 'linux') say(`Capture the PNG: ${proofCommand(fileURLToPath(meta.url), 'web')}`);
    if (status === 'UNVERIFIED' && !collecting) say(`UNVERIFIED: ${verdict.reason}; run ${proofCommand(resolve(import.meta.dir, 'prove.mjs'), app)}`);
    if (!collecting) {
      const gaps = pinCoverageGaps(previousPins);
      if (gaps.length) say(`UNVERIFIED: ${gaps.join('; ')}; run prove --release (with --repin to publish coverage)`);
    }
    writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
    writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
  }
  process.exit(lookFailed ? 1 : proofVerdict({failures, expected:previousPins, pins, collecting}, process.argv).code);
}
