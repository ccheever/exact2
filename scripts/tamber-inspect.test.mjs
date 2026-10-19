// The server's half of inspect (LLP 1122 D5; T2b step 5): currency, the join
// to source with each file checked against its compiled digest, each row's
// editability, and the sites to outline.
import { expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join as joinPath } from 'node:path';
import { buildChannel } from './tamber-builds.mjs';
import { correspondingSite, current, editable, join, plainAt, rowRung, stateRung, mapCard, mapFor, paramsOf, published, pullMap, sitesAt, staleMap } from './tamber-inspect.mjs';

const sha = text => createHash('sha256').update(text).digest('hex');

test('a selection is joined to its site\'s source, each row\'s origin and editability; a file moved since makes it read-only', () => {
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-'));
  try {
    const app = joinPath(dir, 'app.contract'), card = joinPath(dir, 'card.contract');
    writeFileSync(app, 'app'); writeFileSync(card, 'card');
    const map = { digest: 'p1', sources: [{ file: app, sha256: sha('app') }, { file: card, sha256: sha('card') }], nodes: [
      { file: app, line: 3, col: 5, end_col: 9, component: 'App', chain: [], bindings: [] },
      { file: card, line: 2, col: 7, end_col: 20, component: 'Card', chain: [{ file: app, line: 4, col: 7, end_col: 11, component: 'App' }], bindings: [{ row: 'background_color', origin: 'own' }, { row: 'padding', origin: 'class:Panel' }] },
      { file: card, line: 2, col: 7, end_col: 20, component: 'Card', chain: [{ file: app, line: 5, col: 7, end_col: 11, component: 'App' }], bindings: [] },
    ] };
    expect(mapFor(Buffer.from(JSON.stringify(map)), 'p1')).toEqual(map);
    expect(mapFor(Buffer.from(JSON.stringify(map)), 'p2')).toBe(null);
    const selected = { node: 7, site: 1, instance: [], tuple: { 'dark-style': 'dim' }, rows: [
      { name: 'background_color', value: '#fff', source: 'authored', token: { names: ['surface.card'], text: 'surface.card', modes: 'dark-style=dim', implied: false } },
      { name: 'padding', value: 12, source: 'authored' },
      { name: 'text_color', value: '#000', source: 'inherited', from: 3, token: { names: ['text.primary'], modes: '', implied: false } },
    ] };
    const s = join({ id: 'web-1' }, selected, map);
    expect(s).toMatchObject({ build: 'web-1', node: 7, site: 1, tuple: { 'dark-style': 'dim' }, readOnly: null, source: { file: card, line: 2, col: 7, component: 'Card', chain: [{ file: app, line: 4 }] } });
    expect(s.rows.map(r => [r.name, r.origin, r.editable.rung, r.editable.token ?? r.editable.link ?? null])).toEqual([
      ['background_color', 'own', 'token', 'surface.card'],
      ['padding', 'class:Panel', null, null],
      ['text_color', null, null, 'text.primary'],
    ]);
    // (Bindings with no place, D1's address: the source's to edit.)
    expect(s.rows[1].editable.reason).toContain('the map gives no place');
    expect(s.rows[2].editable.reason).toContain('inherited from #3');
    // Every copy of the node: the sites at that source location.
    expect(sitesAt(map, s.source)).toEqual([1, 2]);
    // A file the join reads changed since this build: read-only, said why.
    writeFileSync(card, 'card, edited');
    const moved = join({ id: 'web-1' }, selected, map);
    expect(moved.readOnly).toContain('card.contract changed since this build');
    expect(moved.rows.every(r => r.editable.rung === null)).toBe(true);
    // Read-only, a row still links its token.
    expect(moved.rows.find(r => r.name === 'text_color').editable.link).toBe('text.primary');
    // A map whose sources changed is no place to look a fresh span up.
    expect(staleMap(map)).toContain('card.contract changed since the running plan');
    // No map for its plan: read-only.
    expect(join({ id: 'web-1' }, selected, null).readOnly).toContain('no source map');
    // The design changed since the build: read-only, unless the build runs
    // the design the dev loop serves now (a value edit it took live).
    writeFileSync(card, 'card');
    const design = joinPath(dir, 'app.tamber');
    writeFileSync(design, 'tamber a');
    const withDesign = { ...map, sources: [...map.sources, { file: design, sha256: sha('tamber a') }] };
    const opts = served => ({ design, served });
    expect(join({ id: 'web-1', design: 'd1' }, selected, withDesign, opts(null)).readOnly).toBe(null);
    writeFileSync(design, 'tamber b');
    expect(join({ id: 'web-1', design: 'd1' }, selected, withDesign, opts(null)).readOnly).toContain('app.tamber changed since this build');
    // The build runs the design served now, made from these bytes: fine; a
    // later save the dev loop has not taken (it kept the last good): not.
    expect(join({ id: 'web-1', design: 'd2' }, selected, withDesign, opts({ digest: 'd2', source: sha('tamber b') })).readOnly).toBe(null);
    writeFileSync(design, 'tamber c, broken');
    expect(join({ id: 'web-1', design: 'd2' }, selected, withDesign, opts({ digest: 'd2', source: sha('tamber b') })).readOnly).toContain('changed since this build');
    // A design the map does not name: read-only.
    expect(join({ id: 'web-1' }, selected, map, opts(null)).readOnly).toContain('not in the build');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('an `:active` row lowered to the press paint is joined at its part, read-only but by token, and outlines its node (LLP 1121 D8)', () => {
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-active-'));
  try {
    const app = joinPath(dir, 'app.contract');
    const text = ['component App', '  view', '    text "x"', '', '', '', '    box', '      :active', '        background-color="accent.soft"', '        color="#f00"', ''].join('\n');
    writeFileSync(app, text);
    const part = { file: app, line: 9, col: 9, end_col: 25 };
    const map = { digest: 'p1', sources: [{ file: app, sha256: sha(text) }], nodes: [
      { file: app, line: 7, col: 5, end_col: 11, component: 'App', chain: [], bindings: [{ row: 'press_background_color', origin: 'own', state: 'active', at: part }, { row: 'press_color', origin: 'own', state: 'active', at: { ...part, line: 10 } }] },
    ] };
    const selected = { node: 1, site: 0, instance: [], rows: [
      { name: 'press_background_color', value: '#00f', source: 'authored', token: { names: ['accent.soft'], text: 'accent.soft', modes: '', implied: false } },
      { name: 'press_color', value: '#f00', source: 'authored' },
    ] };
    const s = join({ id: 'web-1' }, selected, map, { app: dir });
    expect(s.rows.map(r => [r.name, r.state, r.at?.line, r.editable.rung])).toEqual([
      ['press_background_color', 'active', 9, 'template'],
      ['press_color', 'active', 10, 'template'],
    ]);
    // Written on the node itself (no style): set-row at its place, with its state, the row as its author wrote it (T5).
    expect(s.rows[0].editable).toEqual({ rung: 'template', file: 'app.contract', at: { line: 9, col: 9 }, sha256: sha(text), row: 'background-color', state: 'active', instances: 1, text: 'accent.soft', token: 'accent.soft', modes: '' });
    expect(s.rows[1].editable).toMatchObject({ row: 'color', state: 'active', text: '#f00' });
    // With no app named, the join can't tell its files from a package's: read-only, its token still offered.
    expect(join({ id: 'web-1' }, selected, map).rows[0].editable).toEqual({ rung: 'token', token: 'accent.soft', modes: '' });
    // `uses` names the part's own place; outlining it finds the node.
    expect(sitesAt(map, part)).toEqual([0]);
    // What is held (LLP 1122 D8): an `:active` row at its part, named by its state.
    const held = join({ id: 'web-1' }, { ...selected, held: { state: 'active', rows: [{ name: 'press_background_color', value: '#00f' }, { name: 'press_scale', value: '0.95' }] } }, map);
    // (Named as its author wrote it, under its part: theme2's 16:13 (2); the press row beside it.)
    expect(held.held.rows.map(r => [r.name, r.row, r.state, r.source?.line])).toEqual([['background-color', 'press_background_color', 'active', 9], ['press_scale', undefined, 'active', 7]]);
    // Named on a node inside the press (stage 9 E): joined at the press's own site, its rows' tokens kept.
    const inner = { ...map, nodes: [...map.nodes, { file: app, line: 12, col: 7, end_col: 11, component: 'App', chain: [], bindings: [] }] };
    const label = { node: 2, site: 1, instance: [], rows: [], held: { state: 'active', node: 1, site: 0, instance: [], rows: [{ name: 'press_background_color', value: '#00f', token: { names: ['accent.soft'], text: 'accent.soft', modes: '', implied: false } }] } };
    const named = join({ id: 'web-1' }, label, inner);
    expect(named.held.rows.map(r => [r.name, r.row, r.source?.line, r.token?.names])).toEqual([['background-color', 'press_background_color', 9, ['accent.soft']]]);
    expect(named.source.line).toBe(12);
    // Held where the build names the press (its held site): the held inline row is set-row's at its place, with
    // its state and the press's own copies (one; the label's chain is not counted for it).
    const offered = join({ id: 'web-1' }, label, inner, { app: dir });
    expect(offered.held.rows[0].editable).toEqual({ rung: 'template', file: 'app.contract', at: { line: 9, col: 9 }, sha256: sha(text), row: 'background-color', state: 'active', instances: 1, text: 'accent.soft', token: 'accent.soft', modes: '' });
    // A held hover's row (a base binding, no state place) is not set-row's: the look is not written there.
    const hover = { ...label, held: { state: 'hover', node: 1, site: 0, instance: [], rows: [{ name: 'background_color', value: '#f00' }] } };
    const based = { ...inner, nodes: [{ ...inner.nodes[0], bindings: [...inner.nodes[0].bindings, { row: 'background_color', origin: 'own', at: part, attr: 'background-color' }] }, inner.nodes[1]] };
    expect(join({ id: 'web-1' }, hover, based, { app: dir }).held.rows[0].editable?.rung ?? null).not.toBe('template');
    // The held node's own file is checked too: one moved since makes the selection read-only.
    const style = joinPath(dir, 'style.contract');
    writeFileSync(style, 'style now');
    const split = { ...inner, sources: [...inner.sources, { file: style, sha256: sha('style then') }], nodes: [{ ...inner.nodes[0], file: style }, inner.nodes[1]] };
    expect(join({ id: 'web-1' }, label, split).readOnly).toContain('style.contract');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a row that reads a component param joins on the param rung, by its full name and the token it aliases; no IR param token reaches a client (LLP 1122 T3)', () => {
  const ir = { tokens: [{ name: 'primary_500', role: null }, { name: 'radius.md', role: null }, { name: '-exact-label', role: 'label' }], components: [{ identity: 'post.contract#PostButton', name: 'PostButton', params: [
    { name: 'post.contract#PostButton.fill', short: 'PostButton.fill', token: 'param.f1a2b3c4d.PostButton.fill', default: 'primary_500', set: null },
    { name: 'post.contract#PostButton.radius', short: 'PostButton.radius', token: 'param.f1a2b3c4d.PostButton.radius', default: 'radius.md', set: { value: '6px', whens: 0 } },
    { name: 'post.contract#PostButton.height', short: null, token: 'param.f1a2b3c4d.PostButton.height', default: '32px', set: null },
    { name: 'post.contract#PostButton.ink', short: 'PostButton.ink', token: 'param.f1a2b3c4d.PostButton.ink', default: 'label', set: null },
    { name: 'post.contract#PostButton.edge', short: 'PostButton.edge', token: 'param.f1a2b3c4d.PostButton.edge', default: 'primary_500', set: { value: 'radius.md', whens: 1 } },
  ] }] };
  const params = paramsOf(ir);
  // Its alias: the value the app has (the line's, else the default) when that is one declared token.
  expect(params.get('param.f1a2b3c4d.PostButton.fill')).toMatchObject({ param: 'post.contract#PostButton.fill', short: 'PostButton.fill', token: 'primary_500' });
  expect(params.get('param.f1a2b3c4d.PostButton.radius').token).toBe(null);
  // A role override, by any spelling of its role, names its token; a param with `when` lines aliases none.
  expect(paramsOf({ ...ir, components: [{ params: [{ name: 'x#C.a', short: 'C.a', token: 'param.f00000000.C.a', default: '-Exact-Label', set: null }] }] }).get('param.f00000000.C.a').token).toBe('-exact-label');
  expect(params.get('param.f1a2b3c4d.PostButton.ink').token).toBe('-exact-label');
  expect(params.get('param.f1a2b3c4d.PostButton.edge').token).toBe(null);
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-param-'));
  const post = joinPath(dir, 'post.contract');
  writeFileSync(post, 'post');
  const map = { digest: 'p1', sources: [{ file: post, sha256: sha('post') }], nodes: [{ file: post, line: 9, col: 5, end_col: 11, component: 'PostButton', chain: [], bindings: [
    { row: 'background_color', origin: 'own' }, { row: 'height', origin: 'own' }, { row: 'border_color', origin: 'own' }, { row: 'color', origin: 'own' }] }] };
  const ir_ = n => `param.f1a2b3c4d.PostButton.${n}`;
  // A synthetic token is named by its whole expression, IR names inside it.
  const mix = `color-mix(in oklab, ${ir_('fill')} 50%, black)`;
  const selected = { node: 1, site: 0, instance: [], tuple: {}, rows: [
    { name: 'background_color', value: '#1083fe', source: 'authored', token: { names: [ir_('fill')], text: ir_('fill'), modes: 'dark-style=dim', implied: false } },
    { name: 'height', value: 32, source: 'authored', token: { names: [ir_('height')], text: ir_('height'), modes: '', implied: false } },
    { name: 'border_color', value: '#000', source: 'authored', token: { names: [mix], text: mix, modes: '', implied: false } },
    { name: 'color', value: '#fff', source: 'authored', token: { names: ['param.f99999999.Other.x'], text: 'param.f99999999.Other.x', modes: '', implied: false } },
    { name: 'text_color', value: '#fff', source: 'inherited', from: 2, token: { names: [ir_('fill')], text: ir_('fill'), modes: '', implied: false } },
  ] };
  const s = join({ id: 'web-1' }, selected, map, { params });
  expect(s.rows[0].editable).toEqual({ rung: 'param', param: 'post.contract#PostButton.fill', short: 'PostButton.fill', token: 'primary_500', modes: 'dark-style=dim' });
  expect(s.rows[1].editable).toEqual({ rung: 'param', param: 'post.contract#PostButton.height', short: null, modes: '' });
  // An expression over a param: read-only, named by the param.
  expect(s.rows[2].editable.rung).toBe(null);
  expect(s.rows[2].token).toMatchObject({ names: ['color-mix(in oklab, post.contract#PostButton.fill 50%, black)'], text: 'color-mix(in oklab, post.contract#PostButton.fill 50%, black)' });
  // A param this server has not read (its describe not answered yet): read-only, said so.
  expect(s.rows[3].editable).toMatchObject({ rung: null, reason: expect.stringContaining('has not read yet') });
  expect(s.rows[3].token.names).toEqual([]);
  // The JS target's custom property for it, in a dynamic row's text; a token that only looks like one is left be.
  const dyn = join({ id: 'web-1' }, { ...selected, rows: [
    { name: 'border_color', value: '#000', source: 'dynamic', token: { names: ['post.contract#PostButton.fill'], text: 'color-mix(in oklab, var(--x-param-f1a2b3c4d-PostButton-fill) 50%, black)', modes: '', implied: false } },
    { name: 'color', value: '#000', source: 'authored', token: { names: ['ui.param.face.fill'], text: 'ui.param.face.fill', modes: '', implied: false } },
  ] }, map, { params });
  expect(dyn.rows[0].token.text).toBe('color-mix(in oklab, post.contract#PostButton.fill 50%, black)');
  expect(dyn.rows[1].token).toMatchObject({ names: ['ui.param.face.fill'], text: 'ui.param.face.fill' });
  expect(dyn.rows[1].editable).toMatchObject({ rung: 'token', token: 'ui.param.face.fill' });
  // Inherited, it links the param.
  expect(s.rows[4].editable).toMatchObject({ rung: null, link: 'post.contract#PostButton.fill' });
  expect(JSON.stringify(s)).not.toMatch(/param\.f[0-9a-f]+\./);
  expect(s.rows.some(r => 'unread' in r)).toBe(false);
  // The raw report the server publishes (`selected`) is scrubbed the same way.
  expect(JSON.stringify(published(selected, params))).not.toMatch(/param\.f[0-9a-f]+\./);
  // A held state's rows too.
  const held = { ...selected, held: { state: 'active', rows: [{ name: 'background_color', state: 'active', token: { names: [ir_('fill')], text: ir_('fill'), modes: '', implied: false } }] } };
  expect(JSON.stringify(published(held, params))).not.toMatch(/param\.f[0-9a-f]+\./);
  expect(JSON.stringify(join({ id: 'web-1' }, held, map, { params }))).not.toMatch(/param\.f[0-9a-f]+\./);
  expect(JSON.stringify(published(selected, new Map()))).not.toMatch(/param\.f[0-9a-f]+\./);
  // Without params read, no IR name either, and no param rung.
  const bare = join({ id: 'web-1' }, selected, map);
  expect(bare.rows.every(r => r.editable.rung === null)).toBe(true);
  expect(JSON.stringify(bare)).not.toMatch(/param\.f[0-9a-f]+\./);
  // Read-only for a moved file, the param still links.
  writeFileSync(post, 'post, edited');
  expect(join({ id: 'web-1' }, selected, map, { params }).rows[0].editable).toMatchObject({ rung: null, link: 'post.contract#PostButton.fill' });
  rmSync(dir, { recursive: true, force: true });
});

test('a style\'s state part row joins on the state rung for set-state, the default: its style as its file declares it, the row as written, its file and text; a token it reads offered beside; a held one where the build names the held node; a package\'s refused (LLP 1122 T4)', () => {
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-state-'));
  try {
    const post = joinPath(dir, 'post.contract');
    const text = ['component X', '  view', '    text "x"', '', 'style Card', '  color="white"', '  :active', '    background-color="primary_600"', '    opacity=0.8', '    color="red"', ''].join('\n');
    writeFileSync(post, text);
    // The map's span names the attribute (the compiler's), not its value.
    const at = (line, attr) => ({ file: post, line, col: 5, end_col: 5 + attr.split('=')[0].length });
    const pkg = joinPath(dir, 'node_modules', 'ui', 'card.contract');
    mkdirSync(joinPath(dir, 'node_modules', 'ui'), { recursive: true });
    writeFileSync(pkg, 'pkg');
    // The compiler renamed this file's `style Card` (another file has one): the map says Card__post.
    const map = { digest: 'p1', sources: [{ file: post, sha256: sha(text) }, { file: pkg, sha256: sha('pkg') }], nodes: [
      { file: post, line: 3, col: 5, end_col: 11, component: 'X', chain: [], bindings: [
        { row: 'press_background_color', origin: 'class:Card__post', state: 'active', at: at(8, 'background-color="primary_600"') },
        { row: 'press_opacity_to', origin: 'class:Card__post', state: 'active', at: at(9, 'opacity=0.8') },
        { row: 'press_color', origin: 'class:A/B', state: 'active', at: at(10, 'color="red"') },
        { row: 'press_scale', origin: 'class:Pkg', state: 'active', at: { file: pkg, line: 3, col: 5, end_col: 9 } },
      ] },
      { file: post, line: 1, col: 1, end_col: 2, component: 'X', chain: [], bindings: [{ row: 'press_opacity_to', origin: 'class:Card__post', state: 'active', at: at(9, 'opacity=0.8') }] },
    ] };
    const selected = { node: 1, site: 0, instance: [], rows: [
      { name: 'press_background_color', value: '#00f', source: 'authored', token: { names: ['primary_600'], text: 'primary_600', modes: '', implied: false } },
      { name: 'press_opacity_to', value: 0.8, source: 'authored' },
      { name: 'press_color', value: '#f00', source: 'authored' },
      { name: 'press_scale', value: 0.95, source: 'authored' },
    ] };
    const s = join({ id: 'web-1' }, selected, map, { app: dir });
    const state = { rung: 'state', style: 'Card', state: 'active', file: 'post.contract' };
    expect(s.rows[0].editable).toEqual({ ...state, row: 'background-color', text: 'primary_600', token: 'primary_600' });
    expect(s.rows[1].editable).toEqual({ ...state, row: 'opacity', text: '0.8' });
    // Under a conditional class: no one style to edit.
    expect(s.rows[2].editable.rung).toBe(null);
    // A package's style: read-only, said so.
    expect(s.rows[3].editable).toMatchObject({ rung: null, reason: expect.stringContaining("a package's style") });
    // Held, where the build names the held node (stage 9 E's site): offered there; else shown only.
    const heldAt = { state: 'active', site: 1, rows: [{ name: 'press_opacity_to', value: '0.8' }] };
    expect(join({ id: 'web-1' }, { ...selected, held: heldAt }, map, { app: dir }).held.rows[0].editable).toEqual({ ...state, row: 'opacity', text: '0.8' });
    expect(join({ id: 'web-1' }, { ...selected, held: { state: 'active', rows: heldAt.rows } }, map, { app: dir }).held.rows[0].editable).toBe(undefined);
    // A file the server does not count as the app's own (a vendored package's): refused.
    expect(join({ id: 'web-1' }, selected, map, { app: dir, own: new Set(['app.contract']) }).rows[1].editable).toMatchObject({ rung: null, reason: expect.stringContaining("a package's style") });
    expect(join({ id: 'web-1' }, selected, map, { app: dir, own: new Set(['post.contract']) }).rows[1].editable.rung).toBe('state');
    // The held node's part in another file, moved since: nothing offered, the selected node's files unmoved.
    const styles = joinPath(dir, 'styles.contract'), was = 'style Press\n  :active\n    opacity=0.5\n';
    writeFileSync(styles, was);
    const two = { ...map, sources: [...map.sources, { file: styles, sha256: sha(was) }], nodes: [...map.nodes, { file: post, line: 2, col: 3, end_col: 7, component: 'X', chain: [], bindings: [{ row: 'press_opacity_to', origin: 'class:Press', state: 'active', at: { file: styles, line: 3, col: 5, end_col: 12 } }] }] };
    const heldTwo = { ...selected, held: { state: 'active', site: 2, rows: heldAt.rows } };
    expect(join({ id: 'web-1' }, heldTwo, two, { app: dir }).held.rows[0].editable).toEqual({ ...state, style: 'Press', file: 'styles.contract', row: 'opacity', text: '0.5' });
    writeFileSync(styles, `style Other\n${was}`);
    const moved = join({ id: 'web-1' }, heldTwo, two, { app: dir });
    expect(moved.readOnly).toBeTruthy();
    expect(moved.held.rows[0].editable).toBe(undefined);
    // The value as written: a whole literal only, bare, quoted or in parentheses; else none (the editor starts empty).
    const textOf = (line, name = line.startsWith('opacity') ? 'press_opacity_to' : 'press_background_color') => stateRung({ name }, { state: 'active', origin: 'class:Card', at: { file: post, line: 2, col: 5, end_col: 21 } }, dir, () => `style Card\n    ${line}\n`)?.text;
    expect(textOf('background-color=("accent.fill")')).toBe('accent.fill');
    // (A bare name is a name in scope, no plain value: none, as set-row's `plain` says.)
    expect(textOf('background-color = accent.fill // a note')).toBe(undefined);
    expect(textOf('background-color = "accent.fill" // a note')).toBe('accent.fill');
    expect(textOf('background-color=""')).toBe('');
    expect(textOf('background-color=("accent.fill"')).toBe(undefined);
    // (An escaped string is a plain string: its value, decoded.)
    expect(textOf('background-color="a \\"b\\""')).toBe('a "b"');
    expect(textOf('background-color=mix(accent.fill, white)')).toBe(undefined);
    expect(textOf('opacity=0.8//note')).toBe('0.8');
    expect(textOf('opacity=0.8 color="red"')).toBe('0.8');
    expect(textOf('background-color="a b" color="red" // c')).toBe('a b');
    // A file moved since: nothing offered.
    writeFileSync(post, `${text}// edited\n`);
    expect(join({ id: 'web-1' }, selected, map, { app: dir }).rows[1].editable.rung).toBe(null);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// LLP 1122 D8 (the lead's 18:30, amending theme2's 07:48 (a)): a held site
// corresponds in a new plan by its place and its call-site chain.
test('a held site corresponds in a new plan by its place and call-site chain: each copy of a component called twice its own, a moved call still, an ambiguous one none', () => {
  const def = { file: '/a/screens.contract', line: 20, col: 5, end_col: 9, component: 'PostButton' };
  const call = (line, col = 7) => ({ file: '/a/home.contract', line, col, end_col: col + 10, component: 'Home' });
  const plan = (...lines) => ({ nodes: [{ file: '/a/home.contract', line: 1, col: 1, end_col: 2, component: 'Home', chain: [] }, ...lines.map(l => ({ ...def, chain: [call(l)] }))] });
  const was = plan(4, 9);
  // A rebuild that moved nothing (a style's value edited): each copy is its own.
  expect(correspondingSite(was, plan(4, 9), 1)).toBe(1);
  expect(correspondingSite(was, plan(4, 9), 2)).toBe(2);
  // In another order in the new plan's sites: found by its chain, not its index.
  expect(correspondingSite(was, { nodes: [plan(4, 9).nodes[0], plan(4, 9).nodes[2], plan(4, 9).nodes[1]] }, 2)).toBe(1);
  // A line written above the second call: the first by its chain, the second the one left over (and two
  // left over, by rank among themselves).
  expect(correspondingSite(was, plan(4, 10), 2)).toBe(2);
  expect(correspondingSite(plan(4, 9, 14), plan(4, 10, 15), 3)).toBe(3);
  // A line above both: by rank in source order.
  expect(correspondingSite(was, plan(5, 10), 1)).toBe(1);
  expect(correspondingSite(was, plan(5, 10), 2)).toBe(2);
  // A third copy, the calls moved: ambiguous, none.
  expect(correspondingSite(was, plan(5, 10, 14), 2)).toBe(null);
  // The node gone: none. One copy left of two, its call moved: not the other's.
  expect(correspondingSite(was, { nodes: [plan().nodes[0]] }, 1)).toBe(null);
  expect(correspondingSite(was, plan(30), 1)).toBe(null);
  expect(correspondingSite(was, plan(4), 1)).toBe(1);
  expect(correspondingSite(was, plan(4), 2)).toBe(null);
  // A lone copy whose call moved: still it.
  expect(correspondingSite(plan(4), plan(30), 1)).toBe(1);
  // Calls moved onto each other's lines (4, 9 to 9, 14): by rank, not the line one had.
  expect(correspondingSite(was, plan(9, 14), 2)).toBe(2);
  expect(correspondingSite(was, plan(9, 14), 1)).toBe(1);
  // A `button "…"` and its label: two sites of one copy, each kept as itself beside a call added or removed.
  const pairs = (...lines) => ({ nodes: lines.flatMap(l => [{ ...def, chain: [call(l)] }, { ...def, chain: [call(l)] }]) });
  expect(correspondingSite(pairs(4, 9), pairs(4), 1)).toBe(1);
  expect(correspondingSite(pairs(4, 9), pairs(4, 9, 14), 3)).toBe(3);
  expect(correspondingSite(pairs(4, 9), pairs(5, 10), 3)).toBe(3);
  // Nested: a wrapper called twice, one button in it, becomes a wrapper called once with two: the deleted
  // wrapper's button is none (though as many copies), the kept one's itself.
  const inner = (line, outer) => ({ ...def, chain: [{ file: '/a/wrap.contract', line, col: 5, end_col: 9, component: 'Wrapper' }, call(outer)] });
  const wrapped = { nodes: [inner(3, 4), inner(3, 9)] };
  expect(correspondingSite(wrapped, { nodes: [inner(3, 4), inner(6, 4)] }, 1)).toBe(null);
  expect(correspondingSite(wrapped, { nodes: [inner(3, 4), inner(6, 4)] }, 0)).toBe(0);
  expect(correspondingSite(wrapped, { nodes: [inner(3, 5), inner(3, 10)] }, 1)).toBe(1);
  // A call cut and pasted above the other (4, 9 to 3, 5): not ranked (the files' calls moved unevenly), the held one cleared.
  expect(correspondingSite(was, plan(3, 5), 2)).toBe(null);
  expect(correspondingSite(was, plan(3, 5), 1)).toBe(null);
  // Used at two depths (direct, and inside a card), a line above both: by rank, each file evenly; none thrown.
  const card = l => ({ ...def, chain: [{ file: '/a/card.contract', line: 3, col: 5, end_col: 9, component: 'Card' }, call(l)] });
  const depths = { nodes: [{ ...def, chain: [call(4)] }, card(8)] };
  expect(correspondingSite(depths, { nodes: [{ ...def, chain: [call(5)] }, card(9)] }, 1)).toBe(1);
  expect(correspondingSite(depths, { nodes: [{ ...def, chain: [call(5)] }, card(9)] }, 0)).toBe(0);
  // A third call in another file, unmoved, while lines are written above Home's two: each by rank in its file.
  const foot = { ...def, chain: [{ file: '/a/foot.contract', line: 10, col: 7, end_col: 17, component: 'Footer' }] };
  expect(correspondingSite({ nodes: [...plan(4, 9).nodes, foot] }, { nodes: [...plan(9, 14).nodes, foot] }, 2)).toBe(2);
  expect(correspondingSite({ nodes: [...plan(4, 9).nodes, foot] }, { nodes: [...plan(9, 14).nodes, foot] }, 3)).toBe(3);
  // A line removed between them (4, 9 to 4, 8): the one below by the edit.
  expect(correspondingSite(was, plan(4, 8), 2)).toBe(2);
  // A copy called from another file is not one called here.
  expect(correspondingSite(was, { nodes: [...plan(5).nodes, { ...def, chain: [{ ...call(10), file: '/a/other.contract' }] }] }, 2)).toBe(null);
});

test('a row its author wrote joins on the template or style rung for set-row, the default: at its place with the file\'s digest, or on its style by its declared name; its plain value as written; the token beside; an expression, a package or a moved file read-only (LLP 1122 T5)', () => {
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-row-'));
  try {
    const card = joinPath(dir, 'card.contract');
    const text = ['component Card', '  view', '    box background-color="surface.card" border-radius=12 opacity=(0.5) width=100 gap="space.md"', '      border-color=(mix ? "a" : "b")', '', 'style Panel', '  color="red" // the panel', '  margin="8px 4px"', ''].join('\n');
    writeFileSync(card, text);
    const pkg = joinPath(dir, 'node_modules', 'ui', 'x.contract');
    mkdirSync(joinPath(dir, 'node_modules', 'ui'), { recursive: true });
    writeFileSync(pkg, '    box color="blue"');
    const col = (line, attr) => text.split('\n')[line - 1].indexOf(attr) + 1;
    const at = (line, attr, file = card) => ({ file, line, col: col(line, attr), end_col: col(line, attr) + attr.length });
    const node = { file: card, line: 3, col: 5, end_col: 8, component: 'Card', chain: [], bindings: [
      { row: 'background_color', origin: 'own', at: at(3, 'background-color'), attr: 'background-color' },
      { row: 'border_radius_top_left', origin: 'own', at: at(3, 'border-radius'), attr: 'border-radius' },
      { row: 'opacity', origin: 'own', at: at(3, 'opacity'), attr: 'opacity' },
      { row: 'border_color_top', origin: 'own', at: at(4, 'border-color'), attr: 'border-color' },
      { row: 'text_color', origin: 'class:Panel__card', at: at(7, 'color'), attr: 'color' },
      { row: 'margin_top', origin: 'class:Panel__card', at: at(8, 'margin'), attr: 'margin', layout: true },
      { row: 'outline_color', origin: 'own', at: { file: pkg, line: 1, col: 9, end_col: 14 }, attr: 'color' },
      { row: 'width', origin: 'own', at: at(3, 'width'), attr: 'width', layout: true },
      { row: 'gap', origin: 'own', at: at(3, 'gap'), attr: 'gap', layout: true },
    ] };
    // Two copies of this source node (a component called twice).
    // Two copies of this source node (a component called twice); the first's generated label shares its span and chain, no third copy.
    const map = { digest: 'p1', sources: [{ file: card, sha256: sha(text) }, { file: pkg, sha256: sha('    box color="blue"') }], nodes: [node, { ...node, chain: [{ file: card, line: 9, col: 1, end_col: 2, component: 'App' }] }, { ...node, bindings: [] }] };
    const rows = [
      { name: 'background_color', value: '#fff', source: 'authored', token: { names: ['surface.card'], text: 'surface.card', modes: '', implied: false } },
      { name: 'border_radius_top_left', value: 12, source: 'authored' },
      { name: 'opacity', value: 0.5, source: 'authored' },
      { name: 'border_color_top', value: '#000', source: 'authored' },
      { name: 'text_color', value: '#f00', source: 'authored' },
      { name: 'margin_top', value: 8, source: 'authored' },
      { name: 'outline_color', value: '#00f', source: 'authored' },
      { name: 'width', value: 100, source: 'authored' },
      { name: 'gap', value: 8, source: 'authored', token: { names: ['space.md'], text: 'space.md', modes: '', implied: false } },
    ];
    const s = join({ id: 'web-1' }, { node: 1, site: 0, instance: [], rows }, map, { app: dir });
    const template = { rung: 'template', file: 'card.contract', sha256: sha(text), instances: 2 };
    expect(s.rows[0].editable).toEqual({ ...template, at: { line: 3, col: col(3, 'background-color') }, row: 'background-color', text: 'surface.card', token: 'surface.card', modes: '' });
    // A longhand of the shorthand written there: the shorthand, whole.
    expect(s.rows[1].editable).toEqual({ ...template, at: { line: 3, col: col(3, 'border-radius') }, row: 'border-radius', text: '12' });
    expect(s.rows[2].editable).toMatchObject({ rung: 'template', row: 'opacity', text: '0.5' });
    expect(s.rows[3].editable).toMatchObject({ rung: null, reason: expect.stringContaining('expression') });
    // On its style, by the name its file declares (the compiler renamed it), in its own file.
    expect(s.rows[4].editable).toEqual({ rung: 'style', style: 'Panel', row: 'color', file: 'card.contract', text: 'red' });
    expect(s.rows[5].editable).toMatchObject({ rung: null, reason: expect.stringContaining('layout row') });
    expect(s.rows[6].editable).toMatchObject({ rung: null, reason: expect.stringContaining("package") });
    // A layout row (the map's `layout`, D14): set-row's reason, up front; the token it reads, offered.
    expect(s.rows[7].editable).toEqual({ rung: null, reason: expect.stringContaining('layout row'), link: null });
    expect(s.rows[8].editable).toEqual({ rung: 'token', token: 'space.md', modes: '' });
    // A file the server does not count as the app's own: a package's.
    const outside = join({ id: 'web-1' }, { node: 1, site: 0, instance: [], rows }, map, { app: dir, own: new Set(['other.contract']) });
    expect(outside.rows[2].editable).toMatchObject({ rung: null, reason: expect.stringContaining('package') });
    // (Its token, whole, still offered: a design edit, not the package's file.)
    expect(outside.rows[0].editable).toEqual({ rung: 'token', token: 'surface.card', modes: '' });
    // A row that reads a param keeps the param rung, the default (T3), with its template's address for a detach.
    const params = new Map([['f1', { param: 'card.contract#Card.fill', short: 'Card.fill', token: null }]]);
    const tpl = { rung: 'template', file: 'card.contract', at: { line: 3, col: 9 }, sha256: 'x', row: 'background-color', instances: 1, text: 'Card.fill' };
    expect(editable({ source: 'authored', token: { names: ['card.contract#Card.fill'], text: 'card.contract#Card.fill', modes: '', implied: false } }, 'own', params, false, null, tpl)).toEqual({ rung: 'param', param: 'card.contract#Card.fill', short: 'Card.fill', modes: '', template: tpl });
    // A style is the nearest `style` above its row through indented lines; another top-level declaration first means none.
    expect(rowRung({ name: 'text_color' }, { origin: 'class:Panel', at: at(7, 'color'), attr: 'color' }, map, dir, 1, () => ['style Other', '  opacity=1', '', 'style Panel', '  // a note', '', '  color="red"'].join('\n'))).toMatchObject({ style: 'Panel' });
    expect(rowRung({ name: 'text_color' }, { origin: 'class:Panel', at: at(7, 'color'), attr: 'color' }, map, dir, 1, () => ['style Panel', '  opacity=1', 'component X', '  view', '', '', '  color="red"'].join('\n'))).toBe(null);
    // The map's columns are UTF-8 bytes: an attribute after wide characters is read at its own place.
    const wide = '    text "你好你好你好" color="red" opacity=0.5';
    expect(plainAt(wide, 1 + Buffer.byteLength('    text "你好你好你好" '), 'color')).toBe('red');
    expect(plainAt(wide, 1 + Buffer.byteLength('    text "你好你好你好" '), 'opacity')).toBe(null);
    // Literals as Contract writes them: an exponent, nested parentheses, a minus, a bare colour, a positional string after.
    expect(['opacity=5e-1', 'opacity=((0.5))', 'opacity=-(0.5)', 'color=#1f9d62', 'color="red" "Hi"'].map(l => plainAt(`    box ${l}`, 9, l.split('=')[0]))).toEqual(['5e-1', '0.5', '-0.5', '#1f9d62', 'red']);
    expect(['color=alpha', 'opacity=0.5 - 1', 'color=(c ? "a" : "b")', 'opacity=(0.5'].map(l => plainAt(`    box ${l}`, 9, l.split('=')[0]))).toEqual([null, null, null, null]);
    // A string's escapes as the lexer decodes them, one by one; a positional in parentheses after.
    expect(['marker-end="url(#a\\\\u{62})"', 'marker-end="url(#a\\$b)"', 'color="red\\u{9}"', 'color="x\\`y"', 'opacity=0.5 ("hello")', 'color="\\q"'].map(l => plainAt(`    box ${l}`, 9, l.split('=')[0]))).toEqual(['url(#a\\u{62})', 'url(#a$b)', 'red\t', 'x`y', '0.5', null]);
    // Moved since: read-only, its token still linked.
    writeFileSync(card, `${text}// edited\n`);
    expect(join({ id: 'web-1' }, { node: 1, site: 0, instance: [], rows }, map, { app: dir }).rows[0].editable).toMatchObject({ rung: null, link: 'surface.card' });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('editability fails closed: only a row the node itself sets to one token, its whole value, offers the token rung', () => {
  const token = (names, implied = false, text = names[0]) => ({ names, text, modes: '', implied });
  expect(editable({ source: 'authored', token: token(['ink']) }, 'own')).toMatchObject({ rung: 'token', token: 'ink' });
  expect(editable({ source: 'dynamic', token: token(['ink']) }, 'own')).toMatchObject({ rung: null, link: 'ink' });
  // A boundary's implied row: read-only, its token still linked.
  expect(editable({ source: 'authored', token: token(['ink'], true) }, 'own')).toMatchObject({ rung: null, link: 'ink' });
  // An expression over tokens (a gradient, a mix) is no token's edit.
  expect(editable({ source: 'authored', token: token(['accent.fill', 'surface.card'], false, 'linear-gradient(90deg, accent.fill, surface.card)') }, 'own')).toMatchObject({ rung: null, link: 'accent.fill' });
  expect(editable({ source: 'authored', token: token(['ink'], false, 'color-mix(in srgb, ink 60%, black)') }, 'own').rung).toBe(null);
  // A role's spelling for the token is the token.
  expect(editable({ source: 'authored', token: token(['secondary-label'], false, '-exact-secondary-label') }, 'own')).toMatchObject({ rung: 'token', token: 'secondary-label' });
  // A synthetic token, named by its whole expression: no rung, no link.
  expect(editable({ source: 'authored', token: token(['color-mix(in srgb, ink 60%, black)']) }, 'own')).toMatchObject({ rung: null, link: null });
  // A row the map does not place: read-only.
  expect(editable({ source: 'authored', token: token(['ink']) }, null).rung).toBe(null);
  expect(editable({ source: 'authored', token: token(['ink']) }, 'tag')).toMatchObject({ rung: null });
  expect(editable({ source: 'authored' }, 'own').reason).toContain('the map gives no place');
  expect(editable({ source: 'initial' }, null).rung).toBe(null);
});

test('a stale selection is refused and the build asked again; a current one is joined, published and outlined on every build that can inspect', () => {
  const events = [], frames = new Map();
  const stream = () => { const got = []; const opened = channel.open(f => got.push(f)); return { got, ...opened }; };
  const map = { digest: 'p1', sources: [{ file: 'exact:a', sha256: 'x' }], nodes: [{ file: 'exact:a', line: 1, col: 1, end_col: 2, component: 'App', chain: [], bindings: [] }] };
  const channel = buildChannel({
    send: e => events.push(e),
    selected: (build, report, ask) => {
      if (!current(build, report)) { ask({ kind: 'inspect', refresh: true }); return { status: 409, body: { message: 'stale' } }; }
      const s = join(build, report.selected, mapFor(Buffer.from(JSON.stringify(map)), report.selected.planDigest));
      events.push({ event: 'selection', ...s });
      channel.outline('tool', () => ({ sites: sitesAt(map, s.source), plan: map.digest }));
      return null;
    },
  });
  const a = stream();
  channel.report(JSON.stringify({ nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g1', design: 'd1' }));
  channel.report(JSON.stringify({ nonce: a.nonce, kind: 'status', scheme: 'light', revision: 0 }));
  const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [] };
  // Not in inspect mode (never on, or ended while the report was on its
  // way): not published.
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 })).status).toBe(409);
  channel.inspect('tool', null, { on: true });
  // From before a batch (another design): refused, and asked again.
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected, plan: 'g1', design: 'd0', revision: 0 })).status).toBe(409);
  expect(a.got.at(-1)).toEqual({ kind: 'inspect', refresh: true });
  expect(events.some(e => e.event === 'selection')).toBe(false);
  // Current, under the command it names: joined, published, and outlined.
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected, inspect: 1, plan: 'g1', design: 'd1', revision: 0 })).status).toBe(204);
  expect(events.find(e => e.event === 'selection')).toMatchObject({ build: 'web-1', source: { file: 'exact:a', line: 1 }, readOnly: null });
  expect(a.got.at(-1)).toEqual({ kind: 'outline', sites: [0], plan: 'p1' });
  // A selection made under a command since replaced is refused, and an
  // exit from it publishes nothing.
  channel.inspect('agent', 'web-1', { on: true });
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected, inspect: 1, plan: 'g1', design: 'd1', revision: 0 })).status).toBe(409);
  events.length = 0;
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected: null, inspect: 1 })).status).toBe(204);
  expect(events).toEqual([]);
  // The outline is its client's: cleared when that client goes.
  channel.clear('tool');
  expect(a.got.filter(f => f.kind === 'outline').at(-1)).toEqual({ kind: 'outline', clear: true });
});

