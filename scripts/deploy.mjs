#!/usr/bin/env node
// `exact deploy` — the publisher (LLP 1030.000 D3 the verb, D4 the policies,
// D5 one library in its own process, D7 where things live; LLP 1030 D3 the
// classifier and D3a the compatibility id; LLP 1026 D11 the signed head).
// The repo has no `exact` binary; this script is the verb:
//
//   node scripts/deploy.mjs <app> [--origin <dir|url>] [--channel <name>]
//       [--only bundle|origin] [--platform <p>]… [--snapshot <sha>] [--dirty]
//       [--release <id>] [--keys <dir>] [--json] [--yes] [--slow-ms <n>]
//   node scripts/deploy.mjs keygen <id> [--keys <dir>] [--json]
//
// In order, as D3 states it: **snapshot** — the app's tree must be committed
// (`--dirty` publishes the working tree and says so loudly) and the snapshot
// is `HEAD` of the repository holding the app; **bake** into a run-specific
// directory, `target/deploy/<release>/web`, never the dev server's shared
// `dist/`, then the compatibility id per platform from `contract compat`;
// **classify** against the live heads on the origin — the origin row (which
// root files change), one row per stream (what changed against that head:
// the plan, each asset; a `.wgsl` whose interface digest is the cohort's is
// an asset), and a binary row for a stream whose cohort is not this
// snapshot's; **print** the table, which is the whole output of a dry run,
// the default; and with `--yes` **publish** through the origin adapter
// (`scripts/origin.mjs`): blobs first, each read back and its digest checked,
// then the stream's `app.plan` and `assets/<name>`, then the signed head as a
// conditional put with `seq` read under the stream's lock — never from a
// local file — and the release record beside it; the web root's hashed files
// first and `index.html` last. A failed step leaves the previous head.
//
// The **bundle** for a platform is `app.plan` plus every asset the bake
// listed in `exact.json` (`assets/`, `deck/`, `shaders/*.wgsl`): the same
// bytes for every platform in v1. Per-platform bundles arrive with modules
// (LLP 1029). The head is signed with Ed25519 over the canonical bytes
// exactly as `update/src/envelope.rs` defines them — `signature` removed,
// keys sorted recursively, no whitespace, integers only — with the private
// key `<keys dir>/<deploy.signing.key>.pem` (PKCS#8; `--keys`,
// `EXACT_SIGNING_KEY_DIR`, default `~/.config/exact/keys`), whose public half
// must be the manifest's `deploy.signing.keys[<id>]`. `keygen` writes a fresh
// key and prints the public half to paste in; the private key never enters
// the repository. `update/tests/publisher.rs` reads a head this script
// produced and proves the client verifies it.
//
// Owed, not built: `--watch` (D5's continuous publisher: the same library,
// a separate least-privileged process watching the deploy branch); an
// object-store adapter (an https origin is read-only here); the binary
// lanes (1030.000 §6). `dev.mjs` prints its conservative line per edit and
// does not yet import the classifier here.
//
// Flags: `--slow-ms <n>` holds a stream's lock for n ms between reading the
// head and writing the next one — a test flag for racing two publishers.
import { spawnSync } from 'node:child_process';
import { createPrivateKey, createPublicKey, generateKeyPairSync, randomBytes, sign, verify } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir, hostname, userInfo } from 'node:os';
import { relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveApp } from './app.mjs';
import { blobPath, openOrigin, OriginUnavailable, sha256, streamPath } from './origin.mjs';
import { listPublicFiles } from '../host/web/serve.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const PLATFORMS = ['web', 'ios', 'macos', 'linux'];
/** The platforms that hold an update store and so a stream, when the manifest names none (`deploy.store`, `deploy.binaries`). The web is the origin row: a fresh load is current. */
const STREAM_PLATFORMS = ['ios', 'macos', 'linux'];
const USAGE = 'usage: node scripts/deploy.mjs <app> [--origin <dir|url>] [--channel <name>] [--only bundle|origin] [--platform <web|ios|macos|linux>]... [--snapshot <sha>] [--dirty] [--release <id>] [--keys <dir>] [--json] [--yes]\n       node scripts/deploy.mjs keygen <id> [--keys <dir>] [--json]';

