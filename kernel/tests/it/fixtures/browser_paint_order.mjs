// Measures browser_paint_order.tsv in Chrome (LLP 1083.000 D6): for each
// case, the topmost box at each probe with the markup as written (bare) and
// with `isolation: isolate` on exactly the boxes the kernel isolates or
// stacks by policy (exact). The kernel's decisions come from the Rust test
// itself, run with EXACT_PAINT_ORDER_DUMP. Rewrites the fixture in place.
//
//   bun kernel/tests/it/fixtures/browser_paint_order.mjs
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../../scripts/agent.mjs';
import { chromium } from '../../../../scripts/agent-launch.mjs';

const here = new URL('.', import.meta.url).pathname;
const root = resolve(here, '../../../..');
const fixture = resolve(here, 'browser_paint_order.tsv');
const scratch = mkdtempSync(resolve(tmpdir(), 'exact-paint-order-'));
const dump = resolve(scratch, 'dump.jsonl');

const lines = readFileSync(fixture, 'utf8').split('\n').filter(Boolean).map(l => l.split('\t'));
// The kernel's decisions, from the test (it writes them instead of checking).
const test = spawnSync('cargo', ['test', '-q', '-p', 'exact-kernel', '--test', 'it', 'browser_paint_order'], { cwd: root, env: { ...process.env, EXACT_PAINT_ORDER_DUMP: dump }, encoding: 'utf8' });
if (test.status !== 0) throw new Error(`the kernel test failed:\n${test.stdout}${test.stderr}`);
const decided = new Map(readFileSync(dump, 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l)).map(d => [d.name, d]));

const { executable } = chromium();
const profile = mkdtempSync(resolve(tmpdir(), 'exact-paint-order-chrome-'));
const child = spawn(executable, ['--headless=new', '--remote-debugging-pipe', '--window-size=600,600', `--user-data-dir=${profile}`, '--no-first-run', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
const out = [];
try {
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const doc = '<!doctype html><style>*{box-sizing:content-box;font:16px/18px monospace}body{margin:0}</style><div id=c></div>';
  const { targetId } = await cdp.send('Target.createTarget', { url: 'data:text/html,' + encodeURIComponent(doc) });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const ev = async e => { const r = await cdp.send('Runtime.evaluate', { expression: e, returnByValue: true }, sessionId); if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails)); return r.result.value; };
  for (let i = 0; !(await ev('!!document.getElementById("c")')); i++) await Bun.sleep(20);
  for (const [name, rootCss, nodesField, probesField] of lines) {
    const d = decided.get(name);
    if (!d) throw new Error(`no kernel decision for ${name}`);
    const nodes = nodesField.split(' & ').map(n => { const [id, parent, ...css] = n.split('>'); return [Number(id), Number(parent), css.join('>')]; });
    const probes = probesField.split(' ').map(p => p.split(',').map(Number));
    const isolate = [...d.isolate, ...d.policy];
    const measure = (iso) => ev(`(() => {
      const c = document.getElementById('c'); c.innerHTML = '';
      const colour = id => 'hsl(' + (id * 67 % 360) + ' 70% 50%)';
      const el = { 1: document.createElement('div') };
      el[1].id = 'n1'; el[1].style.cssText = ${JSON.stringify(rootCss)} + ';background:' + colour(1); c.append(el[1]);
      for (const [id, parent, css] of ${JSON.stringify(nodes)}) { const e = el[id] = document.createElement('div'); e.id = 'n' + id; e.style.cssText = css + ';background:' + colour(id); el[parent].append(e); }
      for (const id of ${JSON.stringify(iso)}) el[id].style.isolation = 'isolate';
      const r = el[1].getBoundingClientRect();
      return ${JSON.stringify(probes)}.map(([x, y]) => { const t = document.elementFromPoint(r.x + x, r.y + y)?.closest('[id^=n]'); return t ? Number(t.id.slice(1)) : 0; });
    })()`);
    const bare = await measure([]), exact = await measure(isolate);
    out.push([name, rootCss, nodesField, probesField, bare.join(' '), exact.join(' '), d.isolate.length ? d.isolate.join(' ') : '-'].join('\t'));
  }
  console.error(await ev('navigator.userAgent'));
} finally {
  child.kill('SIGKILL');
  await new Promise(r => child.once('exit', r));
  rmSync(profile, { recursive: true, force: true });
  rmSync(scratch, { recursive: true, force: true });
}
writeFileSync(fixture, out.join('\n') + '\n');
console.error(`measured ${out.length} cases`);
