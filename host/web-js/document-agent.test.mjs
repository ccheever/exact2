import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';

test('a second navigation root keeps its split list live while its detail supplies the head', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/head-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Head Agent Test', app: { id: 'test.heads', name: 'Head Agent Test' }, host: { web: {} } }));
    // The earlier covered copy must lose testId lookup to the visible split list's control.
    const source = readFileSync(new URL('./conformance/heads-split.contract', import.meta.url), 'utf8').replace('      main navigationKey=', `      main navigationKey="1002" navigationBack="unused" height=24
        column navigationKey="1001"
          button press=toggle testId="list-commit"
            text "Covered control"
        column navigationKey="1002"
          text "Another navigation root"
      main navigationKey=`);
    writeFileSync(resolve(dir, 'app.contract'), source);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'head-agent-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [1000, 900] });
    expect(await browser.carrier.evaluate('document.querySelectorAll("[navigationBack]").length')).toBe(2);
    await browser.tap('open-detail');
    await browser.tap('commit');
    let commits = 1;
    for (const width of [1000, 420, 1000]) {
      await browser.resize(width, 900);
      await browser.clock('settle');
      const wide = width >= 900, copies = (await browser.tree()).nodes.filter(n => n.props.testId === 'list-commit');
      expect(copies).toHaveLength(2);
      // A covered route is hidden too (LLP 1153 D8), and so is the split's list where the detail covers it.
      const [covered, list] = copies, observed = await browser.op({ op: 'check' });
      expect({ covered: [!!covered.inactive, !!covered.hidden], list: [!!list.inactive, !!list.hidden], checked: observed.items.some(item => item.id === list.id && item.control === 'button') })
        .toEqual({ covered: [true, true], list: [!wide, !wide], checked: wide });
      if (wide) {
        expect((await browser.tree('list-commit', { shallow: true })).nodes[0].id).toBe(list.id);
        await browser.tap('list-commit');
        commits++;
      }
      const state = await browser.state();
      expect(state.slots).toMatchObject({ commits, named: true });
      expect(state.head).toMatchObject({ title: 'Ada', description: 'Directory description', canonical: '/directory' });
      expect(await browser.carrier.evaluate('document.title')).toBe('Ada');
    }
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

test('a JavaScript-rendered direct detail selects its retained href at boot', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/split-document-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Split Document Agent Test', app: { id: 'test.split-document', name: 'Split Document Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  home "/" render=build
    alpha "/a" render=build

component SplitDocumentAgent
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%" position="relative" testId="navigation"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0 testId=\`route-\${e.name}\`
          if e.name == "home"
            list
              link href="/a" testId="row-alpha"
                text "Alpha"
          if e.name == "alpha"
            text "Alpha detail"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'split-document-agent-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path === '/a' ? 'a/index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: new URL('a', server.url).href, size: [1280, 844] });
    const state = await browser.state();
    expect(state.navigation).toMatchObject({ url: '/a', route: '1', stack: ['0', '1'] });
    expect(await browser.carrier.evaluate(`(() => ({
      list: document.querySelector('[data-testid="route-home"]').getAttribute('data-exact-split'),
      detail: document.querySelector('[data-testid="route-alpha"]').getAttribute('data-exact-split'),
      selected: document.querySelector('[data-testid="row-alpha"]').hasAttribute('data-exact-split-row'),
    }))()`)).toEqual({ list: 'list', detail: 'detail', selected: true });
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