test('a map pulled from the resident loop is kept only when it matches the card its pipe said, and its plan', async () => {
  const plan = 'a'.repeat(64);
  let body = Buffer.from(JSON.stringify({ digest: plan, nodes: [] })), hosts = [];
  const sha = () => createHash('sha256').update(body).digest('hex');
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: req => {
    hosts.push(req.headers.get('host'));
    // (Any map path answers the body: a card whose hash is not the bytes' must still refuse them.)
    return new URL(req.url).pathname.startsWith('/__dev/map/') ? new Response(body) : new Response('', { status: 404 });
  } });
  const card = () => { const c = mapCard(JSON.stringify({ plan, map: { sha256: sha(), bytes: body.length, url: `/__dev/map/${sha()}` } })); return { plan: c.plan, ...c.map }; };
  try {
    expect((await pullMap(server.port, card(), '127.0.0.1:8765'))?.digest).toBe(plan);
    // Asked as the child's gate answers: the public origin's host.
    expect(hosts.at(-1)).toBe('127.0.0.1:8765');
    // Bytes that are not the card's (another process on the port): none.
    expect(await pullMap(server.port, { ...card(), sha256: 'b'.repeat(64), url: `/__dev/map/${'b'.repeat(64)}` }, '127.0.0.1:8765')).toBe(null);
    // Any other path, or an absolute URL: not asked at all.
    const asked = hosts.length;
    expect(await pullMap(server.port, { ...card(), url: 'http://elsewhere/x' }, '127.0.0.1:8765')).toBe(null);
    expect(hosts.length).toBe(asked);
    // A map of another plan: none.
    body = Buffer.from(JSON.stringify({ digest: 'c'.repeat(64), nodes: [] }));
    expect(await pullMap(server.port, card(), '127.0.0.1:8765')).toBe(null);
    // A plan published with no map; a line that is no card.
    expect(mapCard(JSON.stringify({ plan, map: null }))).toEqual({ plan, map: null });
    expect(mapCard('source map nope')).toBe(null);
  } finally { server.stop(true); }
});

