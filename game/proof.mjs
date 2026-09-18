// Shared lifecycle for game proofs: operations and assertions stay in the game.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, relative, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { open as openSession, render } from '../scripts/agent.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { buildBake, resolveApp } from '../scripts/app.mjs';

export function artifactInventory(host, dist, artifacts) {
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
    return {digest:createHash('sha256').update(JSON.stringify(manifest)).digest('hex'), files:manifest};
  } catch { return null; }
}
export function artifactDigest(host, dist, artifacts) {
  return artifactInventory(host, dist, artifacts)?.digest ?? null;
}

/** Existing proof entry point: world-only data captures; no embedded source is executed. */
export function captureTools({open, identity, host}) {
  const current = () => {
    const receipt = identity();
    if (!receipt?.digest || !Array.isArray(receipt.files) || !receipt.files.length) throw new Error('capture: loaded artifact identity unavailable; use a local authenticated proof build');
    return receipt;
  };
  return {
    async capture(session, target = 'world', {script = '', failure = '', external = false, limits} = {}) {
      if (external) throw new Error('capture refused: unsupported external dependency');
      const receipt = current();
      if (!session.loadedArtifact || session.loadedArtifact !== receipt.digest) throw new Error('capture: loaded versus current artifact differs; start a new isolated proof session');
      await session.world(target).capture('start', {build:receipt.digest, ...(limits ? {limits} : {})});
      return {
        async finish(path) {
          if (current().digest !== receipt.digest) throw new Error('capture: artifacts changed during recording; exact capture refused');
          const reply = await session.world(target).capture('stop');
          const state = await session.state(target, {world:true}), logs = await session.logs();
          const bundle = {format:'exact-game-capture-1', scope:'world-only', reproduction:'simulation', target,
            artifacts:receipt, metadata:{host, input:session.input?.delivery('key') ?? 'unavailable', clock:session.controlled ? 'controlled' : 'live', uiState:'omitted', externalResults:'unsupported', scene:state.world?.resources?.SceneIdentity?.digest ?? 'unavailable'},
            script:String(script).slice(0,16384), operations:session.captureOperations?.() ?? {unavailable:true}, observedFailure:String(failure).slice(0,16384),
            capture:reply.capture, data:reply.data, expected:{tick:reply.capture?.lastReliableTick, hash:reply.capture?.hash},
            logs:JSON.stringify(logs).slice(0,16384), inventory:['world checkpoint','normalized input and live bindings','artifact digests and relative artifact names','bounded logs','author supplied script description and failure; never executed']};
          if (typeof bundle.data !== 'string' || bundle.data.length > 17 * 1024 * 1024) throw new Error('capture payload missing or over limit');
          writeFileSync(resolve(path), JSON.stringify(bundle, null, 2) + '\n');
          return {path:resolve(path), ...reply.capture};
        },
      };
    },
    async replay(path, {through} = {}) {
      if (statSync(path).size > 20 * 1024 * 1024) throw new Error('capture file exceeds 20 MiB');
      const bundle = JSON.parse(readFileSync(path, 'utf8')), receipt = current();
      if (bundle.format !== 'exact-game-capture-1' || bundle.scope !== 'world-only' || bundle.reproduction !== 'simulation') throw new Error('unsupported capture bundle');
      if (bundle.artifacts?.digest !== receipt.digest || !equal(bundle.artifacts.files, receipt.files)) throw new Error('capture build differs from actual local artifact inventory; exact replay refused');
      if (bundle.capture?.complete !== true || bundle.capture?.incomplete) throw new Error('capture incomplete; replay refused');
      if (typeof bundle.data !== 'string' || !/^[0-9a-f]+$/.test(bundle.data) || bundle.data.length > 17 * 1024 * 1024) throw new Error('invalid capture payload');
      if (typeof bundle.target !== 'string' || bundle.target.length > 128) throw new Error('invalid capture world target');
      if (through !== undefined && (!Number.isSafeInteger(through) || through < 0 || through > bundle.capture.records)) throw new Error('invalid seek record boundary');
      const scratch = mkdtempSync(resolve(tmpdir(), 'exact-game-replay-'));
      let session;
      try {
        session = await open({env:{EXACT_SURFACE_STORE:scratch, XDG_DATA_HOME:scratch, XDG_CACHE_HOME:scratch}});
        if (session.loadedArtifact !== receipt.digest || current().digest !== receipt.digest) throw new Error('replay artifacts changed while opening isolated session');
        const reply = await session.world(bundle.target).capture('replay', {build:receipt.digest, data:bundle.data, ...(through !== undefined ? {through} : {})});
        if (through === undefined && (reply.replay?.world?.hash !== bundle.expected?.hash || reply.replay?.world?.tick !== bundle.expected?.tick)) throw new Error('replay final observation differs from capture');
        return {...reply, storage:'isolated scratch', metadata:bundle.metadata};
      } finally { try { await session?.close(); } finally { rmSync(scratch, {recursive:true, force:true}); } }
    },
  };
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

/** Publish only complete, three-mode-equal candidates; tests never edit source pins. */
export function agreePins(directories) {
  if (!Array.isArray(directories) || directories.length !== 3)
    throw new Error('repin refused: exactly three candidate directories required');
  const expected = ['pins.json', 'tick-0.sim', 'tick-180.sim', 'tick-60.sim'];
  const rows = directories.map(dir => {
    const files = readdirSync(dir).sort();
    if (!equal(files, expected)) throw new Error('repin refused: incomplete or unexpected candidate inventory');
    return files.map(file => {
      const path = resolve(dir,file);
      if (statSync(path).size > 16*1024*1024) throw new Error('repin refused: candidate exceeds 16 MiB');
      return readFileSync(path);
    });
  });
  if (rows.slice(1).some(row => row.some((bytes,i) => !bytes.equals(rows[0][i]))))
    throw new Error('repin refused: continuous / Save / FreshGame candidates differ');
  const pins = JSON.parse(rows[0][0]);
  if (!Array.isArray(pins) || pins.length !== 3 || pins.some(p => !/^0x[0-9a-f]{16}$/.test(p)))
    throw new Error('repin refused: expected three tick hashes');
  return rows[0][0];
}

export async function repin(name) {
  // Lanterns is the measured authoring consumer. Other games keep their independent
  // engine-fixture contracts; never pretend an unsupported game was repinned.
  if (name !== 'lanterns') throw new Error(`repin: no current-game pin producer for ${name}`);
  const root = fileURLToPath(new URL('..', import.meta.url));
  const app = resolve(root, 'game/games', name);
  const scratch = mkdtempSync(resolve(app, 'artifacts/repin-'));
  const run = async (command, args, env = {}) => {
    const child = spawn(command,args,{cwd:root, env:{...process.env, EXACT_UPDATE_TRUST:'development',...env},stdio:'inherit'});
    const code = await new Promise((ok,reject) => {child.on('exit',ok);child.on('error',reject);});
    if (code !== 0) throw new Error(`repin refused: ${command} exited ${code}; committed pins unchanged`);
  };
  try {
    const dirs = [];
    for (const mode of ['0','1','fresh-game']) {
      const dir = resolve(scratch,mode); mkdirSync(dir); dirs.push(dir);
      await run('cargo',['test','--manifest-path','game/Cargo.toml','-p','lanterns-logic','--no-fail-fast',
        ...(mode === '0' ? [] : ['--test','timing','--test','difficult_moment','--test','capture'])],
        {EXACT_GAME_PARANOID:mode, EXACT_REPIN_OUT:dir});
    }
    const candidate = agreePins(dirs);
    await run(process.execPath,[resolve(app,'proof.mjs'),'linux','--paranoid']);
    const path = resolve(app,'pins.json'), old = JSON.parse(readFileSync(path,'utf8')), next = JSON.parse(candidate);
    const replacement = resolve(scratch, "agreed-pins.json");
    writeFileSync(replacement,candidate);
    renameSync(replacement,path);
    console.log('REPIN lanterns: continuous / Save / FreshGame bytes agree; historical engine evidence unchanged');
    [0,60,180].forEach((tick,i) => console.log(`  tick ${tick}: ${old[i]} → ${next[i]}`));
  } finally { rmSync(scratch,{recursive:true,force:true}); }
}

export async function proof(meta, script) {
  const app = fileURLToPath(new URL('.', meta.url)), name = basename(app);
  if (process.argv.includes('--repin')) { mkdirSync(resolve(app,'artifacts'),{recursive:true}); await repin(name); return; }
  const root = fileURLToPath(new URL('..', import.meta.url));
  const host = process.argv[2] ?? 'web', out = resolve(app, 'artifacts');
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
  let loadedIdentity = () => null;
  const open = async (options = {}) => {
    const launchReceipt = options.url ? null : loadedIdentity();
    const raw = await openSession({host, app:name, size:[1280,720], webDist:dist, ...options});
    sample();
    if (launchReceipt && loadedIdentity()?.digest !== launchReceipt.digest) { await raw.close(); throw new Error('artifacts changed during proof session launch'); }
    raw.loadedArtifact = launchReceipt?.digest ?? null;
    let closed = false;
    const close = async () => { if (!closed) {
      try {
        if (['beacons', 'greybox', 'lanterns', 'asset-fixture'].includes(name)) {
          const state = await raw.state('world', {world:true});
          const {ready, gpu} = state.world ?? {};
          const paranoid = ['1', 'fresh-game'].includes(process.env.EXACT_GAME_PARANOID);
          const restoreEvents = paranoid ? (gpu?.restoreUploads?.events ?? 0) : 0;
          if (paranoid && restoreEvents) say(`REPORT restore-caused GPU uploads: ${JSON.stringify(gpu.restoreUploads)}`);
          check('ready: no GPU allocations or asset uploads after ready',
            ready === true && gpu?.afterReady?.violations === restoreEvents,
            {ready, gpu, lastAfterReady:gpu?.lastAfterReady});
        }
        if (compareParanoid) {
          const state = await raw.state('world', {world:true});
          const world = state.world;
          const logs = await raw.op({op:'logs', ...await raw.target('world'), world:true, since:0});
          finalWorlds.push({session:id, tick:world?.tick, hash:world?.hash,
            published:world?.published,
            // Native hosts own incremental journal cursors, even for since:0.
            // Include the chunks this script already read as well as the tail.
            journal:[...replies.filter(r => r.session === id && r.method === 'logs')
              .flatMap(r => r.reply?.world ?? []), logs]
              .map(({from, next, lines, tick}) => ({from, next, lines, tick}))});
          if (!world?.hash) throw new Error('paranoid comparison: final world hash missing');
        }
        sample();
      } finally { await raw.close(); closed = true; }
    } };
    sessions.add({close});
    const id = sessions.size;
    raw.captureOperations = () => {
      const steps = replies.filter(r => r.session === id && ['tap','type','clock'].includes(r.method)).slice(-1024);
      const json = JSON.stringify(steps);
      return json.length <= 65536 ? steps : {unavailable:'script transcript exceeds 64 KiB; normalized engine records remain authoritative'};
    };
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
          && file !== 'game/README.md' && !file.startsWith(`${appPrefix}art/`) && !file.startsWith(`${appPrefix}assets/`) && !file.startsWith(`${appPrefix}deck/`))
        || /(^|\/)(proof\.mjs|.*\.test\.mjs)$/.test(file)
        || !existsSync(resolve(root,file))) continue;
      hash.update(file).update(readFileSync(resolve(root,file)));
    }
    const digest = hash.digest('hex'), receipt = resolve(out, `build-${host}.sha256`);
    const appInfo = resolveApp(name);
    const linuxTarget = host === 'linux' ? spawnSync('rustc', ['-vV'], {encoding:'utf8'}).stdout.match(/^host: (.+)$/m)?.[1] : null;
    const artifacts = host === 'linux' ? {binary:resolve(appInfo.target, linuxTarget, `release/${appInfo.crate('linux')}`), module:resolve(appInfo.target, linuxTarget, `release/lib${appInfo.crate('gpu').replaceAll('-','_')}.${process.platform === 'darwin' ? 'dylib' : 'so'}`)} : host === 'web' ? null : appleArtifacts(appInfo, {destination:host === 'macos' ? 'macos' : 'ios-simulator'});
    if (host === 'linux') process.env.EXACT_LINUX_BIN = artifacts.binary;
    let artifact = artifactDigest(host, dist, artifacts);
    const stamp = () => JSON.stringify({inputs:digest, artifact});
    if (!artifact || !existsSync(receipt) || readFileSync(receipt,'utf8') !== stamp()) {
      say(`BUILD ${name} ${host}`);
      const disk = spawnSync('df', ['-Pk', root], {encoding:'utf8'});
      say(disk.stdout.trim());
      if (disk.status !== 0) throw new Error('disk check failed before build');
      const availableKiB = Number(disk.stdout.trim().split('\n').at(-1).trim().split(/\s+/)[3]);
      if (!Number.isFinite(availableKiB) || availableKiB < 25 * 1024 * 1024) throw new Error('build refused: less than 25 GiB available');
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
    loadedIdentity = () => artifactInventory(host, dist, artifacts);
    await script({open, check, equal, out, host, say, ...captureTools({open, identity:loadedIdentity, host})});
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
    if (compareParanoid) {
      finalWorlds.sort((a,b) => a.session - b.session);
      const baseline = resolve(out, `paranoid-${host}-normal.json`);
      writeFileSync(resolve(out, `paranoid-${host}-${process.env.EXACT_GAME_PARANOID}.json`), JSON.stringify(finalWorlds));
      if (process.env.EXACT_GAME_PARANOID === '0') writeFileSync(baseline, JSON.stringify(finalWorlds));
      else check('paranoid final hash, tick, published record and journal equal normal',
        equal(finalWorlds, JSON.parse(readFileSync(baseline, 'utf8'))),
        finalWorlds.map(({session, tick, hash}) => ({session, tick, hash})));
    }
    writeFileSync(resolve(out,'process-cleanup.json'),JSON.stringify({recorded:[...recorded],remaining},null,2)+'\n');
    say(`PROOF ${failures.length ? 'FAIL' : 'PASS'} ${name} ${host}: ${failures.length} failures; ${((performance.now()-started)/1000).toFixed(3)} s`);
    writeFileSync(resolve(out,'proof.txt'),transcript.join('\n')+'\n');
    writeFileSync(resolve(out,'replies.json'),JSON.stringify(replies,null,2)+'\n');
  }
  process.exit(failures.length ? 1 : 0);
}

if (import.meta.main) {
  const name = process.argv[2];
  if (!/^[a-z][a-z0-9-]*$/.test(name ?? '')) throw new Error('usage: bun game/proof.mjs <game> --repin | --paranoid');
  if (process.argv.includes('--repin')) {
    mkdirSync(resolve(import.meta.dirname,'games',name,'artifacts'),{recursive:true});
    await repin(name);
  } else {
    const child = spawn(process.execPath,[resolve(import.meta.dirname,'games',name,'proof.mjs'),'linux',...process.argv.slice(3)],{stdio:'inherit'});
    process.exit(await new Promise((ok,reject) => {child.on('exit',ok);child.on('error',reject);}));
  }
}
