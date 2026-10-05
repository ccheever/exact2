// Tooling-only bridge to directory-owned operations. @ref LLP 1030.002.
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

const root = fileURLToPath(new URL('../', import.meta.url));
let binary;
function executable() {
  if (binary) return binary;
  const directory = resolve(root, 'target/exact-filesystem-tool');
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const target = resolve(directory, `debug/exact-filesystem${suffix}`);
  // A separate tooling target also permits calls from an active app Cargo
  // build script: the helper has no compiler/app dependency and never waits
  // on that app build directory lock. Once per Bun process, Cargo checks
  // sources, lockfile and toolchain.
  // An mtime-only shortcut can silently reuse an obsolete helper dependency.
  const env = { ...process.env };
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
  const started = Date.now(), wait = new Int32Array(new SharedArrayBuffer(4));
  for (;;) {
    try { writeFileSync(claim, owner, { flag: 'wx' }); break; }
    catch (error) {
      if (error.code !== 'EEXIST') throw error;
      if (Date.now() - started >= 60000) throw new Error(`filesystem helper build busy (${claim}); remove a stale claim only after verifying its owner has exited`);
      Atomics.wait(wait, 0, 0, 25);
    }
  }
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
