// LLP 1153 D8, in Chrome, Firefox and WebKit: the wasm runtime's agent `tree` (glue.js `tree`) flags a node `inactive`,
// and `hidden` too, as the navigation glue's projection left the page (navigation.js `routeFlags`, which the JS target's
// tree reads too), not as the runner flags it: the runner knows no pane on the web, so it would hide a split's list
// shown beside its detail. A split list is live beside its detail when wide and hidden under it when narrow; under a
// sheet the page shows, wide the list too, inert; whatever the runner said, the page's flags are the reply's. A
// flowed paragraph's runs, held off the page while its fragments paint (textflow-glue.js), take their paragraph's.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';
import { glueDeclarations } from './glue-fixture.mjs';

const ROOT = resolve(import.meta.dir, '../../..');
// A root whose list route asks for a 320 px split, its detail pushed, and a sheet when `sheet`; the node ids are each
// element's `data-view`, as the glue's `views` holds them.
const routes = (sheet) => `<main navigationKey="${sheet ? 3 : 2}" navigationBack="back" data-view="1" style="position:relative;display:block;height:600px">
  <div navigationKey="1" navigationSplit="320" data-view="10" style="position:absolute;inset:0"><p data-view="11">List</p></div>
  <div navigationKey="2" data-view="20" style="position:absolute;inset:0"><p data-view="21">Detail</p></div>
  ${sheet ? '<div navigationKey="3" navigationPresentation="modal" data-view="30" style="position:absolute;inset:0"><p data-view="31">Sheet</p></div>' : ''}
</main>`;
// The runner's reply as the wasm host's runner gave it: every route its root has not selected hidden. Nodes 12 and 22
// are runs of the paragraphs 11 and 21, which the glue's `views` holds detached, as text flow leaves them.
const reply = (sheet) => ({ nodes: [
  { id: 1, parent: null, type: 'View', props: {} },
  { id: 10, parent: 1, type: 'View', props: {}, inactive: true, hidden: true },
  { id: 11, parent: 10, type: 'Text', props: {}, inactive: true, hidden: true },
  { id: 12, parent: 11, type: 'Text', props: {}, inactive: true, hidden: true },
  { id: 20, parent: 1, type: 'View', props: {}, ...(sheet ? { inactive: true, hidden: true } : {}) },
  { id: 21, parent: 20, type: 'Text', props: {}, ...(sheet ? { inactive: true, hidden: true } : {}) },
  { id: 22, parent: 21, type: 'Text', props: {}, ...(sheet ? { inactive: true, hidden: true } : {}) },
  ...(sheet ? [{ id: 30, parent: 1, type: 'View', props: {} }, { id: 31, parent: 30, type: 'Text', props: {} }] : []),
] });
const flags = (want) => Object.fromEntries(Object.entries(want).map(([id, f]) => [id, { inactive: f > 0, hidden: f > 1 }]));

for (const engine of ['chromium', 'firefox', 'webkit']) test(`${engine}: the wasm runtime's tree flags routes as the page shows them`, async () => {
  const glue = readFileSync(resolve(ROOT, 'host/web/glue.js'), 'utf8');
  const suite = await suiteBrowser(engine, { timeout: 10000 });
  try {
    const page = await suite.newPage({ viewport: { width: 1000, height: 700 } });
    const errors = [];
    page.on('pageerror', (error) => errors.push(String(error)));
    const read = async (sheet, width) => {
      await page.setViewportSize({ width, height: 700 });
      await page.setContent(withLook(readFileSync(resolve(ROOT, 'host/web/index.html'), 'utf8'))
        .replace('<div id="exact-root"></div>', () => `<div id="exact-root">${routes(sheet)}</div>`)
        .replace('<script type="module" src="./glue.js"></script>', () => `<script type="module">${readFileSync(resolve(ROOT, 'host/web/navigation.js'), 'utf8')}
          const shell = document.getElementById('exact-root'); navigation.connect(shell, () => false, () => {}); navigation.project(shell, () => {});
          const views = new Map([...shell.querySelectorAll('[data-view]')].map((e) => [Number(e.dataset.view), e])), ask = () => structuredClone(globalThis.reply);
          for (const id of [12, 22]) views.set(id, document.createElement('span'));
          const shownValue = (e) => e.value, iframeLoading = new Map(), guestOutline = () => null;
          ${glueDeclarations(glue, ['tree'])}
          globalThis.read = () => Object.fromEntries(tree({ op: 'tree' }).nodes.map((n) => [n.id, { inactive: !!n.inactive, hidden: !!n.hidden }]));</script>`));
      await page.evaluate((r) => { globalThis.reply = r; }, reply(sheet));
      await page.waitForFunction(() => globalThis.read);
      return page.evaluate(() => globalThis.read());
    };
    // Wide, the list is live beside its detail; narrow, the detail covers it.
    expect(await read(false, 1000)).toEqual(flags({ 1: 0, 10: 0, 11: 0, 12: 0, 20: 0, 21: 0, 22: 0 }));
    expect(await read(false, 420)).toEqual(flags({ 1: 0, 10: 2, 11: 2, 12: 2, 20: 0, 21: 0, 22: 0 }));
    // A sheet over the split: the page it covers shows, inert, and wide the list beside it too.
    expect(await read(true, 1000)).toEqual(flags({ 1: 0, 10: 1, 11: 1, 12: 1, 20: 1, 21: 1, 22: 1, 30: 0, 31: 0 }));
    expect(await read(true, 420)).toEqual(flags({ 1: 0, 10: 2, 11: 2, 12: 2, 20: 1, 21: 1, 22: 1, 30: 0, 31: 0 }));
    expect(errors).toEqual([]);
  } finally {
    await suite.close();
  }
}, 60000);
