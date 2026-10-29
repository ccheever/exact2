// A native client's first join while the dev loop's resident producers
// start (dtool2's pass 3 #8; theme2's plan approval): its envelope request
// is answered at once, a 503 `application/vnd.exact.dev-status+json` with
// the step the resident is on, not held for the minutes a first build takes;
// once the resident is up it is served as before. A start that fails ends
// the 503s: every request has its error, and none starts it again until an
// edit. A stand-in resident (EXACT_DEV_RESIDENT_SCRIPT) builds nothing.
import { afterEach, expect, test } from 'bun:test';
import { spawnOwned, stopAll } from '../../scripts/test-children.mjs';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
const ENVELOPE = 'application/vnd.exact.envelope+json, text/html;q=0.9';
const PHASE_MODULE = resolve(ROOT, 'host/web/dev-phase.mjs');
afterEach(() => stopAll());

// The stand-in: says a phase, then (STUB_FAIL=build) fails as the
// resident's producer build does (dev-phase.mjs's phased, then a throw), or
// (STUB_FAIL=alive) says a step failed and stays, as a resident whose first
// plan was refused does; else waits for STUB_GO and serves an envelope.
const STUB = `
import { createServer } from 'node:http';
import { existsSync } from 'node:fs';
const { phased, failedPhase } = await import(process.env.STUB_PHASE);
const port = Number(process.argv[process.argv.indexOf('--port') + 1]);
console.log('phase: building the module producer (a stand-in)');
if (process.env.STUB_FAIL === 'build') {
  const built = phased('the module producer (a stand-in)', process.execPath, ['-e', "console.error('error: could not compile \\\`exact-js-bake\\\` (a stand-in)'); process.exit(101)"], {});
  if (built.status !== 0) throw new Error('the module producer did not build');
} else if (process.env.STUB_FAIL === 'alive') {
  setTimeout(() => failedPhase('error: generation refused: a stand-in contract error'), 1500);
  setInterval(() => {}, 1000);
} else {
  const wait = setInterval(() => {
    if (!existsSync(process.env.STUB_GO)) return;
    clearInterval(wait);
    createServer((req, res) => { res.writeHead(200, { 'content-type': 'application/vnd.exact.envelope+json' }); res.end('{"exact":1,"stub":true}'); })
      .listen(port, '127.0.0.1', () => console.log('plan ready: 1 bytes (a stand-in)'));
  }, 50);
}
`;

