// The native-module seam, driven (LLP 1024 D8.3) — `smoke.mjs` runs this for
// the fixture app: `bun scripts/smoke.mjs <web|macos|ios> --app native-fixture`.
// Every assertion is through the eight operations: `tree` shows the status
// object through loading → ready; props replace (several reactive keys,
// clearing, canonical order, escaping); all nine events arrive with their
// payloads; a callback after `destroy` is dropped; loading starts only after
// first pixel; a plan reload does not re-define the element (web); capture
// shows the boxes and the tokened snapshot answers (Apple); and the failure
// family — missing artifact, missing factory, wrong ABI, refused props —
// each yields its named status, an empty box, a log line and a running app.
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { serveStatic } from '../host/web/serve.mjs';
import { decodePng } from './png.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const EXPECTED_EVENTS = 'press;change:changed;hover:true;focus;blur;key:Enter;submit;message:hello;';
const NOTE = 'quote " slash \\ tab\t<&>';

export async function nativeSmoke({ host, open, check: record, webDist, shots }) {
  let checks = 0, failed = 0;
  const check = (ok, what) => { checks += 1; if (!ok) failed += 1; return record(ok, what); };
  const t0 = Date.now();
  const byTestId = (t, id) => t.nodes.find((n) => n.props.testId === id);
  const module = (t, id) => byTestId(t, id)?.module;
  const tmp = mkdtempSync(resolve(tmpdir(), 'exact-native-'));
  const settle = async (s) => { await s.clock('+50'); await sleep(host === 'web' ? 50 : 120); await s.clock('+50'); };
  const until = async (s, what, test, ms = 5000) => {
    const end = Date.now() + ms;
    let last;
    while (Date.now() < end) { last = await s.tree(); if (test(last)) return last; await sleep(40); }
    check(false, `${host} native: ${what}: ${JSON.stringify(last?.nodes.filter((n) => n.type === 'NativeView').map((n) => n.module))}`);
    return last;
  };
  const received = (st) => { const r = st.slots.received; return r.startsWith('props:') ? JSON.parse(r.slice(6)) : null; };
  const pixel = async (s, path, testId) => {
    const shot = await s.screenshot(path);
    const png = decodePng(readFileSync(path));
    const box = (await s.layout()).nodes.find((n) => n.testId === testId);
    if (!box) return null;
    const scale = png.width / shot.w, x = Math.round((box.x + box.w / 2) * scale), y = Math.round((box.y + box.h / 2) * scale);
    const i = (y * png.width + x) * 4;
    return [png.data[i], png.data[i + 1], png.data[i + 2]];
  };
  const near = (rgb, hex) => rgb && [1, 3, 5].every((o, k) => Math.abs(rgb[k] - parseInt(hex.slice(o, o + 2), 16)) <= 12);

  // The standard artifact.
  const s = await open({ host });
  try {
    let t = await until(s, 'the fixture and the plain box reach ready', (t) => module(t, 'box')?.state === 'ready' && module(t, 'plain')?.state === 'ready');
    let logs = await s.logs();
    const lines = logs.lines.join('\n');
    check(/exact-fixture #\d+: loading[\s\S]*exact-fixture #\d+: ready/.test(lines), `${host} native: the log shows loading then ready: ${logs.lines.filter((l) => /native/.test(l)).join(' | ')}`);
    // Missing factory: in the roster, not in the artifact.
    check(module(t, 'absent')?.state === 'error' && /no factory for exact-absent/.test(module(t, 'absent')?.error ?? ''), `${host} native: exact-absent reports its missing factory: ${JSON.stringify(module(t, 'absent'))}`);
    check(/exact-absent #\d+: error: the module artifact has no factory/.test(lines), `${host} native: the missing factory is logged`);
    // After first pixel, never before: the browser's paint entry, or, when
    // headless Chrome records none, the glue's first-frame stamp (a lower bound).
    if (host === 'web') {
      const order = await s.carrier.evaluate(`(() => { const glueEnd = performance.getEntriesByType('resource').find((e) => e.name.endsWith('/glue.js'))?.responseEnd ?? 0; const paint = performance.getEntriesByType('paint')[0]?.startTime ?? (glueEnd + Number(document.getElementById('exact-root').dataset.frameCallbackMs ?? NaN)); const glue = performance.getEntriesByType('resource').find((e) => e.name.endsWith('/native-glue.js'))?.startTime; const table = performance.getEntriesByType('resource').find((e) => e.name.endsWith('/modules/index.js'))?.startTime; return { paint, glue, table }; })()`);
      check(Number.isFinite(order.paint) && order.glue > order.paint && order.table > order.glue, `${host} native: the adapter and the module load after first paint: ${JSON.stringify(order)}`);
    } else {
      const m = /native loading .*libexact_modules\.dylib (-?[\d.]+) ms after first pixel/.exec(lines);
      check(m && Number(m[1]) >= 0, `${host} native: the artifact loads after first pixel: ${m?.[0] ?? 'no load line'}`);
    }
    // Props: the canonical aggregate the plan carries, and what the module got.
    const canonical = `{"count":"0","emit":"0","note":${JSON.stringify(NOTE)},"reject":"false","tint":"#2266ee"}`;
    check(byTestId(t, 'box')?.props.nativeViewProps === canonical, `${host} native: the plan's aggregate is sorted and escaped: ${byTestId(t, 'box')?.props.nativeViewProps}`);
    let st = await s.state();
    check(JSON.stringify(received(st)) === canonical, `${host} native: the module received the aggregate: ${st.slots.received}`);
    check(st.slots.loads === 1, `${host} native: load fired once at create: ${st.slots.loads}`);
    await s.tap('bump'); await s.tap('mode'); await settle(s);
    st = await s.state();
    check(received(st)?.count === '1' && received(st)?.mode === 'wide', `${host} native: two reactive keys replaced the aggregate: ${st.slots.received}`);
    await s.tap('mode'); await settle(s);
    st = await s.state();
    check(received(st) && !('mode' in received(st)) && received(st).count === '1', `${host} native: a none clears its key: ${st.slots.received}`);
    // All nine events, from a background source, in order.
    await s.tap('fire'); await settle(s);
    st = await s.state();
    check(st.slots.events === EXPECTED_EVENTS, `${host} native: nine events with payloads: ${JSON.stringify(st.slots.events)}`);
    check(st.slots.loads === 2, `${host} native: load arrived as an event: ${st.slots.loads}`);
    // The module view takes no hits: the agent's tap is the node's press.
    await s.tap('box'); await settle(s);
    st = await s.state();
    // (A browser's real pointer also hovers and focuses the element first.)
    check(st.slots.events.startsWith(EXPECTED_EVENTS) && st.slots.events.endsWith('press;'), `${host} native: a tap on the box is the node's press: ${JSON.stringify(st.slots.events)}`);
    // Refused props: named, the last accepted kept, the app still running.
    const before = st.slots.received;
    await s.tap('reject'); await settle(s);
    t = await s.tree(); st = await s.state();
    check(module(t, 'box')?.state === 'error' && /props refused: reject=true/.test(module(t, 'box')?.error ?? ''), `${host} native: refused props are named: ${JSON.stringify(module(t, 'box'))}`);
    check(st.slots.received === before, `${host} native: the last accepted props stay active`);
    logs = await s.logs();
    check(logs.lines.some((l) => /exact-fixture #\d+: error: props refused/.test(l)), `${host} native: the refusal is logged`);
    await s.tap('bump'); await settle(s);
    check((await s.state()).slots.count === 2, `${host} native: the app runs after a refusal`);
    await s.tap('reject'); await settle(s);
    t = await s.tree();
    check(module(t, 'box')?.state === 'ready', `${host} native: an accepted replacement clears the refusal: ${JSON.stringify(module(t, 'box'))}`);
    // Capture: the boxes, and on Apple the fixture's tokened snapshot.
    const shot = resolve(shots ?? tmp, `native-fixture-${host}.png`);
    const box = await pixel(s, shot, 'box'), plain = await pixel(s, shot, 'plain'), absent = await pixel(s, shot, 'absent');
    check(near(box, '#2266ee'), `${host} native: the capture shows the fixture's colour: ${box}`);
    check(near(plain, '#11aa44'), `${host} native: the ordinary capture shows the plain box: ${plain}`);
    check(near(absent, '#f3f4f6'), `${host} native: the missing factory leaves its empty box: ${absent}`);
    if (host !== 'web') {
      logs = await s.logs();
      check(logs.lines.some((l) => /exact-fixture #\d+: snapshot token \d+, \d+ bytes/.test(l)), `${host} native: the capture asked the fixture's tokened snapshot: ${logs.lines.filter((l) => /snapshot/.test(l)).join(' | ')}`);
    }
    // Destroy: the nonce dies first; the late background callback is dropped.
    await s.tap('toggle'); await settle(s); await sleep(300); await settle(s);
    st = await s.state(); logs = await s.logs();
    check(!st.slots.events.includes('late'), `${host} native: a callback after destroy never reached the app: ${st.slots.events}`);
    check(logs.lines.some((l) => /dropped message from nonce \d+ after destroy/.test(l)), `${host} native: the dropped callback is logged: ${logs.lines.filter((l) => /native/.test(l)).join(' | ')}`);
    await s.tap('toggle');
    t = await until(s, 'a remount attaches a new instance', (t) => module(t, 'box')?.state === 'ready');
    await settle(s);
    check((await s.state()).slots.loads === 3, `${host} native: the new instance loaded`);
    // A plan reload reuses the defined elements (web).
    if (host === 'web') {
      const defined = await s.carrier.evaluate('exact.nativeDefines?.count');
      await s.carrier.evaluate(`fetch('./app.plan').then((r) => r.arrayBuffer()).then((b) => exact.reload(new Uint8Array(b)))`);
      t = await until(s, 'the reloaded plan attaches again', (t) => module(t, 'box')?.state === 'ready');
      const after = await s.carrier.evaluate('exact.nativeDefines?.count');
      check(defined === 2 && after === 2, `${host} native: a plan reload does not re-define the elements: ${defined} → ${after}`);
    }
  } catch (error) {
    check(false, `${host} native: the fixture drive stopped: ${error.stack ?? error.message}`);
  } finally { await s.close(); }

  // The failure family's load failures: a session each.
  const failing = async (name, options, pattern, cleanup = () => {}) => {
    const f = await open({ host, ...options });
    try {
      const t = await until(f, `${name}: every module node is unavailable`, (t) => ['box', 'plain', 'absent'].every((id) => module(t, id)?.state === 'unavailable'));
      check(['box', 'plain'].every((id) => pattern.test(module(t, id)?.error ?? '')), `${host} native: ${name} is named: ${JSON.stringify(module(t, 'box'))}`);
      const logs = await f.logs();
      check(logs.lines.some((l) => /native .*unavailable/.test(l) && pattern.test(l)), `${host} native: ${name} is logged: ${logs.lines.filter((l) => /native/.test(l)).join(' | ')}`);
      await f.tap('bump'); await f.clock('+50');
      check((await f.state()).slots.count === 1, `${host} native: the app runs with ${name}`);
      const rgb = await pixel(f, resolve(tmp, `${name.replace(/\W+/g, '-')}.png`), 'box');
      check(near(rgb, '#ffffff'), `${host} native: ${name} leaves an empty box: ${rgb}`);
    } catch (error) {
      check(false, `${host} native: ${name} stopped: ${error.message}`);
    } finally { await f.close(); cleanup(); }
  };
  if (host === 'web') {
    // A copy of the build without, and with a skewed, module artifact, served as-is.
    const variant = (name, change) => {
      const dir = resolve(tmp, name);
      cpSync(webDist, dir, { recursive: true });
      change(dir);
      const server = createServer((req, res) => serveStatic(dir, req, res));
      return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok({ url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() })));
    };
    const missing = await variant('missing', (dir) => rmSync(resolve(dir, 'modules'), { recursive: true, force: true }));
    await failing('a missing artifact', { url: missing.url }, /did not load/, missing.close);
    const skewed = await variant('skewed', (dir) => {
      const file = resolve(dir, 'modules/index.js');
      writeFileSync(file, readFileSync(file, 'utf8').replace('export const abi = 1;', 'export const abi = 2;'));
    });
    await failing('a wrong ABI', { url: skewed.url }, /module ABI 2, host ABI 1/, skewed.close);
  } else {
    await failing('a missing artifact', { env: { EXACT_MODULES: resolve(tmp, 'absent/libexact_modules.dylib') } }, /no module artifact/);
    // An artifact built against another ABI: a table whose major is 2.
    const source = resolve(tmp, 'skew.c'), dylib = resolve(tmp, 'libexact_modules.dylib');
    writeFileSync(source, 'static const struct { unsigned major, size; const char *roster; void *f[7]; } t = { 2, 72, "{}", { 0 } };\nconst void *exact_native_abi(void) { return &t; }\n');
    const cc = spawnSync('xcrun', host === 'ios' ? ['--sdk', 'iphonesimulator', 'clang', '-target', 'arm64-apple-ios17.0-simulator', '-dynamiclib', '-o', dylib, source] : ['clang', '-dynamiclib', '-o', dylib, source], { encoding: 'utf8' });
    check(cc.status === 0, `${host} native: the skewed artifact compiles: ${cc.stderr}`);
    await failing('a wrong ABI', { env: { EXACT_MODULES: dylib } }, /module ABI 2, host ABI 1/);
  }
  rmSync(tmp, { recursive: true, force: true });
  console.log(`${host} native: ${checks - failed} of ${checks} checks passed in ${((Date.now() - t0) / 1000).toFixed(1)} s (the LLP 1024 D8 fixture)`);
}
