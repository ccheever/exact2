import { test, expect } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { webCompiler } from './module.mjs';
import { createDocument } from './dom.js';

async function renderedPages(source, check, files = {}) {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..'), dir = mkdtempSync(resolve(root, 'target/head-render-test-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Head Render Test', app: { id: 'test.heads', name: 'Head Render Test' }, host: { web: {} } }));
    writeFileSync(resolve(dir, 'app.contract'), source);
    for (const [name, text] of Object.entries(files)) writeFileSync(resolve(dir, name), text);
    const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'head-render-test', '--out', resolve(dir, 'dist'), '--render', 'js'], {
      cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
    });
    const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
    expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
    const html = path => readFileSync(resolve(dir, 'dist', path, 'index.html'), 'utf8');
    check(path => {
      const page = html(path);
      return [/<title>([^<]*)<\/title>/.exec(page)?.[1], /<meta name="description" content="([^"]*)">/.exec(page)?.[1]];
    }, html);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}

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
// (A render does not run a sequence: its server bundle installs no voice table.)
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