/** A refusal: printed as one line, exit 1. Anything else is a bug and keeps its stack. */
class Refusal extends Error {}
const refuse = (message) => { throw new Refusal(message); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ------------------------------------------------------------------ arguments

function parseArgs(argv) {
  const opts = { platform: [], _: [] };
  const valued = new Set(['--origin', '--channel', '--only', '--platform', '--snapshot', '--release', '--keys', '--slow-ms']);
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--json') opts.json = true;
    else if (a === '--yes') opts.yes = true;
    else if (a === '--dirty') opts.dirty = true;
    else if (a === '--help' || a === '-h') opts.help = true;
    else if (valued.has(a)) {
      const v = argv[++i];
      if (v === undefined || v.startsWith('--')) refuse(`${a} needs a value`);
      if (a === '--platform') opts.platform.push(v);
      else opts[a.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = v;
    } else if (a.startsWith('-')) refuse(`unknown flag ${a}\n${USAGE}`);
    else opts._.push(a);
  }
  for (const p of opts.platform) if (!PLATFORMS.includes(p)) refuse(`--platform ${p}: one of ${PLATFORMS.join(', ')}`);
  if (opts.only && opts.only !== 'bundle' && opts.only !== 'origin') refuse(`--only ${opts.only}: bundle or origin`);
  if (opts.release && !/^[A-Za-z0-9._-]+$/.test(opts.release)) refuse(`--release ${opts.release}: letters, digits, . _ - only (it names a directory and a file)`);
  if (opts.slowMs !== undefined && !(Number(opts.slowMs) >= 0)) refuse(`--slow-ms ${opts.slowMs}: a number of milliseconds`);
  opts.keys = resolve(opts.keys ?? process.env.EXACT_SIGNING_KEY_DIR ?? resolve(homedir(), '.config/exact/keys'));
  return opts;
}

// ---------------------------------------------------------------- the signer

/** Every object's keys sorted (JavaScript's default sort, by UTF-16 code unit, agrees with UTF-8 byte order on every key an envelope has), arrays in order, integers only — the rule `update/src/envelope.rs` states for `canonical_bytes`. */
export function sortKeysDeep(value) {
  if (Array.isArray(value)) return value.map(sortKeysDeep);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map((k) => [k, sortKeysDeep(value[k])]));
  if (typeof value === 'number' && !Number.isInteger(value)) refuse(`the head carries the non-integer number ${value}; canonical bytes are integers only (update/src/envelope.rs)`);
  return value;
}

/** The bytes the signature covers: the head without its top-level `signature`, `JSON.stringify` over recursively sorted keys (LLP 1026 D11; LLP 1030 D3a). */
export function canonicalBytes(head) {
  const { signature, ...rest } = head;
  return Buffer.from(JSON.stringify(sortKeysDeep(rest)), 'utf8');
}

/** An Ed25519 public key from its 32 raw bytes: the SubjectPublicKeyInfo prefix Node wants, then the bytes. */
export function publicKeyFromRaw(raw) {
  if (raw.length !== 32) refuse(`an Ed25519 public key is 32 bytes; this one is ${raw.length}`);
  return createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), raw]), format: 'der', type: 'spki' });
}

/** The 32 raw bytes of an Ed25519 public key object. */
const rawPublic = (key) => key.export({ type: 'spki', format: 'der' }).subarray(-32);

/** The signer for the app's `deploy.signing.key`: the PEM at `<keys>/<id>.pem`, refused by path when missing, and refused when the manifest's public key for that id is not this private key's. A dry run never calls this. */
export function loadSigner(app, keysDir) {
  const signing = app.manifest.deploy?.signing ?? {};
  const keyId = signing.key;
  if (!keyId) refuse(`${app.dir}/app.json names no deploy.signing.key: a publish signs every head (LLP 1026 D11); node scripts/deploy.mjs keygen <id> makes one`);
  const path = resolve(keysDir, `${keyId}.pem`);
  if (!existsSync(path)) refuse(`no signing key at ${path} — node scripts/deploy.mjs keygen ${keyId} --keys ${keysDir} writes one (and prints the public half for app.json's deploy.signing.keys["${keyId}"])`);
  let privateKey;
  try { privateKey = createPrivateKey(readFileSync(path)); } catch (e) { refuse(`${path} is not a PEM private key: ${e.message}`); }
  if (privateKey.asymmetricKeyType !== 'ed25519') refuse(`${path} is a ${privateKey.asymmetricKeyType} key; the head is signed with Ed25519`);
  const declared = signing.keys?.[keyId];
  if (!declared) refuse(`${app.dir}/app.json's deploy.signing.keys has no "${keyId}": the binary verifies with that key (LLP 1030 D3a); paste the public half keygen printed`);
  const manifestKey = publicKeyFromRaw(Buffer.from(declared, 'base64'));
  const probe = Buffer.from('exact deploy: is this key mine?');
  if (!verify(null, probe, manifestKey, sign(null, probe, privateKey))) refuse(`the manifest's public key for ${keyId} is not this private key's (${path}): a head signed here would be refused by every installed binary`);
  return {
    keyId,
    path,
    publicKey: rawPublic(createPublicKey(privateKey)).toString('base64'),
    /** The `signature` member for `head` (which must not carry one yet). */
    sign(head) { return { keyId, ed25519: sign(null, canonicalBytes(head), privateKey).toString('base64') }; },
  };
}

