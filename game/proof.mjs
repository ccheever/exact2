// Shared lifecycle for game proofs: operations and assertions stay in the game.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { open as openSession, render } from '../scripts/agent.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { resolveApp } from '../scripts/app.mjs';

export function artifactDigest(host, dist, artifacts) {
  try {
    if (host === 'web') return createHash('sha256').update(readFileSync(resolve(dist, 'exact.json'))).digest('hex');
    if (!existsSync(artifacts.bundle)) return null;
    const executable = resolve(artifacts.bundle, host === 'macos' ? 'Contents/MacOS/ExactMac' : 'ExactIOS');
    if (!existsSync(executable)) return null;
    const manifest = [];
    const walk = (dir, prefix) => {
      for (const name of readdirSync(dir).sort()) {
        const path = resolve(dir, name), key = `${prefix}/${name}`;
        if (statSync(path).isDirectory()) walk(path, key);
        else manifest.push([key, createHash('sha256').update(readFileSync(path)).digest('hex')]);
      }
    };
    walk(artifacts.bundle, 'bundle');
    // The driver launches the standalone product on macOS, loading its adjacent dylibs/assets.
    if (artifacts.binary) {
      readFileSync(artifacts.binary); // A missing actual carrier always invalidates the receipt.
      walk(artifacts.products ?? resolve(artifacts.binary, '..'), 'product');
    }
    return createHash('sha256').update(JSON.stringify(manifest)).digest('hex');
  } catch { return null; }
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
export async function proof(meta, script) {
  const app = fileURLToPath(new URL('.', meta.url)), name = basename(app);
  const root = fileURLToPath(new URL('..', import.meta.url));
  const host = process.argv[2] ?? 'web', out = resolve(app, 'artifacts');
  const dist = resolve(app, 'dist');
  mkdirSync(out, {recursive:true});
  Object.assign(process.env, {EXACT_APP_DIR:app, EXACT_WEB_DIST:dist,
    EXACT_UPDATE_TRUST:'development'});
  const started = performance.now(), failures = [], transcript = [], replies = [], sessions = new Set();
  const say = line => { transcript.push(line); console.log(line); };
  const check = (label, ok, value) => {
    say(`${ok ? 'PASS' : 'FAIL'} ${label}${value === undefined ? '' : ': ' + JSON.stringify(value)}`);
    if (!ok) failures.push(label);
    return ok;
  };
  const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  // Record this process's descendants while they exist; never signal an unrelated PID.
  const recorded = new Map();
  const inventory = () => {
    const result = spawnSync('ps', ['-axo', 'pid=,ppid=,lstart='], {encoding:'utf8'});
    if (result.status !== 0) return null;
    return result.stdout.trim().split('\n').map(line => {
      const m = line.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);
      return m && {pid:Number(m[1]), parent:Number(m[2]), stamp:m[3]};
    }).filter(Boolean);
  };
  const record = () => {
    const rows = inventory() ?? [], owned = new Set([process.pid]);
    for (let changed = true; changed;) {
      changed = false;
      for (const row of rows) if (owned.has(row.parent) && !owned.has(row.pid)) {
        owned.add(row.pid); recorded.set(row.pid, row.stamp); changed = true;
      }
    }
  };
  const sample = () => { try { record(); } catch {} };
  const monitor = setInterval(sample, 100);
  const open = async (options = {}) => {
    const raw = await openSession({host, app:name, size:[1280,720], webDist:dist, ...options});
    sample();
    let closed = false;
    const close = async () => { if (!closed) { try { sample(); } finally { await raw.close(); closed = true; } } };
    sessions.add({close});
    const id = sessions.size;
    return new Proxy(raw, {get(target, method) {
      if (method === 'close') return close;
      if (!['tap','type','clock','state','tree','layout','logs','screenshot'].includes(method)) return target[method];
      return async (...args) => {
        try {
          const reply = await target[method](...args);
          replies.push({session:id, method, args, reply});
          say(`${method} ${args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ')}\n${render(method,reply)}`);
          return reply;
        } catch (error) {
          replies.push({session:id, method, args, error:error.message, steps:error.steps}); if (error.steps) say(render('type', {steps:error.steps})); throw error;
        }
      };
    }});
  };
  try {
    if (!['web','macos','ios'].includes(host)) throw new Error(`proof host unavailable: ${host}`);
    const files = spawnSync('git', ['ls-files','--cached','--others','--exclude-standard'], {cwd:root, encoding:'utf8'});
    if (files.status !== 0) throw new Error('cannot enumerate build inputs');
    const hash = createHash('sha256').update(host).update(resolveApp(name).target);
    for (const file of [...new Set(files.stdout.trim().split('\n'))].sort()) {
      if (/^(game\/(bench|twins|diaries|artifacts)\/|llp\/)/.test(file)
        || (file.startsWith('game/games/') && !file.startsWith(`game/games/${name}/`))
        || /(^|\/)(artifacts|dist|target|node_modules|tests|examples)\//.test(file)
        || file.startsWith('apps/')
        || (!/\.(rs|toml|lock|contract|ts|js|mjs|wgsl|json|swift|h|c|html|css)$/.test(file)
          && file !== 'game/README.md' && !file.startsWith(`game/games/${name}/assets/`) && !file.startsWith(`game/games/${name}/deck/`))
        || /(^|\/)(proof\.mjs|.*\.test\.mjs)$/.test(file)
        || !existsSync(resolve(root,file))) continue;
      hash.update(file).update(readFileSync(resolve(root,file)));
    }
    const digest = hash.digest('hex'), receipt = resolve(out, `build-${host}.sha256`);
    const artifacts = host === 'web' ? null : appleArtifacts(resolveApp(name), {destination:host === 'macos' ? 'macos' : 'ios-simulator'});
    let artifact = artifactDigest(host, dist, artifacts);
    const stamp = () => JSON.stringify({inputs:digest, artifact});
    if (!artifact || !existsSync(receipt) || readFileSync(receipt,'utf8') !== stamp()) {
      say(`BUILD ${name} ${host}`);
      const child = spawn('bun', [resolve(root,host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios'] : host === 'macos' ? ['--bundle'] : [])], {cwd:root, env:process.env, stdio:'inherit'});
      sample();
      const code = await new Promise((ok, reject) => {child.on('exit',ok); child.on('error',reject);});
      if (code !== 0) throw new Error(`app build exited ${code}`);
      artifact = artifactDigest(host, dist, artifacts);
      if (!artifact) throw new Error('build produced no complete proof artifact');
      writeFileSync(receipt,stamp());
    } else say(`BUILD cached ${name} ${host}`);
    await script({open, check, equal, out, host, say});
  } catch (error) { check('proof interrupted',false,error.stack ?? String(error)); }
  finally {
    await closeSessions(monitor, sample, sessions, check);
    // The carriers await their leaders; also await every recorded descendant.
    let remaining = [];
    for (let round = 0; round < 40; round++) {
      const rows = inventory();
      if (!rows) { check("cleanup inventory available", false); break; }
      remaining = rows.filter(row => recorded.get(row.pid) === row.stamp);
      if (!remaining.length) break;
      if (round === 10) for (const row of remaining) { try { process.kill(row.pid, 'SIGKILL'); } catch {} }
      await new Promise(ok => setTimeout(ok,50));
    }
    check('all recorded children exited', remaining.length === 0, remaining);
    writeFileSync(resolve(out,'process-cleanup.json'),JSON.stringify({recorded:[...recorded],remaining},null,2)+'\n');
    say(`PROOF ${failures.length ? 'FAIL' : 'PASS'} ${name} ${host}: ${failures.length} failures; ${((performance.now()-started)/1000).toFixed(3)} s`);
    writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
    writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
  }
  process.exit(failures.length ? 1 : 0);
}
