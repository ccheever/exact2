import { test, expect } from 'bun:test';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

test('relaunch reloads the browser document and keeps committed storage', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/commands.js') return new Response(Bun.file(new URL('./commands.js', import.meta.url)));
    if (path === '/frame-messages.js') return new Response(Bun.file(new URL('../web/frame-messages.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    if (path === '/prompt-glue.js') return new Response(Bun.file(new URL('../web/prompt-glue.js', import.meta.url)));
    if (path === '/alert-message.js') return new Response(Bun.file(new URL('../web/alert-message.js', import.meta.url)));
    if (path === '/menu-rows.js') return new Response(Bun.file(new URL('../web/menu-rows.js', import.meta.url)));
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
    browser = await suiteBrowser();
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
// action that is not destructive, so the action reads the new value and an
// action the change enables counts; while that action is disabled Return
// presses nothing, never the next one. A field with its own `submit` (the
// runtime marks its Enter `$submit`) keeps Return, and nothing is pressed.
test("a prompt's Return commits its field, then presses its action", async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/commands.js') return new Response(Bun.file(new URL('./commands.js', import.meta.url)));
    if (path === '/frame-messages.js') return new Response(Bun.file(new URL('../web/frame-messages.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('../web/navigation.js', import.meta.url)));
    if (path === '/prompt-glue.js') return new Response(Bun.file(new URL('../web/prompt-glue.js', import.meta.url)));
    if (path === '/alert-message.js') return new Response(Bun.file(new URL('../web/alert-message.js', import.meta.url)));
    if (path === '/menu-rows.js') return new Response(Bun.file(new URL('../web/menu-rows.js', import.meta.url)));
    return new Response(`<dialog role="alertdialog" open><input id="name">
        <button id="delete" data-exact-on="press" data-destructive="true">Delete</button>
        <button id="save" data-exact-on="press" disabled>Save</button>
        <button id="suggest" data-exact-on="press">Use Suggestion</button></dialog><output id="log"></output>
      <script type="module">
        import '/commands.js';
        let draft = '';
        const save = document.querySelector('#save'), log = document.querySelector('#log');
        document.querySelector('#name').addEventListener('change', e => { draft = e.target.value; save.disabled = draft === ''; });
        document.querySelector('#delete').onclick = () => { log.textContent = 'deleted'; };
        save.onclick = () => { log.textContent = 'saved ' + draft; };
        document.querySelector('#suggest').onclick = () => { log.textContent = 'suggested'; };
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    // Return queues the action on a zero-delay timer. Read after a timer
    // queued behind it, including when checking that no action ran.
    const log = () => page.evaluate(() => new Promise(resolve =>
      setTimeout(() => resolve(document.querySelector('#log').textContent))));
    await page.locator('#name').press('Enter');
    expect(await log()).toBe('');
    await page.locator('#name').fill('Kick');
    await page.locator('#name').press('Enter');
    expect(await log()).toBe('saved Kick');
    await page.evaluate(() => {
      const log = document.querySelector('#log');
      log.textContent = '';
      document.querySelector('#name').addEventListener('keydown', e => { if (e.key === 'Enter') { e.$submit = true; log.textContent = 'submitted'; } });
    });
    await page.locator('#name').press('Enter');
    expect(await log()).toBe('submitted');
    // A `change` that closes the prompt: its Save, still mounted, is not pressed.
    await page.evaluate(() => {
      const dialog = document.querySelector('dialog'), name = document.querySelector('#name');
      name.replaceWith(name.cloneNode());
      document.querySelector('#log').textContent = '';
      document.querySelector('#name').addEventListener('change', () => dialog.close());
    });
    await page.locator('#name').fill('Snare');
    await page.locator('#name').press('Enter');
    expect(await log()).toBe('');
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);

// `openURL` hands the browser a scheme app.json's `open_url` declares (`exact.openSchemes`), whatever the case it is
// written in; an undeclared scheme, and one that runs or reads content even if declared, is refused; under the agent
// a declared one is journaled, not opened.
test('openURL opens a declared scheme, refuses the rest, and journals under the agent', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    for (const [name, file] of [['commands.js', './commands.js'], ['frame-messages.js', '../web/frame-messages.js'], ['navigation.js', '../web/navigation.js'], ['prompt-glue.js', '../web/prompt-glue.js'], ['alert-message.js', '../web/alert-message.js'], ['menu-rows.js', '../web/menu-rows.js']]) {
      if (path === `/${name}`) return new Response(Bun.file(new URL(file, import.meta.url)));
    }
    return new Response(`<script type="module">
        import { commands } from '/commands.js';
        globalThis.exact = { openSchemes: ['vscode', 'javascript'] };
        const opened = [], said = [];
        window.open = (url) => { opened.push(url); };
        const page = commands(line => said.push(line)), agent = commands(line => said.push(line), () => true);
        for (const url of ['vscode://vscode-remote/ssh-remote+devbox/p', 'VSCode://file/a', 'zed://ssh/devbox/p', 'javascript:alert(1)']) page.openURL(url);
        agent.openURL('vscode://file/b');
        globalThis.result = { opened, said };
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => globalThis.result);
    const { opened, said } = await page.evaluate(() => globalThis.result);
    expect(opened).toEqual(['vscode://vscode-remote/ssh-remote+devbox/p', 'vscode://file/a']);
    expect(said).toHaveLength(3);
    expect(said[0]).toStartWith('openURL refused: "zed://ssh/devbox/p"');
    expect(said[1]).toStartWith('openURL refused: "javascript:alert(1)"');
    expect(said[2]).toBe('openURL vscode: vscode://file/b (journaled, not opened under the agent)');
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);

// A page cannot open the browser's developer tools (LLP 1020.002 D11): `openDevTools(id)` is refused in the journal.
test('openDevTools is refused on the web', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    for (const [name, file] of [['commands.js', './commands.js'], ['frame-messages.js', '../web/frame-messages.js'], ['navigation.js', '../web/navigation.js'], ['prompt-glue.js', '../web/prompt-glue.js'], ['alert-message.js', '../web/alert-message.js'], ['menu-rows.js', '../web/menu-rows.js']]) {
      if (path === `/${name}`) return new Response(Bun.file(new URL(file, import.meta.url)));
    }
    return new Response(`<iframe id="tab"></iframe><script type="module">
        import { commands } from '/commands.js';
        const said = [];
        commands(line => said.push(line)).openDevTools('tab');
        globalThis.result = said;
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => globalThis.result);
    expect(await page.evaluate(() => globalThis.result)).toEqual([`openDevTools("tab") refused: a page cannot open the browser's developer tools; open them from the browser's menu`]);
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);