/** `keygen <id>`: a fresh Ed25519 key at `<keys>/<id>.pem` (mode 0600, never overwritten) and its public half printed for the manifest. */
function keygen(opts) {
  const id = opts._[1];
  if (!id || !/^[A-Za-z0-9._-]+$/.test(id)) refuse(`keygen <id>: letters, digits, . _ - (it names the PEM and the manifest entry)\n${USAGE}`);
  const path = resolve(opts.keys, `${id}.pem`);
  if (existsSync(path)) refuse(`${path} exists: a signing key is never overwritten — remove it yourself, or pick another id (a rotation is a new cohort, LLP 1030 D3a)`);
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  mkdirSync(opts.keys, { recursive: true, mode: 0o700 });
  try {
    writeFileSync(path, privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600, flag: 'wx' });
  } catch (error) {
    if (error.code === 'EEXIST') refuse(`${path} exists: a signing key is never overwritten — remove it yourself, or pick another id (a rotation is a new cohort, LLP 1030 D3a)`);
    throw error;
  }
  const raw = rawPublic(publicKey).toString('base64');
  if (opts.json) console.log(JSON.stringify({ id, path, publicKey: raw }));
  else console.log(`wrote ${path}\npaste into app.json → deploy.signing.keys["${id}"]: ${JSON.stringify(raw)}`);
  return 0;
}

// -------------------------------------------------------------- the snapshot

/** The snapshot (LLP 1030.000 D3 item 1): `HEAD` of the repository holding the app, with the app's tree clean unless `--dirty`. */
function snapshotOf(app, opts) {
  const git = (...args) => spawnSync('git', args, { cwd: app.dir, encoding: 'utf8' });
  const top = git('rev-parse', '--show-toplevel');
  if (top.status !== 0) refuse(`${app.dir} is not in a git repository: exact deploy publishes a snapshot, never a mutable tree (LLP 1030.000 D3)`);
  const commit = git('rev-parse', 'HEAD').stdout.trim();
  if (!/^[0-9a-f]{40}$/.test(commit)) refuse(`${app.dir}: git has no HEAD commit to snapshot`);
  const status = git('status', '--porcelain', '--', '.').stdout.split('\n').filter(Boolean);
  if (status.length && !opts.dirty) refuse(`the app tree under ${app.dir} has uncommitted changes:\n  ${status.join('\n  ')}\ncommit them, or pass --dirty to publish the working tree as it is (the table says so loudly)`);
  if (opts.snapshot && !commit.startsWith(opts.snapshot.toLowerCase())) refuse(`HEAD is ${commit}, not --snapshot ${opts.snapshot}: the dry run and its --yes are the same snapshot or the second refuses (LLP 1030.000 D3)`);
  return { commit, dirty: status.length > 0, changes: status, repo: top.stdout.trim() };
}

/** A human correlation id with millisecond UTC time, snapshot prefix, and a
 * random run nonce. Two publishers of the same commit in one clock tick do
 * not share the receipt namespace. */
export function defaultRelease(commit, now = new Date(), nonce = randomBytes(8).toString('hex')) {
  return `r-${now.toISOString().replace(/[-:]/g, '')}-${commit.slice(0, 7)}-${nonce}`;
}

/** A private bake directory independent of the correlation id. Explicitly
 * reusing `--release` can never make one process remove another's stage. */
export function deployRun(target, release) {
  const root = resolve(target, 'deploy');
  mkdirSync(root, { recursive: true });
  return mkdtempSync(resolve(root, `${release}-`));
}

// ------------------------------------------------------------------ the bake

/** Bake the web app into `<run>/web` (`host/web/build.mjs` with `EXACT_WEB_DIST`): its output goes to stderr so stdout stays the table. Returns the directory. */
function bake(app, run) {
  mkdirSync(run, { recursive: true });
  const web = resolve(run, 'web');
  const r = spawnSync(process.execPath, [resolve(ROOT, 'host/web/build.mjs'), app.crate('web')], {
    cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: web }, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
  });
  if (r.stdout) process.stderr.write(r.stdout);
  if (r.status !== 0) refuse(`the bake failed: host/web/build.mjs ${app.crate('web')} exited ${r.status ?? r.signal}`);
  if (!existsSync(resolve(web, 'exact.json'))) refuse(`the bake wrote no exact.json under ${web}`);
  return web;
}

/** The bundle the bake produced (LLP 1023 D2's cards with their bytes): the plan and every asset `exact.json` lists — the same bytes for every platform in v1. */
function readBundle(web) {
  const envelope = JSON.parse(readFileSync(resolve(web, 'exact.json'), 'utf8'));
  const plan = readFileSync(resolve(web, 'app.plan'));
  if (sha256(plan) !== envelope.plan.sha256) refuse(`${web}/app.plan is not the plan exact.json names`);
  const assets = (envelope.assets ?? []).map((card) => {
    const bytes = readFileSync(resolve(web, card.name));
    if (sha256(bytes) !== card.sha256) refuse(`${web}/${card.name} is not the asset exact.json names`);
    return { name: card.name, sha256: card.sha256, bytes };
  });
  return { envelope, plan: { sha256: envelope.plan.sha256, bytes: plan, formatVersion: envelope.plan.formatVersion, kernelSchema: envelope.plan.kernelSchema }, assets };
}

