// An import's accent offered as the packages' brand fills (*1.18*, theme2's brand-fill ruling): FollowButton's
// `actions.follow-fill` and EmptyState's `cards.empty-button`, each with its label's ink, white or black per scheme,
// whichever reaches 4.5:1. A list of its own beside `offers`, so a tool that reads only `offers` never shows one and
// never takes one; off until ticked; accepted end to end through the tool.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ROOT, drive, proxied, toolTurns } from './design-tool-harness.mjs';
import { importReview } from './tamber-import-review.mjs';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

toolTurns();

const css = readFileSync(resolve(ROOT, 'tamber/tests/import/shadcn-neutral.css'), 'utf8');
const ROLES = ['-exact-background', '-exact-label', '-exact-secondary-background', '-exact-separator', 'AccentColor'];

/** library-gate's design and app (it uses @exact/actions' FollowButton and @exact/cards' EmptyState), outside the
 * checkout, its packages reached through the checkout's node_modules. */
function gate() {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-brand-'));
  for (const f of ['app.contract', 'app.json', 'app.tamber']) cpSync(resolve(ROOT, 'apps/library-gate', f), join(app, f));
  symlinkSync(resolve(ROOT, 'node_modules'), join(app, 'node_modules'));
  return app;
}

/** `contract tamber import-plan` of shadcn's neutral theme into `app`, as the wire plans one, held as its review. */
function planned(app) {
  const compiler = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug/contract');
  const r = spawnSync(compiler, ['tamber', 'import-plan', join(app, 'app.tamber')], { input: JSON.stringify({ request: { kind: 'shadcn', source: css, name: 'globals.css' }, design: readFileSync(join(app, 'app.tamber'), 'utf8') }), encoding: 'utf8' });
  const out = JSON.parse(r.stdout);
  expect(out.error, JSON.stringify(out.error)).toBeUndefined();
  return importReview(out.ok);
}

test('the review lists both brand fills after the role overrides, each its fill and ink, off until ticked', () => {
  const app = gate();
  try {
    const { ops, review } = planned(app);
    expect(review.offers.map(o => o.role)).toEqual(ROLES);
    expect(review.brands.map(b => b.sets)).toEqual(['actions.follow-fill', 'cards.empty-button']);
    const offered = review.offers.flatMap(o => o.items);
    for (const b of review.brands) {
      expect(b.token).toBe('shadcn.primary');
      expect(b.role).toBeUndefined();
      expect(b.note).toBeUndefined();
      // Its items after the tokens and the role overrides: the fill set to the accent, then its ink.
      expect(b.items.every(i => i >= review.tokens && !offered.includes(i))).toBe(true);
      expect(ops[b.items[0]]).toMatchObject({ op: 'set-token', name: b.sets, value: 'shadcn.primary' });
      // shadcn's neutral primary is near black in light and near white in dark: white ink, then black.
      expect(ops[b.items[1]]).toMatchObject({ op: 'set-token', value: 'light-dark(#ffffff, #000000)' });
      expect(b.contrast.light).toBeGreaterThanOrEqual(4.5);
      expect(b.contrast.dark).toBeGreaterThanOrEqual(4.5);
    }
  } finally { rmSync(app, { recursive: true, force: true }); }
});

test('an older tool, reading only `offers`, shows no row for a brand offer and its accept takes none', () => {
  const app = gate();
  try {
    const { review } = planned(app);
    // The tool's mapping before brand offers (apps/design-tool/app.ts at bb43e8cb05, verbatim): every offer with
    // items or a note is a row, labelled by its role.
    const rows = (review.offers ?? []).filter(o => (o.items ?? []).length > 0 || o.note).map(o => ({ role: String(o.role), token: String(o.token ?? ''), items: (o.items ?? []).join(',') }));
    expect(rows.map(r => r.role)).toEqual(ROLES);
    expect(rows.some(r => r.role === 'undefined' || r.role === '')).toBe(false);
    // Its accept names the items it keeps (choice.ts at bb43e8cb05): the tokens, and each override it ticked.
    const kept = new Set([...Array.from({ length: review.tokens }, (_, i) => i), ...review.offers.flatMap(o => o.items)]);
    expect(review.brands.flatMap(b => b.items).filter(i => kept.has(i))).toEqual([]);
  } finally { rmSync(app, { recursive: true, force: true }); }
});

