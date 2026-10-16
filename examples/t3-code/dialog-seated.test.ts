// A modal dialog is `aria-modal` from the commit after the focus entered it (dialog-trap.contract, below DialogOpenFocus),
// so the Mac host lands VoiceOver on its initial focus rather than on its group; and a focused Remove on the composer's
// shelf hands the focus to the element the reference's next Tab reaches. These checks read the Contract sources and run
// the window's dialog actions; the behavior was read from another process (AXObserver, AXFocusedUIElement) on macOS.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { contractComponent, value, type Env } from './contract-action-fixture';
import { contractFn } from './contract-fn-fixture';

const dir = new URL('./', import.meta.url);
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const files = readdirSync(dir).filter(name => name.endsWith('.contract')).sort();

describe('a modal dialog is modal once the focus is in it', () => {
  test('every dialog that tells the window of its focus is aria-modal by dialogSeated, under the same owner', async () => {
    let count = 0;
    for (const file of files) {
      const lines = (await source(file)).split('\n');
      lines.forEach((line, at) => {
        if (line.trimStart().startsWith('//') || !/role="(alert)?dialog"/.test(line) || !line.includes('aria-modal=')) return;
        const owner = /focusin=dialogEntered\(((?:[^()]|\([^()]*\))*)\)/.exec(line)?.[1];
        if (!owner) return;
        count++;
        expect(`${file}:${at + 1} ${/aria-modal=(\S+)/.exec(line)?.[1]}`).toBe(`${file}:${at + 1} dialogSeated.includes(${owner})`);
        let start = at;
        while (start > 0 && !lines[start]!.startsWith('component ')) start--;
        const head = lines.slice(start, at).join('\n');
        expect(`${file} ${lines[start]}: ${/\n  inject\n(?:    .+\n)*?    dialogSeated: string\[\]\n/.test(head + '\n')}`).toBe(`${file} ${lines[start]}: true`);
      });
    }
    // The legacy sidebar's frame (Rename project, Project grouping), the confirmations (SettingsConfirm and the rest), the
    // file and media previews.
    expect(count).toBeGreaterThanOrEqual(21);
  });

  test('the composer and timeline media previews are Base UI Dialogs: modal, named, focus on their X', async () => {
    for (const [file, owner, close] of [['composer-chip-image.contract', 'chip-image-preview', 'chip-image-close'],
      ['timeline-attachments.contract', 'image-preview', 'image-preview-close'], ['r4-composer.contract', 'video-preview', 'video-preview-close']] as const) {
      const text = await source(file);
      const line = text.split('\n').find(row => row.includes(`focusin=dialogEntered("${owner}")`))!;
      expect(line).toContain(`role="dialog" aria-modal=dialogSeated.includes("${owner}") aria-label=`);
      expect(text).toContain(`DialogOpenFocus(target="${close}")`);
    }
  });

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
      focus() {}, stopPropagation() {},
    };
    expect(await source('app-window.contract')).toContain('\n  state dialogSeated: string[] = []\n');
    const state: Env = { dialogFocusId: '', dialogFocusOwner: '', dialogFocusPopup: '', dialogFocusInvoker: '', dialogFrames: [], dialogSeated: [] };
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
      ['r4-composer.contract', 'VideoDraftTile', 'command']] as const) {
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

  test('each Remove reports its focus and presses removeNow; the next tile\'s buttons carry the ids it names', async () => {
    const editor = await source('composer-editor.contract'), snapshot = await source('snapshot.contract'), video = await source('r4-composer.contract');
    expect(editor).toContain('button press=removeNow focus=removeFocus(true) blur=removeFocus(false) disabled=disabled aria-label=`Remove ${name}`');
    expect(snapshot).toContain('button press=removeNow focus=removeFocus(true) blur=removeFocus(false) disabled=disabled aria-label=`Remove ${image.name}`');
    expect(video).toContain('button press=removeNow focus=removeFocus(true) blur=removeFocus(false) disabled=disabled aria-label=`Remove ${video.name}`');
    expect(editor).toContain('id=`preview-image-${id}`');
    expect(video).toContain('title="" id=`play-video-${video.id}`');
    expect(snapshot).toContain('after=draftFocusAfter(images, videos, i))');
    expect(snapshot).toContain('VideoDraftTile(video=video, disabled=disabled, command=command, after=videoFocusAfter(videos, i))');
  });
});
