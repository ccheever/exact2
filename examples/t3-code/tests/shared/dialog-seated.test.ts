// A modal dialog is `aria-modal` from the commit after the focus entered it (dialog-trap.contract, below DialogOpenFocus),
// so the Mac host lands VoiceOver on its initial focus rather than on its group; and a focused Remove on the composer's
// shelf hands the focus to the element the reference's next Tab reaches. These checks read the Contract sources and run
// the window's dialog actions; the behavior was read from another process (AXObserver, AXFocusedUIElement) on macOS.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { contractFn } from '../support/contract-fn-fixture';
import { appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();

describe('a modal dialog is modal once the focus is in it', () => {
  test('the window seats a dialog at its first focusin and unseats it as it closes', async () => {
    const window = contractComponent('app-window.contract', 'T3Window');
    const live = new Set<string>(['legacy-rename-project']);
    const scope = {
      none: null,
      querySelectorAll: (selector: string) => selector.includes('dialog-return') ? [] : [...live],
      frame: (id: string) => ({ unavailable: !live.has(id), width: live.has(id) ? 400 : 0, height: live.has(id) ? 300 : 0 }),
      dialogFrame: (owner: string, returns: unknown[]) => ({ owner, returns }),
      dialogReturn: (owner: string, target: string, popup: string, invoker: string) => ({ owner, target, popup, invoker }),
      dialogLineage: (frames: { owner: string; returns: unknown[] }[], owner: string) => frames.find(item => item.owner === owner)?.returns ?? [],
      focus() {}, stopPropagation() {}, chipFocusChanged() {},
    };
    expect(await source('app-window.contract')).toContain('\n  state dialogSeated: string[] = []\n');
    const state: Env = { focusEditable: false, dialogFocusId: '', dialogFocusOwner: '', dialogFocusPopup: '', dialogFocusInvoker: '', dialogFrames: [], dialogSeated: [], documentPermissionReturnKey: '', documentPermissionReturnSource: '' };
    window.call('windowFocused', state, scope, [{ target: 'composer' }]);
    // Mounted (DialogFinalFocus's dialogOpened) with the focus still outside: not yet modal.
    window.call('dialogOpened', state, scope, ['legacy-rename-project']);
    expect(state.dialogSeated).toEqual([]);
    window.call('dialogEntered', state, scope, ['legacy-rename-project', { target: 'legacy-project-title' }]);
    expect(state.dialogSeated).toEqual(['legacy-rename-project']);
    // A confirmation over it seats itself and keeps its parent modal; closing it leaves the parent seated.
    live.add('legacy-remove-project');
    window.call('dialogEntered', state, scope, ['legacy-remove-project', { target: 'legacy-remove-project-cancel' }]);
    expect(state.dialogSeated).toEqual(['legacy-rename-project', 'legacy-remove-project']);
    live.delete('legacy-remove-project');
    window.call('dialogClosed', state, scope, ['legacy-remove-project']);
    expect(state.dialogSeated).toEqual(['legacy-rename-project']);
    live.delete('legacy-rename-project');
    window.call('dialogClosed', state, scope, ['legacy-rename-project']);
    expect(state.dialogSeated).toEqual([]);
    expect(await source('app-window.contract')).toMatch(/\n  provide\n(?:.*\n)*?    dialogSeated\n/);
  });
});

describe('a focused Remove on the shelf hands the focus on', () => {
  test('to the next tile, then the first video, then the prompt', async () => {
    const after = await contractFn('snapshot.contract', 'draftFocusAfter', ['images', 'videos', 'at']);
    const images = [{ id: 'a' }, { id: 'b' }], videos = [{ id: 'v' }];
    expect(after(images, videos, 0)).toBe('preview-image-b');
    expect(after(images, videos, 1)).toBe('play-video-v');
    expect(after(images, [], 1)).toBe('composer');
    const videoAfter = await contractFn('snapshot.contract', 'videoFocusAfter', ['videos', 'at']);
    expect(videoAfter([{ id: 'v' }, { id: 'w' }], 0)).toBe('play-video-w');
    expect(videoAfter([{ id: 'v' }], 0)).toBe('composer');
  });

  test('only a Remove that held the focus moves it; a pointer press leaves it where it was', () => {
    for (const [file, name, removed] of [['composer-editor.contract', 'ImageDraftTile', 'remove'], ['snapshot.contract', 'SnapshotTile', 'remove'],
      ['composer-video.contract', 'VideoDraftTile', 'command']] as const) {
      const tile = contractComponent(file, name);
      for (const held of [true, false]) {
        const state = tile.state(), calls: unknown[][] = [], focused: string[] = [];
        const scope = { id: 'a', image: { id: 'a' }, video: { id: 'a' }, after: 'preview-image-b',
          remove: (...args: unknown[]) => calls.push(args), command: (...args: unknown[]) => calls.push(args), focus: (id: string) => focused.push(id) };
        tile.call('removeFocus', state, scope, [held]);
        tile.call('removeNow', state, scope, []);
        expect(`${name} ${held} ${calls.length}`).toBe(`${name} ${held} 1`);
        expect(`${name} ${removed} ${held}: ${focused.join(',')}`).toBe(`${name} ${removed} ${held}: ${held ? 'preview-image-b' : ''}`);
      }
    }
  });
});