test('ticked in the tool and accepted, FollowButton\'s fill is the accent and its ink the chosen one; EmptyState\'s, unticked, is left', async () => {
  const app = gate();
  const original = readFileSync(join(app, 'app.tamber'), 'utf8');
  const tool = await proxied(app);
  try {
    const picked = join(app, 'globals.css');
    cpSync(resolve(ROOT, 'tamber/tests/import/shadcn-neutral.css'), picked);
    const first = await drive(tool.url, `design-tool-brand-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data',
      'type import-kind shadcn', 'tap import-choose', `type @import-file ${picked}`, 'clock data', 'clock +1', 'clock data', 'state',
    ]);
    const review = first.filter(r => r.resources)[0].resources.changes.held.find(h => h.importer === 'shadcn');
    expect(review.brands.map(b => [b.sets, b.on])).toEqual([['actions.follow-fill', false], ['cards.empty-button', false]]);
    expect(review.brands[0].words).toMatch(/^its label \d+(\.\d+)?:1 light, \d+(\.\d+)?:1 dark$/);
    const id = review.proposal, again = id.replace(/-(\d+)$/, (_, n) => `-${Number(n) + 1}`);
    // Tick FollowButton's: the accept names some of the items, so it is held again with them, then accepted.
    await drive(tool.url, `design-tool-brand-${process.pid}`, [
      'clock data', 'clock +1', 'clock data',
      `tap brand-take-${id}-actions.follow-fill`, `tap accept-${id}`, 'clock data', 'clock +1', 'clock data',
      `tap accept-${again}`, 'clock data', 'clock +1', 'clock data', 'state',
    ], { keepStorage: true });
    expect(tool.writes.map(w => [w.method, w.ok])).toEqual([['accept', true], ['accept', true]]);
    expect(tool.writes[0].after).toBe(original);
    const written = tool.writes[1].after;
    expect(written).toContain('actions.follow-fill = shadcn.primary');
    expect(written).toContain('actions.follow-ink = light-dark(#ffffff, #000000)');
    expect(written).not.toContain('cards.empty-button');
    // The design still checks, and no pair falls below 4.5:1.
    const compiler = resolve(process.env.CARGO_TARGET_DIR ?? resolve(ROOT, 'target'), 'debug/contract');
    const checked = spawnSync(compiler, ['tamber', 'check', app], { encoding: 'utf8' });
    expect(checked.status, checked.stderr + checked.stdout).toBe(0);
    expect(checked.stderr + checked.stdout).not.toContain('4.5:1');
  } finally {
    await tool.close();
    rmSync(app, { recursive: true, force: true });
  }
}, 900_000);

/** The design server on `app`, in process, and a call of its wire. */
async function wire(app) {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port, gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const server = tamberServer({ appDir: app, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => { if (!(await server.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); } });
  await server.ready;
  const at = `http://127.0.0.1:${port}`, { token } = await (await fetch(`${at}/__tamber/token`)).json();
  const call = async request => { const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) }); return { status: res.status, ...(await res.json()) }; };
  return { call, close: async () => { await server.close(); listener.close(); } };
}

/** A shadcn theme whose `--primary` is `primary` in both schemes. */
const accentTheme = primary => `:root {\n  --background: #ffffff;\n  --primary: ${primary};\n}\n.dark {\n  --background: #000000;\n  --primary: ${primary};\n}\n`;

