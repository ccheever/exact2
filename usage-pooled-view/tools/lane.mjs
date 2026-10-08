#!/usr/bin/env bun
// usage-pooled-view lane (uncommitted, target/upv/lane): two isolated T3 servers (the staged release)
// on 16410 and 16420, each behind fixture.mjs on 16411 / 16421. Never 3773, never ~/.t3.
//   bun lane.mjs start | stop | pair <a|b> | status
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, openSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const lane = resolve(import.meta.dir);
const checkout = resolve(lane, '../../..');
const example = join(checkout, 'examples/t3-code');
const SERVERS = { a: { port: 16410, proxy: 16411, label: 'Studio' }, b: { port: 16420, proxy: 16421, label: 'Build box' } };
const pin = JSON.parse(readFileSync(join(example, 'server-runtime/runtime-pin.json'), 'utf8'));
const runtime = join(lane, 'runtime', pin.version), t3 = join(runtime, 't3');
if (!existsSync(t3)) {
  mkdirSync(runtime, { recursive: true });
  const archive = join(example, 'server-runtime', pin.asset);
  if (spawnSync('/usr/bin/tar', ['-xzf', archive, '-C', runtime, '--strip-components=1']).status !== 0) throw new Error('could not unpack the staged server');
}
const dir = (name) => join(lane, name);
const env = (name) => {
  const s = dir(name);
  for (const sub of ['home', 't3home', 'codex', 'claude', 'xdg/config', 'xdg/data', 'xdg/cache', 'xdg/state', 'tmp', 'repos/demo']) mkdirSync(join(s, sub), { recursive: true });
  return { PATH: '/usr/bin:/bin:/usr/sbin:/sbin', HOME: join(s, 'home'), SHELL: '/bin/zsh', USER: process.env.USER ?? '', LANG: 'en_US.UTF-8', TMPDIR: join(s, 'tmp'),
    CODEX_HOME: join(s, 'codex'), CLAUDE_CONFIG_DIR: join(s, 'claude'), XDG_CONFIG_HOME: join(s, 'xdg/config'), XDG_DATA_HOME: join(s, 'xdg/data'),
    XDG_CACHE_HOME: join(s, 'xdg/cache'), XDG_STATE_HOME: join(s, 'xdg/state'), T3CODE_HOME: join(s, 't3home'), T3CODE_TELEMETRY_ENABLED: 'false' };
};
const pids = join(lane, 'pids.json');
const recorded = () => (existsSync(pids) ? JSON.parse(readFileSync(pids, 'utf8')) : {});
const alive = (pid) => { if (!pid) return false; try { process.kill(pid, 0); return true; } catch { return false; } };

async function waitReady(port) {
  for (let i = 0; i < 300; i++) {
    try { const r = await fetch(`http://127.0.0.1:${port}/.well-known/t3/environment`, { signal: AbortSignal.timeout(1000) }); if (r.ok) return r.json(); } catch {}
    await Bun.sleep(200);
  }
  throw new Error(`no answer on ${port}`);
}
const [command, arg] = process.argv.slice(2);
if (command === 'start') {
  const started = recorded();
  for (const [name, server] of Object.entries(SERVERS)) {
    const serverEnv = env(name);
    if (!started[name] || !alive(started[name].server)) {
      const log = openSync(join(dir(name), 'server.log'), 'a');
      const child = spawn(t3, ['serve', '--base-dir', join(dir(name), 't3home'), '--port', String(server.port), '--host', '127.0.0.1', join(dir(name), 'repos')],
        { env: serverEnv, cwd: join(dir(name), 'repos'), stdio: ['ignore', log, log], detached: true });
      child.unref();
      started[name] = { ...(started[name] ?? {}), server: child.pid };
    }
    if (!started[name].proxy || !alive(started[name].proxy)) {
      const log = openSync(join(dir(name), 'proxy.log'), 'a');
      const child = spawn(process.execPath, [join(lane, 'fixture.mjs'), name, String(server.proxy), String(server.port), server.label, join(dir(name), 'rpc.log')],
        { env: { ...process.env, UPV_SCENARIO_FILE: join(lane, 'scenario.json') }, stdio: ['ignore', log, log], detached: true });
      child.unref();
      started[name].proxy = child.pid;
    }
  }
  writeFileSync(pids, JSON.stringify(started, null, 2));
  for (const server of Object.values(SERVERS)) { await waitReady(server.port); await waitReady(server.proxy); }
  console.log(JSON.stringify(recorded()));
} else if (command === 'stop') {
  const started = recorded();
  for (const name of arg ? [arg] : Object.keys(started)) {
    for (const role of ['proxy', 'server']) if (started[name]?.[role] && alive(started[name][role])) process.kill(started[name][role], 'SIGTERM');
    delete started[name];
  }
  writeFileSync(pids, JSON.stringify(started, null, 2));
} else if (command === 'stop-server') {
  // Only the T3 server behind one proxy (a stopped environment mid-refresh); the proxy stays.
  const started = recorded();
  if (started[arg]?.server && alive(started[arg].server)) process.kill(started[arg].server, 'SIGTERM');
  started[arg].server = 0;
  writeFileSync(pids, JSON.stringify(started, null, 2));
} else if (command === 'pair') {
  const server = SERVERS[arg];
  const out = spawnSync(t3, ['pair', '--base-dir', join(dir(arg), 't3home'), '--ttl', '15m', '--label', 'Usage lane'], { env: env(arg), cwd: join(dir(arg), 'repos'), encoding: 'utf8' });
  const token = /Token: (\S+)/.exec(out.stdout)?.[1];
  if (!token) throw new Error(`t3 pair printed no token: ${out.stderr.slice(0, 300)}`);
  const file = join(dir(arg), 'pairing-url');
  writeFileSync(file, `http://127.0.0.1:${server.proxy}/pair#token=${token}\n`, { mode: 0o600 });
  console.log(`pairing URL written to ${file}`);
} else if (command === 'status') {
  console.log(JSON.stringify(Object.fromEntries(Object.entries(recorded()).map(([name, roles]) => [name, Object.fromEntries(Object.entries(roles).map(([role, pid]) => [role, pid && alive(pid) ? pid : 0]))]))));
} else {
  console.error('bun lane.mjs start | stop [a|b] | stop-server <a|b> | pair <a|b> | status');
  process.exit(2);
}
