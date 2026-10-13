// The compiled JS target in Chrome: an explicit greedy child overrides inherited balance,
// and clearing a computed placeholder colour restores the browser's own presentation.
import { test, expect, beforeAll, afterAll } from 'bun:test';
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

const root = resolve(new URL('../../..', import.meta.url).pathname);
const dir = mkdtempSync(resolve(tmpdir(), 'exact-textrows-'));
const dist = resolve(dir, 'dist');
let server;
beforeAll(async () => {
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Text rows', app: { id: 'com.example.textrows', name: 'Text rows' }, host: { web: {} } }));
  writeFileSync(resolve(dir, 'app.contract'), `component App
  state on = false
  action flip
    on = not on
  view
    column text-wrap="balance" font-family="Arial" font-size=16
      button press=flip testId="flip"
        text "Flip"
      text "Settle every thread in this section and keep the newest one open" width=180 text-wrap-style="auto" testId="literal"
      text "Settle every thread in this section and keep the newest one open" width=180 text-wrap-style=(on ? "balance" : "auto") testId="bound"
      input placeholder="Search" testId="hint"
        ::placeholder
          color=(on ? "#ff000080" : none)
      input placeholder="Search" testId="default"
`);
  const result = await new Promise((done, fail) => {
    const child = spawn(process.execPath, ['host/web-js/build.mjs', 'textrows', '--out', dist, '--render', 'none'], { cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    child.stdout.on('data', b => { log += b; });
    child.stderr.on('data', b => { log += b; });
    child.on('error', fail);
    child.on('close', code => done({ code, log }));
  });
  expect(result.code, result.log).toBe(0);
  server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
  } });
}, 180000);
afterAll(() => { server?.stop(true); rmSync(dir, { recursive: true, force: true }); });

test('literal and bound auto override balance; a bound placeholder colour clears', async () => {
  const browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}/`);
    await page.locator('[data-testid="bound"]').waitFor();
    const sample = () => page.evaluate(() => {
      const get = id => document.querySelector(`[data-testid="${id}"]`);
      const lines = el => {
        const rows = new Map(), walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        let node;
        while ((node = walker.nextNode())) for (let i = 0; i < node.length; i++) {
          const range = document.createRange(); range.setStart(node, i); range.setEnd(node, i + 1);
          const y = range.getBoundingClientRect().top;
          rows.set(y, (rows.get(y) ?? '') + node.data[i]);
        }
        return [...rows.values()].map(s => s.trim());
      };
      const literal = get('literal'), bound = get('bound');
      const oracle = literal.cloneNode(true); oracle.style.textWrapStyle = 'auto'; literal.after(oracle);
      const greedy = lines(oracle); oracle.remove();
      const placeholder = el => { const css = getComputedStyle(el, '::placeholder'); return [css.color, css.opacity]; };
      return { literal: lines(literal), bound: lines(bound), greedy,
        wrap: [getComputedStyle(literal).textWrapStyle, getComputedStyle(bound).textWrapStyle],
        hint: placeholder(get('hint')), original: placeholder(get('default')) };
    });
    for (const on of [false, true, false]) {
      const got = await sample();
      expect(got.wrap).toEqual(['auto', on ? 'balance' : 'auto']);
      expect(got.literal).toEqual(got.greedy);
      if (on) {
        expect(got.bound).not.toEqual(got.greedy);
        expect(got.hint).toEqual(['rgba(255, 0, 0, 0.5)', '1']);
      } else {
        expect(got.bound).toEqual(got.greedy);
        expect(got.hint).toEqual(got.original);
      }
      await page.locator('[data-testid="flip"]').click();
    }
  } finally { await browser.close(); }
}, 60000);
