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
// then an immutable release record and the signed head as a conditional put
// with `seq` read under the stream's lock — never from a local file. The head
// points directly at the blobs, so no live payload path is overwritten; the
// web root's immutable release comes first and its one pointer last. A failed step
// leaves the previous head and every URL it names.
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
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir, hostname, tmpdir, userInfo } from 'node:os';
import { basename, delimiter, dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveApp } from './app.mjs';
import { blobPath, openOrigin, OriginUnavailable, sha256, streamPath, parseWebRoot, webRootPath, webRootStream, webReleasePath } from './origin.mjs';
import { listPublicFiles, readStaticCandidate } from '../host/web/serve.mjs';

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
  if (opts.release && !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(opts.release)) refuse(`--release ${opts.release}: start with a letter or digit, then use letters, digits, . _ - only (it names a directory and a file)`);
  if (opts.slowMs !== undefined && !(Number(opts.slowMs) >= 0)) refuse(`--slow-ms ${opts.slowMs}: a number of milliseconds`);
  opts.keys = resolve(opts.keys ?? process.env.EXACT_SIGNING_KEY_DIR ?? resolve(homedir(), '.config/exact/keys'));
  return opts;
}

// ---------------------------------------------------------------- the signer

/** One JSON value in the signed representation. Write sorted entries
 * directly: rebuilding an object and then calling `JSON.stringify` is not
 * canonical for integer-like keys because JavaScript enumerates those in
 * numeric order regardless of insertion order. */
function requireUnicodeScalars(text) {
  for (let i = 0; i < text.length; i++) {
    const unit = text.charCodeAt(i);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const low = text.charCodeAt(i + 1);
      if (!(low >= 0xdc00 && low <= 0xdfff)) refuse('the head carries an unpaired UTF-16 surrogate, not a Unicode scalar value');
      i++;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) refuse('the head carries an unpaired UTF-16 surrogate, not a Unicode scalar value');
  }
  return text;
}

function canonicalJson(value, dropSignature = false) {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'string') return JSON.stringify(requireUnicodeScalars(value));
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || Object.is(value, -0)) refuse(`the head carries the unsafe or non-canonical integer ${value}; canonical bytes require exact integers (update/src/envelope.rs)`);
    return String(value);
  }
  if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(',')}]`;
  if (!value || typeof value !== 'object') refuse(`the head carries a ${typeof value}; canonical bytes require JSON values`);
  const keys = Object.keys(value).filter((key) => !dropSignature || key !== 'signature').map(requireUnicodeScalars)
    .sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`;
}

/** Refuse a numeric token whose spelling Rust's canonical parser cannot
 * admit. `JSON.parse` alone loses this evidence by normalizing `7.0`, `7e0`,
 * and `-0` to the number 7 or 0 before the signature check sees it. */
function validateRawIntegers(text) {
  for (let i = 0; i < text.length;) {
    if (text[i] === '"') {
      i++;
      while (i < text.length && text[i] !== '"') {
        if (text[i] !== '\\') { i++; continue; }
        if (text[i + 1] !== 'u') { i += 2; continue; }
        const first = Number.parseInt(text.slice(i + 2, i + 6), 16);
        if (first >= 0xd800 && first <= 0xdbff) {
          const second = text.slice(i + 6, i + 8) === '\\u' ? Number.parseInt(text.slice(i + 8, i + 12), 16) : NaN;
          if (!(second >= 0xdc00 && second <= 0xdfff)) throw new Error('the envelope carries an escaped lone surrogate, not a Unicode scalar value');
          i += 12;
          continue;
        }
        if (first >= 0xdc00 && first <= 0xdfff) throw new Error('the envelope carries an escaped lone surrogate, not a Unicode scalar value');
        i += 6;
      }
      if (text[i] === '"') i++;
      continue;
    }
    if (text[i] !== '-' && (text[i] < '0' || text[i] > '9')) { i++; continue; }
    const token = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(text.slice(i))?.[0];
    if (!token) { i++; continue; } // JSON.parse below reports malformed JSON.
    if (!/^(?:0|-[1-9][0-9]*|[1-9][0-9]*)$/.test(token) || !Number.isSafeInteger(Number(token))) {
      throw new Error(`the envelope carries the non-canonical integer ${token}; canonical bytes require shortest exact integers`);
    }
    i += token.length;
  }
}

/** The bytes the signature covers: the head without its top-level
 * `signature`, keys sorted by UTF-8 bytes and emitted directly. */
export function canonicalBytes(head) {
  if (!head || typeof head !== 'object' || Array.isArray(head)) refuse('the envelope is not a JSON object');
  return Buffer.from(canonicalJson(head, true), 'utf8');
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

const snapshotCaptures = new WeakMap();
const inside = (root, path) => {
  const rel = relative(root, path);
  return rel === '' || (rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel));
};
const unixPath = (path) => path.split(sep).join('/');
function canonicalPath(path) {
  const absolute = resolve(path);
  try { return realpathSync.native(absolute); } catch {
    const parent = dirname(absolute);
    return parent === absolute ? absolute : resolve(canonicalPath(parent), basename(absolute));
  }
}

/** Only actual build outputs are absent from a source snapshot. These are
 * explicit root paths, not recursive basename patterns: `assets/dist/a.png`
 * and `fixtures/target/input.rs` remain inputs when the app carries them. */
function sourcePathspec(repo, app, exactRoot) {
  const workspace = canonicalPath(app.workspace ?? app.dir);
  const outputs = [resolve(repo, 'target'), resolve(repo, 'node_modules'),
    resolve(repo, '.agent-skill-sources'), resolve(repo, '.agent-skill-backups'), resolve(repo, '.llp/ship-runs'),
    canonicalPath(app.target ?? resolve(workspace, 'target')), resolve(workspace, 'target'), resolve(workspace, 'node_modules'),
    resolve(exactRoot, 'target'), resolve(exactRoot, 'node_modules'), resolve(exactRoot, 'host/web/dist'),
    resolve(exactRoot, 'host/web/dist.previous'), resolve(exactRoot, 'host/apple/.build'),
    resolve(exactRoot, 'host/apple/macos/.build'), resolve(exactRoot, '.claude/worktrees')];
  // Keep the lexical path as well as its canonical alias. In particular,
  // `target -> /shared/cache` is still the declared in-repo output root; if
  // we realpath it first, the symlink itself re-enters the source inventory.
  const candidates = outputs.flatMap((path) => [resolve(path), canonicalPath(path)]);
  const excluded = [...new Set(candidates.filter((path) => path !== repo && inside(repo, path))
    .map((path) => unixPath(relative(repo, path))))];
  return ['.', ...excluded.flatMap((path) => [`:(exclude,top,literal)${path}`, `:(exclude,top,glob)${path}/**`])];
}

const SOURCE_MAX_BUFFER = 512 * 1024 * 1024;
function gitResult(repo, args, what, options = {}) {
  const result = spawnSync('git', args, { cwd: repo, maxBuffer: SOURCE_MAX_BUFFER, ...options });
  if (result.status !== 0) refuse(`${repo}: ${what}: ${String(result.stderr ?? result.error?.message ?? '').trim()}`);
  return result;
}

function gitText(repo, args, what, options = {}) {
  return gitResult(repo, args, what, { encoding: 'utf8', ...options }).stdout;
}

function repoTop(cwd, what = 'source') {
  const top = gitText(cwd, ['rev-parse', '--show-toplevel'], `${what} is not in a git repository`).trim();
  if (!top) refuse(`${cwd}: ${what} is not in a git repository`);
  return canonicalPath(top);
}

/** Local Cargo packages outside the app and Exact repositories are source
 * inputs too. A package can inherit fields or read inputs from its workspace
 * root, so the immutable unit is its whole repository, not just its crate. */
