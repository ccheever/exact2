import { beforeAll, afterAll, test, expect } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { webCompiler } from './module.mjs';
import { dragTarget } from './drag-target.js';

const root = resolve(import.meta.dir, '../..');
let dir, server;
beforeAll(async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  dir = mkdtempSync(resolve(root, 'target/drag-bindings-'));
  writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Drag bindings', app: { id: 'test.drag-bindings', name: 'Drag bindings' }, host: { web: {} } }));
  const handle = (prop, events, indent) => [
    'if literal',
    `  column id="self" ${prop}="target" ${events} testId=${prop === 'reorderFor' ? '`handle-${item}`' : '"handle"'} width="100%" height=${prop === 'transformDragFor' ? '"100%"' : '40'} touch-action="none" background-color="#6699aa"`,
    'else',
    `  column id="self" ${prop}=binding ${events} testId=${prop === 'reorderFor' ? '`handle-${item}`' : '"handle"'} width="100%" height=${prop === 'transformDragFor' ? '"100%"' : '40'} touch-action="none" background-color="#6699aa"`,
  ].map(line => ' '.repeat(indent) + line).join('\n');
  writeFileSync(resolve(dir, 'app.contract'), `component App
  state mode = "transform"
  state binding = "target"
  state targetName = "target"
  state duplicate = false
  state literal = false
  state releases = 0
  state height = 200
  state x = 0
  state rows = ["a", "b", "c", "d"]
  action choose(value: string)
    mode = value
  action bind(value: string)
    binding = value
  action rename
    targetName = targetName == "target" ? "renamed" : "target"
  action rootName
    targetName = "exact-root"
    binding = "exact-root"
  action matchName
    binding = targetName
  action duplicateName
    duplicate = !duplicate
  action useLiteral
    literal = !literal
  action geometry(w: number, h: number, pw: number, ph: number)
    x = x
  action transformed(nx: number, ny: number, s: number, vx: number, vy: number, vs: number)
    x = nx
    releases = releases + 1
  action resized(h: number, v: number)
    height = h
    releases = releases + 1
  action reordered(item: string, before: option<string>)
    releases = releases + 1
  view
    column width=400 height=700
      row
        button testId="transform" press=choose("transform")
          text "Transform"
        button testId="height" press=choose("height")
          text "Height"
        button testId="reorder" press=choose("reorder")
          text "Reorder"
      row
        button testId="missing" press=bind("missing")
          text "Missing"
        button testId="empty" press=bind("")
          text "Empty"
        button testId="self" press=bind("self")
          text "Self"
        button testId="sibling" press=bind("sibling")
          text "Sibling"
      row
        button testId="rename" press=rename
          text "Rename"
        button testId="match" press=matchName
          text "Match"
        button testId="duplicate" press=duplicateName
          text "Duplicate"
        button testId="literal" press=useLiteral
          text "Literal"
      row
        button testId="fixed-reorder" press=choose("fixed-reorder")
          text "Literal reorder"
        button testId="svg" press=choose("svg")
          text "SVG"
        button testId="root" press=rootName
          text "Root ID"
      text \`\${releases}\` testId="releases"
      column id="sibling" height=1
      column id=(duplicate ? targetName : "outer")
        if mode == "transform"
          column width=300 height=200 overflow="hidden" padding=0 border-width=0
            column id=targetName testId="target" width="100%" height="100%" box-sizing="border-box" padding=0 border-width=0 scale=2 translate=\`\${x}px 0px\`
${handle('transformDragFor', 'transformgeometry=geometry transformrelease=transformed', 14)}
        if mode == "height"
          column id=targetName testId="target" width=300 height=height box-sizing="border-box"
${handle('heightDragFor', 'heightrelease=resized', 12)}
        if mode == "reorder"
          list id=targetName testId="target" virtualized=true estimated-item-height=60 reorderdrop=reordered width=300 height=240
            for item in rows key=item
              column height=60 box-sizing="border-box"
${handle('reorderFor', '', 16)}
        if mode == "fixed-reorder"
          list id="target" testId="target" virtualized=true estimated-item-height=60 reorderdrop=reordered width=300 height=240
            for item in rows key=item
              column height=60 box-sizing="border-box"
                column reorderFor="target" testId=\`handle-\${item}\` width="100%" height=40 touch-action="none" background-color="#6699aa"
        if mode == "svg"
          svg width=300 height=300
            svg id="viewport" testId="viewport" width=300 height=300
              g id="cards" testId="group"
                foreignObject id="sheet" testId="svg-sheet" width=300 height=300
                  column heightDragFor="sheet" heightrelease=resized testId="svg-handle" width=300 height=40 touch-action="none"
                  list id="cards" virtualized=true estimated-item-height=60 reorderdrop=reordered width=300 height=240
                    for item in rows key=item
                      column height=60 box-sizing="border-box"
                        column reorderFor="cards" testId=\`handle-\${item}\` width="100%" height=40 touch-action="none" background-color="#6699aa"
`);
  const build = Bun.spawn([process.execPath, 'host/web-js/build.mjs', 'drag-bindings', '--out', resolve(dir, 'dist'), '--render', 'none'], {
    cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, stdout: 'pipe', stderr: 'pipe',
  });
  const [status, stdout, stderr] = await Promise.all([build.exited, new Response(build.stdout).text(), new Response(build.stderr).text()]);
  expect({ status, error: status ? stdout + stderr : '' }).toEqual({ status: 0, error: '' });
  server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
    const path = new URL(req.url).pathname;
    const file = resolve(dir, 'dist', path === '/' ? 'index.html' : path.slice(1));
    return existsSync(file) ? new Response(Bun.file(file)) : new Response('Not found', { status: 404 });
  } });
}, 60_000);
afterAll(() => { server?.stop(true); if (dir) rmSync(dir, { recursive: true, force: true }); });

