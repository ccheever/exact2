#!/usr/bin/env bun
import { resolve } from 'node:path';
import { existsSync } from 'node:fs';
import { createServer } from 'node:net';
import { spawn } from 'node:child_process';

const free = (port, host) => new Promise(ok => {
  const probe = createServer().once('error', () => ok(false)).listen(port, host, () => probe.close(() => ok(true)));
});

/** The dev server's port, decided before the first (minute-long) web build:
 * an explicit `--port` must be free; otherwise the first free one from 8765,
 * so a sibling's dev loop on 8765 is not fatal. Returns the arguments to forward. */
export async function devPort(args, {start = 8765, tries = 100} = {}) {
  args = args.flatMap(a => a.startsWith('--port=') ? ['--port', a.slice(7)] : [a]);
  const at = args.indexOf('--port'), host = args.includes('--lan') ? '0.0.0.0' : '127.0.0.1';
  if (at >= 0) {
    const port = Number(args[at + 1]);
    if (!Number.isInteger(port) || port <= 0 || port > 65535) throw new Error(`--port needs a port number, not ${JSON.stringify(args[at + 1] ?? '')}`);
    if (!await free(port, host)) throw new Error(`--port ${port}: ${host}:${port} is in use; choose another --port, or omit --port to take the next free one`);
    return {port, args};
  }
  for (let port = start; port < start + tries; port++) {
    if (await free(port, host)) return {port, args:[...args, '--port', String(port)]};
  }
  throw new Error(`no free port in ${start}–${start + tries - 1} on ${host}; pass --port <n>`);
}

if (import.meta.main) {
  const name = process.argv[2];
  const local = name === '.' || name?.includes('/');
  if (!name || (!local && !/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name))) throw new Error('Usage: bun game/dev.mjs <name|path> [--port <n>] [dev options]');
  const app = local ? resolve(name) : resolve(import.meta.dir, 'games', name);
  if (!existsSync(resolve(app, 'app.contract'))) throw new Error(`No game: ${app}`);
  let chosen, start = 8765;
  const explicit = process.argv.some(a => a === '--port' || a.startsWith('--port='));
  // The port is probed before the minute-long build; another loop can take it
  // meanwhile. A chosen (not explicit) port that is lost at listen moves on.
  for (let attempt = 0; ; attempt++) {
    try { chosen = await devPort(process.argv.slice(3), {start}); } catch (error) { console.error(error.message); process.exit(1); }
    if (chosen.port !== 8765 && !explicit) console.log(`port${chosen.port > 8766 ? `s 8765–${chosen.port - 1} are` : ' 8765 is'} in use; serving on ${chosen.port} (--port chooses one)`);
    const child = spawn(process.execPath, [resolve(import.meta.dir, '../host/web/dev.mjs'), ...chosen.args], {
      env:{...process.env, EXACT_APP_DIR:app, EXACT_UPDATE_TRUST:'development', EXACT_WEB_DIST:resolve(app, 'dist')},
      stdio:['inherit', 'inherit', 'pipe'],
    });
    let taken = false;
    child.stderr.on('data', bytes => { process.stderr.write(bytes); taken ||= /cannot listen on .*EADDRINUSE/.test(String(bytes)); });
    for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => child.kill(signal));
    const code = await new Promise(ok => child.on('exit', (code, signal) => ok(code ?? (signal ? 1 : 0))));
    if (!(taken && !explicit && attempt < 5)) process.exit(code);
    console.log(`port ${chosen.port} was taken during the build; trying the next free port`);
    start = chosen.port + 1;
  }
}
