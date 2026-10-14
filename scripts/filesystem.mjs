// Tooling-only bridge to directory-owned operations. @ref LLP 1030.002.
// And the one removal the tooling does itself: a private directory its own run made (`removePrivateTree`).
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

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
  const target = resolve(directory, `debug/exact-filesystem${suffix}`);
  // A separate tooling target also permits calls from an active app Cargo
  // build script: the helper has no compiler/app dependency and never waits
  // on that app build directory lock. Once per Bun process, Cargo checks
  // sources, lockfile and toolchain.
  // An mtime-only shortcut can silently reuse an obsolete helper dependency.
  const env = { ...process.env, HERMES_LEAN_SYS_OFFLINE: '1' };
  for (const name of ['CARGO_BUILD_TARGET', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTFLAGS', 'RUSTDOCFLAGS']) delete env[name];
  for (const name of ['RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER']) {
    if (env[name] && basename(env[name]) === 'clippy-driver') delete env[name];
  }
  delete env.CLIPPY_ARGS;
  // Cargo protects compilation, but releases its lock before we can exec.
  // Another profile can replace its public binary in that gap. Hold this
  // bootstrap claim through capture, then execute immutable captured bytes.
  // This is not the helper's stream lock; stale build claims are never stolen.
  mkdirSync(directory, { recursive: true });
  const claim = resolve(directory, '.bootstrap.lock'), owner = `${process.pid}-${randomBytes(12).toString('hex')}`;
  claimBuild(claim, owner);
  let held = true;
  const release = () => {
    if (!held) return;
    held = false;
    process.removeListener('exit', release);
    if (existsSync(claim) && readFileSync(claim, 'utf8') === owner) rmSync(claim);
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
    return captured;
  } finally { release(); }
}

/** The wait for the helper's build claim. It ends when the owner releases the claim, and
 * has no duration while that process runs: callers take the claim in turn, some two hundred
 * of them in a cold workspace check, and a fixed bound fails everything still in line
 * together on a loaded Mac. `unowned` bounds only the state with nobody to wait on: a
 * claim that names no running process, which is never taken (LLP 1030.002 D3) and so
 * fails. `now` is the clock that bound reads; a test advances it. */
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
    if (!running && now - since >= helperBuildWait.unowned) {
      const why = pid > 0 ? `pid ${pid} ${recycled ? 'is a later process, not its builder' : 'has stopped'}` : `it reads ${JSON.stringify(text)}, which names no process`;
      throw new Error(`filesystem helper: nothing has been building it for ${Math.round((now - since) / 1000)} s, yet its build claim is held (${claim}: ${why}). This is the tooling's own helper, not the code being built. A stale claim is never taken automatically: remove it only after verifying its owner has exited.`);
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
  const result = spawnSync(executable(), [], { input: `${JSON.stringify(request(input))}\n`, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
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
  child.on('exit', (code, signal) => { rejectAll(new Error(`filesystem lock holder exited (status ${code}, signal ${signal ?? 'none'})${stderr ? `: ${stderr}` : ''}`)); });
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
let reader;
/** Ends the resident reader now (it would otherwise live until this process exits). */
export function closeFilesystemReader() {
  reader?.close();
  reader = undefined;
}
export function filesystemRead(input) {
  if (input.op !== 'get') return Promise.reject(new Error('read session only accepts get'));
  reader ??= createReader();
  return reader.send(input);
}
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
  child.stdin.on('error', fail);
  child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192); });
  child.on('close', (code, signal) => fail(new Error(`filesystem reader exited (status ${code}, signal ${signal ?? 'none'})${stderr ? `: ${stderr}` : ''}`)));
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
      child.stdin.write(`${JSON.stringify(request(input))}\n`, error => { if (error) fail(error); });
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