// @ref LLP 1075.003 §9.14 — a page rendered ahead holds every tab panel, and the boot that adopts it builds
// every one over what was rendered, wiring each panel's presses; none waits on that page.
test('an adopted page builds every tab over the rendered ones', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/tabs-document-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Tabs Document Agent Test', app: { id: 'test.tabs-document', name: 'Tabs Document Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  tab a "/" render=build
  tab b "/b"

component TabsDocument
  state n = 0
  action pick(name: string)
    nav = select(nav, name)
  action bump
    n = n + 1
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 position="relative"
        for t in nav.tabs key=t.name
          column role="tabpanel" id=\`panel-\${t.name}\` testId=\`panel-\${t.name}\` position="absolute" inset=0
            for e in t.stack key=e.id
              column navigationKey=\`\${e.id}\` position="absolute" inset=0
                button press=bump testId=\`bump-\${e.name}\`
                  text \`\${e.name} \${n}\`
      row role="tablist"
        button role="tab" aria-controls="panel-a" aria-selected=(nav.tab == "a") press=pick("a") testId="tab-a"
          text "A"
        button role="tab" aria-controls="panel-b" aria-selected=(nav.tab == "b") press=pick("b") testId="tab-b"
          text "B"
`);
    const dist = resolve(dir, 'dist');
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'tabs-document-agent-test', '--out', dist, '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const html = readFileSync(resolve(dist, 'index.html'), 'utf8');
    expect(html).toContain('data-testid="bump-a"');
    expect(html).toContain('data-testid="bump-b"');
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      // `/b` is served the page rendered for `/`, which selects tab A.
      const file = Bun.file(resolve(dist, path === '/' || path === '/b' ? 'index.html' : path.slice(1)));
      // (the agent asks a page's /exact.json, a dev envelope this build doesn't write)
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [420, 900] });
    expect(await browser.carrier.evaluate('exact.journal.some(l => l.endsWith("adopted the document"))')).toBe(true);
    const panels = async () => Object.fromEntries((await browser.tree()).nodes.filter(n => n.props.accessibilityRole === 'tabpanel').map(n => [n.props.testId, !!n.unopened]));
    expect(await panels()).toEqual({ 'panel-a': false, 'panel-b': false });
    await browser.tap('bump-a');
    expect((await browser.state()).slots).toMatchObject({ n: 1 });
    await browser.tap('tab-b');
    await browser.tap('bump-b');
    expect((await browser.state()).slots).toMatchObject({ n: 2 });
    expect(await panels()).toEqual({ 'panel-a': false, 'panel-b': false });
    // A launch that selects B over the page rendered with A selected adopts it, B's panel included.
    await browser.close();
    browser = await open({ host: 'web', url: new URL('b', server.url).href, size: [420, 900] });
    expect(await browser.carrier.evaluate('exact.journal.some(l => l.endsWith("adopted the document"))')).toBe(true);
    expect((await browser.state()).navigation).toMatchObject({ url: '/b' });
    await browser.tap('bump-b');
    expect((await browser.state()).slots).toMatchObject({ n: 1 });
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 30000);

// LLP 1153 D8 — a flowed paragraph's runs are held off the page while its fragments paint (flow.js), so the tree
// gives them their paragraph's route flags: hidden under a pushed route, inactive under a sheet.
test('a flowed paragraph\'s runs keep their route\'s flags', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/flow-agent-test-'));
  let browser, server;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Flow Agent Test', app: { id: 'test.flow-agent', name: 'Flow Agent Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), `routes nav
  home "/"
    detail "/detail"
    sheet "/sheet"

component FlowAgent
  action open(url: string)
    nav = push(nav, url)
  action back
    nav = back(nav)
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0 navigationPresentation=(e.name == "sheet" ? "modal" : "auto")
          if e.name == "home"
            box display="block" position="relative" width=300 height=160
              text width=300 height=160 testId="prose"
                text "Words that flow around a square, " testId="run-a"
                text "and more words after them." testId="run-b"
              box position="absolute" left=0 top=0 width=60 height=60 wrap-flow="both"
            button press=open("/detail") testId="open-detail"
              text "Detail"
            button press=open("/sheet") testId="open-sheet"
              text "Sheet"
          else
            button press=back testId=\`back-\${e.name}\`
              text "Back"
`);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'flow-agent-test', '--out', resolve(dir, 'dist'), '--render', 'none'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
      const path = new URL(req.url).pathname;
      const file = Bun.file(resolve(dir, 'dist', path.includes('.') ? path.slice(1) : 'index.html'));
      return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
    } });
    browser = await open({ host: 'web', url: server.url.href, size: [420, 900] });
    // The runs are off the page once the flow paints its fragments, which it loads after first paint.
    const flowed = () => browser.carrier.evaluate('!!document.querySelector(\'[data-testid="prose"] > [data-flow-fragment]\') && !document.querySelector(\'[data-testid="run-a"]\')?.isConnected');
    for (let at = Date.now(); !await flowed(); await new Promise((r) => setTimeout(r, 50))) if (Date.now() - at > 20000) throw new Error('the paragraph was never flowed');
    const runs = async () => {
      const { nodes } = await browser.tree();
      return ['run-a', 'run-b'].map((id) => nodes.find((n) => n.props.testId === id)).map((n) => [!!n.inactive, !!n.hidden]);
    };
    expect(await runs()).toEqual([[false, false], [false, false]]);
    await browser.tap('open-detail');
    await browser.clock('settle');
    expect(await flowed()).toBe(true);
    expect(await runs()).toEqual([[true, true], [true, true]]);
    await browser.tap('back-detail');
    await browser.clock('settle');
    await browser.tap('open-sheet');
    await browser.clock('settle');
    expect(await flowed()).toBe(true);
    expect(await runs()).toEqual([[true, false], [true, false]]);
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 60000);

