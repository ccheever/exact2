// B5 (LLP 1071.001 part B; host/web-js/src/lower.rs, lower.js; theme2 07:30 and 07:33): a lowered node's computed
// style is the inline build's, property by property, at every arm. Each page is built twice, the second with
// EXACT_JS_NO_LOWER (the test-only comparison build: no flag, nothing in build.mjs's usage), and driven alike.
//   - b5-edges (conformance/b5-edges.contract): every lowered kind and every refusal, through every arm, at rest
//     and with its looks held; agent `layout` reports the same rows (condition 4); arm rules built once a site an arm.
//   - the bounded feed (bench/dioxus/exact-bounded; its generated data from bench/dioxus/prepare.sh), every run.
//   - Conduit (apps/realworld): nothing lowers, so its program is the comparison build's byte for byte.
//   - a scheme flip and a mode change mid-session (condition 5), a server page's adoption (condition 9), and a
//     generation whose arm literal changed under a kept arms sheet (condition 7).
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { open } from '../../scripts/agent.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const root = resolve(import.meta.dir, '../..');
const scratch = mkdtempSync(resolve(root, 'target/lower-equal-'));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));
const edges = readFileSync(resolve(import.meta.dir, 'conformance/b5-edges.contract'), 'utf8');

/** `source` (with `files` beside it) built as app `name`, lowered or not; its dist. */
async function build(name, source, lowered, { files = {}, render = 'none', appDir, extraEnv = {} } = {}) {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const dir = appDir ?? resolve(scratch, `${name}-${lowered ? 'low' : 'inline'}`), dist = resolve(scratch, `${name}-${lowered ? 'low' : 'inline'}-dist`);
  if (!appDir) {
    mkdirSync(dir, { recursive: true });
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name, app: { id: `test.${name}`, name }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), source);
    for (const [file, text] of Object.entries(files)) writeFileSync(resolve(dir, file), text);
  }
  const env = { ...process.env, EXACT_APP_DIR: dir, ...extraEnv };
  if (lowered) delete env.EXACT_JS_NO_LOWER; else env.EXACT_JS_NO_LOWER = '1';
  const run = Bun.spawn([process.execPath, 'host/web-js/build.mjs', name, '--out', dist, '--render', render], { cwd: root, env, stdout: 'pipe', stderr: 'pipe' });
  const [status, stdout, stderr] = await Promise.all([run.exited, new Response(run.stdout).text(), new Response(run.stderr).text()]);
  expect({ name, lowered, status, error: status ? stdout + stderr : '' }).toEqual({ name, lowered, status: 0, error: '' });
  return dist;
}

const servers = [];
afterAll(() => { for (const s of servers) s.stop(true); });
function serve(dist) {
  const s = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
    const path = new URL(req.url).pathname, file = resolve(dist, path === '/' ? 'index.html' : path.slice(1));
    return new Response(Bun.file(existsSync(file) && !file.endsWith('/') ? file : resolve(dist, 'index.html')));
  } });
  servers.push(s);
  return s.url.href;
}

// Every element under the root: its tag and test id, and every computed property (the type companions too).
const COMPANIONS = ['--exact-style-font-weight', '--exact-said-font-weight', '--exact-said-line-height', '--exact-tint'];
const snapshot = (companions) => [...document.querySelectorAll('#exact-root *')].filter(e => !e.closest('[data-exact-list] [aria-hidden="true"]')).map(e => {
  const cs = getComputedStyle(e), style = {};
  for (const p of cs) style[p] = cs.getPropertyValue(p);
  for (const p of companions) style[p] = cs.getPropertyValue(p);
  return [e.localName, e.getAttribute('data-testid'), (e.textContent ?? '').slice(0, 24), style];
});
/** The properties that differ between two snapshots, as `where prop: lowered | inline`. */
function differences(low, inline) {
  const out = [];
  if (low.length !== inline.length) return [`element count ${low.length} | ${inline.length}`];
  low.forEach(([tag, id, text, style], i) => {
    const [tag2, id2, text2, style2] = inline[i];
    if (tag !== tag2 || id !== id2 || text !== text2) return out.push(`#${i} ${tag}/${id}/${text} | ${tag2}/${id2}/${text2}`);
    for (const p of new Set([...Object.keys(style), ...Object.keys(style2)])) if (style[p] !== style2[p]) out.push(`#${i} ${id ?? tag} ${p}: ${style[p]} | ${style2[p]}`);
  });
  return out;
}
// Two frames, and every running transition or animation (a tapped button's wash) finished, on both pages alike.
const settled = page => page.evaluate(async () => { await Promise.race([Promise.all(document.getAnimations().map(a => a.finished.catch(() => {}))), new Promise(r => setTimeout(r, 2000))]); await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))); });

