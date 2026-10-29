// An import's accent offered as the packages' brand fills (*1.18*, theme2's brand-fill ruling): FollowButton's
// `actions.follow-fill` and EmptyState's `cards.empty-button`, each with its label's ink, white or black per scheme,
// whichever reaches 4.5:1. A list of its own beside `offers`, so a tool that reads only `offers` never shows one and
// never takes one; off until ticked; accepted end to end through the tool.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { ROOT, drive, proxied, toolTurns } from './design-tool-harness.mjs';
import { importReview } from './tamber-import-review.mjs';

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