/** The compatibility id and its inputs for `platform` (LLP 1030 D3a), from `contract compat` with the platform's default target. */
function compatOf(app, platform) {
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'compat', app.dir, '--platform', platform, '--json'], { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 16 * 1024 * 1024 });
  if (r.status !== 0) refuse(`contract compat --platform ${platform} failed:\n${r.stderr}`);
  const compat = JSON.parse(r.stdout);
  if (!/^[0-9a-f]{32}$/.test(compat.id ?? '')) refuse(`contract compat --platform ${platform} printed no id`);
  return compat;
}

// --------------------------------------------------------------- classifying

/** The channel the manifest bakes in: `deploy.channel`, else the only key of `deploy.channels`, else `prod` — the same rule as `contract/cli/src/compat.rs`. */
function channelOf(manifest) {
  const deploy = manifest.deploy ?? {};
  if (deploy.channel) return deploy.channel;
  const names = Object.keys(deploy.channels ?? {});
  return names.length === 1 ? names[0] : 'prod';
}

/** The platforms with a stream: those `deploy.store` and `deploy.binaries` name (an `off` binary still has its cohort), `STREAM_PLATFORMS` when neither does; the web is the origin row. */
function streamPlatforms(manifest) {
  const deploy = manifest.deploy ?? {};
  const named = [...new Set([...Object.keys(deploy.store ?? {}), ...Object.keys(deploy.binaries ?? {})])].filter((p) => p !== 'web');
  const unknown = named.filter((p) => !PLATFORMS.includes(p));
  if (unknown.length) refuse(`app.json names the platform${unknown.length > 1 ? 's' : ''} ${unknown.join(', ')} under deploy; this repo builds ${PLATFORMS.join(', ')}`);
  return PLATFORMS.filter((p) => (named.length ? named : STREAM_PLATFORMS).includes(p));
}

/** What the bundle changes against a head: `app.plan` and each asset by digest — new, changed, removed — with a `.wgsl` whose cohort is this one marked an asset (its interface digest is an input to the id, so an unchanged id is an unchanged interface). */
function changesAgainst(bundle, head) {
  const changes = [];
  if (!head) {
    changes.push({ name: 'app.plan', change: 'new' });
    for (const a of bundle.assets) changes.push({ name: a.name, change: 'new' });
    return changes;
  }
  if (head.plan?.sha256 !== bundle.plan.sha256) changes.push({ name: 'app.plan', change: 'changed' });
  const before = new Map((head.assets ?? []).map((a) => [a.name, a.sha256]));
  for (const a of bundle.assets) {
    const had = before.get(a.name);
    if (had === undefined) changes.push({ name: a.name, change: 'new' });
    else if (had !== a.sha256) changes.push({ name: a.name, change: 'changed', note: a.name.endsWith('.wgsl') ? 'interface unchanged: asset' : undefined });
    before.delete(a.name);
  }
  for (const name of before.keys()) changes.push({ name, change: 'removed' });
  return changes;
}

/** The unsigned stream head. One producer keeps its identity/name and blob
 * pointers identical across classification fixtures and publication. */
export function streamHead({ app, bundle, stream, seq, release, sunset = null }) {
  const head = {
    exact: 1,
    app: { id: app.id, name: app.displayName },
    plan: { url: './app.plan', sha256: bundle.plan.sha256, bytes: bundle.plan.bytes.length,
      formatVersion: bundle.plan.formatVersion, kernelSchema: bundle.plan.kernelSchema },
    assets: bundle.assets.map((asset) => ({ name: asset.name, url: `./assets/${asset.name}`,
      sha256: asset.sha256, bytes: asset.bytes.length })),
    stream: { app: app.id, channel: stream.channel, compatibilityId: stream.compatibilityId, seq },
    release,
  };
  if (sunset) head.sunset = sunset.store ? { message: sunset.message, store: sunset.store } : { message: sunset.message };
  return head;
}

/** What a live head's signature says against the manifest's verification keys — the check an installed binary makes (LLP 1026 D11), so a head the classifier reads that no binary would take is named in the table. */
function headSignature(head, manifest) {
  const keys = manifest.deploy?.signing?.keys ?? {};
  const signature = head.signature;
  if (!signature) return Object.keys(keys).length ? 'unsigned: a binary with keys refuses it' : 'unsigned';
  const declared = keys[signature.keyId];
  if (!declared) return `signed by ${signature.keyId}, which app.json does not name`;
  try {
    return verify(null, canonicalBytes(head), publicKeyFromRaw(Buffer.from(declared, 'base64')), Buffer.from(signature.ed25519 ?? '', 'base64')) ? null : `signed by ${signature.keyId} but the signature does not verify`;
  } catch (e) { return `signed by ${signature.keyId} but cannot be checked: ${e.message}`; }
}