/** Two pages of one browser, the lowered build's and the inline one's. */
async function pair(lowDist, inlineDist, { init, viewport = { width: 420, height: 900 } } = {}) {
  const suite = await suiteBrowser();
  const pages = [];
  for (const dist of [lowDist, inlineDist]) {
    const page = await (await suite.newContext({ viewport })).newPage();
    const errors = [];
    page.on('pageerror', e => errors.push(String(e)));
    // A test page checks that an arm name built once stands for the same declarations everywhere (lower.js).
    await page.addInitScript(() => { globalThis.__exactArmsCheck = true; });
    if (init) await page.addInitScript(init);
    await page.goto(serve(dist));
    await page.waitForSelector('[data-testid="root"], [data-testid^="row-"]', { timeout: 30000 });
    await settled(page);
    pages.push(Object.assign(page, { errors }));
  }
  return { suite, low: pages[0], inline: pages[1] };
}
async function same(low, inline, at) {
  const [a, b] = await Promise.all([low.evaluate(snapshot, COMPANIONS), inline.evaluate(snapshot, COMPANIONS)]);
  expect({ at, differences: differences(a, b).slice(0, 20) }).toEqual({ at, differences: [] });
  return a.length;
}
// Counts the arms sheet's rules as they are built (the first use of an arm at a site).
const countRules = () => {
  const insert = CSSStyleSheet.prototype.insertRule;
  window.__armRules = [];
  CSSStyleSheet.prototype.insertRule = function (rule, at) { if (this.ownerNode?.id === 'exact-arms') window.__armRules.push(rule); return insert.call(this, rule, at); };
};

// b5-edges' nodes by test id: the ones that lower and the ones a refusal keeps inline (lower_tests.rs names why).
const LOWERED = ['colour', 'look', 'role', 'border', 'token', 'blank', 'wide', 'eased'];
const KEPT = ['match', 'tinted', 'r1', 'r2', 'r3', 'r4', 'r5', 'r9', 'two', 'many'];
let edgesLow, edgesInline;
beforeAll(async () => {
  edgesLow = await build('b5-edges', edges, true);
  edgesInline = await build('b5-edges', edges, false);
}, 600000);