for (const render of ['none', 'js']) for (const literal of [false, true]) {
  test(`${render} navigation preserves ${literal ? 'literal' : 'bound'} inert on its tabs`, async () => {
    if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
    const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/inert-tabs-test-'));
    let browser, server;
    try {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Inert Tabs', app: { id: 'test.inert-tabs', name: 'Inert Tabs' }, host: { web: {} } }));
      writeFileSync(resolve(dir, 'app.contract'), `routes nav
  tab a "/" render=build
  tab b "/b"

component InertTabs
  state covered = false
  action cover
    covered = not covered
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=\`\${top(nav).id}\` width="100%" height="100%" display="flex" flex-direction="column"
      column flex=1 position="relative"
        for tab in nav.tabs key=tab.name
          column role="tabpanel" id=\`panel-\${tab.name}\` position="absolute" inset=0
            for entry in tab.stack key=entry.id
              column navigationKey=\`\${entry.id}\` position="absolute" inset=0
                text entry.name
      row role="tablist" appearance="none" inert=${literal ? 'true' : 'covered'} testId="tabs"
        button role="tab" aria-controls="panel-a" aria-selected=(nav.tab == "a") press=pick("a") testId="tab-a"
          text "A"
        button role="tab" aria-controls="panel-b" aria-selected=(nav.tab == "b") press=pick("b") testId="tab-b"
          text "B"
      button press=cover testId="cover"
        text "Toggle cover"
`);
      const dist = resolve(dir, 'dist');
      const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'inert-tabs-test', '--out', dist, '--render', render], {
        cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
      });
      const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
      expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
      server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(req) {
        const path = new URL(req.url).pathname;
        const file = Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1)));
        return await file.exists() ? new Response(file) : new Response('not found', { status: 404 });
      } });
      browser = await open({ host: 'web', url: server.url.href, size: [420, 900] });
      if (render === 'js') expect(await browser.carrier.evaluate('exact.journal.some(l => l.endsWith("adopted the document"))')).toBe(true);
      await browser.tap('cover');
      expect(await browser.carrier.evaluate('document.querySelector("[data-testid=tabs]").inert')).toBe(true);
      const { x, y } = await browser.carrier.evaluate('(()=>{const r=document.querySelector("[data-testid=tab-b]").getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()');
      await browser.carrier.call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 });
      await browser.carrier.call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1 });
      await browser.clock('settle');
      expect((await browser.state()).navigation.url).toBe('/');
      await expect(browser.tap('tab-b')).rejects.toThrow(/hidden or inert/);
      await browser.tap('cover');
      expect(await browser.carrier.evaluate('document.querySelector("[data-testid=tabs]").inert')).toBe(literal);
      if (!literal) {
        await browser.tap('tab-b');
        expect((await browser.state()).navigation.url).toBe('/b');
      }
    } finally {
      await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
    }
  }, 30000);
}