test('a bare accept takes a brand offer whose ink reaches 4.5:1, never one that falls short unless it is named', async () => {
  for (const [primary, landsBare] of [['#006aff', true], ['rgb(0 106 255 / 50%)', false]]) {
    const app = gate(), server = await wire(app);
    try {
      const held = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme(primary), name: 'globals.css' });
      expect(held.status, JSON.stringify(held)).toBe(200);
      const { review, ops } = held.ok;
      expect(review.brands.map(b => b.sets)).toEqual(['actions.follow-fill', 'cards.empty-button']);
      expect(review.brands.every(b => (b.note === undefined) === landsBare)).toBe(true);
      const bare = await server.call({ method: 'accept', client: 'agent:t', proposal: held.ok.proposal });
      expect(bare.status, JSON.stringify(bare)).toBe(200);
      const written = readFileSync(join(app, 'app.tamber'), 'utf8');
      expect(written).toContain('shadcn.primary =');
      expect(written.includes('actions.follow-fill = shadcn.primary')).toBe(landsBare);
      expect(written.includes('cards.empty-button = shadcn.primary')).toBe(landsBare);
      if (!landsBare) {
        // Named, every item: it lands, warning and all.
        const again = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme(primary), name: 'globals.css' });
        // A re-import whose only news is its brand offers is held for them.
        expect(again.ok.review.brands.map(b => b.sets)).toEqual(['actions.follow-fill', 'cards.empty-button']);
        const all = again.ok.ops.map(o => o.index);
        expect((await server.call({ method: 'accept', client: 'agent:t', proposal: again.ok.proposal, items: all })).status).toBe(200);
        expect(readFileSync(join(app, 'app.tamber'), 'utf8')).toContain('actions.follow-fill = shadcn.primary');
      }
      expect(ops.length).toBeGreaterThan(0);
    } finally { await server.close(); rmSync(app, { recursive: true, force: true }); }
  }
}, 600_000);

test('a partial accept holds again with the brand offers it kept, renumbered to their operations', async () => {
  const app = gate(), server = await wire(app);
  try {
    const held = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: css, name: 'globals.css' });
    const { review } = held.ok, follow = review.brands.find(b => b.sets === 'actions.follow-fill');
    const items = [...Array.from({ length: review.tokens }, (_, i) => i), ...follow.items];
    const re = await server.call({ method: 'accept', client: 'agent:t', proposal: held.ok.proposal, items });
    expect(re.ok?.recomputed, JSON.stringify(re).slice(0, 400)).toBe(true);
    // Only FollowButton's is offered again, its items the fill and the ink in the new operations.
    expect(re.ok.review.brands.map(b => b.sets)).toEqual(['actions.follow-fill']);
    const [fill, ink] = re.ok.review.brands[0].items.map(i => re.ok.ops[i].op);
    expect([fill.name, fill.value]).toEqual(['actions.follow-fill', 'shadcn.primary']);
    expect(ink.name).toBe('actions.follow-ink');
  } finally { await server.close(); rmSync(app, { recursive: true, force: true }); }
}, 600_000);

test('a warned brand chosen in a partial accept lands on the bare accept of the proposal held again; one kept without its accent does not', async () => {
  const app = gate(), server = await wire(app);
  try {
    const held = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('rgb(0 106 255 / 50%)'), name: 'globals.css' });
    const { review } = held.ok, follow = review.brands.find(b => b.sets === 'actions.follow-fill');
    expect(follow.note).toContain('not opaque');
    const tokens = Array.from({ length: review.tokens }, (_, i) => i);
    const re = await server.call({ method: 'accept', client: 'agent:t', proposal: held.ok.proposal, items: [...tokens, ...follow.items] });
    expect(re.ok.review.brands.map(b => b.sets)).toEqual(['actions.follow-fill']);
    // Chosen, so a bare accept of the proposal held again takes it, warning and all.
    expect((await server.call({ method: 'accept', client: 'agent:t', proposal: re.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(app, 'app.tamber'), 'utf8')).toContain('actions.follow-fill = shadcn.primary');
  } finally { await server.close(); rmSync(app, { recursive: true, force: true }); }
  const app2 = gate(), server2 = await wire(app2);
  try {
    const held = await server2.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('#006aff'), name: 'globals.css' });
    const { review, ops } = held.ok, follow = review.brands.find(b => b.sets === 'actions.follow-fill');
    const accent = ops.findIndex(o => o.op.name === 'shadcn.primary');
    expect(accent).toBeGreaterThanOrEqual(0);
    // The brand without its accent: left out with it, so nothing names an accent it wasn't measured on.
    const items = [...Array.from({ length: review.tokens }, (_, i) => i).filter(i => i !== accent), ...follow.items];
    const re = await server2.call({ method: 'accept', client: 'agent:t', proposal: held.ok.proposal, items });
    expect(re.ok.review.brands ?? []).toEqual([]);
    expect(re.ok.ops.some(o => o.op.name === 'actions.follow-fill')).toBe(false);
  } finally { await server2.close(); rmSync(app2, { recursive: true, force: true }); }
}, 600_000);

