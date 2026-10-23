import { test, expect } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { createDocument } from './dom.js';
import { open } from '../../scripts/agent.mjs';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

async function renderedPages(source, check, files = {}, manifest = {}, renderer = 'js') {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/head-render-test-'));
  try {
    if (renderer === 'js') writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Head Render Test', app: { id: 'test.heads', name: 'Head Render Test' }, host: { web: {} }, ...manifest }));
    writeFileSync(resolve(dir, 'app.contract'), source);
    for (const [name, text] of Object.entries(files)) {
      mkdirSync(dirname(resolve(dir, name)), { recursive: true });
      writeFileSync(resolve(dir, name), text);
    }
    const app = renderer === 'rust' ? ['caltrain', '--contract', resolve(dir, 'app.contract')] : ['head-render-test'];
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', ...app, '--out', resolve(dir, 'dist'), '--render', renderer], {
      cwd: root, env: { ...process.env, ...(renderer === 'js' ? { EXACT_APP_DIR: dir } : {}) }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const html = path => readFileSync(resolve(dir, 'dist', path, 'index.html'), 'utf8');
    await check(path => {
      const page = html(path);
      return [/<title>([^<]*)<\/title>/.exec(page)?.[1], /<meta name="description" content="([^"]*)">/.exec(page)?.[1]];
    }, html, resolve(dir, 'dist'));
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

test('bare text styles keep their fills and inherited rows through server adoption', async () => {
  const source = 'routes nav\n  home "/" render=build\n\n' + readFileSync(new URL('./conformance/bare-text-style.contract', import.meta.url), 'utf8');
  for (const renderer of ['rust', 'js']) for (const weighted of [false, true]) await renderedPages(source.replace('state weighted = false', `state weighted = ${weighted}`), async (_, html, dist) => {
    const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    let suite, browser;
    try {
      const measure = `Object.fromEntries(['alone','bound','clear-above','clear-own','nested','relative','shorthand-child','heading-child','control-child','control-own','dynamic','dynamic-own','dynamic-shorthand-child','cleared-shorthand-child','removed-style-inherits','removed-style-fills'].map(id => {
        const e = document.querySelector('[data-testid="'+id+'"]'), s = getComputedStyle(e);
        return [id, [s.fontWeight, s.lineHeight]];
      }))`;
      suite = await suiteBrowser();
      const page = await suite.newPage({ javaScriptEnabled: false });
      await page.goto(server.url.href);
      const before = await page.evaluate(measure);
      const changing = on => ({ bound: on ? ['700', '40px'] : ['600', 'normal'],
        dynamic: [on ? '400' : '600', 'normal'], 'dynamic-own': on ? ['700', '40px'] : ['600', 'normal'],
        'dynamic-shorthand-child': on ? ['700', '40px'] : ['600', 'normal'], 'cleared-shorthand-child': [on ? '400' : '600', 'normal'],
        'removed-style-inherits': on ? ['700', '40px'] : ['300', '24px'], 'removed-style-fills': on ? ['600', 'normal'] : ['300', '24px'] });
      expect(before).toEqual({
        alone: ['600', 'normal'], 'clear-above': ['600', 'normal'],
        'clear-own': ['400', 'normal'], nested: ['400', 'normal'], relative: ['700', '40px'],
        'shorthand-child': ['600', 'normal'], 'heading-child': ['700', 'normal'], 'control-child': ['500', '20px'], 'control-own': ['500', '20px'], ...changing(weighted),
      });
      // Static fills must become classes, so adoption cannot leave a stale inline weight.
      const alone = html('').match(/<[^>]*data-testid="alone"[^>]*>/)?.[0];
      expect(alone).toContain('class=');
      expect(alone).not.toContain('style=');
      browser = await open({ host: 'web', url: server.url.href });
      expect(await browser.carrier.evaluate(`globalThis.exact.journal.some(l => l.endsWith('adopted the document'))`)).toBe(true);
      expect(await browser.carrier.evaluate(measure)).toEqual(before);
      await browser.tap('flip');
      expect(await browser.carrier.evaluate(measure)).toMatchObject(changing(!weighted));
      await browser.tap('flip');
      expect(await browser.carrier.evaluate(measure)).toEqual(before);
      if (weighted) await browser.tap('flip');
      expect(await browser.carrier.evaluate(`document.querySelector('[data-testid="bound"]').parentElement.style.cssText`)).not.toContain('--exact-said-');
    } finally {
      await browser?.close(); await suite?.close(); server.stop(true);
    }
  }, {}, {}, renderer);
}, 180000);

async function textStylePage(ids, check, weighted = false) {
  const source = 'routes nav\n  home "/" render=build\n\n' + readFileSync(new URL('./conformance/bare-text-style.contract', import.meta.url), 'utf8').replace('state weighted = false', `state weighted = ${weighted}`);
  for (const renderer of ['rust', 'js']) await renderedPages(source, async (_, _html, dist) => {
    const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    let suite, browser;
    const measure = `Object.fromEntries(${JSON.stringify(ids)}.map(id => {
      const e = document.querySelector('[data-testid="'+id+'"]'), s = getComputedStyle(e);
      return [id, [s.fontWeight, s.lineHeight]];
    }))`;
    try {
      suite = await suiteBrowser();
      const page = await suite.newPage({ javaScriptEnabled: false });
      await page.goto(server.url.href);
      const before = await page.evaluate(measure);
      browser = await open({ host: 'web', url: server.url.href });
      expect(await browser.carrier.evaluate(`globalThis.exact.journal.some(l => l.endsWith('adopted the document'))`)).toBe(true);
      await check(before, () => browser.carrier.evaluate(measure), id => browser.tap(id), browser);
    } finally { await browser?.close(); await suite?.close(); server.stop(true); }
  }, {}, {}, renderer);
}

test('keyed text styles track independent shorthand and longhand fields', async () => {
  await textStylePage(['keyed-child'], async (before, read, tap, browser) => {
    expect(before['keyed-child']).toEqual(['400', 'normal']);
    expect(await read()).toEqual(before);
    await tap('style-item');
    expect((await read())['keyed-child']).toEqual(['600', 'normal']);
    expect(await browser.carrier.evaluate(`['font-weight','line-height'].map(row => document.querySelector('[data-testid="keyed-child"]').parentElement.style.cssText.includes('--exact-said-'+row+':'))`)).toEqual([true, true]);
    await tap('bold-item');
    expect((await read())['keyed-child']).toEqual(['700', 'normal']);
    await tap('lead-item');
    expect((await read())['keyed-child']).toEqual(['700', '48px']);
    await tap('style-item');
    expect((await read())['keyed-child']).toEqual(['700', '48px']);
    await tap('bold-item');
    expect((await read())['keyed-child']).toEqual(['400', '48px']);
    await tap('lead-item');
    expect(await read()).toEqual(before);
    await tap('style-item');
    expect((await read())['keyed-child']).toEqual(['600', 'normal']);
    await tap('style-item');
    expect(await read()).toEqual(before);
    expect(await browser.carrier.evaluate(`document.querySelector('[data-testid="keyed-child"]').parentElement.style.cssText`)).not.toContain('--exact-said-');
  });
}, 60000);

test('an unnamed dynamic text style inherits its ancestor rows', async () => {
  await textStylePage(['no-style'], async (before, read, tap) => {
    expect(before['no-style']).toEqual(['600', '40px']);
    expect(await read()).toEqual(before);
    await tap('flip');
    expect((await read())['no-style']).toEqual(['700', '48px']);
    await tap('flip');
    expect(await read()).toEqual(before);
  });
}, 60000);

test('blank text rows clear a shorthand start through adoption', async () => {
  await textStylePage(['blank-child'], async (before, read, tap) => {
    expect(before['blank-child']).toEqual(['600', 'normal']);
    expect(await read()).toEqual(before);
    await tap('flip');
    expect((await read())['blank-child']).toEqual(['400', '40px']);
    await tap('flip');
    expect(await read()).toEqual(before);
  });
}, 60000);

test('select and date text styles preserve shell leading only under a said row', async () => {
  const ids = ['select-said','select-bound-said','date-said','time-bound-said','select-unsaid','select-bound-unsaid','date-unsaid','time-bound-unsaid'];
  for (const weighted of [false, true]) await textStylePage(ids, async (before, read, tap, browser) => {
    const expected = (small, coarse = false) => Object.fromEntries(ids.map(id => {
      const size = small && id.includes('bound') ? 11 : 16;
      return [id, ['400', id.endsWith('unsaid') ? 'normal' : `${size * (id.startsWith('select') ? (coarse ? 2.625 : 2.125) : 2)}px`]];
    }));
    expect(before).toEqual(expected(weighted));
    expect(await read()).toEqual(before);
    await tap('flip');
    expect(await read()).toEqual(expected(!weighted));
    await tap('flip');
    expect(await read()).toEqual(before);
    await browser.prefer({ pointer: 'coarse' });
    expect(await read()).toEqual(expected(weighted, true));
  }, weighted);
}, 60000);

test('removing a control style and its bound leading restores the shell', async () => {
  const expected = removed => ({ 'select-removed': ['400', removed ? '34px' : '24px'], 'datetime-removed': ['400', removed ? '32px' : '24px'] });
  for (const weighted of [false, true]) await textStylePage(['select-removed', 'datetime-removed'], async (before, read, tap) => {
    expect(before).toEqual(expected(weighted));
    expect(await read()).toEqual(before);
    await tap('flip');
    expect(await read()).toEqual(expected(!weighted));
    await tap('flip');
    expect(await read()).toEqual(before);
  }, weighted);
}, 60000);

test('JavaScript-rendered strings use the document language with an English translation available', async () => {
  await renderedPages(`routes nav
  home "/" render=build

component Greeting
  view
    main
      head title=t("hello")
      text t("hello")
`, (page, html) => {
    expect(page('')[0]).toBe('Bonjour');
    expect(html('')).toContain('<html lang="fr" dir="ltr">');
    expect(html('')).toContain('>Bonjour<');
  }, { 'strings/fr.json': '{"hello":"Bonjour"}', 'strings/en.json': '{"hello":"Hello"}' }, { lang: 'fr', strings: { base: 'fr' } });
}, 30000);

test('the render context reports UTC independently of the machine zone', async () => {
  const dir = mkdtempSync(resolve(import.meta.dir, '../../target/place-render-test-'));
  try {
    mkdirSync(resolve(dir, '.gen'));
    writeFileSync(resolve(dir, 'shell.html'), '<html><head><title>Place</title><meta name="viewport" content="width=device-width">\n</head><body><div id="exact-root"></div><script type="module" src="./app.js"></script></body></html>');
    const entry = resolve(dir, 'probe.js');
    writeFileSync(entry, `import { placeReporter } from ${JSON.stringify(resolve(import.meta.dir, '../web/navigation.js'))};
globalThis.__start = () => ({ activate: 'eager' });
globalThis.__render = () => {
  const [language, zone] = placeReporter(new URLSearchParams())().split('\\0');
  const explicit = new Intl.DateTimeFormat('en', { timeZone: 'Asia/Tokyo' }).resolvedOptions().timeZone;
  const inherited = new Intl.DateTimeFormat('en', Object.create({ hour: '2-digit', hourCycle: 'h23' })).resolvedOptions();
  const caught = [Intl.NumberFormat, Intl.DateTimeFormat, Intl.Collator].map(C => {
    try { new C('bad_locale'); } catch (error) { return error instanceof RangeError; }
  });
  const counts = [Intl.NumberFormat, Intl.DateTimeFormat, Intl.Collator].map(C => {
    C.renderCount = (C.renderCount ?? 0) + 1;
    C.prototype.renderCount = (C.prototype.renderCount ?? 0) + 1;
    return C.renderCount + ':' + C.prototype.renderCount;
  });
  return { root: '', title: [language, zone, explicit, inherited.timeZone, inherited.hour, typeof Intl.DateTimeFormat.supportedLocalesOf].join('/'), description: caught.join(',') + '/' + counts.join(','), time: 0, answers: '[]', pending: [] };
};`);
    const bundle = await Bun.build({ entrypoints: [entry], target: 'browser', format: 'iife' });
    expect(bundle.success).toBe(true);
    writeFileSync(resolve(dir, '.gen/server.js'), await bundle.outputs[0].text());
    const render = Bun.spawn([process.execPath, resolve(import.meta.dir, 'render.mjs'), dir, '/', '/'], {
      env: { ...process.env, TZ: 'Pacific/Honolulu' }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, html, stderr] = await Promise.all([render.exited, new Response(render.stdout).text(), new Response(render.stderr).text()]);
    expect({ status, stderr }).toEqual({ status: 0, stderr: '' });
    expect(html).toContain('<title>en-US/UTC/Asia/Tokyo/UTC/2-digit/function</title>');
    expect(html).toContain('<html lang="en-US" dir="ltr">');
    expect([...html.matchAll(/<meta name="description" content="([^"]*)">/g)].map(m => m[1]))
      .toEqual(['true,true,true/1:1,1:1,1:1', 'true,true,true/1:1,1:1,1:1']);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 30000);

test('rendered TypeScript sources catch Intl errors in their own realm and keep prototypes local', async () => {
  await renderedPages(`routes nav
  home "/" render=build
  next "/next" render=build

component IntlFacts
  resource facts = facts() as shape string
  view
    main
      head title=facts
      text facts
`, page => {
    expect([page('')[0], page('next')[0]]).toEqual(['true,true,true/1', 'true,true,true/1']);
  }, { 'app.ts': `import type { Answer } from './app.contract.d.ts';
export const appId = 'test.heads', grants = '';
export const answer: Answer = () => {
  const caught = [() => new Intl.NumberFormat('bad_locale'), () => new Intl.DateTimeFormat('bad_locale'), () => new Intl.Collator('bad_locale')].map(make => {
    try { make(); } catch (error) { return error instanceof RangeError; }
    return false;
  });
  const count = Number(Reflect.get(Intl.NumberFormat.prototype, 'renderCount') ?? 0) + 1;
  Reflect.set(Intl.NumberFormat.prototype, 'renderCount', count);
  return caught.join(',') + '/' + count;
};` });
}, 30000);

test('JavaScript-rendered page uses server facts without a reader’s device', async () => {
  await renderedPages(`routes nav
  home "/" render=build

shape Page
  visibilityState: string
  onLine: bool
  canShare: bool
  canShareFiles: bool
  canSharePictures: bool
  canOpenFiles: bool
  hasFocus: bool
  navigationType: string

component PageFacts
  resource page = exactPage() as shape Page
  view
    main
      head title=\`\${page.visibilityState}/\${page.onLine}/\${page.canShare}/\${page.canShareFiles}/\${page.canSharePictures}/\${page.canOpenFiles}/\${page.hasFocus}/\${page.navigationType}\`
      text "Page facts"
`, page => {
    expect(page('')[0]).toBe('visible/true/false/false/false/false/true/navigate');
  });
}, 30000);

test('JavaScript-rendered viewport uses default media facts without a reader’s device', async () => {
  await renderedPages(`routes nav
  home "/" render=build

shape Viewport
  width: number
  height: number
  prefersReducedMotion: bool
  prefersReducedTransparency: bool
  prefersContrast: string
  prefersColorScheme: string
  pointer: string
  hover: string
  colorGamut: string
  dynamicRange: string
  displayMode: string

component MediaFacts
  resource viewport = exactViewport() as shape Viewport
  view
    main
      head title=\`\${viewport.width}x\${viewport.height}/\${viewport.prefersReducedMotion}/\${viewport.prefersReducedTransparency}/\${viewport.prefersContrast}/\${viewport.prefersColorScheme}/\${viewport.pointer}/\${viewport.hover}/\${viewport.colorGamut}/\${viewport.dynamicRange}/\${viewport.displayMode}\`
      text "Viewport facts"
`, page => {
    expect(page('')[0]).toBe('390x844/false/false/no-preference/light/fine/hover/srgb/standard/standalone');
  });
}, 30000);

// The runtime answers `exactTransport` and `exactMIDI` itself from the boot's
// first settlement, as the runner does; asked of a TypeScript data module,
// which answers neither, they refused the boot. Two `midimessage` listeners
// are put in document order as the boot commit stands (midi.js `listeners`).
test('JavaScript-rendered boot answers the transport and MIDI beside a TypeScript data module', async () => {
  await renderedPages(`routes nav
  home "/" render=build

shape Transport
  state: string
  bpm: number

shape MIDI
  state: string

component ReservedBoot
  state takes = 0 persist
  resource transport = exactTransport() as shape Transport
  resource midi = exactMIDI() as shape MIDI
  action start
    takes = takes + 1
  action hear(e: MIDIMessageEvent)
    takes = takes + 1
  view
    main
      head title=\`\${transport.state} \${transport.bpm} \${midi.state} \${takes}\`
      button "Start" press=start
      column midimessage=hear midiinputs=["keys"]
        text "Keys"
      column midimessage=hear midiinputs=["keys", "pads"]
        text "Pads"
`, page => {
    expect(page('')[0]).toBe('stopped 0 denied 0');
  }, { 'app.ts': "import type { Answer } from './app.contract.d.ts';\nexport const appId = 'test.heads', grants = '';\nexport const answer: Answer = name => { throw new Error(`no source ${name}`); };\n" });
}, 30000);

test('JavaScript-rendered routes build with stopped and playing sequences and declared sounds', async () => {
  await renderedPages(`sound "assets/kick.wav"

routes nav
  home "/" render=build
  playing "/playing" render=build

shape Step
  beat: number
  src: string
  gain: number
  group: string
  note: number
  velocity: number
  channel: number
  duration: number
  haptic: string

shape Track
  beats: number
  loops: number
  offset: number
  output: string
  steps: Step[]

component SequenceRender
  sequence groove when top(nav).name == "playing"
    play([Track(beats=4, loops=0, offset=0, output="", steps=[Step(beat=0, src="assets/kick.wav", gain=1, group="", note=-1, velocity=100, channel=0, duration=0.25, haptic="")])], bpm=120)
  view
    main
      head title=\`\${top(nav).name} sequence\`
      text "Sequence ready"
`, (page, html) => {
    expect(page('')[0]).toBe('home sequence');
    expect(page('playing')[0]).toBe('playing sequence');
    expect(html('playing')).toContain('>Sequence ready<');
  }, { 'assets/kick.wav': readFileSync(new URL('./conformance/transport/assets/kick.wav', import.meta.url)) });
}, 30000);

test('JavaScript-rendered pages exclude a covered route’s deeper head', async () => {
  await renderedPages(`routes nav
  tab home "/" render=build
    detail "/detail" render=build
    plain "/plain" render=build

component Heads
  view
    column
      head title="Root" description="Root description"
      main navigationKey=\`\${top(nav).id}\` navigationBack="back"
        for e in stack(nav) key=e.id
          column navigationKey=\`\${e.id}\`
            if e.name == "home"
              column
                head title="Home" description="Home description"
            if e.name == "detail"
              head title="Detail" description="Detail description"
            text e.name
`, page => {
    expect(page('')).toEqual(['Home', 'Home description']);
    expect(page('detail')).toEqual(['Detail', 'Detail description']);
    expect(page('plain')).toEqual(['Root', 'Root description']);
  });
}, 30000);

test('JavaScript-rendered navigation keeps split facts without browser geometry', async () => {
  await renderedPages(`routes nav
  home "/" render=build activate=interaction
    detail "/detail" render=build activate=interaction

component SplitDocument
  action open
    nav = push(nav, path("detail"))
  action back
    nav = back(nav)
  view
    main navigationKey=\`\${top(nav).id}\` navigationBack="back" position="relative"
      for e in stack(nav) key=e.id
        column navigationKey=\`\${e.id}\` position="absolute" inset=0
          if e.name == "home"
            list
              button "Open" press=open
          if e.name == "detail"
            button "Back" id="back" press=back
`, (_, html) => {
    for (const path of ['', 'detail']) {
      const page = html(path);
      expect(page).toContain('data-exact-list');
      expect(page).toContain('data-exact-router-press="true"');
      expect(page).not.toMatch(/\sdata-exact-split="/);
    }
  });
}, 30000);

test('JavaScript-rendered pages exclude the other retained tabs’ heads', async () => {
  const source = readFileSync(new URL('./conformance/heads-tabs.contract', import.meta.url), 'utf8')
    .replace('tab home "/"', 'tab home "/" render=build')
    .replace('tab saved "/saved"', 'tab saved "/saved" render=build');
  await renderedPages(source, page => {
    expect(page('')).toEqual(['Home', 'Base description']);
    expect(page('saved')).toEqual(['Saved', 'Saved description']);
  });
}, 30000);

// LLP 1115.000 D4.15: the page marks a route's bar (`data-exact-bar`) as the glue will, so its item's symbol is the
// bar's size, 22/17 of the 16 px body in its 1.2em box, over the 13 px written on it, before the page adopts and after:
// no jump. A header whose heading sits in a menu's button (`popovertarget`) is no bar, before or after, and its symbol
// keeps the 13 px in a native face's 1.5em box.
test('JavaScript-rendered pages mark the bar, whose symbols are the bar\'s size before the page adopts and after', async () => {
  await renderedPages(`routes nav
  home "/" render=build

component Bars
  state added = 0
  action add
    added = added + 1
  view
    column
      main navigationKey=\`\${top(nav).id}\` navigationBack="back"
        for e in stack(nav) key=e.id
          column navigationKey=\`\${e.id}\`
            header display="flex" align-items="center"
              text "Inbox" role="heading" aria-level=1
              button press=add testId="add" aria-label="Add"
                image "symbol:add" font-size=13
      main navigationKey="menu" navigationBack="back"
        column navigationKey="menu"
          header display="flex" align-items="center"
            button popovertarget="menu-items"
              text "Menu" role="heading" aria-level=1
            button press=add testId="menu-add" aria-label="Add"
              image "symbol:add" font-size=13
          column popover="auto" id="menu-items"
            text "Item"
`, async (_page, _html, dist) => {
    const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    let suite, browser;
    try {
      // Each header's mark, and its Add symbol's width to within 0.06 px of the bar's size or of the written one.
      const measure = `[...document.querySelectorAll('[navigationkey] > header')].map(h => {
        const width = h.querySelector('[data-testid$="add"] img').getBoundingClientRect().width;
        return [h.hasAttribute('data-exact-bar'), Math.abs(width - 16 * 22 / 17 * 1.2) <= 0.06 ? 'bar' : Math.abs(width - 13 * 1.5) <= 0.06 ? 'written' : width];
      })`;
      suite = await suiteBrowser('chromium', { timeout: 10000 });
      const page = await suite.newPage({ javaScriptEnabled: false });
      await page.goto(server.url.href);
      expect(await page.evaluate(measure)).toEqual([[true, 'bar'], [false, 'written']]);
      browser = await open({ host: 'web', url: server.url.href });
      expect(await browser.carrier.evaluate(`globalThis.exact.journal.some(l => l.endsWith('adopted the document'))`)).toBe(true);
      expect(await browser.carrier.evaluate(measure)).toEqual([[true, 'bar'], [false, 'written']]);
    } finally {
      await browser?.close(); await suite?.close(); server.stop(true);
    }
  });
}, 180000);

// A page rendered ahead of time, by either renderer, works before its runtime runs as it will after: a wheel over the
// content scrolls the route its URL selects, every other tab hidden and inert as the projection leaves it (LLP 1038
// D6), while the document and the tab bar keep their place and the page cannot bounce (LLP 1115.000 D4.12). It adopts
// without moving.
test('pages rendered ahead of time scroll the selected route before they adopt, and adopt without moving', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/prerender-scroll-test-')), servers = {};
  try {
    const source = resolve(dir, 'app.contract');
    writeFileSync(source, `routes nav
  tab first "/" render=build
  tab second "/second" render=build

component Tabs
  derive current = top(nav)
  action pick(name: string)
    nav = select(nav, name)
  view
    main navigationKey=\`\${current.id}\` navigationBack="back" height="100%" display="flex" flex-direction="column"
      column flex=1 min-height=0 position="relative"
        for t in nav.tabs key=t.name
          column role="tabpanel" id=\`panel-\${t.name}\` position="absolute" inset=0
            for e in t.stack key=e.id
              column navigationKey=\`\${e.id}\` position="absolute" inset=0 display="flex" flex-direction="column"
                header padding="8px 16px"
                  text e.name role="heading" aria-level=1
                scroll testId=\`content-\${e.name}\` flex=1 min-height=0
                  for i in range(30) key=i
                    text \`\${e.name} \${i}\` padding=16
      row role="tablist"
        button role="tab" aria-controls="panel-first" aria-selected=(nav.tab == "first") press=pick("first") flex=1
          text "First"
        button role="tab" aria-controls="panel-second" aria-selected=(nav.tab == "second") press=pick("second") flex=1
          text "Second"
`);
    for (const how of ['js', 'rust']) {
      const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'caltrain', '--contract', source, '--out', resolve(dir, how), '--render', how], {
        cwd: root, env: process.env, stdout: 'pipe', stderr: 'pipe',
      });
      const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
      expect({ how, status, error: status ? stdout + stderr : '' }).toEqual({ how, status: 0, error: '' });
      servers[how] = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
        const path = new URL(req.url).pathname;
        return new Response(Bun.file(resolve(dir, how, /\.\w+$/.test(path) ? `.${path}` : `.${path}/index.html`)));
      } });
    }
    // Each panel, route, bar and scroller: its box, whether it is seen, and whether it is inert.
    const layout = () => [...document.querySelectorAll('[role=tabpanel], [navigationkey], header, [data-scroll], [role=tablist]')].map(e =>
      [e.localName, e.id || e.getAttribute('data-testid'), ...Object.values(e.getBoundingClientRect().toJSON()).slice(0, 4), getComputedStyle(e).visibility, e.inert]);
    // Whether the tab's scroller has scrolled, the document's offset, and the tab bar's box.
    const scroll = tab => [document.querySelector(`[data-testid="content-${tab}"]`).scrollTop > 0, document.scrollingElement.scrollTop,
      Object.values(document.querySelector('[role=tablist]').getBoundingClientRect().toJSON()).slice(0, 4)];
    for (const engine of ['chromium', 'firefox', 'webkit']) {
      const suite = await suiteBrowser(engine, { timeout: 10000 });
      try {
        for (const how of ['js', 'rust']) for (const [path, tab] of [['', 'first'], ['/second', 'second']]) {
          const url = `${servers[how].url.href}${path.slice(1)}`, at = [engine, how, tab];
          const before = await (await suite.newContext({ viewport: { width: 390, height: 844 }, javaScriptEnabled: false })).newPage();
          await before.goto(url);
          expect([...at, await before.evaluate(() => [document.documentElement, document.body].map(e => getComputedStyle(e).overscrollBehaviorY))]).toEqual([...at, ['none', 'none']]);
          const built = await before.evaluate(layout), [, , bar] = await before.evaluate(scroll, tab);
          const content = await before.locator(`[data-testid="content-${tab}"]`).boundingBox();
          await before.mouse.move(content.x + content.width / 2, content.y + content.height / 2);
          await before.mouse.wheel(0, 300);
          // Poll from the driver because JavaScript-disabled pages do not run the page's animation-frame poller
          for (let i = 0; i < 50 && !(await before.evaluate(scroll, tab))[0]; i++) {
            await before.waitForTimeout(50);
          }
          expect([...at, ...(await before.evaluate(scroll, tab))]).toEqual([...at, true, 0, bar]);
          const after = await (await suite.newContext({ viewport: { width: 390, height: 844 } })).newPage();
          await after.goto(url);
          await after.waitForFunction(() => globalThis.exact?.journal?.some(l => l.endsWith('adopted the document')), null, { timeout: 15000 });
          await after.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
          expect([...at, await after.evaluate(layout)]).toEqual([...at, built]);
        }
      } finally { await suite.close(); }
    }
  } finally {
    for (const server of Object.values(servers)) server.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 240000);

test('JavaScript prerender keeps every sheet and its underlying page visible through adoption', async () => {
  await renderedPages(`routes nav
  home "/" render=build

component Sheets
  view
    main navigationKey="top" navigationBack="back" height="100%"
      column navigationKey="old" id="old" position="absolute" inset=0
        text "Earlier page"
      column navigationKey="base" id="base" position="absolute" inset=0
        text "Base page"
      column navigationKey="lower" id="lower" navigationPresentation="modal" height=500
        text "Lower sheet"
      column navigationKey="top" id="top" navigationPresentation="modal" height=200
        text "Top sheet"
`, async (_, _html, dist) => {
    const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    const visibility = () => ['old', 'base', 'lower', 'top'].map(id => {
      const node = document.getElementById(id);
      return [id, getComputedStyle(node).visibility, node.inert];
    });
    const expected = [['old', 'hidden', true], ['base', 'visible', true], ['lower', 'visible', true], ['top', 'visible', false]];
    try {
      for (const engine of ['chromium', 'firefox', 'webkit']) {
        const suite = await suiteBrowser(engine);
        try {
          const before = await suite.newPage({ javaScriptEnabled: false });
          await before.goto(server.url.href);
          expect([engine, 'prerender', await before.evaluate(visibility)]).toEqual([engine, 'prerender', expected]);
          const after = await suite.newPage();
          await after.goto(server.url.href);
          await after.waitForFunction(() => globalThis.exact?.journal?.some(line => line.endsWith('adopted the document')));
          expect([engine, 'adopted', await after.evaluate(visibility)]).toEqual([engine, 'adopted', expected]);
        } finally { await suite.close(); }
      }
    } finally { server.stop(true); }
  });
}, 60000);

test('both document renderers preserve browser scrolling before startup', async () => {
  for (const how of ['js', 'rust']) {
    await renderedPages(`routes nav
  home "/" render=build

component Page
  view
    scroll document height="100%"
      text "Document" height=1600
`, async (_, html, dist) => {
      expect(html('')).toMatch(/<html[^>]* data-scrolldocument/);
      if (how === 'js') {
        const { renderer } = await import('./render.mjs');
        const page = await (await renderer(dist).begin('/')).finish();
        expect(page.rest.startsWith('<script>document.documentElement.setAttribute("data-scrolldocument","")</script>')).toBe(true);
      }
      const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: () => new Response(Bun.file(resolve(dist, 'index.html'))) });
      try {
        for (const engine of ['chromium', 'firefox', 'webkit']) {
          const browser = await suiteBrowser(engine);
          try {
            const page = await browser.newPage({ viewport: { width: 390, height: 844 }, javaScriptEnabled: false });
            await page.goto(server.url.href);
            expect([how, engine, await page.evaluate(() => [document.documentElement, document.body].map(e => getComputedStyle(e).overscrollBehaviorY))]).toEqual([how, engine, ['auto', 'auto']]);
            await page.mouse.move(195, 400);
            await page.mouse.wheel(0, 300);
            const scrolled = () => page.evaluate(() => document.scrollingElement.scrollTop > 0);
            for (let i = 0; i < 50 && !await scrolled(); i++) {
              await page.waitForTimeout(50);
            }
            expect(await scrolled()).toBe(true);
          } finally { await browser.close(); }
        }
      } finally { server.stop(true); }
    }, {}, {}, how);
  }
}, 180000);