test('b5-edges: every lowered node and every refusal computes as inline, at every arm, held or not', async () => {
  const program = readFileSync(resolve(edgesLow, '.gen/app.js'), 'utf8');
  // (The feed's run and the inheriting column carry no test id.)
  expect(program.match(/Pc\(e\d+/g)?.length).toBe(LOWERED.length + 2);
  expect(readFileSync(resolve(edgesInline, '.gen/app.js'), 'utf8')).not.toContain('Pc(');
  const { suite, low, inline } = await pair(edgesLow, edgesInline, { init: countRules });
  try {
    let checks = 0;
    const tap = async id => { await Promise.all([low, inline].map(async p => { await p.click(`[data-testid="${id}"]`); await p.mouse.move(0, 0); })); await Promise.all([low, inline].map(settled)); };
    // Which nodes lowered (an arm class on), and which kept their inline rows.
    const classes = await low.evaluate(ids => Object.fromEntries(ids.map(id => [id, /\bxa\w+-\d+\b/.test(document.querySelector(`[data-testid="${id}"]`)?.className ?? '')])), [...LOWERED, ...KEPT]);
    expect(classes).toEqual(Object.fromEntries([...LOWERED.map(id => [id, true]), ...KEPT.map(id => [id, false])]));
    // Every arm of `s` (and the rest), the second field, the option, and the runs' kinds moved; `to-b,to-c` changes
    // the eased node's arm while its colour transition from the last one runs.
    for (const step of ['to-a', 'to-b', 'to-c', 'to-z', 'flip', 'shift', 'fill', 'to-b', 'to-a', 'to-b,to-c', 'shift', 'fill', 'flip']) {
      if (step.includes(',')) {
        const [first, then] = step.split(',');
        // The first arm's transition held at 100 ms on both pages, so the second starts from the same colour.
        const hold = p => p.evaluate(() => { const el = document.querySelector('[data-testid="eased"]'), running = document.getAnimations().filter(a => a.effect?.target === el); for (const a of running) { a.pause(); a.currentTime = 100; } return running.length; });
        await Promise.all([low, inline].map(async p => { await p.click(`[data-testid="${first}"]`); expect(await hold(p)).toBe(1); await p.click(`[data-testid="${then}"]`); await p.mouse.move(0, 0); }));
        // The interrupted transition, sampled at the same time on both pages: each page's colour transition is paused
        // at 150 ms of its 300, and the presented colour compared; then both run to the end (`same`, below).
        const sample = p => p.evaluate(() => {
          const el = document.querySelector('[data-testid="eased"]'), running = document.getAnimations().filter(a => a.effect?.target === el);
          for (const a of running) { a.pause(); a.currentTime = 150; }
          const at = { transitions: running.map(a => a.transitionProperty), color: getComputedStyle(el).color };
          for (const a of running) a.play();
          return at;
        });
        const [mid, midInline] = [await sample(low), await sample(inline)];
        expect(mid.transitions).toEqual(['color']);
        expect(mid).toEqual(midInline);
        await Promise.all([low, inline].map(settled));
      } else await tap(step);
      await same(low, inline, step);
      checks++;
      // The looks, held as the input glue holds them, then by the pointer and the keyboard.
      for (const held of ['hover', 'focus-visible']) {
        await Promise.all([low, inline].map(p => p.evaluate(h => { for (const e of document.querySelectorAll('[data-exact-looks]')) e.setAttribute('data-exact-held', h); }, held)));
        await same(low, inline, `${step} held ${held}`);
        await Promise.all([low, inline].map(p => p.evaluate(() => { for (const e of document.querySelectorAll('[data-exact-held]')) e.removeAttribute('data-exact-held'); })));
      }
      await Promise.all([low, inline].map(p => p.hover('[data-testid="look"]')));
      await Promise.all([low, inline].map(settled));
      await same(low, inline, `${step} hovered`);
      await Promise.all([low, inline].map(async p => { await p.mouse.move(0, 0); await p.focus('[data-testid="look"]'); }));
      await Promise.all([low, inline].map(settled));
      await same(low, inline, `${step} focused`);
      await Promise.all([low, inline].map(p => p.evaluate(() => document.activeElement?.blur())));
    }
    expect(checks).toBe(13);
    // Condition (8) and x2-perf4's ask: a rule a site an arm, never an instance (the feed's seven runs share one site).
    const rules = await low.evaluate(() => window.__armRules);
    const names = rules.map(r => /\.(xa[\w-]+)/.exec(r)[1]);
    expect(new Set(names).size).toBe(names.length);
    const bySite = Object.groupBy(names, n => n.replace(/-\d+$/, ''));
    expect(Object.keys(bySite).length).toBe(LOWERED.length + 2);
    for (const [site, built] of Object.entries(bySite)) expect({ site, n: built.length <= 7 }).toEqual({ site, n: true });
    expect(bySite[Object.keys(bySite).find(s => program.includes(`"${s}",[[5,15`))].length).toBe(7);
    expect(await inline.evaluate(() => window.__armRules.length)).toBe(0);
    expect([low.errors, inline.errors]).toEqual([[], []]);
  } finally { await suite.close(); }
}, 240000);

test('b5-edges: agent layout reports a lowered node\'s rows as it reports the inline node\'s (condition 4)', async () => {
  const sessions = [];
  try {
    for (const dist of [edgesLow, edgesInline]) sessions.push(await open({ host: 'web', url: serve(dist) }));
    const [low, inline] = sessions;
    for (const step of ['to-a', 'to-b', 'to-c', 'to-z']) {
      await low.tap(step); await inline.tap(step);
      for (const id of ['colour', 'look', 'role', 'border', 'token']) {
        const [a, b] = [await low.layout(id), await inline.layout(id)];
        expect({ step, id, style: a.node.style }).toEqual({ step, id, style: b.node.style });
      }
    }
  } finally { for (const s of sessions) await s.close(); }
}, 240000);

test('a scheme flip and a mode change mid-session repaint a lowered node as the inline one (condition 5)', async () => {
  const design = 'tamber t\n\naxes\n  ink = tint | plain\n\ntheme\n  surface.card = light-dark(#ffffff, #101010)\n    when ink=plain: light-dark(#eeeeee, #202020)\n  text.primary = light-dark(#0f1419, #e7e9ea)\n    when ink=plain: light-dark(#333333, #cccccc)\n';
  const source = 'component App\n  state s = "a"\n  state plain = false\n  action set(v: string)\n    s = v\n  action ink\n    plain = not plain\n  view\n    column testId="root" -exact-mode=(plain ? "ink=plain" : "ink=tint")\n      button testId="to-b" press=set("b") "b"\n      button testId="ink" press=ink "ink"\n      text testId="card" "card" background-color=(s == "a" ? "surface.card" : "-exact-secondary-background") border-color=(s == "a" ? "text.primary" : "AccentColor") border-style="solid" border-width=2\n';
  const [lowDist, inlineDist] = [await build('b5-modes', source, true, { files: { 'app.tamber': design } }), await build('b5-modes', source, false, { files: { 'app.tamber': design } })];
  expect(readFileSync(resolve(lowDist, '.gen/app.js'), 'utf8')).toContain('Pc(');
  const { suite, low, inline } = await pair(lowDist, inlineDist);
  try {
    await same(low, inline, 'light, tint, a');
    for (const [what, act] of [['dark', p => p.emulateMedia({ colorScheme: 'dark' })], ['ink', p => p.click('[data-testid="ink"]')], ['b', p => p.click('[data-testid="to-b"]')],
      ['light', p => p.emulateMedia({ colorScheme: 'light' })], ['ink again', p => p.click('[data-testid="ink"]')]]) {
      await Promise.all([low, inline].map(act));
      await Promise.all([low, inline].map(settled));
      await same(low, inline, what);
    }
  } finally { await suite.close(); }
}, 600000);

test('adoption swaps a server page\'s inline rows for the arm class with no moment showing neither (condition 9)', async () => {
  const source = 'routes nav\n  home "/" render=build\n\n' + edges;
  const dist = await build('b5-adopt', source, true, { render: 'js' }), inlineDist = await build('b5-adopt', source, false, { render: 'js' });
  // The server page is the inline build's, its lowered nodes' rows inline, byte for byte.
  const rendered = d => /<div id="exact-root"[\s\S]*<\/div>/.exec(readFileSync(resolve(d, 'index.html'), 'utf8').replace(/<script[\s\S]*?<\/script>/g, ''))[0];
  expect(rendered(dist)).toContain('style="font-family:ui-monospace');
  expect(rendered(dist)).toBe(rendered(inlineDist));
  const suite = await suiteBrowser();
  try {
    const page = await (await suite.newContext({ viewport: { width: 420, height: 900 } })).newPage();
    await page.addInitScript(() => {
      // Each removal of an inline row on a lowered node: the computed value before and after it, and the class then.
      window.__swaps = [];
      const remove = CSSStyleDeclaration.prototype.removeProperty;
      CSSStyleDeclaration.prototype.removeProperty = function (prop) {
        const el = window.__lowered?.find(e => e.style === this);
        if (!el) return remove.call(this, prop);
        const before = getComputedStyle(el).getPropertyValue(prop), out = remove.call(this, prop);
        window.__swaps.push([prop, el.className, before, getComputedStyle(el).getPropertyValue(prop)]);
        return out;
      };
      new MutationObserver(() => { window.__lowered = [...document.querySelectorAll('[data-exact-text] > span, [data-testid]')]; }).observe(document, { subtree: true, childList: true });
    });
    await page.goto(serve(dist));
    await page.waitForFunction(() => globalThis.exact?.journal?.some(l => l.endsWith('adopted the document')), null, { timeout: 30000 });
    const swaps = await page.evaluate(() => window.__swaps);
    expect(swaps.length).toBeGreaterThan(0);
    for (const [prop, cls, before, after] of swaps) expect({ prop, cls: /\bxa\w+-\d+\b/.test(cls), same: before === after }).toEqual({ prop, cls: true, same: true });
    // Nothing of the renderer's rows is left inline on a lowered node.
    expect(await page.evaluate(() => [...document.querySelectorAll('[class*="xa"]')].map(e => e.getAttribute('style') ?? '').filter(Boolean))).toEqual([]);
  } finally { await suite.close(); }
}, 600000);

test('a generation whose arm literal changed shows the new value under a kept arms sheet (condition 7)', async () => {
  const one = await build('b5-gen', edges, true), was = readFileSync(resolve(one, '.gen/app.js'), 'utf8');
  const changed = edges.replace('color=(s == "a" ? "#000000" : s == "b" ? "#007AFF"', 'color=(s == "a" ? "#123456" : s == "b" ? "#007AFF"');
  const two = await build('b5-gen2', changed, true), now = readFileSync(resolve(two, '.gen/app.js'), 'utf8');
  const nameOf = js => /"(xa\w+)",\[\["#\w+",/.exec(js.slice(js.indexOf('"colour"')))[1];
  expect(nameOf(was)).not.toBe(nameOf(now));
  const suite = await suiteBrowser();
  try {
    const first = await (await suite.newContext()).newPage();
    await first.goto(serve(one));
    await first.waitForSelector('[data-testid="colour"]');
    const kept = await first.evaluate(() => { const s = document.getElementById('exact-arms'); return { rules: [...s.sheet.cssRules].map(r => r.cssText), host: s.$host }; });
    // Synthetic coverage: an ordinary dev reload replaces the document (dev.mjs), so here the old generation's sheet is
    // put in place by hand, as a runtime reload under a kept document would leave it: before any of
    // the page's scripts run, by the same host (its rules reused) and by another (started over).
    for (const sameHost of [true, false]) {
      const page = await (await suite.newContext()).newPage();
      await page.addInitScript(([{ rules, host }, sameHost]) => {
        new MutationObserver((_, o) => {
          if (!document.head) return;
          o.disconnect();
          const s = document.createElement('style'); s.id = 'exact-arms'; s.$seeded = true; document.head.append(s);
          s.$host = sameHost ? host : 'another host'; s.$built = new Map();
          for (const r of rules) { s.sheet.insertRule(r, s.sheet.cssRules.length); s.$built.set(/\.(xa[\w-]+)/.exec(r)[1], true); }
        }).observe(document, { childList: true, subtree: true });
      }, [kept, sameHost]);
      await page.goto(serve(two));
      await page.waitForSelector('[data-testid="colour"]');
      const seen = await page.evaluate(() => { const all = document.querySelectorAll('#exact-arms'), s = all[0]; return { count: all.length, seeded: !!s.$seeded, built: [...s.$built.keys()], colour: getComputedStyle(document.querySelector('[data-testid="colour"]')).color }; });
      expect({ sameHost, count: seen.count, seeded: seen.seeded, colour: seen.colour }).toEqual({ sameHost, count: 1, seeded: true, colour: 'rgb(18, 52, 86)' });
      // The same host kept the old colour arm's rule beside the changed one's; another host started over without it.
      expect({ sameHost, old: seen.built.some(n => n.startsWith(`${nameOf(was)}-`)), now: seen.built.some(n => n.startsWith(`${nameOf(now)}-`)) }).toEqual({ sameHost, old: sameHost, now: true });
    }
  } finally { await suite.close(); }
}, 600000);

test('the bounded feed: every run computes as inline, and its site builds at most its seven arms', async () => {
  const app = resolve(root, 'bench/dioxus/exact-bounded');
  // The feed's generated data is a prerequisite, not a skip.
  if (!existsSync(resolve(app, 'data/messages.json'))) throw new Error('lower-equal: bench/dioxus/exact-bounded/data/messages.json is missing: run bench/dioxus/prepare.sh first');
  const extraEnv = { CARGO_TARGET_DIR: resolve(app, 'target') };
  const [lowDist, inlineDist] = [await build('exact-bounded', '', true, { appDir: app, extraEnv }), await build('exact-bounded', '', false, { appDir: app, extraEnv })];
  const { suite, low, inline } = await pair(lowDist, inlineDist, { init: countRules });
  try {
    await same(low, inline, 'first window');
    // Rows built again further down, and back.
    for (const y of [3000, 9000, 0]) {
      await Promise.all([low, inline].map(p => p.evaluate(to => { for (const e of document.querySelectorAll('*')) if (e.scrollHeight > e.clientHeight + 100 && /auto|scroll/.test(getComputedStyle(e).overflowY)) e.scrollTop = to; }, y)));
      // Each page's list settles (its rows built for the new offset) and its element count holds over two frames, not a fixed
      // wait: under load a fixed wait compared one page mid-build (252 against 281 elements).
      await Promise.all([low, inline].map(p => p.evaluate(async () => {
        const frame = () => new Promise(r => requestAnimationFrame(() => r()));
        for (let i = 0, last = -1, held = 0; i < 200 && held < 2; i++) {
          await globalThis.exact?.lists?.pending?.(); await globalThis.exact?.lists?.settle?.(); await frame();
          const n = document.querySelectorAll('*').length; held = n === last ? held + 1 : 0; last = n;
        }
      })));
      await same(low, inline, `scrolled to ${y}`);
    }
    // All seven arms were built (each once) and so seen and compared above.
    const rules = await low.evaluate(() => window.__armRules);
    expect(rules.map(r => /\.xa\w+-(\d+)/.exec(r)[1]).sort()).toEqual(['0', '1', '2', '3', '4', '5', '6']);
  } finally { await suite.close(); }
}, 900000);

test('Conduit lowers nothing: its program and sheet are the comparison build\'s byte for byte', () => {
  const contract = resolve(root, 'apps/realworld/app.contract'), out = mode => resolve(scratch, `conduit-${mode}`);
  for (const [mode, off] of [['low', false], ['inline', true]]) {
    const env = { ...process.env };
    if (off) env.EXACT_JS_NO_LOWER = '1'; else delete env.EXACT_JS_NO_LOWER;
    const { cmd, pre } = webCompiler();
    const r = Bun.spawnSync([cmd, ...pre, 'js', contract, '-o', out(mode)], { cwd: root, env, stdout: 'pipe', stderr: 'pipe' });
    expect({ mode, status: r.exitCode }).toEqual({ mode, status: 0 });
  }
  const read = (mode, f) => readFileSync(resolve(out(mode), f), 'utf8');
  expect(read('low', 'app.js')).not.toContain('Pc(');
  expect(read('low', 'app.js')).toBe(read('inline', 'app.js'));
  expect(read('low', 'app.css')).toBe(read('inline', 'app.css'));
});
