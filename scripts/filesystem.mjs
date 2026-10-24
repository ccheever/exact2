// Tooling-only bridge to directory-owned operations. @ref LLP 1030.002.
// The host deployment target and rustc wrapper every cargo call shares (`withHostDeployment`).
// And the one removal the tooling does itself: a private directory its own run made (`removePrivateTree`);
// and the checkout's node_modules, installed from its lock when it has none or one did not finish (`installedModules`).
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { existsSync, linkSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { homedir, tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

// The macOS deployment target every cargo call on a Mac gives what it builds for the Mac
// itself: build scripts, proc macros and the developer tools (the bake tool, the filesystem
// helper). A build script that records MACOSX_DEPLOYMENT_TARGET (ring's, objc2's) is rebuilt
// whenever two calls sharing a target directory differ in it, so one value for all of them:
// an iOS build's host side, the bake tool's build and the helper's, from inside a build
// script or not. A macOS app's own build still says its product's target (host/apple/build.mjs).
export const HOST_MACOS = '14.0';

// With a deployment target of 12.0 or later, ld-27037.1 (Xcode 27.1) links with chained fixups and can
// write a dylib that dyld on macOS 27.0.1 refuses: "mis-aligned LINKEDIT string pool". rustc then fails
// on the proc macro it cannot load, with that message or as "can't find crate for `zerofrom_derive`".
// A deployment target is not in a proc macro's fingerprint, so a plain cargo call sharing the target
// directory (caps' wasm check, a release build) takes such a dylib too. So the scripts' cargo calls run
// rustc through `scripts/rustc/exact-rustc`, which links proc macros with `-no_fixup_chains`. Cargo
// leaves RUSTC_WRAPPER out of its fingerprints, so plain and scripted calls still share every unit; it
// is a name found on PATH so that kept modules' groups, which read it (scripts/kept.mjs), are the same
// in every checkout. Remove both when ld is fixed.
const WRAPPER = 'exact-rustc';

/** `env` with the host deployment target when it names none, and the rustc wrapper when it names none (on a Mac). */
export function withHostDeployment(env) {
  if (process.platform !== 'darwin') return env;
  const out = { ...env, MACOSX_DEPLOYMENT_TARGET: env.MACOSX_DEPLOYMENT_TARGET || HOST_MACOS };
  if (!out.RUSTC_WRAPPER) {
    const dir = resolve(root, 'scripts/rustc');
    out.RUSTC_WRAPPER = WRAPPER;
    out.PATH = [dir, ...(env.PATH ?? '').split(':').filter((entry) => entry && entry !== dir)].join(':');
  }
  return out;
}

const root = fileURLToPath(new URL('../', import.meta.url));
let binary;
const HELPER = 'EXACT_FILESYSTEM_HELPER';
/** The helper this process runs, checked current: the bake names it to the Bun processes it spawns. */
export const filesystemHelper = () => executable();
function executable() {
  if (binary) return binary;
  const directory = resolve(root, 'target/exact-filesystem-tool');
  const suffix = process.platform === 'win32' ? '.exe' : '';
  // A caller that checked the helper itself may name its captured copy in `EXACT_FILESYSTEM_HELPER`
  // for the processes it spawns (the bake does, once per process, for its many Bun calls). The copy
  // is taken after confirming it is this checkout's and still has the digest its name gives;
  // anything else is checked here as usual
  const inherited = process.env[HELPER];
  if (inherited && dirname(inherited) === directory) {
    const digest = new RegExp(`^exact-filesystem-([0-9a-f]{64})${suffix.replace('.', '\\.')}$`).exec(basename(inherited))?.[1];
    if (digest && existsSync(inherited) && createHash('sha256').update(readFileSync(inherited)).digest('hex') === digest) return binary = inherited;
  }
  // A helper another process checked from these same inputs: a record named by the digest of what the build reads
  // (`helperInputs`, never trusted by time), naming a capture whose bytes still have the digest its name gives.
  const inputs = helperInputs(), record = inputs && resolve(directory, `checked-${inputs}`);
  if (record) try {
    const named = readFileSync(record, 'utf8').trim(), digest = new RegExp(`^exact-filesystem-([0-9a-f]{64})${suffix.replace('.', '\\.')}$`).exec(named)?.[1];
    const captured = resolve(directory, named);
    if (digest && createHash('sha256').update(readFileSync(captured)).digest('hex') === digest) return binary = captured;
  } catch { /* none checked yet */ }
  const target = resolve(directory, `debug/exact-filesystem${suffix}`);
  // A separate tooling target also permits calls from an active app Cargo
  // build script: the helper has no compiler/app dependency and never waits
  // on that app build directory lock. Otherwise Cargo checks sources, lockfile
  // and toolchain, once per Bun process.
  // An mtime-only shortcut can silently reuse an obsolete helper dependency.
  // The host's deployment target always, whether a build script's (an app's macOS minimum) or not: its own target directory.
  const own = { ...process.env, HERMES_LEAN_SYS_OFFLINE: '1' };
  delete own.MACOSX_DEPLOYMENT_TARGET;
  const env = withHostDeployment(own);
  for (const name of ['CARGO_BUILD_TARGET', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTFLAGS', 'RUSTDOCFLAGS']) delete env[name];
  for (const name of ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER']) {
    if (env[name] && basename(env[name]) === 'clippy-driver') delete env[name];
  }
  delete env.CLIPPY_ARGS;
  // Cargo protects compilation, but releases its lock before we can exec.
  // Another profile can replace its public binary in that gap. Hold this
  // bootstrap claim through capture, then execute immutable captured bytes.
  // This is not the helper's stream lock; a verifiably dead builder can be replaced.
  mkdirSync(directory, { recursive: true });
  const claim = resolve(directory, '.bootstrap.lock'), owner = `${process.pid}-${randomBytes(12).toString('hex')}`;
  const releaseTicket = buildTicket(resolve(directory, '.bootstrap.claims'), owner);
  try { claimBuild(claim, owner); } catch (error) { releaseTicket(); throw error; }
  let held = true;
  const release = () => {
    if (!held) return;
    held = false;
    process.removeListener('exit', release);
    try { if (existsSync(claim) && readFileSync(claim, 'utf8') === owner) rmSync(claim); }
    finally { releaseTicket(); }
  };
  process.once('exit', release);
  try {
    const built = spawnSync('cargo', ['build', '--quiet', '--locked', '--offline', '-p', 'exact-filesystem', '--target-dir', directory], { cwd: root, env, encoding: 'utf8' });
    if (built.error) throw built.error;
    if (built.status !== 0) throw new Error(built.stderr || `could not build exact-filesystem (status ${built.status}, signal ${built.signal ?? 'none'})`);
    const bytes = readFileSync(target), digest = createHash('sha256').update(bytes).digest('hex');
    const captured = resolve(directory, `exact-filesystem-${digest}${suffix}`);
    if (!existsSync(captured)) {
      const temporary = `${captured}.${owner}.tmp`;
      try {
        writeFileSync(temporary, bytes, { flag: 'wx', mode: 0o755 });
        renameSync(temporary, captured);
      } finally { rmSync(temporary, { force: true }); }
    }
    binary = captured;
    // Recorded only when the inputs are what they were before the build: an edit while it ran could be in either.
    if (record && helperInputs() === inputs) {
      const temporary = `${record}.${owner}.tmp`;
      try { writeFileSync(temporary, basename(captured)); renameSync(temporary, record); } catch { rmSync(temporary, { force: true }); }
    }
    return captured;
  } finally { release(); }
}

/** The digest of what building the helper reads: its crate's files, the workspace manifest, lock and toolchain pin,
 * and Cargo's configuration (the checkout's, its parents', `CARGO_HOME`'s), with the platform it is built for; null
 * where the checkout has no helper crate to read. */
function helperInputs() {
  const hash = createHash('sha256').update(`${process.platform} ${process.arch} ${HOST_MACOS}\n`);
  const files = (dir) => readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))
    .flatMap((entry) => entry.isDirectory() ? files(resolve(dir, entry.name)) : [resolve(dir, entry.name)]);
  let crate;
  try { crate = files(resolve(root, 'filesystem')); } catch { return null; }
  const configs = [];
  for (let at = root; ; at = dirname(at)) { configs.push(resolve(at, '.cargo/config.toml'), resolve(at, '.cargo/config')); if (at === dirname(at)) break; }
  const home = process.env.CARGO_HOME ?? resolve(homedir(), '.cargo');
  for (const path of [...crate, ...['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'].map((f) => resolve(root, f)), ...configs, resolve(home, 'config.toml'), resolve(home, 'config')]) {
    hash.update(`${path}\n`);
    try { hash.update(readFileSync(path)); } catch { hash.update('missing'); }
    hash.update('\n');
  }
  return hash.digest('hex').slice(0, 32);
}

/** The wait for the helper's build claim. It ends when the owner releases the claim, and
 * has no duration while that process runs: callers take the claim in turn, some two hundred
 * of them in a cold workspace check, and a fixed bound fails everything still in line
 * together on a loaded Mac. `unowned` bounds only the state with nobody to wait on: a
 * malformed claim that names no process. A claim whose owner has verifiably exited
 * is taken over immediately. `now` is the clock that bound reads; a test advances it. */
export const helperBuildWait = { now: () => Date.now(), unowned: 60_000 };
const readClaim = (path) => {
  try { return readFileSync(path, 'utf8'); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
};
const pidRuns = (pid) => {
  try { process.kill(pid, 0); return true; } catch (error) { return error.code !== 'ESRCH'; }
};
// A killed builder's pid comes round again: a process that started after the claim was
// written is not its owner. `lstart` has whole seconds. Where `ps` cannot say (Windows),
// or the claim has gone, a running pid is believed. So is the pid of a claim seconds
// old, which no pid has had time to come round to: that spares the line of callers a
// `ps` each at every turn.
function pidRecycled(pid, claim) {
  let written;
  try { written = statSync(claim).mtimeMs; } catch { return false; }
  if (Date.now() - written < 10_000) return false;
  const ps = spawnSync('ps', ['-o', 'lstart=', '-p', String(pid)], { encoding: 'utf8', env: { ...process.env, LC_ALL: 'C' } });
  return Date.parse(ps.stdout?.trim() ?? '') > written + 1000;
}
// Bakery tickets serialize bootstrap acquisition and recovery without removing another
// live caller's claim. Each caller publishes choosing, then its ordered ticket atomically.
function buildTicket(directory, owner) {
  mkdirSync(directory, { recursive: true });
  const mine = resolve(directory, owner), ready = `${mine}.ready`;
  let held = true;
  const release = () => {
    if (!held) return;
    held = false;
    process.removeListener('exit', release);
    rmSync(mine, { force: true }); rmSync(ready, { force: true });
  };
  const owners = () => readdirSync(directory).filter(name => /^\d+-[0-9a-f]{24}$/.test(name));
  const recycled = new Map();
  const live = name => {
    const pid = Number(name.split('-')[0]);
    if (!pidRuns(pid)) return false;
    const now = Date.now(), before = recycled.get(name);
    if (!before || now - before.at > 10_000) recycled.set(name, { at: now, value: pidRecycled(pid, resolve(directory, name)) });
    return !recycled.get(name).value;
  };
  // Empty (including a writer interrupted while publishing) means choosing.
  const ticket = name => Number(readClaim(resolve(directory, name))) || 0;
  writeFileSync(mine, '', { flag: 'wx' });
  process.once('exit', release);
  try {
    const number = owners().filter(live).reduce((max, name) => Math.max(max, ticket(name)), 0) + 1;
    writeFileSync(ready, String(number), { flag: 'wx' }); renameSync(ready, mine);
    const pause = new Int32Array(new SharedArrayBuffer(4));
    for (;;) {
      const waiting = owners().some(name => {
        if (name === owner || !live(name)) return false;
        const value = ticket(name);
        return value === 0 || value < number || (value === number && name < owner);
      });
      if (!waiting) return release;
      helperBuildWait.now();
      Atomics.wait(pause, 0, 0, 25);
    }
  } catch (error) { release(); throw error; }
}
// One condition, the claim is free, and one bound, `unowned`. Nothing notifies, so it polls.
function claimBuild(claim, owner) {
  const pause = new Int32Array(new SharedArrayBuffer(4));
  let seen, since, recycled;
  for (;;) {
    try { writeFileSync(claim, owner, { flag: 'wx' }); return; }
    catch (error) { if (error.code !== 'EEXIST') throw error; }
    const text = readClaim(claim);
    if (text === null) continue; // released between the two calls
    const now = helperBuildWait.now(), pid = Number(/^(\d+)-[0-9a-f]{24}$/.exec(text)?.[1]);
    if (text !== seen) { seen = text; since = now; recycled = pid > 0 && pidRuns(pid) && pidRecycled(pid, claim); }
    const running = pid > 0 && !recycled && pidRuns(pid);
    if (pid > 0 && !running) {
      // Move the observed claim aside, compare its token, and preserve a replacement.
      // Never unlink a path on the strength of an earlier read of it.
      const aside = `${claim}.${owner}.dead`;
      try { renameSync(claim, aside); } catch (error) { if (error.code === 'ENOENT') continue; throw error; }
      if (readClaim(aside) !== text) {
        try { linkSync(aside, claim); } catch (error) { if (error.code !== 'EEXIST') throw error; }
      }
      rmSync(aside, { force: true });
      continue;
    }
    if (!running && now - since >= helperBuildWait.unowned) {
      const why = pid > 0 ? `pid ${pid} ${recycled ? 'is a later process, not its builder' : 'has stopped'}` : `it reads ${JSON.stringify(text)}, which names no process`;
      throw new Error(`filesystem helper: nothing has been building it for ${Math.round((now - since) / 1000)} s, yet its build claim is held (${claim}: ${why}). This is the tooling's own helper, not the code being built. A malformed claim cannot be recovered automatically. Remove it after verifying no builder owns it, or set EXACT_FILESYSTEM_HELPER to this checkout's intact digest-named capture.`);
    }
    Atomics.wait(pause, 0, 0, 25);
  }
}
export function filesystemErrorCode(errno, platform = process.platform) {
  const codes = platform === 'win32'
    ? {2:'ENOENT',3:'ENOENT',4:'EMFILE',5:'EACCES',32:'EBUSY',33:'EBUSY',80:'EEXIST',183:'EEXIST',145:'ENOTEMPTY',206:'ENAMETOOLONG',112:'ENOSPC',1117:'EIO'}
    : {2:'ENOENT',13:'EACCES',1:'EPERM',5:'EIO',24:'EMFILE',23:'ENFILE'};
  return codes[errno] ?? 'EXACT_FS_REFUSED';
}
function decode(response) {
  if (!response.error) return response.value;
  const error = new Error(response.error);
  // Windows reports Win32 error codes; 5 is access denied rather than Unix EIO.
  error.code = filesystemErrorCode(response.errno);
  throw error;
}
const request = (input) => ({ ...input, token: randomBytes(24).toString('hex') });
export function filesystem(input) {
  // A game asset tree includes every GPU texture family; base64 also adds a
  // third to the on-disk size. A 210 MiB native bundle exceeds a 256 MiB reply.
  const result = spawnSync(executable(), [], { input: `${JSON.stringify(request(input))}\n`, encoding: 'utf8', maxBuffer: 512 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`exact-filesystem exited before replying (status ${result.status}, signal ${result.signal ?? 'none'})${result.stderr ? `: ${result.stderr}` : ''}`);
  return decode(JSON.parse(result.stdout));
}
/** One child owns the root and OS lock throughout the asynchronous callback. */
export async function filesystemLock(root, path, fn) {
  const child = spawn(executable(), [], { stdio: ['pipe', 'pipe', 'pipe'] });
  const closed = new Promise((resolve) => child.once('close', resolve));
  const lines = createInterface({ input: child.stdout });
  const pending = [];
  let failure = null, stderr = '';
  child.stderr.on('data', (data) => { stderr += data; });
  const rejectAll = (error) => { failure = error; for (const item of pending.splice(0)) item.reject(error); };
  child.on('error', rejectAll);
  child.stdin.on('error', rejectAll);
  // Fail what is pending only once its output is read: a reply written just before exiting (a held lock's refusal)
  // answers its request, as the reader's does
  child.on('close', (code, signal) => { rejectAll(new Error(`filesystem lock holder exited (status ${code}, signal ${signal ?? 'none'})${stderr ? `: ${stderr}` : ''}`)); });
  lines.on('line', (line) => {
    const item = pending.shift();
    if (!item) return;
    try { item.resolve(decode(JSON.parse(line))); } catch (error) { item.reject(error); }
  });
  const send = (input) => new Promise((resolve, reject) => {
    if (failure) return reject(failure);
    pending.push({ resolve, reject });
    child.stdin.write(`${JSON.stringify(request(input))}\n`, (error) => { if (error) rejectAll(error); });
  });
  try {
    await send({ root, op: 'lock', path });
    return await fn(send);
  } finally {
    child.stdin.end();
    await closed;
    lines.close();
  }
}

// HTTP reads share a resident process: per-read exec can stall in macOS
// executable assessment. Idle pipes do not keep Bun alive; pending reads do.
// Parent exit terminates its helper, including one still awaiting OS startup.
let reader, closes = 0;
/** Ends the resident reader now (it would otherwise live until this process exits). */
export function closeFilesystemReader() {
  closes++;
  reader?.close();
  reader = undefined;
}
export function filesystemRead(input) {
  if (input.op !== 'get') return Promise.reject(new Error('read session only accepts get'));
  // A helper that exits with reads pending (killed, or crashed) answered none of them, and a caller would take the
  // failure for a missing file (serve.mjs's 404). A get changes nothing, so each is asked once more of a fresh
  // helper; a second exit is the read's failure. A read a close (`closeFilesystemReader`) came after is not asked
  // again, whichever was seen first, the close or the exit: nothing starts a helper after the close.
  const at = closes;
  return (reader ??= createReader()).send(input).catch((error) => {
    if (error.code !== READER_EXITED || closes !== at) throw error;
    return (reader ??= createReader()).send(input);
  });
}
const READER_EXITED = 'EXACT_FS_READER_EXITED';
function createReader() {
  const child = spawn(executable(), ['--serve-reads'], {stdio:['pipe','pipe','pipe']});
  const lines = createInterface({input:child.stdout});
  const stop = () => child.kill();
  process.once('exit', stop);
  const pending = [];
  let failure, stderr = '', referenced = true;
  const reference = active => {
    // Concurrent reads share one liveness reference. Repeated stderr.ref()
    // calls on Bun can otherwise leave the idle reader keeping its parent alive.
    if (referenced === active) return;
    referenced = active;
    for (const handle of [child, child.stdout, child.stderr]) handle[active ? 'ref' : 'unref']();
  };
  const fail = error => {
    if (failure) return;
    failure = error;
    if (reader === session) reader = undefined;
    for (const item of pending.splice(0)) item.reject(error);
    process.removeListener('exit', stop);
    lines.close(); child.stdin.destroy(); child.kill(); reference(false);
  };
  child.on('error', fail);
  // Its input pipe breaking (EPIPE: it exited before reading) is its exit too, whichever is seen first.
  const gone = error => fail(Object.assign(new Error(`filesystem reader exited (${error.message})`), { code: READER_EXITED, cause: error }));
  child.stdin.on('error', gone);
  child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192); });
  child.on('close', (code, signal) => fail(Object.assign(new Error(`filesystem reader exited (status ${code}, signal ${signal ?? 'none'})${stderr ? `: ${stderr}` : ''}`), { code: READER_EXITED })));
  lines.on('line', line => {
    const item = pending.shift();
    if (!item) { fail(new Error('unexpected filesystem reply')); return; }
    try { item.resolve(decode(JSON.parse(line))); } catch (error) { item.reject(error); }
    if (!pending.length) reference(false);
  });
  const session = {send(input) {
    if (failure) return Promise.reject(failure);
    reference(true);
    return new Promise((resolve, reject) => {
      pending.push({resolve, reject});
      child.stdin.write(`${JSON.stringify(request(input))}\n`, error => { if (error) gone(error); });
    });
  }, close() { fail(new Error('filesystem reader closed')); }};
  reference(false);
  return session;
}

/** Remove a private mkdtemp directory owned by this invocation. Bun 1.4.2's
 * recursive rm can silently leave entries in large captured Git repositories. */
export function removePrivateTree(path) {
  path = resolve(path);
  if (dirname(path) === path) throw new Error(`refusing to remove a filesystem root: ${path}`);
  if (process.platform === 'win32') {
    rmSync(path, {recursive:true, force:true, maxRetries:3, retryDelay:100});
    if (existsSync(path)) throw new Error(`could not remove private directory ${path}: directory remains`);
    return;
  }
  const result = spawnSync('/bin/rm', ['-rf', '--', path], { encoding: 'utf8' });
  if (result.status !== 0 || existsSync(path)) {
    throw new Error(`could not remove private directory ${path}: ${result.error?.message || result.stderr || result.signal || 'directory remains'}`);
  }
}

if (import.meta.main && process.argv[2] === '--helper') process.stdout.write(filesystemHelper());

/** This checkout's node_modules, installed from its lock when it has none (a new worktree's) or when an
 * install of ours did not finish. The install is complete when node_modules has no `.exact-install` in it:
 * the directory appears with that marker already inside (made aside, then renamed into place), and only a
 * successful `bun install` removes it, so one killed partway leaves it and is installed again, while one made
 * by hand (`bun install`, deploy's capture) has none. One process installs at a time, holding a lock file of
 * its pid and a token; the others wait. A lock whose process is gone, or older than any install, is taken
 * over by renaming exactly what was read aside, never by deleting a path read earlier. */
export function installedModules(modules = resolve(root, 'node_modules'), lock = resolve(tmpdir(), `exact-install-${createHash('sha256').update(root).digest('hex').slice(0, 16)}.lock`)) {
  const marker = resolve(modules, '.exact-install'), mine = `${process.pid} ${randomBytes(8).toString('hex')}`;
  const read = (path) => { try { return readFileSync(path, 'utf8'); } catch { return null; } };
  // Removes `path` only while it still holds `seen`: renamed aside, compared, and put back when another's.
  const drop = (path, seen) => {
    const aside = `${path}.${process.pid}`;
    try { renameSync(path, aside); } catch { return; }
    if (read(aside) !== seen) try { linkSync(aside, path); } catch { /* a newer lock stands */ }
    rmSync(aside, { force: true });
  };
  const stale = (seen) => {
    try { if (Date.now() - statSync(lock).mtimeMs > 600_000) return true; process.kill(Number(seen.split(' ')[0]), 0); return false; }
    catch (e) { return e.code === 'ESRCH'; }
  };
  while (!existsSync(modules) || existsSync(marker)) {
    try { writeFileSync(lock, mine, { flag: 'wx' }); } catch {
      const seen = read(lock);
      if (seen !== null && stale(seen)) drop(lock, seen); else Bun.sleepSync(200);
      continue;
    }
    try {
      if (existsSync(modules) && !existsSync(marker)) break;
      if (!existsSync(modules)) {
        const made = `${modules}.${process.pid}`;
        rmSync(made, { recursive: true, force: true });
        mkdirSync(made); writeFileSync(resolve(made, '.exact-install'), '');
        renameSync(made, modules);
      }
      console.error(`${resolve(root)}: ${existsSync(resolve(modules, '.bin')) ? 'an install of node_modules did not finish' : 'no node_modules'}; bun install --frozen-lockfile there`);
      if (spawnSync(process.execPath, ['install', '--frozen-lockfile'], { cwd: root, stdio: ['ignore', 2, 2] }).status !== 0) throw new Error(`${resolve(root)}: bun install --frozen-lockfile failed (above); run it there, then this again`);
      rmSync(marker, { force: true });
    } finally { drop(lock, mine); }
  }
  // What is installed is what `bun.lock` says (LLP 1133.001 D9): installed again from it when not, and
  // refused by name when that does not mend it.
  if (!lockMismatches(dirname(modules)).length) return;
  console.error(`${resolve(root)}: node_modules differs from bun.lock (${lockMismatches(dirname(modules)).join('; ')}); bun install --frozen-lockfile there`);
  if (spawnSync(process.execPath, ['install', '--frozen-lockfile'], { cwd: dirname(modules), stdio: ['ignore', 2, 2] }).status !== 0) throw new Error(`${dirname(modules)}: bun install --frozen-lockfile failed (above)`);
  const left = lockMismatches(dirname(modules));
  if (left.length) throw new Error(`${dirname(modules)}: node_modules still differs from bun.lock: ${left.join('; ')}`);
}

/** Each package `dir/bun.lock` pins at the top of its `node_modules` that is installed at another version,
 * or not at all: a registry package's `name@version` (a workspace's, `file:`'s and `github:`'s have none to
 * compare). `[]` without a lock. */
export function lockMismatches(dir = root) {
  let lock;
  try { lock = JSON.parse(readFileSync(resolve(dir, 'bun.lock'), 'utf8').replace(/,(\s*[}\]])/g, '$1')); } catch { return []; }
  const out = [];
  for (const [key, entry] of Object.entries(lock.packages ?? {})) {
    // A nested install (`rolldown/@oxc-project/types`) is the package beside its dependent, not at the top.
    if (key.split('/').length > (key.startsWith('@') ? 2 : 1)) continue;
    // A package for one platform (its `os` or `cpu`) is installed only there.
    if (Array.isArray(entry) && entry.some(e => e && typeof e === 'object' && (e.os || e.cpu))) continue;
    const spec = Array.isArray(entry) ? entry[0] : '', at = spec.lastIndexOf('@');
    const version = at > 0 ? spec.slice(at + 1) : '';
    if (!/^\d/.test(version)) continue;
    let installed = null;
    try { installed = JSON.parse(readFileSync(resolve(dir, 'node_modules', key, 'package.json'), 'utf8')).version; } catch { /* absent */ }
    if (installed !== version) out.push(`${key} ${installed ?? 'missing'}, bun.lock ${version}`);
  }
  return out;
}
