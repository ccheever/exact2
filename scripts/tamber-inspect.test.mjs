// The server's half of inspect (LLP 1122 D5; T2b step 5): currency, the join
// to source with each file checked against its compiled digest, each row's
// editability, and the sites to outline.
import { expect, test } from 'bun:test';
import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join as joinPath } from 'node:path';
import { buildChannel } from './tamber-builds.mjs';
import { current, editable, join, mapFor, sitesAt, staleMap } from './tamber-inspect.mjs';

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
      channel.outline('tool', sitesAt(map, s.source), map.digest);
      return null;
    },
  });
  const a = stream();
  channel.report(JSON.stringify({ nonce: a.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin', 'inspect'], plan: 'g1', design: 'd1' }));
  channel.report(JSON.stringify({ nonce: a.nonce, kind: 'status', scheme: 'light', revision: 0 }));
  const selected = { node: 2, site: 0, instance: [], planDigest: 'p1', tuple: {}, rows: [] };
  // From before a batch (another design): refused, and asked again.
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected, plan: 'g1', design: 'd0', revision: 0 })).status).toBe(409);
  expect(a.got.at(-1)).toEqual({ kind: 'inspect', refresh: true });
  expect(events.some(e => e.event === 'selection')).toBe(false);
  // Current: joined, published, and outlined.
  expect(channel.report(JSON.stringify({ nonce: a.nonce, kind: 'selected', selected, plan: 'g1', design: 'd1', revision: 0 })).status).toBe(204);
  expect(events.find(e => e.event === 'selection')).toMatchObject({ build: 'web-1', source: { file: 'exact:a', line: 1 }, readOnly: null });
  expect(a.got.at(-1)).toEqual({ kind: 'outline', sites: [0], plan: 'p1' });
  // The outline is its client's: cleared when that client goes.
  channel.clear('tool');
  expect(a.got.at(-1)).toEqual({ kind: 'outline', clear: true });
});
