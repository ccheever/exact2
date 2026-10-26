// The proposal panel docked at the window's bottom (the second hands-on pass, finding 2: a param's proposal opened
// about 16,500 px below its field, after the token list): seen wherever the edit began, never over the edited row,
// one panel; a typed draft keeps its field's focus, a press's proposal takes it (review2 06:54).
import { expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { drive, proxied, ROOT, STUB_APP, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('a param edit\'s proposal shows docked below its row, not over it, once; typing keeps the field\'s focus, a reset\'s takes the dock', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-dock-'));
  // A long token list after the components, as the clone has: the panel's own place is far below.
  const many = Array.from({ length: 120 }, (_, i) => `  filler.t${i} = #${String(i).padStart(6, '0')}`).join('\n');
  writeFileSync(join(app, 'app.tamber'), `${readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8').trimEnd()}\n${many}\n`);
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  const tool = await proxied(app);
  try {
    // Each step's proposal lands (`clock data` waits on its request), its ready task runs a commit later (`clock +1`),
    // and the focus it moves settles: the clock's own, where 4 s of real time stood.
    const field = 'dial-app.contract#Card.fill-field', wait = ['clock data', 'clock +1', 'clock data', 'clock settle', 'clock data'];
    const replies = await drive(tool.url, `design-tool-dock-${process.pid}`, [
      'type name Ada', 'tap save-name', 'clock data', 'clock +1', 'clock data',
      // A typed draft: its proposal comes, the field keeps the focus (the next key is the field's).
      `type ${field} #ff0000`, `type ${field} key Enter`, ...wait, 'tree', 'layout',
      'tap cancel-edit', ...wait,
      // A press (Back to the default): its proposal takes the focus, on the dock, once its Apply can be pressed.
      'tap param-Card.pad-reset', ...wait, 'tree', 'layout',
    ]);
    const trees = replies.filter(r => r.nodes && !('x' in (r.nodes[0] ?? {}))), layouts = replies.filter(r => r.nodes?.some(n => 'x' in n));
    const one = (t, id) => t.nodes.filter(n => n.props?.testId === id);
    const rect = (l, id) => l.nodes.find(n => n.testId === id);
    const overlap = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
    // The typed draft: one panel, said as ready, the field focused, Apply not.
    expect(one(trees[0], 'proposal')).toHaveLength(1);
    expect(one(trees[0], 'proposal-ready')[0]?.text ?? one(trees[0], 'proposal-ready')[0]?.props?.text).toBeDefined();
    expect(one(trees[0], field)[0]?.focused).toBe(true);
    expect(one(trees[0], 'apply')[0]?.focused).toBeUndefined();
    // Docked in the window (its own place is far below, after 120 more tokens), not over the field.
    const [dock, at, view] = [rect(layouts[0], 'proposal'), rect(layouts[0], field), layouts[0].viewport];
    expect(dock.y).toBeGreaterThanOrEqual(0);
    expect(dock.y + dock.h).toBeLessThanOrEqual(view.h + 1);
    // The edited field whole in the window, above the dock.
    expect(at.y).toBeGreaterThanOrEqual(0);
    expect(at.y + at.h).toBeLessThanOrEqual(dock.y);
    // The press's proposal: the dock focused (Tab reaches its Apply).
    expect(one(trees[1], 'proposal')).toHaveLength(1);
    expect(one(trees[1], 'proposal')[0]?.focused).toBe(true);
    const [dock2, row] = [rect(layouts[1], 'proposal'), rect(layouts[1], 'param-Card.pad')];
    expect(dock2.y + dock2.h).toBeLessThanOrEqual(layouts[1].viewport.h + 1);
    expect(row.y).toBeGreaterThanOrEqual(0);
    expect(row.y + row.h).toBeLessThanOrEqual(dock2.y);
    expect(overlap(dock2, row)).toBe(false);
  } finally { await tool.close(); rmSync(app, { recursive: true, force: true }); }
}, 1_200_000);