/** The latest release record under a stream, when the origin has any: it names the platform and carries the cohort's inputs, so two ids that differ are explained field by field. */
async function latestRecord(origin, stream) {
  const names = await origin.list(`${streamPath(stream)}/releases`);
  if (!names?.length) return null;
  const records = [];
  for (const name of names.filter((n) => n.endsWith('.json'))) {
    const bytes = await origin.get(`${streamPath(stream)}/releases/${name}`);
    try { records.push(JSON.parse(bytes.toString('utf8'))); } catch { /* not a record */ }
  }
  records.sort((a, b) => (a.at < b.at ? -1 : a.at > b.at ? 1 : 0));
  return records.at(-1) ?? null;
}

/** Which top-level compat inputs differ between two cohorts, by name. */
const inputsDiff = (a, b) => Object.keys({ ...a, ...b }).filter((k) => JSON.stringify(sortKeysDeep(a?.[k] ?? null)) !== JSON.stringify(sortKeysDeep(b?.[k] ?? null))).sort();

/** The table (LLP 1030 D3; 1030.000 D3 item 3): the origin row, a row per stream, a binary row per stream whose cohort this snapshot is not. Nothing is written. */
export async function classify({ app, opts, origin, channel, snapshot, release, web, bundle, compat, platforms, wantOrigin }) {
  const notes = [];
  if (snapshot.dirty) notes.push(`UNCOMMITTED CHANGES under ${relative(snapshot.repo, app.dir) || '.'} are in this snapshot (--dirty): ${snapshot.changes.join(', ')}`);
  const rows = [];

  if (wantOrigin) {
    const files = { new: [], changed: [], current: [] };
    let unavailable = null;
    for (const rel of listPublicFiles(web)) {
      const bytes = readFileSync(resolve(web, rel));
      let have;
      try { have = await origin.get(rel); }
      catch (error) { if (!(error instanceof OriginUnavailable)) throw error; unavailable = error.message; break; }
      if (!have) files.new.push(rel);
      else if (!have.equals(bytes)) files.changed.push(rel);
      else files.current.push(rel);
    }
    rows.push({ kind: 'origin', compatibilityId: compat.web?.id ?? null,
      action: unavailable ? 'unavailable' : files.new.length + files.changed.length ? 'publish' : 'current', files,
      ...(unavailable ? { reason: unavailable } : {}) });
  }

  const own = platforms.map((platform) => ({ platform, compatibilityId: compat[platform].id }));
  for (const { platform, compatibilityId } of own) {
    const stream = { channel, compatibilityId };
    let head;
    try { head = await origin.head(stream); }
    catch (error) {
      if (!(error instanceof OriginUnavailable)) throw error;
      rows.push({ kind: 'stream', platform, channel, compatibilityId,
        cohort: { L: compat[platform].inputs?.store?.L ?? '?', E: compat[platform].inputs?.executors ?? [] },
        head: null, action: 'unavailable', changes: [], reason: error.message });
      continue;
    }
    if (head) {
      const h = head.json;
      if (h.stream?.compatibilityId !== compatibilityId) refuse(`the head at ${origin.describe()}/${streamPath(stream)}/exact.json names the cohort ${h.stream?.compatibilityId}, not its own path's: the origin is inconsistent`);
      if (h.app?.id !== app.id) refuse(`the head at ${origin.describe()}/${streamPath(stream)}/exact.json is ${h.app?.id}'s, not ${app.id}'s`);
      if (!Number.isInteger(h.stream.seq) || h.stream.seq < 0) refuse(`the head at ${origin.describe()}/${streamPath(stream)}/exact.json has no integer seq`);
      const problem = headSignature(h, app.manifest);
      if (problem) notes.push(`the head of ${streamPath(stream)} (seq ${h.stream.seq}) is ${problem}`);
    }
    const inputs = compat[platform].inputs ?? {};
    const changes = changesAgainst(bundle, head?.json);
    rows.push({
      kind: 'stream', platform, channel, compatibilityId,
      cohort: { L: inputs.store?.L ?? '?', E: inputs.executors ?? [] },
      head: head ? { seq: head.json.stream.seq, sha256: head.sha256, release: head.json.release ?? null } : null,
      action: changes.length ? 'bundle' : 'current',
      seq: changes.length ? (head?.json.stream.seq ?? 0) + 1 : head.json.stream.seq,
      changes,
    });
  }

  // Streams on the origin (or in `deploy.streams`) that are not this snapshot's cohorts: a retired cohort's stream, which needs a binary (LLP 1030 D3 rule 3). Not when the run wants no stream at all (`--only origin`, `--platform web`).
  const declared = app.manifest.deploy?.streams?.filter((s) => s.channel === channel) ?? null;
  let others;
  if (!platforms.length) others = [];
  else if (declared) others = declared.map((s) => s.compatibilityId);
  else {
    try {
      const listed = await origin.list(`.exact/${channel}`);
      if (listed === null && origin.kind === 'https') notes.push(`an https origin cannot be listed: the streams classified are this snapshot's own; name deploy.streams in app.json to classify a retired cohort's`);
      others = listed ?? [];
    } catch (error) {
      if (!(error instanceof OriginUnavailable)) throw error;
      notes.push(`stream discovery unavailable at ${origin.describe()}/.exact/${channel}: ${error.message}; the snapshot's own streams are still classified`);
      others = [];
    }
  }
  for (const compatibilityId of others.filter((id) => !own.some((o) => o.compatibilityId === id))) {
    const stream = { channel, compatibilityId };
    let head, record;
    try {
      head = await origin.head(stream);
      record = await latestRecord(origin, stream);
    } catch (error) {
      if (!(error instanceof OriginUnavailable)) throw error;
      rows.push({ kind: 'stream', platform: null, channel, compatibilityId,
        cohort: null, head: null, action: 'unavailable', changes: [], reason: error.message });
      continue;
    }
    const platform = record?.platform ?? null;
    if (opts.platform.length && platform && !opts.platform.includes(platform)) continue;
    if (!head && !record && !declared) continue; // an empty directory is not a stream
    const problem = head ? headSignature(head.json, app.manifest) : null;
    if (problem) notes.push(`the head of ${streamPath(stream)} (seq ${head.json.stream?.seq}) is ${problem}`);
    const cohort = record?.compat?.inputs ?? null;
    const ours = platform && compat[platform] ? compat[platform] : null;
    const differs = cohort && ours ? inputsDiff(cohort, ours.inputs) : null;
    const reason = `the snapshot's cohort${platform ? ` for ${platform} is ${ours?.id ?? '(not built this run)'}` : ` is ${own.map((o) => `${o.platform} ${o.compatibilityId}`).join(', ')}`}, not ${compatibilityId}` +
      (differs?.length ? ` (differs in ${differs.join(', ')})` : platform ? '' : ' (no release record names its platform)') + ': a binary is needed';
    rows.push({
      kind: 'stream', platform, channel, compatibilityId,
      cohort: cohort ? { L: cohort.store?.L ?? '?', E: cohort.executors ?? [] } : null,
      head: head ? { seq: head.json.stream?.seq ?? null, sha256: head.sha256, release: head.json.release ?? null } : null,
      action: 'binary', changes: [], reason,
    });
  }

  return { release, snapshot: { commit: snapshot.commit, dirty: snapshot.dirty, changes: snapshot.changes }, app: { id: app.id, name: app.displayName }, channel, origin: { kind: origin.kind, location: origin.describe() }, dryRun: !opts.yes, notes, rows };
}