async function devLoop(dir, env) {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-native-start-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  dev.stdout.on('data', d => { out += d; });
  dev.stderr.on('data', d => { out += d; });
  const stop = async () => { await dev.stop(30_000); rmSync(dist, { recursive: true, force: true }); };
  const until = async (pass, ms, what) => { const end = Date.now() + ms; while (!(await pass())) { if (Date.now() > end) throw new Error(`${what}: not seen in ${out.slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
  await until(() => out.includes('plan ready') && out.includes('design reader ready'), 900_000, 'the JS loop up');
  return { base: `http://127.0.0.1:${port}/`, stop, said: () => out, until };
}

function app(name) {
  const dir = mkdtempSync(join(tmpdir(), `exact-${name}-`));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name, app: { id: `com.exact.${name.toLowerCase()}`, name } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "native start"\n');
  writeFileSync(join(dir, 'stub-resident.mjs'), STUB);
  return dir;
}

const discover = async (base) => {
  const started = Date.now();
  const res = await fetch(base, { headers: { accept: ENVELOPE } });
  return { status: res.status, type: res.headers.get('content-type') ?? '', retry: res.headers.get('retry-after'), body: await res.text(), ms: Date.now() - started };
};

test('a native join while the resident starts: a 503 with its phase at once, then the envelope', async () => {
  const dir = app('NativeStart'), go = join(dir, 'go');
  let loop = null;
  try {
    loop = await devLoop(dir, { EXACT_DEV_RESIDENT_SCRIPT: join(dir, 'stub-resident.mjs'), STUB_PHASE: PHASE_MODULE, STUB_GO: go });
    const first = await discover(loop.base);
    expect(first.status).toBe(503);
    expect(first.ms).toBeLessThan(2000);
    expect(first.type).toContain('application/vnd.exact.dev-status+json');
    expect(first.retry).toBe('2');
    expect(JSON.parse(first.body)).toMatchObject({ exact: 1, status: 'preparing' });
    // The resident's phase line, once it has said one.
    await loop.until(async () => JSON.parse((await discover(loop.base)).body).message === 'building the module producer (a stand-in)', 30_000, 'the phase');
    expect(loop.said()).toContain('[resident] phase: building the module producer (a stand-in)');
    writeFileSync(go, '');
    await loop.until(async () => (await discover(loop.base)).status === 200, 30_000, 'the envelope');
    expect(JSON.parse((await discover(loop.base)).body)).toEqual({ exact: 1, stub: true });
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test('a resident start that fails ends the 503s: its error to every request, no restart until an edit', async () => {
  const dir = app('NativeFail');
  let loop = null;
  try {
    loop = await devLoop(dir, { EXACT_DEV_RESIDENT_SCRIPT: join(dir, 'stub-resident.mjs'), STUB_PHASE: PHASE_MODULE, STUB_FAIL: 'build' });
    expect((await discover(loop.base)).status).toBe(503);
    await loop.until(async () => (await discover(loop.base)).status === 500, 30_000, 'the failure');
    const failed = await discover(loop.base);
    expect(failed.type).toContain('application/vnd.exact.dev-status+json');
    const body = JSON.parse(failed.body);
    expect(body).toMatchObject({ exact: 1, status: 'failed' });
    // The compiler's own diagnostic, plain: not the throw's trace after it.
    expect(body.message).toContain('error: could not compile `exact-js-bake` (a stand-in)');
    expect(body.message).not.toMatch(/\bat \/|\x1b\[|Bun v/);
    // Asked again: the same error, and the resident not started again.
    const starts = () => (loop.said().match(/starting the resident loop's producers/g) ?? []).length;
    expect(starts()).toBe(1);
    await new Promise(ok => setTimeout(ok, 1000));
    expect((await discover(loop.base)).status).toBe(500);
    expect(starts()).toBe(1);
    // An edit: the next request starts it again.
    writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "native start, edited"\n');
    await loop.until(async () => (await discover(loop.base)).status === 503, 30_000, 'a start after the edit');
    await loop.until(() => starts() === 2, 30_000, 'the resident started again');
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);

test('a step the resident says failed before its first plan ends the 503s too; it stays, and an edit has it prepare again', async () => {
  const dir = app('NativeRefused');
  let loop = null;
  try {
    loop = await devLoop(dir, { EXACT_DEV_RESIDENT_SCRIPT: join(dir, 'stub-resident.mjs'), STUB_PHASE: PHASE_MODULE, STUB_FAIL: 'alive' });
    // A request that waits on the resident (not discovery) before its step fails: answered with the failure.
    const waiting = fetch(`${loop.base}__dev/open`).then(async r => ({ status: r.status, type: r.headers.get('content-type') ?? '', body: await r.text() }));
    const waited = await waiting;
    expect(waited.status).toBe(500);
    expect(waited.type).toContain('application/vnd.exact.dev-status+json');
    expect(JSON.parse(waited.body)).toMatchObject({ status: 'failed', message: 'error: generation refused: a stand-in contract error' });
    await loop.until(async () => (await discover(loop.base)).status === 500, 30_000, 'the failed step');
    expect(JSON.parse((await discover(loop.base)).body)).toMatchObject({ status: 'failed', message: 'error: generation refused: a stand-in contract error' });
    writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "native start, fixed"\n');
    await loop.until(async () => (await discover(loop.base)).status === 503, 30_000, 'preparing again');
    expect((loop.said().match(/starting the resident loop's producers/g) ?? []).length).toBe(1);
  } finally {
    await loop?.stop().catch(() => {});
    rmSync(dir, { recursive: true, force: true });
  }
}, 1_200_000);
