// Tooling-only bridge to directory-owned operations. @ref LLP 1030.002.
import { spawn, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

const root = fileURLToPath(new URL('../', import.meta.url));
let binary;
function executable() {
  if (binary) return binary;
  const target = resolve(root, 'target/exact-filesystem-tool/debug/exact-filesystem');
  // A separate tooling target also permits calls from an active app Cargo
  // build script: the helper has no compiler/app dependency and never waits
  // on that app build directory lock. Once per Node process, Cargo checks
  // sources, lockfile and toolchain.
  // An mtime-only shortcut can silently reuse an obsolete helper dependency.
  const env = { ...process.env };
  for (const name of ['CARGO_BUILD_TARGET', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTFLAGS', 'RUSTDOCFLAGS']) delete env[name];
  const built = spawnSync('cargo', ['build', '--quiet', '--locked', '--offline', '-p', 'exact-filesystem', '--target-dir', resolve(root, 'target/exact-filesystem-tool')], { cwd: root, env, encoding: 'utf8' });
  if (built.error) throw built.error;
  if (built.status !== 0) throw new Error(built.stderr || 'could not build exact-filesystem');
  binary = target;
  return target;
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
  if (result.status !== 0) throw new Error(result.stderr || 'exact-filesystem exited before replying');
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
  child.on('exit', (code) => { rejectAll(new Error(stderr || `filesystem lock holder exited ${code}`)); });
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
