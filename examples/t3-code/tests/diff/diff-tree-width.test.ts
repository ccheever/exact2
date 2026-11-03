// The Changes and PR Code file trees resize from their left edge (T3 Code 5d3d78ab93, #28244: FileExplorerPane.tsx with
// defaultWidthFraction, useResizableWidth.ts): until a width is chosen each takes 40% of its row (Changes up to 256 and
// at least 160, PR Code up to 320 and at least 192); the width a drag or a key ends on is shared, saved as
// "t3code:diff-file-tree-width", here the preference file's `diffFileTreeWidth`.
import { describe, expect, test } from 'bun:test';
import { adoptFilesPrefs } from '../../src/panel/files-prefs';
import { diffTreeWidth, setDiffTreeWidth } from '../../src/diff/diff-tree-width';
import { contractComponent, contractFns } from '../support/contract-action-fixture';

const fns = contractFns('tree-resize.contract', { max: Math.max, min: Math.min });
const width = fns.treePaneWidth as (row: number, saved: number, base: number, least: number) => number;
const most = fns.treePaneMax as (row: number, base: number, least: number) => number;

describe('the file tree width', () => {
  test('its default is 40% of the row between the least and the base; a saved width is held to the room', () => {
    expect([width(500, -1, 256, 160), width(800, -1, 256, 160), width(300, -1, 256, 160)]).toEqual([200, 256, 160]);
    expect([width(500, -1, 320, 192), width(1000, -1, 320, 192)]).toEqual([200, 320]);
    // The most leaves the content 256, but never less than the default, and never over 560.
    expect([most(500, 256, 160), most(300, 256, 160), most(1200, 256, 160)]).toEqual([244, 160, 560]);
    expect([width(800, 500, 256, 160), width(800, 100, 256, 160), width(500, 480, 256, 160)]).toEqual([500, 160, 244]);
    // Before the row is measured: the base, the most 560.
    expect([width(0, -1, 256, 160), most(0, 256, 160)]).toEqual([256, 560]);
  });

  test('the Changes tree saves the width a drag ends on, and arrows, Home and End choose within its room', () => {
    const tree = contractComponent('diff-tree.contract', 'DiffTree'), sent: unknown[][] = [];
    const scope = { ...fns, max: Math.max, min: Math.min, rowWidth: 800, data: { diffTreeWidth: -1 }, command: (...args: unknown[]) => sent.push(args) };
    const state = tree.state(scope);
    expect(tree.derives(state, scope)).toMatchObject({ width: 256, most: 544 });
    tree.call('resize', state, scope, [-40, 0]);
    expect(tree.derives(state, scope).width).toBe(296);
    tree.call('resized', state, scope, [0, 0]);
    tree.call('set', state, scope, [9999]);
    expect(sent).toEqual([['diff-view', 'tree-width', '296', 0], ['diff-view', 'tree-width', '544', 0]]);
  });

  test('PR Code reads the same saved width; the preference file keeps it', () => {
    const client = { local: {} as object };
    expect(diffTreeWidth(client)).toBe(-1);
    setDiffTreeWidth(client, '301.6');
    setDiffTreeWidth(client, 'wide');
    expect(diffTreeWidth(client)).toBe(302);
    const next = {} as { diffFileTreeWidth?: number };
    adoptFilesPrefs(next, { diffFileTreeWidth: 302 });
    expect(next.diffFileTreeWidth).toBe(302);
  });
});