// The render's DOM orders nodes as DOM §4.4 does (midi.js sorts its listeners by it).
test('the render DOM compares document positions, across trees one way each', () => {
  const doc = createDocument(), root = doc.getElementById('exact-root');
  const a = doc.createElement('div'), b = doc.createElement('div');
  root.append(a, b);
  a.textContent = 'text';
  expect([a.compareDocumentPosition(b), b.compareDocumentPosition(a)]).toEqual([4, 2]);
  expect([a.compareDocumentPosition(a.firstChild), a.firstChild.compareDocumentPosition(a)]).toEqual([16 | 4, 8 | 2]);
  const loose = doc.createElement('p'), other = createDocument().getElementById('exact-root');
  for (const [x, y] of [[a, loose], [loose, other], [a, other]]) {
    const there = x.compareDocumentPosition(y), back = y.compareDocumentPosition(x);
    expect([there & 1, there & 32, back & 1, back & 32]).toEqual([1, 32, 1, 32]);
    expect([there & 6, back & 6].sort()).toEqual([2, 4]);
    expect(x.compareDocumentPosition(y)).toBe(there);
  }
});

test('Rust-rendered spinners follow parent display after adoption without a position binding', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/progress-document-agent-test-'));
  let browser, server;
  try {
    const source = resolve(dir, 'app.contract'), dist = resolve(dir, 'dist');
    writeFileSync(source, `routes nav
  home "/" render=build

component ProgressDocument
  state grid = false
  action toggle
    grid = not grid
  view
    column
      button "Toggle" testId="toggle" press=toggle width=100 height=24
      box display=(grid ? "grid" : "block") width=240 height=40
        progress testId="spinner"
      box display=(grid ? "grid" : "block") justify-items="center" width=240 height=40
        progress testId="sized" width=20 height=20
`);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'caltrain', '--contract', source, '--out', dist, '--render', 'rust'], {
      cwd: root, env: process.env, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    expect(readFileSync(resolve(dist, 'index.html'), 'utf8')).toContain('justify-self:start;');
    server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
      const path = new URL(req.url).pathname;
      return new Response(Bun.file(resolve(dist, path === '/' ? 'index.html' : path.slice(1))));
    } });
    browser = await open({ host: 'web', url: server.url.href });
    expect(await browser.carrier.evaluate(`globalThis.exact.journal.some(l => l.endsWith('adopted the document'))`)).toBe(true);
    const frames = () => browser.carrier.evaluate(`['spinner','sized'].map(id => {
      const e = document.querySelector('[data-testid="'+id+'"]'), r = e.getBoundingClientRect(), p = e.parentElement.getBoundingClientRect();
      return { x: r.x - p.x, width: r.width };
    })`);
    expect(await frames()).toEqual([{ x: 0, width: 20 }, { x: 0, width: 20 }]);
    await browser.tap('toggle');
    expect(await frames()).toEqual([{ x: 0, width: 240 }, { x: 110, width: 20 }]);
    await browser.tap('toggle');
    expect(await frames()).toEqual([{ x: 0, width: 20 }, { x: 0, width: 20 }]);
  } finally {
    await browser?.close(); server?.stop(true); rmSync(dir, { recursive: true, force: true });
  }
}, 180000);