test('an edit of the accent drops the brand offers set to it, operations and all, and says why', async () => {
  const app = gate(), server = await wire(app);
  try {
    const held = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('#006aff'), name: 'globals.css' });
    const index = held.ok.ops.findIndex(o => o.op.name === 'shadcn.primary');
    const edited = await server.call({ method: 'edit-held', client: 'agent:t', proposal: held.ok.proposal, index, value: '#ffd400' });
    expect(edited.status, JSON.stringify(edited).slice(0, 400)).toBe(200);
    const { review } = edited.ok;
    expect(review.brands ?? []).toEqual([]);
    expect((review.notes ?? []).join(' ')).toContain('actions.follow-fill and cards.empty-button aren\'t offered here');
    // Its operations gone with it: a bare accept writes no brand fill with an ink chosen for the old accent.
    expect(edited.ok.ops.some(o => /^(actions\.follow|cards\.empty-button)/.test(o.op.name ?? ''))).toBe(false);
    expect((await server.call({ method: 'accept', client: 'agent:t', proposal: edited.ok.proposal })).status).toBe(200);
    const written = readFileSync(join(app, 'app.tamber'), 'utf8');
    expect(written).toContain('shadcn.primary = #ffd400');
    expect(written).not.toContain('actions.follow');
  } finally { await server.close(); rmSync(app, { recursive: true, force: true }); }
}, 600_000);

test('a brand kept in part is left out; a chosen one stays chosen through an unrelated edit; one the design moved under goes', async () => {
  const app = gate(), server = await wire(app);
  try {
    // The fill without its ink: left out whole, with a note.
    const held = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('#006aff'), name: 'globals.css' });
    const { review } = held.ok, follow = review.brands.find(b => b.sets === 'actions.follow-fill');
    const tokens = Array.from({ length: review.tokens }, (_, i) => i);
    const part = await server.call({ method: 'accept', client: 'agent:t', proposal: held.ok.proposal, items: [...tokens, follow.items[0]] });
    expect(part.ok.review.brands ?? []).toEqual([]);
    expect(part.ok.ops.some(o => o.op.name === 'actions.follow-fill')).toBe(false);
    expect(part.ok.review.notes.join(' ')).toContain('actions.follow-fill isn\'t offered here');
    await server.call({ method: 'drop', client: 'agent:t', proposal: part.ok.proposal });
  } finally { await server.close(); rmSync(app, { recursive: true, force: true }); }
  const app2 = gate(), server2 = await wire(app2);
  try {
    // A warned brand chosen in a partial accept, then an unrelated edit: still chosen, so the bare accept takes it.
    const held = await server2.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('rgb(0 106 255 / 50%)'), name: 'globals.css' });
    const { review } = held.ok, follow = review.brands.find(b => b.sets === 'actions.follow-fill');
    const re = await server2.call({ method: 'accept', client: 'agent:t', proposal: held.ok.proposal, items: [...Array.from({ length: review.tokens }, (_, i) => i), ...follow.items] });
    const background = re.ok.ops.findIndex(o => o.op.name === 'shadcn.background');
    const edited = await server2.call({ method: 'edit-held', client: 'agent:t', proposal: re.ok.proposal, index: background, value: 'light-dark(#fefefe, #010101)' });
    expect(edited.ok.review.brands.map(b => b.sets)).toEqual(['actions.follow-fill']);
    expect((await server2.call({ method: 'accept', client: 'agent:t', proposal: edited.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(app2, 'app.tamber'), 'utf8')).toContain('actions.follow-fill = shadcn.primary');
  } finally { await server2.close(); rmSync(app2, { recursive: true, force: true }); }
  const app3 = gate(), server3 = await wire(app3);
  try {
    // The accent already the design's (a brand-only re-import), then the design's accent changes under the held
    // proposal: its accept conflicts, and the recompute leaves the brands out, their inks measured on the old value.
    const first = await server3.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('#006aff'), name: 'globals.css' });
    const items = [...Array.from({ length: first.ok.review.tokens }, (_, i) => i), ...first.ok.review.offers.flatMap(o => o.items)];
    const whole = await server3.call({ method: 'accept', client: 'agent:t', proposal: first.ok.proposal, items });
    if (whole.ok?.recomputed) await server3.call({ method: 'accept', client: 'agent:t', proposal: whole.ok.proposal });
    const again = await server3.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('#006aff'), name: 'globals.css' });
    expect(again.ok.review.brands.map(b => b.sets)).toEqual(['actions.follow-fill', 'cards.empty-button']);
    const text = readFileSync(join(app3, 'app.tamber'), 'utf8');
    writeFileSync(join(app3, 'app.tamber'), text.replace('shadcn.primary = #006aff', 'shadcn.primary = #ffd400'));
    await Bun.sleep(1500);
    const moved = await server3.call({ method: 'accept', client: 'agent:t', proposal: again.ok.proposal });
    expect(moved.ok?.recomputed, JSON.stringify(moved).slice(0, 400)).toBe(true);
    expect(moved.ok.review.brands ?? []).toEqual([]);
    expect(moved.ok.ops.some(o => /^actions\.follow/.test(o.op.name ?? ''))).toBe(false);
  } finally { await server3.close(); rmSync(app3, { recursive: true, force: true }); }
}, 900_000);

