// The edit dock (the design tool's third hands-on pass, #7; theme2's ruling): a grid cell's editor opened about a screen
// below the grid, after the icons. Now one dock at the window's bottom holds the selected cell's editor and the edit's
// proposal; the selected cell is marked, for a screen reader too (aria-current), and scrolled clear of the dock; a press
// moves the focus into the dock's editor, and Escape (or Close) with nothing pending gives it back to the cell.
import { expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { drive, proxied, ROOT, STUB_APP, toolTurns } from './design-tool-harness.mjs';

toolTurns();

test('a cell pressed opens its editor in the dock, in the window, the cell marked and clear of it; Escape gives the focus back', async () => {
  const app = mkdtempSync(join(tmpdir(), 'exact-design-tool-edit-dock-'));
  // A long token list, as the clone has: the editor's old place was below it all.
  const many = Array.from({ length: 120 }, (_, i) => `  filler.t${i} = #${String(i).padStart(6, '0')}`).join('\n');
  writeFileSync(join(app, 'app.tamber'), `${readFileSync(resolve(ROOT, 'apps/design-tool/stub/dials.tamber'), 'utf8').trimEnd()}\n${many}\n`);
  writeFileSync(join(app, 'app.contract'), STUB_APP);
  // The shot theme2 asked for (a 900 px window), kept where a run can look at it.
  const shots = process.env.EXACT_DOCK_SHOTS ?? mkdtempSync(join(tmpdir(), 'exact-design-tool-edit-dock-shots-'));
  mkdirSync(shots, { recursive: true });
  const tool = await proxied(app);
  try {
    const cell = 'cell-surface.page-0-light', wait = ['clock data', 'clock +1', 'clock data', 'clock settle', 'clock data'];
    const replies = await drive(tool.url, `design-tool-edit-dock-${process.pid}`, [
      'resize 1280x900',
      'type name Ada', 'tap save-name', ...wait, ...wait,
      `tap ${cell}`, ...wait, 'tree', 'layout', `screenshot ${join(shots, 'edit-dock-900.png')}`,
      // Escape with nothing pending: the dock goes, the focus back on the cell.
      'type branch-value key Escape', ...wait, 'tree',
      // A tall editor (a shadow list's typed fields) and its draft's proposal joining it: the dock at its tallest, the
      // cell still clear above it.
      'tap cell-shadow.ring-0-light', ...wait, 'type part-0.spread-field 1px', ...wait, 'layout', `screenshot ${join(shots, 'edit-dock-900-tall.png')}`,
      // Cancel: the pressed button gone, the focus stays in the dock, where Escape reaches it.
      'tap cancel-edit', ...wait, 'tree',
      // A token that follows others: the dock focused (no field), then Pin brings the field and the focus into it.
      'tap cell-accent.fill-0-light', ...wait, 'tree', 'tap pin', ...wait, 'tree',
    ]);
    const trees = replies.filter(r => r.nodes && !('x' in (r.nodes[0] ?? {}))), layouts = replies.filter(r => r.nodes?.some(n => 'x' in n));
    const one = (t, id) => t.nodes.find(n => n.props?.testId === id);
    const rect = (l, id) => l.nodes.find(n => n.testId === id);
    // The editor in the dock, its value field focused (the next key is the field's).
    expect(one(trees[0], 'edit-dock')).toBeDefined();
    expect(one(trees[0], 'branch')).toBeDefined();
    expect(one(trees[0], 'branch-value')?.focused).toBe(true);
    // The cell says it is the one selected, not only by its ring's colour; the others don't.
    const marked = trees[0].nodes.filter(n => /^cell-/.test(n.props?.testId ?? '') && n.props.accessibilityCurrent === 'true');
    expect(marked.map(n => n.props.testId)).toEqual([cell]);
    // Docked in the window, the cell whole above it.
    const [dock, at, view] = [rect(layouts[0], 'edit-dock'), rect(layouts[0], cell), layouts[0].viewport];
    expect(view.h).toBe(900);
    expect(dock.y).toBeGreaterThanOrEqual(0);
    expect(dock.y + dock.h).toBeLessThanOrEqual(view.h + 1);
    expect(at.y).toBeGreaterThanOrEqual(0);
    expect(at.y + at.h).toBeLessThanOrEqual(dock.y);
    // Escape: no dock, the focus on the cell again.
    expect(one(trees[1], 'edit-dock')).toBeUndefined();
    expect(one(trees[1], cell)?.focused).toBe(true);
    const [tall, ring] = [rect(layouts[1], 'edit-dock'), rect(layouts[1], 'cell-shadow.ring-0-light')];
    expect(rect(layouts[1], 'proposal')).toBeDefined();
    expect(tall.y + tall.h).toBeLessThanOrEqual(view.h + 1);
    expect(ring.y).toBeGreaterThanOrEqual(0);
    expect(ring.y + ring.h).toBeLessThanOrEqual(tall.y);
    expect(one(trees[2], 'edit-dock')?.focused).toBe(true);
    expect(one(trees[3], 'branch-value')).toBeUndefined();
    expect(one(trees[3], 'edit-dock')?.focused).toBe(true);
    expect(one(trees[4], 'branch-value')?.focused).toBe(true);
  } finally { await tool.close(); rmSync(app, { recursive: true, force: true }); }
}, 1_200_000);
