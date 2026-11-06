import { beforeAll, afterAll, test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createServer } from 'node:http';
import { open } from '../../scripts/agent.mjs';
import { serveBuildTree } from '../web/serve.mjs';
import { need } from '../../scripts/coverage.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const root = resolve(import.meta.dir, '../..'), builds = new Map();
let directory;
const plans = new Map();
beforeAll(async () => {
  directory = mkdtempSync(resolve(root, 'target/list-structures-'));
  for (const fixture of ['list-structures', 'list-routes', 'list-modals']) {
    const source = fixture === 'list-modals' ? resolve(directory, 'list-modals.contract') : resolve(import.meta.dir, `conformance/${fixture}.contract`);
    if (fixture === 'list-modals') writeFileSync(source, readFileSync(resolve(import.meta.dir, 'conformance/list-routes.contract'), 'utf8').replace('state depth = 1', 'state depth = 3'));
    const plan = resolve(directory, `${fixture}.plan`);
    plans.set(fixture, plan);
    const compiled = spawnSync(resolve(root, 'target/debug/contract'), ['build', source, '-o', plan], { encoding: 'utf8' });
    expect(compiled.status, compiled.stderr).toBe(0);
    for (const renderer of ['js', 'rust', 'none']) {
      const dist = resolve(directory, `${fixture}-${renderer}`);
      const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'caltrain', '--contract', source, '--out', dist, '--render', renderer], { cwd: root, stdout: 'pipe', stderr: 'pipe' });
      const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
      expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
      builds.set(`${fixture}-${renderer}`, dist);
    }
  }
}, 1_800_000);
afterAll(() => { if (directory) rmSync(directory, { recursive: true, force: true }); });

async function serverFor(dist) {
  const server = createServer((request, response) => serveBuildTree(dist, request, response));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return server;
}

async function inTarget(target, drive, fixture = 'list-structures') {
  let session, server;
  try {
    if (target !== 'wasm') {
      server = await serverFor(builds.get(`${fixture}-${target}`));
      session = await open({ host: 'web', url: `http://127.0.0.1:${server.address().port}/` });
      const journal = await session.carrier.evaluate('exact.journal');
      if (target !== 'none') expect(journal.some(line => line.endsWith('adopted the document'))).toBe(true);
    } else {
      session = await open({ host: 'web', plan: plans.get(fixture), webDist: process.env.EXACT_WEB_DIST });
    }
    await session.clock('settle');
    await drive(session, expression => session.carrier.evaluate(expression));
  } finally { await session?.close(); if (server) await new Promise(resolve => server.close(resolve)); }
}