function cargoDependencyRoots(app, exactRoot) {
  const workspace = canonicalPath(app.workspace ?? app.dir);
  if (!existsSync(resolve(workspace, 'Cargo.toml'))) return [];
  const result = spawnSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
    cwd: workspace, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) refuse(`${workspace}: cargo cannot resolve the locked local source graph: ${result.stderr.trim()}`);
  let metadata;
  try { metadata = JSON.parse(result.stdout); }
  catch (error) { refuse(`${workspace}: cargo metadata was not JSON: ${error.message}`); }
  const owned = new Set([repoTop(app.dir, 'app source'), repoTop(exactRoot, 'Exact source')]);
  const repos = new Set();
  for (const pkg of metadata.packages ?? []) {
    if (pkg.source !== null || typeof pkg.manifest_path !== 'string') continue;
    const packageDir = canonicalPath(dirname(pkg.manifest_path));
    const repo = repoTop(packageDir, 'local Cargo dependency');
    if (owned.has(repo)) continue;
    repos.add(repo);
  }
  return [...repos].sort().map((cwd) => ({ role: 'cargo', cwd }));
}

function parseTreeEntries(repo, env, tree) {
  const listed = gitText(repo, ['ls-tree', '-rz', '-l', '--full-tree', tree], `could not inventory captured tree ${tree}`, { env });
  return listed.split('\0').filter(Boolean).map((line) => {
    const match = /^(\d{6}) ([a-z]+) ([0-9a-f]+)\s+(\d+|-)\t([\s\S]+)$/.exec(line);
    if (!match) refuse(`${repo}: malformed entry in captured tree ${tree}`);
    return { mode: match[1], type: match[2], oid: match[3], bytes: match[4] === '-' ? null : Number(match[4]), name: match[5] };
  });
}

/** Refuse captured objects that could lead checkout outside the private
 * source root. Regular files are materialized by checkout-index below so Git
 * clean/smudge filters (including LFS-style pointers) keep their worktree form. */
function validateCapturedTree(source, sources, env, tree) {
  const entries = parseTreeEntries(source.repo, env, tree);
  const absoluteLinks = [];
  for (const entry of entries.filter((item) => item.type === 'commit')) {
    const nested = canonicalPath(resolve(source.repo, entry.name));
    if (!sources.some((candidate) => candidate.repo === nested)) {
      refuse(`${source.repo}: ${entry.name} is a Git submodule whose repository is not in the captured Cargo source graph`);
    }
  }
  for (const entry of entries.filter((item) => item.type === 'blob')) {
    if (entry.mode === '100644' || entry.mode === '100755') continue;
    if (entry.mode !== '120000') refuse(`${source.repo}: captured source ${entry.name} has unsupported Git mode ${entry.mode}`);
    const content = gitResult(source.repo, ['cat-file', 'blob', entry.oid], `could not read captured symlink ${entry.name}`, { env }).stdout;
    const target = content.toString('utf8');
    if (!Buffer.from(target).equals(content) || !target || target.includes('\0')) refuse(`${source.repo}: captured symlink ${entry.name} has a non-text target`);
    // Resolve against the captured repository layout only. realpath here
    // would traverse a live sibling link after the tree was frozen and could
    // redirect an otherwise immutable absolute-link relocation.
    const originalTarget = resolve(source.repo, dirname(entry.name), target);
    if (!sources.some((candidate) => inside(candidate.repo, originalTarget))) {
      refuse(`${source.repo}: captured symlink ${entry.name}${isAbsolute(target) ? ' has an absolute target that' : ''} escapes the captured source repositories`);
    }
    if (isAbsolute(target)) absoluteLinks.push({ name: entry.name, originalTarget });
  }
  return absoluteLinks;
}

function addCapturedPaths(repo, env, paths, force, what) {
  if (!paths.length) return;
  gitResult(repo, ['--literal-pathspecs', 'add', ...(force ? ['--force'] : []), '--all',
    '--pathspec-from-file=-', '--pathspec-file-nul'], what,
  { env, input: Buffer.from(`${[...new Set(paths)].sort().join('\0')}\0`) });
}