// ------------------------------------------------------------------ printing

/** The table in D3's shape: the release line, one row per line with its carrier on the right. */
function renderTable(table) {
  const line = (label, detail, action) => `${label.padEnd(8)} ${detail.padEnd(60)} → ${action}`;
  const out = [`release ${table.release} · snapshot ${table.snapshot.commit.slice(0, 12)}${table.snapshot.dirty ? ' (DIRTY)' : ''} · channel ${table.channel} · origin ${table.origin.location}${table.origin.kind === 'https' ? ' (read-only)' : ''}`];
  for (const note of table.notes) out.push(`!! ${note}`);
  for (const row of table.rows) {
    if (row.kind === 'origin') {
      const f = row.files;
      if (row.action === 'unavailable') { out.push(line('origin', `web app: ${row.reason}`, 'unavailable')); continue; }
      const summary = [f.new.length ? `${f.new.length} new` : '', f.changed.length ? `${f.changed.length} changed` : '', f.current.length ? `${f.current.length} current` : ''].filter(Boolean).join(', ');
      const named = [...f.changed, ...f.new].filter((n) => !n.startsWith('assets/') && !n.startsWith('deck/') && !n.startsWith('shaders/')).slice(0, 6);
      out.push(line('origin', `web app${row.compatibilityId ? ` (cohort ${row.compatibilityId.slice(0, 8)})` : ''}: ${summary}${named.length ? ` — ${named.join(', ')}` : ''}`, row.action === 'publish' ? 'publish (index.html last)' : 'current'));
      continue;
    }
    const cohort = row.cohort ? ` (L=${row.cohort.L}, E={${row.cohort.E.join(',')}})` : '';
    const head = row.head ? ` — head seq ${row.head.seq}${row.head.release ? ` (${row.head.release})` : ''}` : ' — no head';
    out.push(`${(row.platform ?? '?').padEnd(8)} stream ${row.compatibilityId.slice(0, 8)}${cohort}${head}`);
    if (row.action === 'unavailable') { out.push(line('', row.reason, 'unavailable')); continue; }
    if (row.action === 'binary') { out.push(line('', row.reason, 'binary')); continue; }
    if (row.action === 'current') { out.push(line('', 'app.plan and every asset as the head names them', `current (seq ${row.seq})`)); continue; }
    const assets = row.changes.filter((c) => c.name !== 'app.plan');
    const detail = !row.head
      ? `app.plan, ${assets.length} asset${assets.length === 1 ? '' : 's'}: new`
      : row.changes.map((c) => `${c.name} ${c.change}${c.note ? ` (${c.note})` : ''}`).join(', ');
    out.push(line('', detail, `bundle seq ${row.seq}`));
  }
  const binaries = table.rows.filter((r) => r.kind === 'stream' && r.action === 'binary');
  if (binaries.length) out.push(`binary needed: ${binaries.map((r) => `${r.platform ?? '?'} ${r.compatibilityId.slice(0, 8)}`).join(', ')} — a later verb (LLP 1030.000 §6); this run publishes nothing to those streams`);
  return out.join('\n');
}

