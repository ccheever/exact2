import {mkdirSync, readFileSync, writeFileSync, renameSync, rmSync, statSync} from 'node:fs';
import {resolve} from 'node:path';
import {randomUUID} from 'node:crypto';

const alive = pid => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
export function readStatus(app, now = Date.now()) {
  let rows; try { rows = JSON.parse(readFileSync(resolve(app, '.exact/status.json'), 'utf8')); } catch { return []; }
  return rows.map(row => row.state === 'running' ? {...row, elapsedMs:now - row.startedAt, state:alive(row.pid) ? 'running' : 'unknown'} : row);
}
function update(app, row) {
  const dir = resolve(app, '.exact'), lock = resolve(dir, 'status.lock');
  mkdirSync(dir, {recursive:true});
  const deadline = Date.now() + 2000;
  for (;;) {
    try { mkdirSync(lock); writeFileSync(resolve(lock,'owner'), String(process.pid)); break; } catch (e) {
      if (e.code !== 'EEXIST') throw e;
      try {
        const age = Date.now() - statSync(lock).mtimeMs;
        let owner; try { owner = Number(readFileSync(resolve(lock,'owner'),'utf8')); } catch {}
        if ((owner && !alive(owner)) || age > 2000) { rmSync(lock, {recursive:true, force:true}); continue; }
      } catch (error) { if (error.code === 'ENOENT') continue; throw error; }
      if (Date.now() >= deadline) throw e;
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5);
    }
  }
  try {
    const rows = readStatus(app).filter(r => r.invocation !== row.invocation);
    rows.push(row);
    const temp = resolve(dir, `status.${process.pid}.tmp`);
    writeFileSync(temp, JSON.stringify(rows, null, 2) + '\n');
    renameSync(temp, resolve(dir, 'status.json'));
  } finally { rmSync(lock, {recursive:true}); }
}
export function startStatus(app, verb, extra = {}) {
  const previous = readStatus(app).filter(r => r.verb === verb && r.lastDurationMs != null).at(-1);
  let row = {verb, invocation:randomUUID(), pid:process.pid, phase:'starting', startedAt:Date.now(), elapsedMs:0, lastDurationMs:previous?.lastDurationMs ?? null, state:'running', exitCode:null, ...extra};
  const set = patch => { row = {...row, elapsedMs:Date.now() - row.startedAt, ...patch}; update(app, row); return row; };
  set({});
  process.once('exit', code => set({state:code ? 'failed' : 'exited', exitCode:code}));
  return {set, get:() => ({...row, elapsedMs:Date.now() - row.startedAt})};
}
export function formatStatus(rows) {
  return rows.map(r => `${r.verb} ${r.invocation} pid=${r.pid} ${r.state} ${r.phase} elapsed=${r.elapsedMs}ms last=${r.lastDurationMs ?? '?'}ms exit=${r.exitCode ?? '-'}${r.url ? ` ${r.url}` : ''}`).join('\n') || 'No web or prove invocations recorded.';
}
export async function waitReady(app, {timeout = 600, invocation} = {}) {
  if (!Number.isFinite(timeout) || timeout <= 0) throw new Error('--timeout needs positive seconds');
  const deadline = Date.now() + timeout * 1000;
  while (Date.now() < deadline) {
    const row = readStatus(app).filter(r => r.verb === 'web' && (invocation ? r.invocation === invocation : r.state === 'running')).at(-1);
    if (row) {
      invocation ??= row.invocation;
      if (row.state !== 'running') throw new Error(`web ${invocation}: ${row.state} (${row.phase}); run web to start a server`);
      if (row.url) {
        try {
          const response = await fetch(new URL('__exact/ready', row.url), {signal:AbortSignal.timeout(1000)});
          const status = await response.json();
          if (response.ok && status.invocation === invocation && status.phase === 'ready') return row;
          if (status.phase === 'failed') throw new Error('web build failed; inspect status and compiler output');
        } catch (e) { if (e.message.startsWith('web build failed')) throw e; }
      }
    }
    await new Promise(ok => setTimeout(ok, 50));
  }
  throw new Error(`web --ready timed out after ${timeout}s; run status for the current phase`);
}