test.each(['transform', 'height', 'reorder'])('%s resolves computed bindings and changing ancestor IDs', async mode => {
  const browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
  try {
    await browser.tap(mode);
    let releases = 0;
    const drag = async valid => {
      await browser.tap(mode === 'reorder' ? 'handle-a' : 'handle', { drag: { dx: mode === 'transform' ? 25 : 0, dy: mode === 'reorder' ? 130 : mode === 'height' ? -20 : 0, over: 300 } });
      await browser.clock('settle');
      if (valid) releases++;
      expect((await browser.state()).slots.releases).toBe(releases);
    };
    await drag(true);
    for (const name of ['missing', 'empty', 'self', 'sibling']) {
      await browser.tap(name);
      await drag(false);
    }
    await browser.tap('match');
    await drag(true);
    await browser.tap('rename');
    await drag(false);
    await browser.tap('match');
    await drag(true);
    await browser.tap('duplicate');
    await drag(false);
    await browser.tap('duplicate');
    await drag(true);
    // A literal handle must also follow changes to a computed ancestor id.
    await browser.tap('rename');
    await browser.tap('literal');
    await drag(true);
    await browser.tap('rename');
    await drag(false);
    await browser.tap('rename');
    await drag(true);
    await browser.tap('duplicate');
    await drag(false);
    await browser.tap('duplicate');
    await drag(true);
    const handle = mode === 'reorder' ? 'handle-a' : 'handle';
    const move = { dx: mode === 'transform' ? 25 : 0, dy: mode === 'reorder' ? 130 : mode === 'height' ? -20 : 0, ms: 300 };
    await browser.tap(handle, { down: true });
    await browser.pointer('move', move);
    // Change the binding during a contact without moving the pointer to a button.
    await browser.carrier.evaluate('document.querySelector(\'[data-testid="rename"]\').click()');
    await browser.pointer('up');
    await browser.clock('settle');
    expect((await browser.state()).slots.releases).toBe(releases);
    await browser.tap('rename');
    await drag(true);
  } finally { await browser.close(); }
}, 60_000);

test('height target can share the mounting container ID', async () => {
  const browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
  try {
    await browser.tap('height');
    await browser.tap('root');
    const drag = async releases => {
      await browser.tap('handle', { drag: { dx: 0, dy: -20, over: 300 } });
      await browser.clock('settle');
      expect((await browser.state()).slots.releases).toBe(releases);
    };
    await drag(1);
    await browser.tap('duplicate');
    await drag(1);
    await browser.tap('duplicate');
    await drag(2);
  } finally { await browser.close(); }
}, 60_000);

test('literal reorder cancels a contact when an ancestor ID becomes ambiguous', async () => {
  const browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
  try {
    await browser.tap('fixed-reorder');
    const drag = async releases => {
      await browser.tap('handle-a', { drag: { dx: 0, dy: 130, over: 300 } });
      await browser.clock('settle');
      expect((await browser.state()).slots.releases).toBe(releases);
    };
    await drag(1);
    await browser.tap('handle-a', { down: true });
    await browser.pointer('move', { dx: 0, dy: 130, ms: 300 });
    await browser.carrier.evaluate('document.querySelector(\'[data-testid="duplicate"]\').click()');
    await browser.pointer('up');
    await browser.clock('settle');
    expect((await browser.state()).slots.releases).toBe(1);
    await drag(1);
    await browser.tap('duplicate');
    await drag(2);
  } finally { await browser.close(); }
}, 60_000);

test('rewritten SVG IDs resolve by their authored names', async () => {
  const browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
  try {
    await browser.tap('svg');
    // Resolve against the built DOM without requiring an SVG height owner to be eligible.
    const resolve = name => browser.carrier.evaluate(`(() => {
      const handle = document.querySelector('[data-testid="svg-handle"]');
      handle.dataset.heightdragfor = ${JSON.stringify(name)};
      return (${dragTarget})(handle, 'heightdragfor')?.dataset.testid ?? null;
    })()`);
    expect(await resolve('sheet')).toBe('svg-sheet');
    expect(await resolve('cards')).toBe('group');
    expect(await resolve('viewport')).toBe('viewport');
    const domID = await browser.carrier.evaluate('document.querySelector(\'[data-testid="svg-sheet"]\').id');
    expect(domID).not.toBe('sheet');
    expect(await resolve(domID)).toBeNull();
  } finally { await browser.close(); }
}, 60_000);

test('a rewritten SVG ancestor still makes a reorder binding ambiguous', async () => {
  const browser = await open({ host: 'web', url: server.url.href, size: [800, 800] });
  try {
    await browser.tap('svg');
    await browser.tap('handle-a', { drag: { dx: 0, dy: 130, over: 300 } });
    await browser.clock('settle');
    expect((await browser.state()).slots.releases).toBe(0);
  } finally { await browser.close(); }
}, 60_000);