// ---------------------------------------------------------------- publishing

/** Publish one stream (LLP 1030.000 D3 item 5): blobs, read back; then under the lock the head is read again, `seq` allocated from it, the stream's files written whole-or-absent, the signed head put conditionally on the digest read, and the release record written last. */
async function publishStream({ origin, row, bundle, compat, app, signer, release, snapshot, opts, log }) {
  const stream = { channel: row.channel, compatibilityId: row.compatibilityId };
  const files = [{ name: 'app.plan', sha256: bundle.plan.sha256, bytes: bundle.plan.bytes }, ...bundle.assets];
  let written = 0;
  for (const file of files) {
    if (await origin.put(blobPath(file.sha256), file.bytes, { immutable: true }) === 'written') written++;
    const back = await origin.get(blobPath(file.sha256));
    if (!back || sha256(back) !== file.sha256) refuse(`the blob ${file.sha256} (${file.name}) read back from ${origin.describe()} is not what was written`);
  }
  log(`  ${row.platform} ${row.compatibilityId.slice(0, 8)}: ${files.length} blobs on the origin (${written} written, ${files.length - written} present), each read back and checked`);
  return origin.withLock(stream, async () => {
    const current = await origin.head(stream);
    const previousDigest = current?.sha256 ?? null;
    if (current && !changesAgainst(bundle, current.json).length) return { ...row, action: 'current', seq: current.json.stream.seq, note: 'the head already names this bundle (published meanwhile)' };
    const seq = (current?.json.stream.seq ?? 0) + 1;
    if (opts.slowMs) await sleep(Number(opts.slowMs));
    const base = streamPath(stream);
    await origin.put(`${base}/app.plan`, bundle.plan.bytes);
    for (const a of bundle.assets) await origin.put(`${base}/assets/${a.name}`, a.bytes);
    const sunset = app.manifest.deploy?.sunset?.[`${stream.channel}/${stream.compatibilityId}`] ?? app.manifest.deploy?.sunset?.[stream.compatibilityId];
    const head = streamHead({ app, bundle, stream, seq, release, sunset });
    head.signature = signer.sign(head);
    const bytes = Buffer.from(JSON.stringify(head) + '\n', 'utf8');
    await origin.putHead(stream, bytes, { previousDigest });
    const digest = sha256(bytes);
    const record = {
      release, at: new Date().toISOString(), by: userInfo().username, host: hostname(),
      platform: row.platform, stream: head.stream, seq, snapshot,
      head: { sha256: digest, bytes: bytes.length, keyId: signer.keyId }, previous: previousDigest,
      compat: { id: compat.id, inputs: compat.inputs }, row: { action: row.action, changes: row.changes },
    };
    await origin.put(`${base}/releases/${release}.json`, Buffer.from(JSON.stringify(record, null, 2) + '\n', 'utf8'));
    return { ...row, action: 'published', seq, head: { seq, sha256: digest, release }, previous: previousDigest };
  });
}

/** Publish the web root: every new or changed file, hashed files first, `index.html` last. */
async function publishRoot({ origin, row, web, log }) {
  const files = [...row.files.new, ...row.files.changed].sort((a, b) => (a === 'index.html') - (b === 'index.html') || (a < b ? -1 : 1));
  for (const rel of files) await origin.put(rel, readFileSync(resolve(web, rel)));
  log(`  origin: ${files.length} files written${files.includes('index.html') ? ', index.html last' : ''}`);
  return { ...row, action: 'published', written: files };
}

// -------------------------------------------------------------------- deploy

