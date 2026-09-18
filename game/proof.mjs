// Shared lifecycle for game proofs: operations and assertions stay in the game.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { basename, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { open as openSession, render } from '../scripts/agent.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { buildBake, resolveApp } from '../scripts/app.mjs';

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
    } else if (host === 'linux') {
      for (const path of [artifacts.binary, artifacts.module]) manifest.push([basename(path), createHash('sha256').update(readFileSync(path)).digest('hex')]);
    } else {
      const executable = resolve(artifacts.bundle, host === 'macos' ? 'Contents/MacOS/ExactMac' : 'ExactIOS');
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

export async function proof(meta, script) {
  const app = fileURLToPath(new URL('.', meta.url)), name = basename(app);
  const root = fileURLToPath(new URL('..', import.meta.url));
  const host = process.argv[2] ?? 'linux', out = resolve(process.env.EXACT_PROOF_OUT ?? resolve(app, 'artifacts'));
  const buildOut = resolve(app, 'artifacts');
  mkdirSync(buildOut, {recursive:true});
  const appPrefix = relative(root, app) + '/';
  const dist = resolve(app, 'dist');
  mkdirSync(out, {recursive:true});
  // Re-execute the actual proof, comparing every session's final simulation state.
  if (process.argv.includes('--paranoid')) {
    if (!['web', 'linux'].includes(host)) throw new Error('--paranoid supports web and linux');
    let failed = false;
    for (const mode of ['0', '1', 'fresh-game']) {
      const started = performance.now();
      const child = spawn(process.execPath, [fileURLToPath(meta.url), host], {
        env:{...process.env, EXACT_GAME_PARANOID:mode, EXACT_GAME_PARANOID_COMPARE:'1'}, stdio:'inherit',
      });
      const code = await new Promise((ok, reject) => { child.on('exit', ok); child.on('error', reject); });
      console.log(`PARANOID ${name} ${host} ${mode}: ${((performance.now()-started)/1000).toFixed(3)} s (including build)`);
      failed ||= code !== 0;
    }
    process.exit(failed ? 1 : 0);
  }
  const finalWorlds = [];
  const compareParanoid = process.env.EXACT_GAME_PARANOID_COMPARE === '1';
  Object.assign(process.env, {EXACT_APP_DIR:app, EXACT_WEB_DIST:dist,
    EXACT_UPDATE_TRUST:'development'});
  const started = performance.now(), failures = [], transcript = [], replies = [], sessions = new Set();
  const say = line => { transcript.push(line); console.log(line); };
  const check = (label, ok, value) => {
    say(`${ok ? 'PASS' : 'FAIL'} ${label}${value === undefined ? '' : ': ' + JSON.stringify(value)}`);
    if (!ok) failures.push(label);
    return ok;
  };
  // The GPU-less host has no process tree to discover: retain the process
  // handles from the carrier and await them. Global ps can block indefinitely
  // on this Mac; an optional web descendant audit is bounded and never delays
  // headless gameplay verification.
  const children = [], recorded = new Map();
  const onProcess = child => { children.push(child); recorded.set(child.pid, 'carrier'); };
  let auditUnavailable = false, inventoryPending;
  const inventory = () => new Promise(resolve => {
    const child = spawn('ps', ['-axo', 'pid=,ppid=,lstart='], {stdio:['ignore','pipe','ignore']});
    let output = '', done = false;
    const finish = rows => { if (done) return; done = true; clearTimeout(timer); resolve(rows); };
    const timer = setTimeout(() => {
      auditUnavailable = true;
      child.kill('SIGKILL'); // This invocation's recorded ps, never a name/pattern.
      child.stdout.destroy(); child.unref(); finish(null);
    }, 200);
    child.stdout.on('data', data => output += data);
    child.on('error', () => { auditUnavailable = true; finish(null); });
    child.on('exit', code => finish(code === 0 ? output.trim().split('\n').map(line => {
      const m = line.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);
      return m && {pid:Number(m[1]), parent:Number(m[2]), stamp:m[3]};
    }).filter(Boolean) : null));
  });
  const sample = () => {
    if (host === 'linux' || auditUnavailable || inventoryPending) return;
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
  const open = async (options = {}) => {
    const raw = await openSession({host, app:name, size:[1280,720], webDist:dist, onProcess, ...options});
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
        }
        sample();
      } finally { await raw.close(); closed = true; }
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
    if (!['web','macos','ios','linux'].includes(host)) throw new Error(`proof host unavailable: ${host}`);
    const files = spawnSync('git', ['ls-files','--cached','--others','--exclude-standard'], {cwd:root, encoding:'utf8'});
    // Fleet source exports have no .git directory. Walk the same source tree,
    // excluding build outputs; the extension/path filters below still apply.
    if (files.status !== 0) {
      const walk = (dir, prefix = '') => readdirSync(dir, {withFileTypes:true}).flatMap(entry => {
        if (['.git','target','node_modules','.build','.shells','dist','artifacts'].includes(entry.name)) return [];
        const path = prefix + entry.name;
        return entry.isDirectory() ? walk(resolve(dir,entry.name), path + '/') : entry.isFile() ? [path] : [];
      });
      files.stdout = walk(root).join('\n');
    }
    const hash = createHash('sha256').update(host).update(resolveApp(name).target);
    if (host === 'web') hash.update(process.env.EXACT_GAME_PARANOID ?? '0');
    for (const file of [...new Set(files.stdout.trim().split('\n'))].sort()) {
      if ((/^(game\/(bench|twins|diaries|artifacts)\/|llp\/)/.test(file) && !file.startsWith(appPrefix))
        || (file.startsWith('game/games/') && !file.startsWith(appPrefix))
        || /(^|\/)(artifacts|dist|target|node_modules|tests|examples)\//.test(file)
        || file.startsWith('apps/')
        || (!/\.(rs|toml|lock|contract|ts|js|mjs|wgsl|json|swift|h|c|html|css)$/.test(file)
          && file !== 'game/README.md' && !file.startsWith(`game/games/${name}/art/`) && !file.startsWith(`game/games/${name}/assets/`) && !file.startsWith(`game/games/${name}/deck/`))
        || /(^|\/)(proof\.mjs|.*\.test\.mjs)$/.test(file)
        || !existsSync(resolve(root,file))) continue;
      hash.update(file).update(readFileSync(resolve(root,file)));
    }
    const digest = hash.digest('hex'), receipt = resolve(buildOut, `build-${host}.sha256`);
    const appInfo = resolveApp(name);
    const linuxTarget = host === 'linux' ? spawnSync('rustc', ['-vV'], {encoding:'utf8'}).stdout.match(/^host: (.+)$/m)?.[1] : null;
    const artifacts = host === 'linux' ? {binary:resolve(appInfo.target, linuxTarget, `release/${appInfo.crate('linux')}`), module:resolve(appInfo.target, linuxTarget, `release/lib${appInfo.crate('gpu').replaceAll('-','_')}.${process.platform === 'darwin' ? 'dylib' : 'so'}`)} : host === 'web' ? null : appleArtifacts(appInfo, {destination:host === 'macos' ? 'macos' : 'ios-simulator'});
    if (host === 'linux') process.env.EXACT_LINUX_BIN = artifacts.binary;
    let artifact = artifactDigest(host, dist, artifacts);
    const stamp = () => JSON.stringify({inputs:digest, artifact});
    if (!artifact || !existsSync(receipt) || readFileSync(receipt,'utf8') !== stamp()) {
      say(`BUILD ${name} ${host}`);
      const disk = spawnSync('df', ['-h', '/System/Volumes/Data'], {encoding:'utf8'});
      say(disk.stdout.trim());
      if (disk.status !== 0) throw new Error('disk check failed before build');
      if (host === 'linux') {
        if (!linuxTarget) throw new Error('rustc did not report its target');
        buildBake(appInfo, 'linux', linuxTarget);
      } else {
        const child = spawn('bun', [resolve(root,host === 'web' ? 'host/web/build.mjs' : 'host/apple/build.mjs'), ...(host === 'ios' ? ['--ios'] : host === 'macos' ? ['--bundle'] : [])], {cwd:root, env:process.env, stdio:'inherit'});
        sample();
        const code = await new Promise((ok, reject) => {child.on('exit',ok); child.on('error',reject);});
        if (code !== 0) throw new Error(`app build exited ${code}`);
      }
      artifact = artifactDigest(host, dist, artifacts);
      if (!artifact) throw new Error('build produced no complete proof artifact');
      writeFileSync(receipt,stamp());
    } else say(`BUILD cached ${name} ${host}`);
    if (!process.argv.includes('--build-only')) await script({open, check, equal, out, host, say});
  } catch (error) { check('proof interrupted',false,error.stack ?? String(error)); }
  finally {
    await closeSessions(monitor, sample, sessions, check);
    await inventoryPending;
    let remaining = children.filter(child => child.exitCode === null && child.signalCode === null)
      .map(child => ({pid:child.pid}));
    if (host !== 'linux' && !auditUnavailable) {
      const rows = await inventory();
      if (rows) remaining.push(...rows.filter(row => recorded.get(row.pid) === row.stamp));
    }
    if (auditUnavailable) say('SKIP descendant process audit: ps stalled; carrier close still awaited every recorded host process.');
    check('all recorded children exited', remaining.length === 0, remaining);
    if (compareParanoid) {
      finalWorlds.sort((a,b) => a.session - b.session);
      const baseline = resolve(out, `paranoid-${host}-normal.json`);
      writeFileSync(resolve(out, `paranoid-${host}-${process.env.EXACT_GAME_PARANOID}.json`), JSON.stringify(finalWorlds));
      if (process.env.EXACT_GAME_PARANOID === '0') writeFileSync(baseline, JSON.stringify(finalWorlds));
      else check('paranoid final hash, tick, published record and journal equal normal',
        equal(finalWorlds, JSON.parse(readFileSync(baseline, 'utf8'))),
        finalWorlds.map(({session, tick, hash}) => ({session, tick, hash})));
    }
    writeFileSync(resolve(out,'process-cleanup.json'),JSON.stringify({recorded:[...recorded],remaining,auditUnavailable},null,2)+'\n');
    const saves = [...new Set(replies.filter(r => r.method === 'screenshot' && r.args[2] === 'save' && !r.error).map(r => r.args[0]))].sort().map(path => {
      const name = basename(path), bytes = readFileSync(path);
      return {name, bytes:bytes.length, sha256:createHash('sha256').update(bytes).digest('hex')};
    });
    writeFileSync(resolve(out,'summary.json'), JSON.stringify({name, host, failures, seconds:(performance.now()-started)/1000, worlds:finalWorlds, saves, auditUnavailable}, null, 2)+'\n');
    say(`PROOF ${failures.length ? 'FAIL' : 'PASS'} ${name} ${host}: ${failures.length} failures; ${((performance.now()-started)/1000).toFixed(3)} s`);
    writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
    writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
  }
  process.exit(failures.length ? 1 : 0);
}