test('a brand is held again only whole, and leaving its operations renumbers the rest', async () => {
  const { brandsLeftOut, withoutItems } = await import('./tamber-brands.mjs');
  const review = { tokens: 2, offers: [{ role: 'AccentColor', items: [1] }], brands: [{ sets: 'actions.follow-fill', items: [2, 3] }, { sets: 'cards.empty-button', items: [4, 5] }] };
  expect(brandsLeftOut(review, [0, 1, 2, 3]).gone).toEqual([]);
  expect(brandsLeftOut(review, [0, 1, 2, 4, 5]).gone).toEqual(['actions.follow-fill']);
  const { ops, review: after, choice } = withoutItems(['a', 'b', 'c', 'd', 'e', 'f'], review, { choice: [0, 3, 4] }, new Set([2, 3]));
  expect(ops).toEqual(['a', 'b', 'e', 'f']);
  expect(after.brands).toEqual([{ sets: 'cards.empty-button', items: [2, 3] }]);
  expect(after.offers[0].items).toEqual([1]);
  expect(choice.choice).toEqual([0, 2]);
});

test('measured again on the design as it is: an edited ink, a moved accent, or an alias changed under it drops the brand, operations and all', async () => {
  const app = gate(), server = await wire(app);
  try {
    // An edit of the brand's own ink to its fill's blue: the measure no longer finds it white, so it goes.
    const held = await server.call({ method: 'import', client: 'agent:t', kind: 'shadcn', source: accentTheme('#006aff'), name: 'globals.css' });
    const ink = held.ok.ops.findIndex(o => o.op.name === 'actions.follow-ink');
    const edited = await server.call({ method: 'edit-held', client: 'agent:t', proposal: held.ok.proposal, index: ink, value: '#006aff' });
    expect(edited.status, JSON.stringify(edited).slice(0, 300)).toBe(200);
    expect(edited.ok.review.brands.map(b => b.sets)).toEqual(['cards.empty-button']);
    expect(edited.ok.ops.some(o => /^actions\.follow/.test(o.op.name ?? ''))).toBe(false);
    await server.call({ method: 'drop', client: 'agent:t', proposal: edited.ok.proposal });
  } finally { await server.close(); rmSync(app, { recursive: true, force: true }); }
}, 600_000);
