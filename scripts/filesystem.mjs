// Tooling-only bridge to directory-owned operations. @ref LLP 1030.002.
import { spawn, spawnSync } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { homedir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

const root = fileURLToPath(new URL('../', import.meta.url));
let binary;
// The helper has one local source tree and registry dependencies. Reuse its
// captured executable only after hashing that complete source closure. Cargo
// remains the fallback for custom configurations, wrappers or local dependencies.
// No source freshness decision depends on mtimes or the mutable debug binary.
function toolSignature(trees, env, names) {
  const hash = createHash('sha256'), source = createHash('sha256');
  const visit = path => {
    source.update(JSON.stringify(path));
    if (!existsSync(path)) { source.update('missing'); return; }
    const stat = lstatSync(path);
    if (stat.isSymbolicLink()) throw new Error('linked helper input');
    if (stat.isDirectory()) {
      for (const name of readdirSync(path).sort()) visit(resolve(path, name));
    } else if (stat.isFile()) source.update(createHash('sha256').update(readFileSync(path)).digest());
    else throw new Error('nonregular helper input');
  };
  // Config can select arbitrary tools and environment-dependent build behavior.
  // Let Cargo handle that configuration rather than approximate its semantics.
  const configs = [resolve(env.CARGO_HOME ?? resolve(homedir(), '.cargo'))];
  for (let path = resolve(root);;) {
    configs.push(resolve(path, '.cargo'));
    if (dirname(path) === path) break;
    path = dirname(path);
  }
  if (configs.some(path => ['config','config.toml'].some(name => existsSync(resolve(path,name))))
    || Object.keys(env).some(name => /^(?:RUSTC|CARGO_BUILD_RUSTC).*WRAPPER$/.test(name) && env[name])
    || env.RUSTC || env.CARGO_BUILD_RUSTC) return null;
  for (const command of ['rustc', 'cargo']) {
    const version = spawnSync(command, ['-Vv'], {cwd:root, env, encoding:'utf8'});
    if (version.status !== 0) return null;
    hash.update(version.stdout);
  }
  for (const name of [...new Set([...names, ...Object.keys(env).filter(name => /^(?:CARGO|RUST|CC|CXX|AR|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS|SDKROOT|MACOSX_DEPLOYMENT_TARGET|PATH$|HOME$)/.test(name))])].sort()) {
    hash.update(JSON.stringify([name, env[name] ?? null]));
  }
  for (const path of [...new Set([...trees, resolve(root,'Cargo.toml'), resolve(root,'Cargo.lock'), resolve(root,'rust-toolchain'), resolve(root,'rust-toolchain.toml')])].sort()) visit(path);
  const bytes = source.digest('hex');
  return {source:bytes, signature:hash.update(bytes).digest('hex')};
}
function toolInputs(messages, directory) {
  const trees = new Set(), names = new Set(), packages = new Map();
  for (const message of messages) {
    if (message.reason !== 'compiler-artifact') continue;
    let path = dirname(message.target.src_path);
    while (!existsSync(resolve(path,'Cargo.toml'))) {
      if (dirname(path) === path) return null;
      path = dirname(path);
    }
    if (path !== resolve(root,'filesystem') && !message.package_id.startsWith('registry+')) return null;
    trees.add(path);
    packages.set(Bun.TOML.parse(readFileSync(resolve(path,'Cargo.toml'),'utf8')).package.name, path);
  }
  // Cargo's build-script and rustc environment dependencies are part of the
  // identity even when a variable has no Cargo/Rust prefix (e.g. libc's flags).
  const walk = path => {
    for (const entry of readdirSync(path, {withFileTypes:true})) {
      const file = resolve(path,entry.name);
      if (entry.isDirectory()) walk(file);
      else if (entry.name.endsWith('.d') || entry.name === 'output') {
        const text = readFileSync(file,'utf8');
        for (const match of text.matchAll(/(?:# env-dep:|cargo::?rerun-if-env-changed=)([^=\r\n]+)(?:=[^\r\n]*)?/g)) names.add(match[1]);
        if (entry.name === 'output') {
          const owner = packages.get(basename(path).replace(/-[a-f0-9]+$/, ''));
          if (!owner) throw new Error('unknown helper build script');
          for (const match of text.matchAll(/cargo::?rerun-if-changed=([^\r\n]+)/g)) {
            const input = resolve(owner, match[1]);
            if (input !== owner && !input.startsWith(owner + '/')) throw new Error('helper input outside package');
          }
        }
      }
    }
  };
  for (const dir of ['deps','build']) walk(resolve(directory,'debug',dir));
  return trees.has(resolve(root,'filesystem')) ? {trees:[...trees].sort(), names:[...names].sort()} : null;
}
function executable() {
  if (binary) return binary;
  const directory = resolve(root, 'target/exact-filesystem-tool');
  const target = resolve(directory, 'debug/exact-filesystem');
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
  const receiptPath = resolve(directory, 'captured.json');
  let previous, before, damaged;
  const reuse = () => {
    previous = before = undefined; damaged = false;
    try {
      const receipt = JSON.parse(readFileSync(receiptPath,'utf8'));
      if (receipt.version === 3) { previous = receipt; before = toolSignature(receipt.trees, env, receipt.names); }
      const capture = receipt.version === 3 && before && [receipt, ...(receipt.captures ?? [])]
        .find(capture => capture.signature && capture.signature === before.signature && /^[a-f0-9]{64}$/.test(capture.digest));
      if (capture) {
        const captured = resolve(directory, `exact-filesystem-${capture.digest}`);
        damaged = true;
        if (createHash('sha256').update(readFileSync(captured)).digest('hex') === capture.digest) return binary = captured;
      }
    } catch { /* A missing, damaged or unsupported capture goes through Cargo. */ }
  };
  if (reuse()) return binary;
  // Cargo protects compilation, but releases its lock before we can exec.
  // Another profile can replace its public binary in that gap. Hold this
  // bootstrap claim through capture, then execute immutable captured bytes.
  // This is not the helper's stream lock; stale build claims are never stolen.
  mkdirSync(directory, { recursive: true });
  const claim = resolve(directory, '.bootstrap.lock'), owner = `${process.pid}:${randomBytes(12).toString('hex')}`;
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
    // Another caller can complete the same capture while this one waits.
    if (reuse()) return binary;
    // Cargo's own freshness uses timestamps. A byte-invalidated capture must
    // not relabel an old executable after a same-mtime source edit. Also force
    // the seed's verification build and recovery from damaged captured bytes.
    // Running readers and the bootstrap claim live outside this private output.
    if (previous && before && (previous.rebuild || previous.source !== before.source || damaged)) {
      rmSync(resolve(directory, 'debug'), {recursive:true,force:true});
    }
    const built = spawnSync('cargo', ['build', '--quiet', '--locked', '--offline', '-p', 'exact-filesystem', '--target-dir', directory, '--message-format=json'], { cwd: root, env, encoding: 'utf8', maxBuffer:64*1024*1024 });
    if (built.error) throw built.error;
    if (built.status !== 0) throw new Error(built.stderr || `could not build exact-filesystem (status ${built.status}, signal ${built.signal ?? 'none'})`);
    const bytes = readFileSync(target), digest = createHash('sha256').update(bytes).digest('hex');
    const captured = resolve(directory, `exact-filesystem-${digest}`);
    if (!existsSync(captured) || createHash('sha256').update(readFileSync(captured)).digest('hex') !== digest) {
      const temporary = `${captured}.${owner}.tmp`;
      try {
        writeFileSync(temporary, bytes, { flag: 'wx', mode: 0o755 });
        renameSync(temporary, captured);
      } finally { rmSync(temporary, { force: true }); }
    }
    binary = captured;
    try {
      const inputs = toolInputs(built.stdout.trim().split('\n').filter(Boolean).map(line => JSON.parse(line)), directory);
      if (inputs) {
        const after = toolSignature(inputs.trees, env, inputs.names);
        // Unsupported custom configurations must use Cargo, but need not evict
        // captures whose complete source/configuration identity we can verify.
        if (!after) return captured;
        // A new closure is only a seed. Its next Cargo build must observe the
        // same complete inputs before and after compilation before reuse begins.
        const signature = before && before.signature === after?.signature
          && JSON.stringify(inputs) === JSON.stringify({trees:previous.trees,names:previous.names}) ? after.signature : null;
        // Build scripts and interactive tools have different Cargo environments.
        // Keep a bounded set for this exact source closure, so alternating
        // callers do not turn every launch into another Cargo invocation.
        const captures = previous?.source === after.source
          && JSON.stringify(inputs) === JSON.stringify({trees:previous.trees,names:previous.names})
          ? [previous, ...(previous.captures ?? [])].filter(c => c.signature && c.signature !== signature)
            .slice(0, 7).map(({signature,digest}) => ({signature,digest})) : [];
        const temporary = `${receiptPath}.${owner}.tmp`;
        try {
          writeFileSync(temporary, JSON.stringify({version:3,...inputs,source:after.source,signature,rebuild:!signature,digest,captures}));
          renameSync(temporary, receiptPath);
        } finally { rmSync(temporary, {force:true}); }
      }
    } catch { /* Capturing is an optimization; Cargo already verified this run. */ }
    return captured;
  } finally { release(); }
}
function decode(response) {
  if (!response.error) return response.value;
  const error = new Error(response.error);
  // errno values shared by Darwin and Linux for these availability failures.
  error.code = ({ 2: 'ENOENT', 13: 'EACCES', 1: 'EPERM', 5: 'EIO', 24: 'EMFILE', 23: 'ENFILE' })[response.errno] ?? 'EXACT_FS_REFUSED';
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