for (const target of ['js', 'rust', 'none', 'wasm']) {
  const check = target === 'wasm' ? need('fixture', process.env.EXACT_WEB_DIST ? null : 'name an all-capabilities wasm build with EXACT_WEB_DIST') : test;
  check(`${target}: list route stacks move focus on push and back`, () => inTarget(target, async (s, read) => {
    await s.tap('open');
    expect(await read(`document.activeElement?.getAttribute('data-testid')`)).toBe('detail-title');
    await s.tap('back');
    expect(await read(`document.activeElement?.getAttribute('data-testid')`)).toBe('open');
  }, 'list-routes'), 120000);

  check(`${target}: list modal cards ignore top-layer rows`, () => inTarget(target, async (s, read) => {
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="modal-card"]')).display`)).toBe('grid');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="modal-title"]').closest('header').parentElement).display`)).toBe('contents');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="modal-title"]')).marginTop`)).toBe('0px');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="modal-body"]')).minHeight`)).toBe('40px');
    const layout = await read(`['modal-title', 'modal-body', 'reveal'].map(id => { const r = document.querySelector('[data-testid="'+id+'"]').getBoundingClientRect(); return { top: r.top, bottom: r.bottom, height: r.height }; })`);
    expect(layout[0].bottom).toBeLessThanOrEqual(layout[1].top);
    expect(layout[1].height).toBeGreaterThanOrEqual(40);
    expect(layout[1].bottom).toBeLessThanOrEqual(layout[2].top);
    await s.tap('reveal');
    expect(await read(`document.querySelector('[data-testid="modal-popover"]').hasAttribute('popover')`)).toBe(false);
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="modal-card"]')).display`)).not.toBe('grid');
  }, 'list-modals'), 120000);

  check(`${target}: wrapped route headers keep native styling`, () => inTarget(target, async (s, read) => {
    await s.tap('open');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="detail-title"]').closest('header')).position`)).toBe('sticky');
    expect(await read(`parseFloat(getComputedStyle(document.querySelector('[data-testid="detail-title"]')).marginTop)`)).toBeGreaterThan(0);
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="late-title"]')).marginTop`)).toBe('0px');
  }, 'list-routes'), 120000);

  check(`${target}: a list navigation root keeps its sidebar layout`, () => inTarget(target, async (s, read) => {
    await read(`document.querySelector('[data-testid="tabs"]').style.appearance = 'auto'`);
    await s.carrier.call('Emulation.setDeviceMetricsOverride', { width: 1000, height: 900, deviceScaleFactor: 1, mobile: false });
    await read(`dispatchEvent(new Event('resize'))`);
    await s.clock('settle');
    const sidebar = await read(`(() => { const root = document.querySelector('[data-testid="root"]'), bar = document.querySelector('[data-testid="tabs"]'), tab = document.querySelector('[data-testid="tab-home"]'); return { sidebar: root.hasAttribute('data-exact-sidebar'), width: getComputedStyle(bar).width, direction: getComputedStyle(bar).flexDirection, tabHeight: getComputedStyle(tab).minHeight }; })()`);
    expect(sidebar).toEqual({ sidebar: true, width: '240px', direction: 'column', tabHeight: '36px' });
  }), 120000);

  if (target === 'none' || target === 'wasm') check(`${target}: list panels stay unopened until selected`, () => inTarget(target, async (s, read) => {
    expect(await read(`!!document.querySelector('[data-testid="route-saved"]')`)).toBe(false);
    await s.tap('tab-saved');
    expect(await read(`!!document.querySelector('[data-testid="route-saved"]')`)).toBe(true);
  }), 120000);

  check(`${target}: list radio groups retain keyboard and selected styling`, () => inTarget(target, async (s, read) => {
    await s.type('radio-a', { key: 'ArrowRight' });
    expect(await read(`document.querySelector('[data-testid="radio-b"]').getAttribute('aria-checked')`)).toBe('true');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="radio-b"]')).boxShadow`)).not.toBe('none');
  }), 120000);

  check(`${target}: list tablists retain navigation and selected styling`, () => inTarget(target, async (s, read) => {
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="panel-saved"]')).visibility`)).toBe('hidden');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="tab-home"]')).borderTopLeftRadius`)).toBe('0px');
    await s.tap('tab-saved');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="panel-home"]')).visibility`)).toBe('hidden');
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="panel-saved"]')).visibility`)).toBe('visible');
    const colors = await read(`['tab-home', 'tab-saved'].map(id => getComputedStyle(document.querySelector('[data-testid="'+id+'"]')).color)`);
    expect(colors[0]).not.toBe(colors[1]);
    const sidebar = await read(`(() => { const tab = document.querySelector('[data-testid="tab-saved"]'); tab.closest('[role="tablist"]').parentElement.setAttribute('data-exact-sidebar', ''); return { height: parseFloat(getComputedStyle(tab).minHeight), expected: 2.25 * parseFloat(getComputedStyle(document.documentElement).fontSize) }; })()`);
    expect(sidebar.height).toBe(sidebar.expected);
  }), 120000);

  check(`${target}: grouped list cards retain corners, empty edges and press`, () => inTarget(target, async (s, read) => {
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="empty-section"] > div')).borderTopStyle`)).toBe('none');
    expect(await read(`parseFloat(getComputedStyle(document.querySelector('[data-testid="filled-section"] > div')).borderTopLeftRadius)`)).toBeGreaterThan(12);
    await s.tap('grouped-row', { down: true });
    expect(await read(`getComputedStyle(document.querySelector('[data-testid="grouped-row"]')).opacity`)).toBe('1');
    await s.pointer('up');
  }), 120000);

  check(`${target}: resizing rows keep their authored roles`, () => inTarget(target, async (s, read) => {
    await s.tap('resize');
    await s.tap('change-role');
    const during = await read(`(() => { const row = document.querySelector('[data-testid="resizing-row"]'), wrapper = row.parentElement; return { surface: wrapper.querySelectorAll('[data-exiting]').length, roles: [...wrapper.querySelectorAll('[role="listitem"]')].length + Number(wrapper.getAttribute('role') === 'listitem') }; })()`);
    expect(during).toEqual({ surface: 1, roles: 1 });
  }), 120000);

  check(`${target}: resizing rows exit when removed`, () => inTarget(target, async (s, read) => {
    await s.tap('resize');
    await s.tap('drop');
    expect(await read(`document.querySelector('[data-testid="resizing-row"]')?.hasAttribute('data-exiting')`)).toBe(true);
    await s.clock('settle');
    expect(await read(`!!document.querySelector('[data-testid="resizing"] [data-exiting]')`)).toBe(false);
  }), 120000);

  check(`${target}: direct and nested list paragraphs flow around shapes`, () => inTarget(target, async (s, read) => {
    const flowed = await read(`['direct-text', 'nested-text'].map(id => document.querySelector('[data-testid="'+id+'"]')?.querySelectorAll('[data-flow-fragment]').length > 0)`);
    const logs = await s.logs();
    expect(flowed, JSON.stringify(logs)).toEqual([true, true]);
    expect(JSON.stringify(logs)).not.toContain('auto height and is not flowed');
  }), 120000);
}

for (const renderer of ['js', 'rust']) test(`${renderer}: inactive list panels are hidden before JavaScript`, async () => {
  const server = await serverFor(builds.get(`list-structures-${renderer}`));
  const browser = await suiteBrowser('chromium');
  try {
    const page = await browser.newPage({ javaScriptEnabled: false });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    expect(await page.locator('[data-testid="panel-saved"]').evaluate(el => ({ hidden: getComputedStyle(el).visibility, inert: el.inert }))).toEqual({ hidden: 'hidden', inert: true });
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
}, 120000);

for (const renderer of ['js', 'rust']) test(`${renderer}: list modal cards keep their frame before JavaScript`, async () => {
  const server = await serverFor(builds.get(`list-modals-${renderer}`));
  const browser = await suiteBrowser('chromium');
  try {
    const page = await browser.newPage({ javaScriptEnabled: false });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    expect(await page.locator('[data-testid="modal-card"]').evaluate(el => getComputedStyle(el).display)).toBe('grid');
    expect(await page.locator('[data-testid="modal-title"]').evaluate(el => getComputedStyle(el.closest('header').parentElement).display)).toBe('contents');
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
}, 120000);