function captureRepository(source, sources, captureRoot, stagedRoot, common, app, exactRoot) {
  const scratch = mkdtempSync(resolve(captureRoot, '.git-index-'));
  const objects = resolve(scratch, 'objects');
  mkdirSync(objects);
  const originalObjects = canonicalPath(resolve(source.repo,
    gitText(source.repo, ['rev-parse', '--git-path', 'objects'], 'could not locate Git objects').trim()));
  const env = { ...process.env };
  for (const name of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY']) delete env[name];
  env.GIT_INDEX_FILE = resolve(scratch, 'index');
  env.GIT_OBJECT_DIRECTORY = objects;
  env.GIT_ALTERNATE_OBJECT_DIRECTORIES = [originalObjects, process.env.GIT_ALTERNATE_OBJECT_DIRECTORIES].filter(Boolean).join(delimiter);
  try {
    const pathspec = sourcePathspec(source.repo, app, exactRoot);
    const excluded = pathspec.slice(1).filter((path) => path.startsWith(':(exclude,top,literal)'))
      .map((path) => path.slice(':(exclude,top,literal)'.length));
    gitResult(source.repo, ['read-tree', source.commit], 'could not start the captured Git tree', { env });
    if (excluded.length) gitResult(source.repo, ['--literal-pathspecs', 'rm', '-r', '-f', '--cached', '--ignore-unmatch', '--', ...excluded],
      'could not remove generated outputs from the captured tree', { env });
    const allowed = (path) => !excluded.some((root) => path === root || path.startsWith(`${root}/`));
    const committed = gitText(source.repo, ['ls-tree', '-rz', '--name-only', source.commit],
      'could not inventory committed source').split('\0').filter((path) => path && allowed(path));
    const ordinary = gitText(source.repo, ['ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', ...pathspec],
      'could not inventory tracked and untracked source').split('\0').filter((path) => path && allowed(path));
    // The temporary index starts empty, so a tracked file beneath a newly
    // ignored parent looks ignored to `git add`; force only this inventoried set.
    addCapturedPaths(source.repo, env, [...committed, ...ordinary], true, 'could not capture tracked and untracked source');
    // Gitignore is not a source/output declaration: build.rs or another
    // committed tool can read beneath an ignored directory. Capture every
    // ignored file except the precise generated roots in sourcePathspec.
    const ignored = gitText(source.repo, ['ls-files', '-z', '--others', '--ignored', '--exclude-standard', '--',
      ...pathspec], 'could not inventory ignored source inputs').split('\0').filter((path) => path && allowed(path));
    addCapturedPaths(source.repo, env, ignored, true, 'could not capture ignored source inputs');
    const tree = gitText(source.repo, ['write-tree'], 'could not freeze the captured source tree', { env }).trim();
    const status = gitText(source.repo,
      ['diff-tree', '-r', '--no-commit-id', '--name-status', '-z', '--no-renames', source.commit, tree, '--', ...pathspec],
      'could not inventory captured changes', { env }).split('\0').filter(Boolean);
    if (status.length % 2 !== 0) refuse(`${source.repo}: malformed captured change inventory`);
    const changes = [];
    for (let index = 0; index < status.length; index += 2) changes.push(`${status[index].padEnd(2)} ${status[index + 1]}`);
    const destination = resolve(stagedRoot, relative(common, source.repo));
    if (!inside(stagedRoot, destination)) refuse(`${source.repo}: cannot be placed under the private source root`);
    mkdirSync(destination, { recursive: true });
    const absoluteLinks = validateCapturedTree(source, sources, env, tree);
    gitResult(source.repo, ['checkout-index', '--all', '--force', `--prefix=${destination}${sep}`],
      'could not materialize the captured source tree', { env });
    // An absolute link into a captured repository has safe source semantics,
    // but its literal checkout would point back at the live tree. Relocate it
    // to the corresponding captured path before any bake can observe it.
    for (const link of absoluteLinks) {
      const path = resolve(destination, link.name);
      const target = resolve(stagedRoot, relative(common, link.originalTarget));
      if (!inside(destination, path) || !inside(stagedRoot, target)) refuse(`${source.repo}: captured symlink ${link.name} cannot be relocated safely`);
      rmSync(path);
      symlinkSync(relative(dirname(path), target) || '.', path);
    }
    return { ...source, tree, pathspec,
      workingSha256: sha256(Buffer.from(`exact2 working source tree v3\n${tree}\n`)), changes };
  } finally { rmSync(scratch, { recursive: true, force: true }); }
}

/** The snapshot (LLP 1030.000 D3 item 1): capture every repository and every
 * dirty source byte the bake can read. Ignored files are source too unless
 * they are under a precise generated-output root. */
export function snapshotOf(app, opts, exactRoot = ROOT) {
  const sourceRoots = [
    { role: 'app', cwd: canonicalPath(app.dir) },
    { role: 'exact2', cwd: canonicalPath(exactRoot) },
    ...cargoDependencyRoots(app, exactRoot),
  ];
  const repos = new Map();
  for (const { role, cwd } of sourceRoots) {
    const repo = repoTop(cwd, `source for ${role}`);
    const existing = repos.get(repo);
    if (existing) { existing.roles.add(role); continue; }
    const commit = gitText(repo, ['rev-parse', 'HEAD'], 'git has no HEAD commit to snapshot').trim();
    if (!/^[0-9a-f]{40}$/.test(commit)) refuse(`${repo}: git has no HEAD commit to snapshot`);
    repos.set(repo, { repo, roles: new Set([role]), commit });
  }
  const sourceList = [...repos.values()].map((source) => ({ ...source,
    roles: [...source.roles].sort() }));
  const common = commonParent(sourceList.map((source) => source.repo));
  // Keep even the pre-run capture outside every live repository. Cargo walks
  // ancestor directories for configuration, so a stage beneath `target/`
  // would still let a mutable checkout influence the supposedly frozen bake.
  const captureRoot = privateCaptureRoot(sourceList);
  const stagedSourceRoot = resolve(captureRoot, 'source');
  mkdirSync(stagedSourceRoot);
  try {
    const captured = sourceList.map((source) => captureRepository(source, sourceList, captureRoot,
      stagedSourceRoot, common, app, exactRoot));
    const changes = captured.flatMap((source) => source.changes.map((change) => `${source.roles.join('+')} ${change}`));
    if (changes.length && !opts.dirty) refuse(`the source repository${captured.length === 1 ? '' : 'ies'} this bake reads ${captured.length === 1 ? 'has' : 'have'} uncommitted or ignored source files:\n  ${changes.join('\n  ')}\ncommit them, or pass --dirty to publish those captured bytes (the table says so loudly)`);
    const id = captured.length === 1 && changes.length === 0 ? captured[0].commit
      : sha256(Buffer.from(`exact2 source snapshot v3\n${captured.map((source) => `${source.roles.join('+')} ${source.commit} ${source.workingSha256}`).join('\n')}\n`)).slice(0, 40);
    if (opts.snapshot && !id.startsWith(opts.snapshot.toLowerCase())) refuse(`the source snapshot is ${id}, not --snapshot ${opts.snapshot}: the dry run and its --yes must name the same complete source set (LLP 1030.000 D3)`);
    const sources = captured.map(({ repo, roles, commit, workingSha256, changes: sourceChanges }) => ({ repo, roles, commit, workingSha256, changes: sourceChanges }));
    const snapshot = { id, commit: sources[0].commit, dirty: changes.length > 0, changes, repo: sources[0].repo, sources };
    snapshotCaptures.set(snapshot, { sources: captured, common, captureRoot, stagedSourceRoot,
      appDir: canonicalPath(app.dir), appWorkspace: canonicalPath(app.workspace ?? app.dir), exactRoot: canonicalPath(exactRoot) });
    return snapshot;
  } catch (error) {
    rmSync(captureRoot, { recursive: true, force: true });
    throw error;
  }
}

function commonParent(paths) {
  let common = dirname(paths[0]);
  while (!paths.every((path) => inside(common, path))) {
    const parent = dirname(common);
    if (parent === common) refuse(`source repositories on unrelated filesystem roots cannot share one materialized bake: ${paths.join(', ')}`);
    common = parent;
  }
  return common;
}

/** Allocate a private capture that is proved not to sit beneath any mutable
 * source checkout. TMPDIR is caller-controlled and commonly points at a
 * project-local target directory, so it is only a candidate, never trust. */
function privateCaptureRoot(sources) {
  let problem = '';
  for (const base of [...new Set([tmpdir(), resolve(sep, 'tmp')])]) {
    let candidate;
    try { candidate = canonicalPath(mkdtempSync(resolve(base, 'exact-source-capture-'))); }
    catch (error) { problem = `${base}: ${error.message}`; continue; }
    if (!sources.some((source) => inside(source.repo, candidate))) return candidate;
    problem = `${candidate} is inside a captured source repository`;
    rmSync(candidate, { recursive: true, force: true });
  }
  refuse(`could not allocate a source capture outside the live repositories${problem ? `: ${problem}` : ''}`);
}

/** Child tools may ask Git about their source directory. Keep discovery and
 * explicit Git process state inside the private snapshot boundary. */
function sealedSourceEnv(sourceRoot, extra = {}) {
  const env = { ...process.env, ...extra };
  for (const name of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE',
    'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES']) delete env[name];
  env.GIT_CEILING_DIRECTORIES = sourceRoot;
  env.GIT_DISCOVERY_ACROSS_FILESYSTEM = '0';
  return env;
}

/** Prove Cargo will consume only the captured tree. Cargo canonicalizes path
 * dependencies in its own graph, so this catches absolute paths and symlink
 * aliases that would otherwise lead a staged build back into a live checkout. */
function assertMaterializedCargoClosure(workspaces, sourceRoot, target) {
  const capturedRoot = canonicalPath(sourceRoot);
  for (const workspace of [...new Set(workspaces)]) {
    if (!existsSync(resolve(workspace, 'Cargo.toml'))) continue;
    const result = spawnSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
      cwd: workspace, env: sealedSourceEnv(sourceRoot, { CARGO_TARGET_DIR: target }), encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    });
    if (result.status !== 0) refuse(`${workspace}: materialized Cargo graph does not resolve: ${result.stderr.trim()}`);
    let metadata;
    try { metadata = JSON.parse(result.stdout); }
    catch (error) { refuse(`${workspace}: materialized cargo metadata was not JSON: ${error.message}`); }
    for (const pkg of metadata.packages ?? []) {
      if (pkg.source !== null) continue;
      const inputs = [['manifest', pkg.manifest_path],
        ...(pkg.targets ?? []).map((target) => [`target ${target.name ?? '(unnamed)'}`, target.src_path])];
      for (const [kind, input] of inputs) {
        if (typeof input !== 'string') refuse(`${workspace}: materialized Cargo package ${pkg.name ?? '(unnamed)'} has no ${kind} path`);
        const path = canonicalPath(input);
        if (!inside(capturedRoot, path)) {
          refuse(`${workspace}: materialized Cargo package ${pkg.name ?? '(unnamed)'} ${kind} resolves outside the captured source root at ${path}`);
        }
      }
    }
  }
}

/** Bind the already-materialized capture to this run. Source stays in its
 * private temporary root so Cargo cannot discover live ancestor config. */
