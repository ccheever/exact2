// The server's half of inspect (LLP 1122 D5; T2b step 5): currency, the join
// to source with each file checked against its compiled digest, each row's
// editability, and the sites to outline.
import { expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join as joinPath } from 'node:path';
import { buildChannel } from './tamber-builds.mjs';
import { current, editable, join, mapCard, mapFor, paramsOf, published, pullMap, sitesAt, staleMap } from './tamber-inspect.mjs';

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
    expect(s.rows[1].editable.reason).toContain('template editing (T5)');
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
    writeFileSync(app, 'app');
    const part = { file: app, line: 9, col: 9, end_col: 20 };
    const map = { digest: 'p1', sources: [{ file: app, sha256: sha('app') }], nodes: [
      { file: app, line: 7, col: 5, end_col: 11, component: 'App', chain: [], bindings: [{ row: 'press_background_color', origin: 'own', state: 'active', at: part }, { row: 'press_color', origin: 'own', state: 'active', at: { ...part, line: 10 } }] },
    ] };
    const selected = { node: 1, site: 0, instance: [], rows: [
      { name: 'press_background_color', value: '#00f', source: 'authored', token: { names: ['accent.soft'], text: 'accent.soft', modes: '', implied: false } },
      { name: 'press_color', value: '#f00', source: 'authored' },
    ] };
    const s = join({ id: 'web-1' }, selected, map);
    expect(s.rows.map(r => [r.name, r.state, r.at?.line, r.editable.rung])).toEqual([
      ['press_background_color', 'active', 9, 'token'],
      ['press_color', 'active', 10, null],
    ]);
    expect(s.rows[1].editable.reason).toContain('set-state');
    // `uses` names the part's own place; outlining it finds the node.
    expect(sitesAt(map, part)).toEqual([0]);
    // What is held (LLP 1122 D8): an `:active` row at its part, named by its state.
    const held = join({ id: 'web-1' }, { ...selected, held: { state: 'active', rows: [{ name: 'press_background_color', value: '#00f' }, { name: 'press_scale', value: '0.95' }] } }, map);
    expect(held.held.rows.map(r => [r.name, r.state, r.source?.line])).toEqual([['press_background_color', 'active', 9], ['press_scale', 'active', 7]]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a row that reads a component param joins on the param rung, by its full name and the token it aliases; no IR param token reaches a client (LLP 1122 T3)', () => {
  const ir = { tokens: [{ name: 'primary_500', role: null }, { name: 'radius.md', role: null }, { name: '-exact-label', role: 'label' }], components: [{ identity: 'post.contract#PostButton', name: 'PostButton', params: [
    { name: 'post.contract#PostButton.fill', short: 'PostButton.fill', token: 'param.f1a2b.PostButton.fill', default: 'primary_500', set: null },
    { name: 'post.contract#PostButton.radius', short: 'PostButton.radius', token: 'param.f1a2b.PostButton.radius', default: 'radius.md', set: { value: '6px', whens: 0 } },
    { name: 'post.contract#PostButton.height', short: null, token: 'param.f1a2b.PostButton.height', default: '32px', set: null },
    { name: 'post.contract#PostButton.ink', short: 'PostButton.ink', token: 'param.f1a2b.PostButton.ink', default: 'label', set: null },
    { name: 'post.contract#PostButton.edge', short: 'PostButton.edge', token: 'param.f1a2b.PostButton.edge', default: 'primary_500', set: { value: 'radius.md', whens: 1 } },
  ] }] };
  const params = paramsOf(ir);
  // Its alias: the value the app has (the line's, else the default) when that is one declared token.
  expect(params.get('param.f1a2b.PostButton.fill')).toEqual({ param: 'post.contract#PostButton.fill', short: 'PostButton.fill', token: 'primary_500' });
  expect(params.get('param.f1a2b.PostButton.radius').token).toBe(null);
  // A role override's spelling names its token; a param with `when` lines aliases none.
  expect(params.get('param.f1a2b.PostButton.ink').token).toBe('-exact-label');
  expect(params.get('param.f1a2b.PostButton.edge').token).toBe(null);
  const dir = mkdtempSync(joinPath(tmpdir(), 'exact-inspect-param-'));
  const post = joinPath(dir, 'post.contract');
  writeFileSync(post, 'post');
  const map = { digest: 'p1', sources: [{ file: post, sha256: sha('post') }], nodes: [{ file: post, line: 9, col: 5, end_col: 11, component: 'PostButton', chain: [], bindings: [
    { row: 'background_color', origin: 'own' }, { row: 'height', origin: 'own' }, { row: 'border_color', origin: 'own' }, { row: 'color', origin: 'own' }] }] };
  const ir_ = n => `param.f1a2b.PostButton.${n}`;
  // A synthetic token is named by its whole expression, IR names inside it.
  const mix = `color-mix(in oklab, ${ir_('fill')} 50%, black)`;
  const selected = { node: 1, site: 0, instance: [], tuple: {}, rows: [
    { name: 'background_color', value: '#1083fe', source: 'authored', token: { names: [ir_('fill')], text: ir_('fill'), modes: 'dark-style=dim', implied: false } },
    { name: 'height', value: 32, source: 'authored', token: { names: [ir_('height')], text: ir_('height'), modes: '', implied: false } },
    { name: 'border_color', value: '#000', source: 'authored', token: { names: [mix], text: mix, modes: '', implied: false } },
    { name: 'color', value: '#fff', source: 'authored', token: { names: ['param.f9999.Other.x'], text: 'param.f9999.Other.x', modes: '', implied: false } },
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
  // Inherited, it links the param.
  expect(s.rows[4].editable).toMatchObject({ rung: null, link: 'post.contract#PostButton.fill' });
  expect(JSON.stringify(s)).not.toMatch(/param\.f[0-9a-f]+\./);
  expect(s.rows.some(r => 'unread' in r)).toBe(false);
  // The raw report the server publishes (`selected`) is scrubbed the same way.
  expect(JSON.stringify(published(selected, params))).not.toMatch(/param\.f[0-9a-f]+\./);
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
  expect(editable({ source: 'authored' }, 'own').reason).toContain('not a token');
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
