// The Files explorer beside a file resizes from its left edge (T3 Code 2359643cff: #25737, FileExplorerPane.tsx and
// useResizableWidth.ts): 160 to 560 wide, 256 at first, leaving the file 256; the width persists as
// "t3code.fileExplorerWidth", here the preference file's `files.explorerWidth`.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { filesLocal } from '../../src/panel/files-surface';
import { adoptFilesPrefs, filesPrefs } from '../../src/panel/files-prefs';
import { contractComponent } from '../support/contract-action-fixture';

const surface = contractComponent('files-surface.contract', 'FilesSurface');
const handle = contractComponent('files-surface.contract', 'ExplorerResizeHandle');
const native: Native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) };

/** FilesSurface over a 800-wide row beside a file, with the saved `width`, and the `local` ops it sent. */
function mounted(width = 256, row = 800) {
  const sent: unknown[][] = [];
  const scope = { max: Math.max, min: Math.min, files: { path: 'src/a.ts', preview: 'code', explorerWidth: width }, local: (...args: unknown[]) => sent.push(args) };
  const state = surface.state(scope);
  surface.call('rowSized', state, scope, [row, 600]);
  return { state, scope, sent };
}

describe('Files explorer width', () => {
  test('a drag from the left edge widens it, stays inside 160 to the room left beside 256 of file, and saves on release', () => {
    const { state, scope, sent } = mounted();
    surface.call('explorerResize', state, scope, [-100, 0]);
    expect(state.userWidth).toBe(356);
    surface.call('explorerResize', state, scope, [-1000, 0]);
    // 800 - 256 leaves the explorer 544 at most.
    expect(state.userWidth).toBe(544);
    surface.call('explorerResize', state, scope, [2000, 0]);
    expect(state.userWidth).toBe(160);
    surface.call('explorerResized', state, scope, [0, 0]);
    expect(sent).toEqual([['surface-files-explorer-width', '', '160']]);
  });

  test('a wide row caps it at 560, and a narrow one keeps 160', () => {
    const wide = mounted(256, 2000);
    wide.scope.files.explorerWidth = 256;
    surface.call('explorerSet', wide.state, wide.scope, [900]);
    expect(wide.state.userWidth).toBe(560);
    const narrow = mounted(256, 300);
    surface.call('explorerSet', narrow.state, narrow.scope, [300]);
    expect(narrow.state.userWidth).toBe(160);
    expect(narrow.sent).toEqual([['surface-files-explorer-width', '', '160']]);
  });

  test('← widens and → narrows by 16, Home takes the least and End the most; a modified key is left alone', () => {
    const set: number[] = [], prevented: string[] = [];
    const scope = { width: 256, max: 544, set: (width: number) => set.push(width), resize() {}, finish() {},
      preventDefault: () => prevented.push('default'), stopPropagation: () => prevented.push('propagation') };
    const state = handle.state(scope);
    const key = { altKey: false, ctrlKey: false, metaKey: false };
    for (const k of ['ArrowLeft', 'ArrowRight', 'Home', 'End']) handle.call('keyed', state, scope, [k, key]);
    expect(set).toEqual([272, 240, 160, 544]);
    expect(prevented).toHaveLength(8);
    handle.call('keyed', state, scope, ['ArrowLeft', { ...key, metaKey: true }]);
    handle.call('keyed', state, scope, ['ArrowUp', key]);
    expect(set).toHaveLength(4);
  });

  test('the width is saved in the preference record, clamped, and read back at load', async () => {
    const client = new T3Client();
    expect(filesPrefs(client).explorerWidth).toBeUndefined();
    await filesLocal(client, native, 'explorer-width', '', '300');
    expect(filesPrefs(client).explorerWidth).toBe(300);
    await filesLocal(client, native, 'explorer-width', '', '9999');
    expect(filesPrefs(client).explorerWidth).toBe(560);
    await filesLocal(client, native, 'explorer-width', '', 'wide');
    expect(filesPrefs(client).explorerWidth).toBe(560);
    const next: { files?: { explorerWidth?: number } } = {};
    adoptFilesPrefs(next, { files: { explorerWidth: 320 } });
    expect(next.files?.explorerWidth).toBe(320);
    const malformed: { files?: { explorerWidth?: number } } = {};
    adoptFilesPrefs(malformed, { files: { explorerWidth: '320' } });
    expect(malformed.files?.explorerWidth).toBeUndefined();
  });
});