export function materializeSnapshot(snapshot, run, app) {
  const capture = snapshotCaptures.get(snapshot);
  if (!capture) refuse('the source snapshot was not captured by this deploy process and cannot be materialized');
  const sourceRoot = capture.stagedSourceRoot;
  // An invalid gitfile is a hard discovery boundary for a child that clears
  // the Git ceiling; an empty .git directory is skipped when an outer repo exists.
  writeFileSync(resolve(sourceRoot, '.git'), 'exact deploy source boundary\n', { flag: 'wx' });
  const destinations = new Map();
  for (const source of capture.sources) {
    const destination = resolve(sourceRoot, relative(capture.common, source.repo));
    if (!inside(sourceRoot, destination)) refuse(`source repository ${source.repo} cannot be placed under the private run`);
    destinations.set(source.repo, destination);
  }
  const stagedPath = (path) => {
    const source = capture.sources.filter((candidate) => inside(candidate.repo, path)).sort((a, b) => b.repo.length - a.repo.length)[0];
    if (!source) refuse(`${path} is not in the captured source repositories`);
    return resolve(destinations.get(source.repo), relative(source.repo, path));
  };
  const dir = stagedPath(capture.appDir);
  const workspace = stagedPath(capture.appWorkspace);
  const exactRoot = stagedPath(capture.exactRoot);
  // An explicitly supplied Cargo target is already the caller's chosen
  // isolation boundary (the deploy smoke uses one private cache for all of
  // its runs). Otherwise keep absolute source paths out of the live target by
  // giving this materialized generation its own cache.
  const target = process.env.CARGO_TARGET_DIR ? canonicalPath(process.env.CARGO_TARGET_DIR) : resolve(run, 'cargo-target');
  assertMaterializedCargoClosure([workspace, exactRoot], sourceRoot, target);
  return {
    exactRoot, sourceRoot,
    // Identity and policy are deliberately not copied from the launcher's
    // already-loaded module graph. The captured deploy process resolves them.
    app: { name: app.name, dir, workspace, target,
      crate: (kind) => `${app.name}-${kind}` },
  };
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
function bake(app, run, exactRoot, sourceRoot) {
  mkdirSync(run, { recursive: true });
  const web = resolve(run, 'web');
  const env = sealedSourceEnv(sourceRoot, { EXACT_WEB_DIST: web, CARGO_TARGET_DIR: app.target, EXACT_UPDATE_TRUST: 'production' });
  if (app.workspace === exactRoot) delete env.EXACT_APP_DIR;
  else env.EXACT_APP_DIR = app.dir;
  const r = spawnSync(process.execPath, [resolve(exactRoot, 'host/web/build.mjs'), app.crate('web')], {
    cwd: exactRoot, env, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
  });
  if (r.stdout) process.stderr.write(r.stdout);
  if (r.status !== 0) refuse(`the bake failed: host/web/build.mjs ${app.crate('web')} exited ${r.status ?? r.signal}`);
  if (!existsSync(resolve(web, 'exact.json'))) refuse(`the bake wrote no exact.json under ${web}`);
  return web;
}

/** The bundle the bake produced (LLP 1023 D2's cards with their bytes): the plan and every asset `exact.json` lists — the same bytes for every platform in v1. */
function readBundle(web, app) {
  const envelope = JSON.parse(readFileSync(resolve(web, 'exact.json'), 'utf8'));
  if (envelope.app?.id !== app.id || envelope.app?.name !== app.displayName) {
    refuse(`${web}/exact.json names ${JSON.stringify(envelope.app ?? null)}, not the captured app ${JSON.stringify({ id: app.id, name: app.displayName })}`);
  }
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
function compatOf(app, platform, exactRoot, sourceRoot) {
  const r = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'compat', app.dir, '--platform', platform, '--json'], {
    cwd: exactRoot, env: sealedSourceEnv(sourceRoot, { CARGO_TARGET_DIR: app.target, EXACT_UPDATE_TRUST: 'production' }), encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 16 * 1024 * 1024,
  });
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

/** Native deployment targets, including binary-only L=0. Classification decides the carrier before touching a stream. */
export function nativePlatforms(manifest) {
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
  const plan = head.plan;
  if (plan?.sha256 !== bundle.plan.sha256 || plan?.bytes !== bundle.plan.bytes.length
    || plan?.url !== `../../blobs/${bundle.plan.sha256}` || plan?.formatVersion !== bundle.plan.formatVersion
    || plan?.kernelSchema !== bundle.plan.kernelSchema) changes.push({ name: 'app.plan', change: 'changed' });
  const before = new Map((head.assets ?? []).map((a) => [a.name, a]));
  for (const a of bundle.assets) {
    const had = before.get(a.name);
    if (had === undefined) changes.push({ name: a.name, change: 'new' });
    else if (had.sha256 !== a.sha256 || had.bytes !== a.bytes.length || had.url !== `../../blobs/${a.sha256}`) {
      changes.push({ name: a.name, change: 'changed', note: a.name.endsWith('.wgsl') ? 'interface unchanged: asset' : undefined });
    }
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
    plan: { url: `../../blobs/${bundle.plan.sha256}`, sha256: bundle.plan.sha256, bytes: bundle.plan.bytes.length,
      formatVersion: bundle.plan.formatVersion, kernelSchema: bundle.plan.kernelSchema },
    assets: bundle.assets.map((asset) => ({ name: asset.name, url: `../../blobs/${asset.sha256}`,
      sha256: asset.sha256, bytes: asset.bytes.length })),
    stream: { app: app.id, channel: stream.channel, compatibilityId: stream.compatibilityId, seq },
    release,
  };
  if (sunset) head.sunset = sunset.store ? { message: sunset.message, store: sunset.store } : { message: sunset.message };
  return head;
}

function strictBase64(text, length, label) {
  if (typeof text !== 'string') throw new Error(`${label} is not base64 text`);
  const decoded = Buffer.from(text, 'base64');
  if (decoded.length !== length || decoded.toString('base64') !== text) throw new Error(`${label} is not canonical base64 of ${length} bytes`);
  return decoded;
}

function fileCard(object, name) {
  if (!object || typeof object !== 'object' || Array.isArray(object)) throw new Error(`the envelope names no ${name}`);
  if (typeof object.url !== 'string') throw new Error(`${name} has no url`);
  if (typeof object.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(object.sha256)) throw new Error(`${name}'s sha256 is not 64 lowercase hex digits`);
  if (!Number.isSafeInteger(object.bytes) || object.bytes < 0) throw new Error(`${name} has no exact nonnegative byte count`);
}

function safeAssetName(name) {
  if (typeof name !== 'string' || !name) throw new Error('an asset has no nonempty name');
  if (name.startsWith('/') || name.startsWith('\\') || name.includes('\\') || name.includes(':')
    || name.split('/').some((part) => !part || part === '.' || part === '..')) throw new Error(`the asset name ${name} is not a relative path`);
}

/** Parse and authenticate one origin head by the same pre-download rules as
 * exact-update. An unusable head contributes no sequence: even a syntactically
 * valid number is attacker-controlled until its signature has verified. */
export function inspectHead(found, app, stream) {
  try {
    if (!Buffer.isBuffer(found?.bytes)) throw new Error('the origin returned no head bytes');
    if (found.bytes.length > 64 * 1024) throw new Error(`the envelope is ${found.bytes.length} bytes; the most is ${64 * 1024}`);
    const text = new TextDecoder('utf-8', { fatal: true }).decode(found.bytes);
    validateRawIntegers(text);
    const head = JSON.parse(text);
    if (!head || typeof head !== 'object' || Array.isArray(head)) throw new Error('the envelope is not a JSON object');
    canonicalBytes(head); // recursively rejects every inexact JSON number
    if (head.exact !== 1) throw new Error(`the envelope is exact ${head.exact ?? '(missing)'}; this binary reads exact 1`);
    if (!head.app || typeof head.app !== 'object' || Array.isArray(head.app)) throw new Error('the envelope names no app');
    if (typeof head.app.id !== 'string' || !head.app.id) throw new Error('the envelope names no nonempty app id');
    fileCard(head.plan, 'app.plan');
    if (!head.stream || typeof head.stream !== 'object' || Array.isArray(head.stream)) throw new Error('the envelope names no stream');
    if (typeof head.stream.channel !== 'string') throw new Error('the stream names no channel');
    if (typeof head.stream.compatibilityId !== 'string') throw new Error('the stream names no compatibility id');
    if (!Number.isSafeInteger(head.stream.seq) || head.stream.seq < 0) throw new Error('the stream names no exact nonnegative seq');
    if (head.assets !== undefined && !Array.isArray(head.assets)) throw new Error("the envelope's assets are not a list");
    const names = new Set();
    for (const asset of head.assets ?? []) {
      if (!asset || typeof asset !== 'object' || Array.isArray(asset)) throw new Error('an asset is not an object');
      safeAssetName(asset.name);
      if (names.has(asset.name)) throw new Error(`the envelope names the asset ${asset.name} twice`);
      names.add(asset.name);
      fileCard(asset, asset.name);
    }
    if (head.sunset !== undefined) {
      if (!head.sunset || typeof head.sunset !== 'object' || Array.isArray(head.sunset)) throw new Error('the sunset card is not an object');
      if (typeof head.sunset.message !== 'string') throw new Error('the sunset card has no message');
    }
    let signature = null;
    if (head.signature !== undefined) {
      if (!head.signature || typeof head.signature !== 'object' || Array.isArray(head.signature)) throw new Error('the signature is not an object');
      if (typeof head.signature.keyId !== 'string') throw new Error('the signature names no key id');
      signature = strictBase64(head.signature.ed25519, 64, 'the signature');
    }
    if (head.app.id !== app.id) throw new Error(`the head is for ${head.app.id}; this binary is ${app.id}`);
    if (head.app.name !== app.displayName) throw new Error(`the head calls ${app.id} ${JSON.stringify(head.app.name)}, not ${JSON.stringify(app.displayName)}`);
    if (typeof head.stream.app === 'string' && head.stream.app !== app.id) throw new Error(`the head's stream is for ${head.stream.app}; this binary is ${app.id}`);
    if (head.stream.channel !== stream.channel) throw new Error(`the head is for channel ${head.stream.channel}; this binary is ${stream.channel}`);
    if (head.stream.compatibilityId !== stream.compatibilityId) throw new Error(`the head is for cohort ${head.stream.compatibilityId}; this binary is ${stream.compatibilityId}`);
    const keys = app.manifest.deploy?.signing?.keys ?? {};
    let authenticated = false;
    if (Object.keys(keys).length) {
      if (!signature) throw new Error('the head is unsigned and this binary carries keys');
      if (!Object.hasOwn(keys, head.signature.keyId)) throw new Error(`the head is signed by ${head.signature.keyId}, which this binary does not carry`);
      const key = strictBase64(keys[head.signature.keyId], 32, `the embedded key ${head.signature.keyId}`);
      if (!verify(null, canonicalBytes(head), publicKeyFromRaw(key), signature)) throw new Error(`the head's signature by ${head.signature.keyId} does not verify`);
      authenticated = true;
    }
    return { usable: true, authenticated, head, seq: head.stream.seq, problem: null };
  } catch (error) {
    return { usable: false, authenticated: false, head: found?.json ?? null, seq: null, problem: error.message || String(error) };
  }
}

/** The largest sequence authenticated inside an immutable release record.
 * The record wrapper is audit metadata; only its embedded signed envelope is
 * authority for a client's rollback floor. Malformed, foreign, and unsigned
 * records do not contribute a number. */
async function authenticatedReleaseFloor(origin, app, stream) {
  // Hidden names were accepted by older publishers. Include them while
  // recovering the rollback floor even though new release ids cannot begin
  // with a dot.
  const names = await origin.list(`${streamPath(stream)}/releases`, { includeHidden: true });
  // A missing directory on the writable filesystem origin is authoritative
  // emptiness. Other adapters use null when they cannot enumerate history;
  // that is not evidence that no client has observed a higher sequence.
  if (names === null && origin.kind !== 'directory') return { known: false, empty: false, floor: null };
  if (names === null || names.length === 0) return { known: true, empty: true, floor: null };
  let floor = null;
  for (const name of names.filter((entry) => entry.endsWith('.json'))) {
    const bytes = await origin.get(`${streamPath(stream)}/releases/${name}`);
    if (!bytes) throw new OriginUnavailable(`the listed release record ${origin.describe()}/${streamPath(stream)}/releases/${name} disappeared while establishing the authenticated sequence floor`);
    try {
      const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
      const record = JSON.parse(text);
      if (!record.envelope || typeof record.envelope !== 'object' || Array.isArray(record.envelope)) continue;
      const recordApp = { ...app, displayName: record.envelope.app?.name };
      const envelopeBytes = Buffer.from(JSON.stringify(record.envelope), 'utf8');
      const admission = inspectHead({ bytes: envelopeBytes, json: record.envelope, sha256: sha256(envelopeBytes) }, recordApp, stream);
      if (admission.usable && admission.authenticated) floor = Math.max(floor ?? 0, admission.seq);
    } catch { /* an unauthenticated audit record has no say in the floor */ }
  }
  return { known: true, empty: false, floor };
}

/** Allocate after an admitted head. Repairing an unusable head instead uses
 * the maximum signed immutable history; if none exists, overwriting would
 * guess at the clients' rollback floor and is refused. */
async function nextSeq(origin, app, stream, admission, at) {
  let floor;
  if (admission?.usable) floor = admission.seq;
  else {
    const history = await authenticatedReleaseFloor(origin, app, stream);
    if (!history.known) throw new OriginUnavailable(`${at} ${admission ? `is unusable (${admission.problem})` : 'is missing'}, but ${origin.describe()} cannot enumerate its authenticated release history; a rollback-safe sequence cannot be allocated`);
    floor = history.floor;
    if (floor === null && !admission && history.empty) return 1;
    if (floor === null) refuse(`${at} ${admission ? `is unusable (${admission.problem}) and its` : 'is missing, and its nonempty'} release history has no authenticated sequence floor; restore a signed release record before repairing it`);
  }
  if (floor === Number.MAX_SAFE_INTEGER) refuse(`${at} is at the largest exact JavaScript seq; a higher repair seq cannot be allocated safely`);
  return floor + 1;
}

/** The latest release record under a stream, when the origin has any: it names the platform and carries the cohort's inputs, so two ids that differ are explained field by field. */
async function latestRecord(origin, stream) {
  const names = await origin.list(`${streamPath(stream)}/releases`, { includeHidden: true });
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
const inputsDiff = (a, b) => Object.keys({ ...a, ...b }).filter((k) => canonicalJson(a?.[k] ?? null) !== canonicalJson(b?.[k] ?? null)).sort();

/** The table (LLP 1030 D3; 1030.000 D3 item 3): the origin row, a row per stream, a binary row per stream whose cohort this snapshot is not. Nothing is written. */
export async function classify({ app, opts, origin, channel, snapshot, release, web, bundle, compat, platforms, wantOrigin }) {
  const notes = [];
  if (snapshot.dirty) {
    const shown = snapshot.changes.slice(0, 20);
    const remainder = snapshot.changes.length - shown.length;
    notes.push(`UNCOMMITTED CHANGES under ${relative(snapshot.repo, app.dir) || '.'} are in this snapshot (--dirty): ${shown.join(', ')}${remainder ? `, … and ${remainder} more (all are in --json)` : ''}`);
  }
  const rows = [];

  if (wantOrigin) {
    const files = { new: [], changed: [], current: [], removed: [] };
    let unavailable = null;
    try {
      const before = await origin.get(webRootPath);
      const previous = before ? parseWebRoot(before) : null;
      const names = listPublicFiles(web);
      const oldNames = previous ? previous.files.map((f) => f.name)
        : origin.dir && existsSync(origin.dir) ? listPublicFiles(origin.dir) : [];
      for (const rel of names) {
        const bytes = readStaticCandidate(web, rel);
        const card = previous?.files.find((f) => f.name === rel);
        const have = await origin.get(previous ? `${webReleasePath(previous.id)}/${rel}` : rel);
        if (!have || previous && !card) files.new.push(rel);
        else if (previous ? card.sourceSha256 !== sha256(bytes) || card.sha256 !== sha256(have) || card.bytes !== have.length : !have.equals(bytes)) files.changed.push(rel);
        else files.current.push(rel);
      }
      files.removed = oldNames.filter((name) => !names.includes(name));
      // A fixed-file root is never a completed release, even when bytes match.
      if (!previous && !files.new.length && !files.changed.length && names.length) {
        files.changed.push(files.current.shift());
      }
    } catch (error) { if (!(error instanceof OriginUnavailable)) throw error; unavailable = error.message; }
    rows.push({ kind: 'origin', compatibilityId: compat.web?.id ?? null,
      action: unavailable ? 'unavailable' : files.new.length + files.changed.length + files.removed.length ? 'publish' : 'current', files,
      ...(unavailable ? { reason: unavailable } : {}) });
  }

  const own = platforms.map((platform) => ({ platform, compatibilityId: compat[platform].id }));
  for (const { platform, compatibilityId } of own) {
    const inputs = compat[platform].inputs ?? {};
    if (inputs.store?.L === '0') {
      rows.push({ kind: 'binary', platform, compatibilityId,
        cohort: { L: '0', E: inputs.executors ?? [] }, action: 'binary',
        reason: 'this app links no update store (L=0); deliver changes in the platform binary' });
      continue;
    }
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
    const admission = head ? inspectHead(head, app, stream) : null;
    if (admission && !admission.usable) notes.push(`the head of ${streamPath(stream)} (seq ${admission.seq ?? '?'}) is unusable: ${admission.problem}; this deploy will repair it`);
    const changes = admission?.usable
      ? changesAgainst(bundle, admission.head)
      : head ? [{ name: 'exact.json', change: 'repair', note: admission.problem }] : changesAgainst(bundle, null);
    let seq = admission?.seq;
    if (changes.length) {
      try { seq = await nextSeq(origin, app, stream, admission, `the head at ${origin.describe()}/${streamPath(stream)}/exact.json`); }
      catch (error) {
        if (!(error instanceof OriginUnavailable)) throw error;
        rows.push({
          kind: 'stream', platform, channel, compatibilityId,
          cohort: { L: inputs.store?.L ?? '?', E: inputs.executors ?? [] },
          head: head ? { seq: admission.seq, sha256: head.sha256, release: typeof head.json.release === 'string' ? head.json.release : null,
            ...(admission.usable ? {} : { unusable: admission.problem }) } : null,
          action: 'unavailable', changes: [], reason: error.message,
        });
        continue;
      }
    }
    rows.push({
      kind: 'stream', platform, channel, compatibilityId,
      cohort: { L: inputs.store?.L ?? '?', E: inputs.executors ?? [] },
      head: head ? { seq: admission.seq, sha256: head.sha256, release: typeof head.json.release === 'string' ? head.json.release : null,
        ...(admission.usable ? {} : { unusable: admission.problem }) } : null,
      action: changes.length ? 'bundle' : 'current',
      seq: changes.length ? seq : admission.seq,
      changes,
    });
  }

  // Streams on the origin (or in `deploy.streams`) that are not this snapshot's cohorts: a retired cohort's stream, which needs a binary (LLP 1030 D3 rule 3). Not when the run wants no stream at all (`--only origin`, `--platform web`).
  const declared = app.manifest.deploy?.streams?.filter((s) => s.channel === channel) ?? null;
  let others;
  if (!platforms.some((platform) => compat[platform]?.inputs?.store?.L !== '0')) others = [];
  else if (declared) others = declared.map((s) => s.compatibilityId);
  else {
    try {
      const listed = await origin.list(`.exact/${channel}`);
      if (listed === null && origin.kind === 'https') notes.push(`an https origin cannot be listed: the streams classified are this snapshot's own; name deploy.streams in app.json to classify a retired cohort's`);
      // The web pointer is publisher metadata, never a retired native cohort.
      others = (listed ?? []).filter((id) => channel !== webRootStream.channel || id !== webRootStream.compatibilityId);
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
    if (platform && compat[platform]?.inputs?.store?.L === '0') continue;
    if (opts.platform.length && platform && !opts.platform.includes(platform)) continue;
    if (!head && !record && !declared) continue; // an empty directory is not a stream
    const admission = head ? inspectHead(head, app, stream) : null;
    if (admission && !admission.usable) notes.push(`the head of ${streamPath(stream)} (seq ${admission.seq ?? '?'}) is unusable: ${admission.problem}`);
    const cohort = record?.compat?.inputs ?? null;
    const ours = platform && compat[platform] ? compat[platform] : null;
    const differs = cohort && ours ? inputsDiff(cohort, ours.inputs) : null;
    const reason = `the snapshot's cohort${platform ? ` for ${platform} is ${ours?.id ?? '(not built this run)'}` : ` is ${own.map((o) => `${o.platform} ${o.compatibilityId}`).join(', ')}`}, not ${compatibilityId}` +
      (differs?.length ? ` (differs in ${differs.join(', ')})` : platform ? '' : ' (no release record names its platform)') + ': a binary is needed';
    rows.push({
      kind: 'stream', platform, channel, compatibilityId,
      cohort: cohort ? { L: cohort.store?.L ?? '?', E: cohort.executors ?? [] } : null,
      head: head ? { seq: admission.seq, sha256: head.sha256, release: typeof head.json.release === 'string' ? head.json.release : null,
        ...(admission.usable ? {} : { unusable: admission.problem }) } : null,
      action: 'binary', changes: [], reason,
    });
  }

  return { release, snapshot: { id: snapshot.id ?? snapshot.commit, commit: snapshot.commit, dirty: snapshot.dirty, changes: snapshot.changes,
    ...(snapshot.sources ? { sources: snapshot.sources.map(({ roles, commit, workingSha256 }) => ({ roles, commit, workingSha256 })) } : {}) },
  app: { id: app.id, name: app.displayName }, channel, origin: { kind: origin.kind, location: origin.describe() }, dryRun: !opts.yes, notes, rows };
}

// ------------------------------------------------------------------ printing

/** The table in D3's shape: the release line, one row per line with its carrier on the right. */
export function renderTable(table) {
  const line = (label, detail, action) => `${label.padEnd(8)} ${detail.padEnd(60)} → ${action}`;
  const out = [`release ${table.release} · snapshot ${(table.snapshot.id ?? table.snapshot.commit).slice(0, 12)}${table.snapshot.dirty ? ' (DIRTY)' : ''} · channel ${table.channel} · origin ${table.origin.location}${table.origin.kind === 'https' ? ' (read-only)' : ''}`];
  for (const note of table.notes) out.push(`!! ${note}`);
  for (const row of table.rows) {
    if (row.kind === 'origin') {
      const f = row.files;
      if (row.action === 'unavailable') { out.push(line('origin', `web app: ${row.reason}`, 'unavailable')); continue; }
      const summary = [f.new.length ? `${f.new.length} new` : '', f.changed.length ? `${f.changed.length} changed` : '', f.current.length ? `${f.current.length} current` : '', f.removed?.length ? `${f.removed.length} removed` : ''].filter(Boolean).join(', ');
      const named = [...f.changed, ...f.new, ...(f.removed ?? [])].filter((n) => !n.startsWith('assets/') && !n.startsWith('deck/') && !n.startsWith('shaders/')).slice(0, 6);
      out.push(line('origin', `web app${row.compatibilityId ? ` (cohort ${row.compatibilityId.slice(0, 8)})` : ''}: ${summary}${named.length ? ` — ${named.join(', ')}` : ''}`, row.action === 'publish' ? 'publish (atomic web root)' : 'current'));
      continue;
    }
    if (row.kind === 'binary') {
      out.push(line(row.platform, row.reason, 'binary'));
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
  const binaries = table.rows.filter((r) => r.action === 'binary');
  if (binaries.length) out.push(`binary needed: ${binaries.map((r) => `${r.platform ?? '?'} ${r.compatibilityId.slice(0, 8)}`).join(', ')} — a later verb (LLP 1030.000 §6); this run publishes nothing to those streams`);
  return out.join('\n');
}

// ---------------------------------------------------------------- publishing

/** Publish one stream (LLP 1030.000 D3 item 5): under its lock, refuse a
 * reused immutable receipt, read the head and allocate seq, put and verify
 * content-addressed blobs, prepare the immutable release record, then swap
 * the signed head conditionally. A failure before that last operation leaves
 * every URL in the prior head untouched and its bytes still retrievable. */
export async function publishStream({ origin, row, bundle, compat, app, signer, release, snapshot, opts, log }) {
  const stream = { channel: row.channel, compatibilityId: row.compatibilityId };
  const base = streamPath(stream);
  const recordPath = `${base}/releases/${release}.json`;
  const files = [{ name: 'app.plan', sha256: bundle.plan.sha256, bytes: bundle.plan.bytes }, ...bundle.assets];
  if (await origin.get(recordPath)) refuse(`release ${release} already has an immutable record at ${origin.describe()}/${recordPath}; choose another --release`);
  return origin.withLock(stream, async () => {
    if (await origin.get(recordPath)) refuse(`release ${release} already has an immutable record at ${origin.describe()}/${recordPath}; choose another --release`);
    const current = await origin.head(stream);
    const previousDigest = current?.sha256 ?? null;
    const admission = current ? inspectHead(current, app, stream) : null;
    const changes = admission?.usable
      ? changesAgainst(bundle, admission.head)
      : current ? [{ name: 'exact.json', change: 'repair', note: admission.problem }] : changesAgainst(bundle, null);
    if (admission?.usable && !changes.length) return { ...row, action: 'current', seq: admission.seq, note: 'the head is admissible and already names this bundle (published meanwhile)' };
    const seq = await nextSeq(origin, app, stream, admission, `the locked head at ${origin.describe()}/${base}/exact.json`);
    if (opts.slowMs) await sleep(Number(opts.slowMs));
    let written = 0;
    for (const file of files) {
      if (await origin.put(blobPath(file.sha256), file.bytes, { immutable: true }) === 'written') written++;
      const back = await origin.get(blobPath(file.sha256));
      if (!back || sha256(back) !== file.sha256) refuse(`the blob ${file.sha256} (${file.name}) read back from ${origin.describe()} is not what was written`);
    }
    log(`  ${row.platform} ${row.compatibilityId.slice(0, 8)}: ${files.length} blobs on the origin (${written} written, ${files.length - written} present), each read back and checked`);
    const sunset = app.manifest.deploy?.sunset?.[`${stream.channel}/${stream.compatibilityId}`] ?? app.manifest.deploy?.sunset?.[stream.compatibilityId];
    const head = streamHead({ app, bundle, stream, seq, release, sunset });
    head.signature = signer.sign(head);
    const bytes = Buffer.from(JSON.stringify(head) + '\n', 'utf8');
    const originDigest = sha256(bytes);
    const entryDigest = sha256(canonicalBytes(head));
    const record = {
      release, at: new Date().toISOString(), by: userInfo().username, host: hostname(),
      platform: row.platform, stream: head.stream, seq, snapshot,
      head: { entryDigest, originDigest, bytes: bytes.length, keyId: signer.keyId }, previous: previousDigest,
      compat: { id: compat.id, inputs: compat.inputs }, row: { action: row.action, changes },
      envelope: head,
    };
    await origin.put(recordPath, Buffer.from(JSON.stringify(record, null, 2) + '\n', 'utf8'), { immutable: true });
    try {
      await origin.putHead(stream, bytes, { previousDigest });
    } catch (error) {
      let observed;
      try {
        observed = await origin.head(stream);
      } catch (readError) {
        const unknown = new Error(`the head write failed (${error.message}) and its outcome could not be read back (${readError.message})`);
        unknown.headOutcomeUnknown = true;
        throw unknown;
      }
      if (!observed?.bytes.equals(bytes)) throw error;
      log(`  ${row.platform} ${row.compatibilityId.slice(0, 8)}: the head write response failed, but readback confirms seq ${seq}`);
    }
    return { ...row, action: 'published', seq, changes, head: { seq, sha256: originDigest, entryDigest, release }, previous: previousDigest };
  });
}

/** Capture one complete web graph. The identity binds every original byte
 * and this encoding version; generated links all name that immutable tree. */
export function webRelease(web) {
  const files = listPublicFiles(web).map((name) => ({ name, body: readStaticCandidate(web, name) }));
  const source = files.map(({ name, body }) => ({ name, sha256: sha256(body) }));
  const id = sha256(canonicalBytes({ webRoot: 1, source }));
  const prefix = `/${webReleasePath(id)}/`;
  for (const file of files) {
    file.sourceSha256 = sha256(file.body);
    if (file.name === 'index.html') {
      const html = file.body.toString('utf8');
      if (/<base\b/i.test(html)) refuse('the baked index already defines a base URL');
      file.body = Buffer.from(html.replace(/(<meta charset="utf-8">)/i, `$1\n<base href="${prefix}">`));
      if (!file.body.toString('utf8').includes('<base ')) file.body = Buffer.from(`<base href="${prefix}">\n${html}`);
    } else if (file.name === 'manifest.json') {
      const manifest = JSON.parse(file.body.toString('utf8'));
      // Manifest navigation remains canonical after moving the manifest
      // itself. W3C appmanifest resolves id against the start URL's origin;
      // other navigation members resolve against the manifest URL.
      const origin = 'https://exact.invalid';
      const canonical = (value) => {
        if (typeof value !== 'string' || !value) return value;
        const url = new URL(value, origin + '/manifest.json');
        return url.origin === origin ? url.pathname + url.search + url.hash : value;
      };
      for (const key of ['start_url', 'scope', 'id']) if (key in manifest) manifest[key] = canonical(manifest[key]);
      manifest.start_url ||= '/';
      const asset = (row) => {
        if (typeof row?.src !== 'string') return;
        const url = new URL(row.src, origin + '/manifest.json');
        const name = decodeURIComponent(url.pathname.slice(1));
        if (url.origin === origin && files.some((f) => f.name === name)) row.src = prefix + name.split('/').map(encodeURIComponent).join('/') + url.search + url.hash;
      };
      for (const row of [...(manifest.icons ?? []), ...(manifest.screenshots ?? [])]) asset(row);
      for (const shortcut of manifest.shortcuts ?? []) {
        shortcut.url = canonical(shortcut.url);
        for (const icon of shortcut.icons ?? []) asset(icon);
      }
      if (manifest.share_target?.action) manifest.share_target.action = canonical(manifest.share_target.action);
      for (const row of manifest.protocol_handlers ?? []) row.url = canonical(row.url);
      for (const row of manifest.file_handlers ?? []) row.action = canonical(row.action);
      file.body = Buffer.from(JSON.stringify(manifest));
    } else if (file.name === 'exact.json') {
      const envelope = JSON.parse(file.body.toString('utf8'));
      for (const card of [envelope.plan, ...(envelope.assets ?? [])]) {
        const name = card === envelope.plan ? 'app.plan' : card.name;
        const captured = files.find((f) => f.name === name);
        if (!captured || card.sha256 !== sha256(captured.body) || card.bytes !== captured.body.length) refuse(`web envelope does not bind ${name}`);
        card.url = prefix + name.split('/').map(encodeURIComponent).join('/');
      }
      file.body = canonicalBytes(envelope);
    }
  }
  const pointer = { webRoot: 1, id, files: files.map(({ name, body, sourceSha256 }) => ({ name, sourceSha256, sha256: sha256(body), bytes: body.length })) };
  return { pointer, files };
}

/** Every payload lands immutably and is read back before the only mutable
 * pointer moves. A failed or concurrent publish cannot damage the prior graph. */
export async function publishRoot({ origin, row, web, log = () => {} }) {
  const { pointer, files } = webRelease(web);
  return origin.withLock(webRootStream, async () => {
    const before = await origin.get(webRootPath);
    if (before && parseWebRoot(before).id === pointer.id) {
      const complete = await Promise.all(pointer.files.map(async (card) => {
        const have = await origin.get(`${webReleasePath(pointer.id)}/${card.name}`);
        return have && sha256(have) === card.sha256 && have.length === card.bytes;
      }));
      if (complete.every(Boolean)) return { ...row, action: 'current', root: pointer.id };
    }
    for (const file of files) {
      const path = `${webReleasePath(pointer.id)}/${file.name}`;
      await origin.put(path, file.body, { immutable: true });
      const have = await origin.get(path);
      if (!have?.equals(file.body)) refuse(`web release readback failed: ${path}`);
    }
    const bytes = canonicalBytes(pointer);
    try { await origin.putHead(webRootStream, bytes, { previousDigest: before ? sha256(before) : null }); }
    catch (error) {
      // A transport failure after commit is success only when exact readback
      // proves this pointer won; otherwise preserve the original refusal.
      if (!(await origin.get(webRootPath))?.equals(bytes)) throw error;
    }
    log(`  origin: ${files.length} immutable files verified; atomic web root ${pointer.id.slice(0, 12)}`);
    return { ...row, action: 'published', root: pointer.id, written: files.map((f) => f.name) };
  });
}

// -------------------------------------------------------------------- deploy

async function deployCaptured(opts, capsule) {
  if (capsule?.version !== 1 || typeof capsule.run !== 'string'
    || typeof capsule.sourceRoot !== 'string' || typeof capsule.exactRoot !== 'string'
    || typeof capsule.release !== 'string' || !capsule.snapshot || !capsule.app
    || !Array.isArray(capsule.snapshot.sources)
    || capsule.snapshot.sources.some((source) => typeof source?.repo !== 'string')) {
    refuse('the private deploy capsule is malformed');
  }
  const run = canonicalPath(capsule.run);
  const sourceRoot = canonicalPath(capsule.sourceRoot);
  const exactRoot = canonicalPath(capsule.exactRoot);
  const liveRepos = capsule.snapshot.sources.map((source) => canonicalPath(source.repo));
  if (basename(sourceRoot) !== 'source' || !basename(dirname(sourceRoot)).startsWith('exact-source-capture-')
    || liveRepos.some((repo) => inside(repo, sourceRoot)) || !inside(sourceRoot, exactRoot)
    || inside(run, sourceRoot) || inside(sourceRoot, run) || exactRoot !== canonicalPath(ROOT)) {
    refuse('the private deploy capsule does not name this captured source tree');
  }
  process.env.CARGO_TARGET_DIR = canonicalPath(capsule.app.target);
  if (capsule.app.external) process.env.EXACT_APP_DIR = canonicalPath(capsule.app.dir);
  else delete process.env.EXACT_APP_DIR;
  const app = resolveApp(opts._[0]);
  if (canonicalPath(app.dir) !== canonicalPath(capsule.app.dir)
    || canonicalPath(app.workspace) !== canonicalPath(capsule.app.workspace)) {
    refuse('the captured app resolver does not select the app and workspace frozen by the launcher');
  }
  const snapshot = capsule.snapshot;
  const release = capsule.release;
  const channel = opts.channel ?? channelOf(app.manifest);
  if (channel === 'blobs' || !/^[A-Za-z0-9._-]+$/.test(channel)) refuse(`the channel ${JSON.stringify(channel)} cannot name a directory under .exact/`);
  const originSpec = opts.origin ?? app.manifest.deploy?.channels?.[channel] ?? app.origin;
  if (!originSpec) refuse(`no origin for the channel ${channel}: pass --origin <dir|url> or name deploy.channels.${channel} in app.json`);
  const origin = openOrigin(originSpec);
  const wantOrigin = opts.only !== 'bundle' && (!opts.platform.length || opts.platform.includes('web'));
  const platforms = opts.only === 'origin' ? [] : nativePlatforms(app.manifest).filter((p) => !opts.platform.length || opts.platform.includes(p));
  if (!wantOrigin && !platforms.length) refuse(`nothing to classify: ${opts.only ? `--only ${opts.only}` : ''} ${opts.platform.length ? `--platform ${opts.platform.join(',')}` : ''} leaves no row`);
  const log = (text) => process.stderr.write(`${text}\n`);

  // Refuse before the bake what the bake cannot fix: a read-only origin, a missing or mismatched key.
  if (opts.yes && !origin.writable) refuse(`${origin.describe()} is an https origin, read-only in v1: point --origin at the directory the host serves (an object-store adapter with a conditional put is owed)`);
  const signer = opts.yes && platforms.length ? loadSigner(app, opts.keys) : null;
  if (signer) log(`signing as ${signer.keyId} (${signer.path})`);

  log(`snapshot ${snapshot.id}${snapshot.sources.length > 1 ? ` (${snapshot.sources.map((source) => `${source.roles.join('+')} ${source.commit.slice(0, 7)}`).join(', ')})` : ''}${snapshot.dirty ? ' + uncommitted changes (--dirty)' : ''}; baking into ${run}`);
  const web = bake(app, run, exactRoot, sourceRoot);
  const bundle = readBundle(web, app);
  const compat = {};
  for (const platform of ['web', ...platforms]) compat[platform] = compatOf(app, platform, exactRoot, sourceRoot);
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
    const name = `${row.platform ?? '?'} ${row.kind} ${row.compatibilityId.slice(0, 8)}`;
    if (row.action === 'unavailable') { failed.push({ platform: row.platform, compatibilityId: row.compatibilityId, error: row.reason }); log(`  ${name}: unavailable — ${row.reason}`); continue; }
    if (row.action === 'binary') { refused.push({ platform: row.platform, compatibilityId: row.compatibilityId, reason: row.reason }); log(`  ${name}: refused — ${row.reason}`); continue; }
    try {
      const result = await publishStream({ origin, row, bundle, compat: compat[row.platform], app, signer, release, snapshot: table.snapshot, opts, log });
      published.push(result);
      log(result.action === 'published' ? `  ${name}: head seq ${result.seq} (${result.head.sha256.slice(0, 12)}), previous ${result.previous ? result.previous.slice(0, 12) : 'none'}` : `  ${name}: ${result.note}`);
    } catch (e) {
      failed.push({ platform: row.platform, compatibilityId: row.compatibilityId, error: e.message });
      log(`  ${name}: failed — ${e.message}; ${e.headOutcomeUnknown ? 'the head outcome is unknown and must be inspected' : "this release's head is not visible"}`);
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

/** The live module graph is only a launcher: freeze and relocate all source,
 * then execute the publisher itself from that captured Exact tree. This keeps
 * app resolution, bake, classification, signing, and publication on one
 * immutable implementation even when the checkout changes during the run. */
async function deploy(opts) {
  const locatedApp = resolveApp(opts._[0]);
  const snapshot = snapshotOf(locatedApp, opts);
  const capture = snapshotCaptures.get(snapshot);
  try {
    const release = opts.release ?? defaultRelease(snapshot.id);
    const run = deployRun(locatedApp.target, release);
    const materialized = materializeSnapshot(snapshot, run, locatedApp);
    const capsule = {
      version: 1, snapshot, release, run, sourceRoot: materialized.sourceRoot,
      exactRoot: materialized.exactRoot,
      app: {
        name: locatedApp.name, dir: materialized.app.dir,
        workspace: materialized.app.workspace, target: materialized.app.target,
        external: canonicalPath(locatedApp.workspace) !== canonicalPath(ROOT),
      },
    };
    const capsulePath = resolve(materialized.sourceRoot, '.deploy-capsule.json');
    writeFileSync(capsulePath, `${JSON.stringify(capsule)}\n`, { flag: 'wx', mode: 0o600 });
    const env = sealedSourceEnv(materialized.sourceRoot, {
      CARGO_TARGET_DIR: materialized.app.target,
      EXACT_DEPLOY_CAPSULE: capsulePath,
    });
    const child = spawnSync(process.execPath,
      [canonicalPath(resolve(materialized.exactRoot, 'scripts/deploy.mjs')), ...process.argv.slice(2)],
      { cwd: process.cwd(), env, stdio: 'inherit' });
    const consumed = !existsSync(capsulePath);
    if (child.error) refuse(`could not execute the captured deploy publisher: ${child.error.message}`);
    if (child.status === null) refuse(`the captured deploy publisher ended on signal ${child.signal ?? 'unknown'}`);
    if (!consumed) refuse('the captured deploy publisher exited without consuming its private capsule');
    return child.status;
  } finally {
    if (capture) rmSync(capture.captureRoot, { recursive: true, force: true });
  }
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (opts.help || !opts._.length) { console.log(USAGE); return opts.help ? 0 : 2; }
  if (opts._[0] === 'keygen') return keygen(opts);
  const capsulePath = process.env.EXACT_DEPLOY_CAPSULE;
  if (capsulePath) {
    delete process.env.EXACT_DEPLOY_CAPSULE;
    let capsule;
    try { capsule = JSON.parse(readFileSync(capsulePath, 'utf8')); }
    catch (error) { refuse(`could not read the private deploy capsule: ${error.message}`); }
    rmSync(capsulePath, { force: true });
    return deployCaptured(opts, capsule);
  }
  return deploy(opts);
}

if (process.argv[1] && canonicalPath(process.argv[1]) === canonicalPath(fileURLToPath(import.meta.url))) {
  main().then((code) => { process.exitCode = code; }, (e) => {
    if (e instanceof Refusal) { console.error(`exact deploy: ${e.message}`); process.exitCode = 1; }
    else { console.error(e); process.exitCode = 1; }
  });
}
