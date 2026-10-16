import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../scripts/agent-launch.mjs';

test('relaunch reloads the browser document and keeps committed storage', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/commands.js') return new Response(Bun.file(new URL('./commands.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    return new Response(`<button id="restart">Restart</button><output id="state"></output>
      <script type="module">
        import { commands } from '/commands.js';
        const boots = Number(sessionStorage.getItem('boots') ?? 0) + 1;
        sessionStorage.setItem('boots', boots);
        document.querySelector('#state').textContent = boots + ':' + (localStorage.getItem('saved') ?? '');
        document.querySelector('#restart').onclick = () => {
          localStorage.setItem('saved', 'written before relaunch');
          commands(console.error).relaunch();
        };
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelector('#state').textContent === '1:');
    await Promise.all([page.waitForEvent('load'), page.locator('#restart').click()]);
    await page.waitForFunction(() => document.querySelector('#state').textContent === '2:written before relaunch');
    expect(await page.locator('#state').textContent()).toBe('2:written before relaunch');
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);

// A prompt's Return commits the field's `change` before it presses the first
// enabled action that is not destructive, so the action reads the new value
// and an action the change enables counts.
test("a prompt's Return commits its field, then presses its action", async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/commands.js') return new Response(Bun.file(new URL('./commands.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    return new Response(`<dialog role="alertdialog" open><input id="name">
        <button id="delete" data-exact-on="press" data-destructive="true">Delete</button>
        <button id="save" data-exact-on="press" disabled>Save</button></dialog><output id="log"></output>
      <script type="module">
        import '/commands.js';
        let draft = '';
        const save = document.querySelector('#save'), log = document.querySelector('#log');
        document.querySelector('#name').addEventListener('change', e => { draft = e.target.value; save.disabled = draft === ''; });
        document.querySelector('#delete').onclick = () => { log.textContent = 'deleted'; };
        save.onclick = () => { log.textContent = 'saved ' + draft; };
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.locator('#name').fill('Kick');
    await page.locator('#name').press('Enter');
    expect(await page.locator('#log').textContent()).toBe('saved Kick');
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);
