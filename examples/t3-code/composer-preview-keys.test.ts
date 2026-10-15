// The composer's file and image previews under the keyboard: Tab follows Keyboard Navigation (LLP 1115.000 D7.2) through
// ControlGuard and the inert window behind them, a press on the backdrop keeps the focus inside so closing returns it,
// and ← and → step through the draft's images as ExpandedImageDialog's onKeyDown does. What Tab reaches is the host's
// to decide, so these read the sources and run the actions; a drive presses the keys.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { contractComponent } from './contract-action-fixture';

const source = (name: string) => readFileSync(join(import.meta.dir, name), 'utf8');
const line = (file: string, needle: string) => source(file).split('\n').find(entry => entry.includes(needle))!;

describe('Tab follows Keyboard Navigation', () => {
  test('ControlGuard is a button with no tabindex, so it is a Tab stop exactly when the dialog\'s buttons are', () => {
    const lines = source('dialog-trap.contract').split('\n');
    const view = lines.slice(lines.indexOf('component ControlGuard')).find(entry => entry.trimStart().startsWith('button id=guardId'))!;
    expect(view).toContain('aria-hidden=true focusin=enter key=hold');
    expect(view).not.toContain('tabindex');
    expect(view).toContain('pointer-events="none"');
  });

  test('both previews trap with ControlGuard, and the window behind them is inert while either is open', () => {
    for (const [file, base] of [['composer-chip-file.contract', 'chip-file-preview'], ['composer-chip-image.contract', 'chip-image-preview']] as const) {
      expect(source(file)).toContain(`ControlGuard(guardId="${base}-guard-start", targets=dialogTargets, popup="${base}")`);
      expect(source(file)).toContain(`ControlGuard(guardId="${base}-guard-end", targets=dialogTargets, popup="${base}")`);
      expect(source(file)).not.toContain('FocusGuard(');
    }
    expect(line('app-window.contract', 'row width="100%" height="100%" min-width=0 inert=('))
      .toContain('inert=(modal or shell.panel.deviceSetup or shell.composerFile.open or data.composerImagePreview.length > 0)');
  });
});

describe('a press on the backdrop keeps the focus in the dialog, so closing gives it back', () => {
  test.each([['composer-chip-file.contract', 'chip-file-preview-backdrop'], ['composer-chip-image.contract', 'chip-image-backdrop']])('%s', (file, testId) => {
    const backdrop = line(file, `testId="${testId}"`);
    // A clicked `tabindex` box takes the focus on the Mac; retainFocus leaves it on the dialog's control.
    expect(backdrop).toContain('tabindex=-1 retainFocus=true');
    expect(backdrop).toContain('aria-keyshortcuts="Escape"');
  });
});

describe('the image preview\'s ← and →', () => {
  const dialog = contractComponent('composer-chip-image.contract', 'ComposerImageDialog');
  const run = (many: boolean, k: string, modifiers: Record<string, boolean> = {}) => {
    const calls: string[] = [];
    const preview = { open: true, id: 'a', name: 'a.png', position: many ? '(1/2)' : '', many };
    const scope = {
      preview, windowWidth: 1280, windowHeight: 840, rem: 16,
      dialogTargets: dialog.derives({}, { preview, windowWidth: 1280, windowHeight: 840, rem: 16, min: Math.min }).dialogTargets,
      command: (op: string) => calls.push(op),
      dialogFallback: (popup: string, targets: string[], key: string) => calls.push(`fallback:${popup}:${targets.join(',')}:${key}`),
      preventDefault: () => calls.push('prevent'), stopPropagation: () => calls.push('stop'),
    };
    dialog.call('keys', {}, scope, [k, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...modifiers }]);
    return calls;
  };
  test('over several images they step as Previous and Next, with or without modifiers', () => {
    expect(run(true, 'ArrowLeft')).toEqual(['prevent', 'stop', 'editorlocal:chip-image-previous']);
    expect(run(true, 'ArrowRight')).toEqual(['prevent', 'stop', 'editorlocal:chip-image-next']);
    expect(run(true, 'ArrowRight', { shiftKey: true })).toEqual(['prevent', 'stop', 'editorlocal:chip-image-next']);
  });
  test('over one image they do nothing of the dialog\'s; Tab and other keys reach the trap\'s fallback', () => {
    expect(run(false, 'ArrowLeft')).toEqual(['fallback:chip-image-preview:,chip-image-close,:ArrowLeft']);
    expect(run(true, 'Tab')).toEqual(['fallback:chip-image-preview:chip-image-previous,chip-image-close,chip-image-next:Tab']);
  });
});
