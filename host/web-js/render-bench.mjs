// Per-request rendering, the two renderers side by side (LLP 1071, 1048.002):
// a server is started cold, asked for its first page (cold start), then
// driven at a fixed concurrency with `Cache-Control: no-store` (every
// request renders); it reports render latency p50/p95, CPU time per page
// (the server process's user+system time over the pages it rendered),
// resident memory after warm-up and at the end, and throughput.
//
//   bun host/web-js/render-bench.mjs --name <label> --path <location> [--requests 200] [--concurrency 8] -- <server command…>
// The server command must print nothing required and listen on --port (given as {port}).
import { spawn, spawnSync } from 'node:child_process';

const args = process.argv.slice(2);
const sep = args.indexOf('--');
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 || i > sep ? d : args[i + 1]; };
const name = opt('--name', 'server'), path = opt('--path', '/');
const requests = Number(opt('--requests', 200)), concurrency = Number(opt('--concurrency', 8));
const port = 8900 + Math.floor(Math.random() * 90);
const command = args.slice(sep + 1).map(a => a.replace('{port}', String(port)));
const sleep = ms => new Promise(r => setTimeout(r, ms));
const cpuOf = pid => { const r = spawnSync('ps', ['-o', 'cputime=,rss=', '-p', String(pid)], { encoding: 'utf8' }).stdout.trim().split(/\s+/); const [m, s] = r[0].split(':'); return { cpu: Number(m) * 60 + Number(s), rss: Number(r[1]) * 1024 }; };
const t0 = performance.now();
const child = spawn(command[0], command.slice(1), { stdio: ['ignore', 'ignore', 'inherit'] });
const url = `http://127.0.0.1:${port}${path}`;
const get = async () => { const t = performance.now(); const r = await fetch(url, { headers: { 'cache-control': 'no-store', 'accept-encoding': 'identity' } }); const body = await r.text(); return { ms: performance.now() - t, status: r.status, bytes: body.length }; };
let first;
for (;;) { try { first = await get(); break; } catch { if (performance.now() - t0 > 60000) throw new Error('the server never answered'); await sleep(5); } }
const cold = performance.now() - t0;
for (let i = 0; i < 5; i++) await get();
const warm = cpuOf(child.pid);
const times = [];
let next = 0;
const start = performance.now();
await Promise.all(Array.from({ length: concurrency }, async () => { while (next++ < requests) times.push((await get()).ms); }));
const elapsed = (performance.now() - start) / 1000;
const end = cpuOf(child.pid);
child.kill('SIGKILL');
times.sort((a, b) => a - b);
const q = p => times[Math.min(times.length - 1, Math.floor(times.length * p))];
const out = { name, path, status: first.status, bytes: first.bytes, coldStartMs: Math.round(cold), firstRenderMs: Math.round(first.ms), p50: Math.round(q(0.5)), p95: Math.round(q(0.95)), cpuMsPerPage: +((end.cpu - warm.cpu) * 1000 / requests).toFixed(2), rssWarmMB: +(warm.rss / 2 ** 20).toFixed(1), rssEndMB: +(end.rss / 2 ** 20).toFixed(1), pagesPerSec: +(requests / elapsed).toFixed(1), concurrency };
console.log(JSON.stringify(out));