test('an own or class row carries the attribute it came from, with that file\'s digest (set-row\'s address, tamber/1.12)', () => {
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-'));
  try {
    const app = joinPath(dir, 'app.contract'), style = joinPath(dir, 'card.contract');
    writeFileSync(app, 'app'); writeFileSync(style, 'card');
    const at = (file, line, col) => ({ file, line, col, end_col: col + 7 });
    const map = { digest: 'p1', sources: [{ file: app, sha256: sha('app') }, { file: style, sha256: sha('card') }], nodes: [
      { file: app, line: 3, col: 5, end_col: 11, component: 'App', chain: [], bindings: [
        { row: 'padding_top', origin: 'own', at: at(app, 3, 12), attr: 'padding' },
        { row: 'background_color', origin: 'class:Raised', at: at(style, 2, 3), attr: 'background-color' },
        { row: 'display', origin: 'tag' },
      ] },
    ] };
    const selected = { node: 1, site: 0, instance: [], tuple: {}, rows: [
      { name: 'padding_top', value: 4, source: 'authored' },
      { name: 'background_color', value: '#fff', source: 'authored' },
      { name: 'display', value: 'flex', source: 'authored' },
    ] };
    const rows = join({ id: 'web-1' }, selected, map).rows;
    expect(rows[0]).toMatchObject({ at: { ...at(app, 3, 12), sha256: sha('app') }, attr: 'padding' });
    expect(rows[0].state).toBeUndefined();
    expect(rows[1]).toMatchObject({ at: { ...at(style, 2, 3), sha256: sha('card') }, attr: 'background-color' });
    expect(rows[2].at).toBeUndefined();
    // An attribute's place is no node's: the sites at it are none.
    expect(sitesAt(map, at(app, 3, 12))).toEqual([]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