async function deploy(opts) {
  const app = resolveApp(opts._[0]);
  const channel = opts.channel ?? channelOf(app.manifest);
  if (channel === 'blobs' || !/^[A-Za-z0-9._-]+$/.test(channel)) refuse(`the channel ${JSON.stringify(channel)} cannot name a directory under .exact/`);
  const originSpec = opts.origin ?? app.manifest.deploy?.channels?.[channel] ?? app.origin;
  if (!originSpec) refuse(`no origin for the channel ${channel}: pass --origin <dir|url> or name deploy.channels.${channel} in app.json`);
  const origin = openOrigin(originSpec);
  const wantOrigin = opts.only !== 'bundle' && (!opts.platform.length || opts.platform.includes('web'));
  const platforms = opts.only === 'origin' ? [] : streamPlatforms(app.manifest).filter((p) => !opts.platform.length || opts.platform.includes(p));
  if (!wantOrigin && !platforms.length) refuse(`nothing to classify: ${opts.only ? `--only ${opts.only}` : ''} ${opts.platform.length ? `--platform ${opts.platform.join(',')}` : ''} leaves no row`);
  const log = (text) => process.stderr.write(`${text}\n`);

  // Refuse before the bake what the bake cannot fix: a read-only origin, a missing or mismatched key.
  if (opts.yes && !origin.writable) refuse(`${origin.describe()} is an https origin, read-only in v1: point --origin at the directory the host serves (an object-store adapter with a conditional put is owed)`);
  const signer = opts.yes && platforms.length ? loadSigner(app, opts.keys) : null;
  if (signer) log(`signing as ${signer.keyId} (${signer.path})`);

  const snapshot = snapshotOf(app, opts);
  const release = opts.release ?? defaultRelease(snapshot.commit);
  const run = deployRun(app.target, release);
  log(`snapshot ${snapshot.commit}${snapshot.dirty ? ' + uncommitted changes (--dirty)' : ''}; baking into ${run}`);
  const web = bake(app, run);
  const bundle = readBundle(web);
  const compat = {};
  for (const platform of ['web', ...platforms]) compat[platform] = compatOf(app, platform);
  log(`compatibility ids: ${Object.entries(compat).map(([p, c]) => `${p} ${c.id}`).join(', ')}`);

  const table = await classify({ app, opts, origin, channel, snapshot, release, web, bundle, compat, platforms, wantOrigin });
  if (!opts.json) console.log(renderTable(table));

  if (!opts.yes) {
    if (opts.json) console.log(JSON.stringify(table));
    else console.log('dry run: nothing written — add --yes to publish');
    return 0;
  }

  const published = [];
  const refused = [];
  const failed = [];
  log('publishing');
  for (const row of table.rows) {
    if (row.kind === 'origin') {
      if (row.action === 'unavailable') { failed.push({ kind: 'origin', error: row.reason }); log(`  origin: unavailable — ${row.reason}`); continue; }
      if (row.action !== 'publish') { published.push({ ...row, action: 'current' }); continue; }
      // A step that fails leaves what was there (D3 item 5); the run goes on to the next row and exits 1.
      try { published.push(await publishRoot({ origin, row, web, log })); } catch (e) { failed.push({ kind: 'origin', error: e.message }); log(`  origin: failed — ${e.message}`); }
      continue;
    }
    const name = `${row.platform ?? '?'} stream ${row.compatibilityId.slice(0, 8)}`;
    if (row.action === 'unavailable') { failed.push({ platform: row.platform, compatibilityId: row.compatibilityId, error: row.reason }); log(`  ${name}: unavailable — ${row.reason}`); continue; }
    if (row.action === 'binary') { refused.push({ platform: row.platform, compatibilityId: row.compatibilityId, reason: row.reason }); log(`  ${name}: refused — ${row.reason}`); continue; }
    if (row.action === 'current') { published.push({ ...row }); log(`  ${name}: current (seq ${row.seq})`); continue; }
    try {
      const result = await publishStream({ origin, row, bundle, compat: compat[row.platform], app, signer, release, snapshot: table.snapshot, opts, log });
      published.push(result);
      log(result.action === 'published' ? `  ${name}: head seq ${result.seq} (${result.head.sha256.slice(0, 12)}), previous ${result.previous ? result.previous.slice(0, 12) : 'none'}` : `  ${name}: ${result.note}`);
    } catch (e) {
      failed.push({ platform: row.platform, compatibilityId: row.compatibilityId, error: e.message });
      log(`  ${name}: failed — ${e.message}; the previous head stands`);
    }
  }
  const outcome = { ...table, dryRun: false, published, refused, failed };
  if (opts.json) console.log(JSON.stringify(outcome));
  else {
    const heads = published.filter((p) => p.kind === 'stream' && p.action === 'published');
    console.log(`published ${release}: ${heads.length} head${heads.length === 1 ? '' : 's'}${published.some((p) => p.kind === 'origin' && p.action === 'published') ? ', the web root' : ''}${refused.length ? `; ${refused.length} refused (binary)` : ''}${failed.length ? `; ${failed.length} FAILED` : ''}`);
  }
  return failed.length ? 1 : 0;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (opts.help || !opts._.length) { console.log(USAGE); return opts.help ? 0 : 2; }
  if (opts._[0] === 'keygen') return keygen(opts);
  return deploy(opts);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().then((code) => { process.exitCode = code; }, (e) => {
    if (e instanceof Refusal) { console.error(`exact deploy: ${e.message}`); process.exitCode = 1; }
    else { console.error(e); process.exitCode = 1; }
  });
}
