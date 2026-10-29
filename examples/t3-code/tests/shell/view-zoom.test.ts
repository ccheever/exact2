// View › Actual Size, Zoom In and Zoom Out (settings-shortcuts.contract ViewZoom, app.contract rootFontApply): the
// reference's half Chromium zoom levels (DesktopWindow.ts zoomWindow: setZoomLevel(level ± 0.5), a factor of 1.2^level),
// as the root font size.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractFns } from '../support/contract-fn-fixture';
import { contractSource, contractComponent } from '../support/contract-action-fixture';
import { appPath } from '../support/app-files';

const { zoomFactor } = contractFns('src/settings/settings-shortcuts.contract') as unknown as { zoomFactor: (step: number) => number };

describe('View zoom', () => {
  test('each step is half a Chromium zoom level, 25% to 500%', () => {
    for (let step = -15; step <= 17; step++) expect(zoomFactor(step)).toBeCloseTo(1.2 ** (step / 2), 5);
    expect(zoomFactor(1)).toBeCloseTo(1.0954, 4); // 109.5%, where Safari's next level is 115%
    expect(zoomFactor(18)).toBe(1);
  });
  test('the app\'s ⌘0, ⌘= and ⌘- take the host items\' places, ⇧⌘= is a hidden Zoom In, and the root font follows the step', () => {
    const source = readFileSync(appPath('src/settings/settings-shortcuts.contract'), 'utf8');
    for (const chord of ['"Meta+0"', '"Meta+="', '"Meta+-"']) expect(source).toContain(chord);
    expect(source).toMatch(/button -exact-menu="none" press=zoomKey\("meta\+shift\+=", min\(17, step \+ 1\)\) aria-keyshortcuts=\(taken.includes\("meta\+shift\+="\) \? "" : "Meta\+Shift\+="\)/);
    const root = contractSource('app.contract');
    expect(root).toContain('setRootFontSize((data.look.fontSize > 0 ? data.look.fontSize : 16) * zoomFactor(zoomStep))');
    expect(root).toContain('task rootFont key=`${data.look.fontSize}|${zoomStep}`');
  });
  test('over a shown Browser page, ⌘R and the zoom chords ask the data module first; ⇧⌘R is always the window\'s', () => {
    const asked: unknown[][] = [], run = (component: string, action: string, page: string, args: unknown[] = []) => {
      const calls: string[] = [];
      contractComponent('src/settings/settings-shortcuts.contract', component).call(action, {}, {
        page, step: 2, local: (...values: unknown[]) => { asked.push(values); calls.push('local'); },
        reload: () => calls.push('reload'), zoom: (to: number) => calls.push(`zoom ${to}`),
      }, args);
      return calls;
    };
    expect(run('ViewReload', 'reloadKey', 'browser-frame-1')).toEqual(['local']);
    expect(asked.pop()).toEqual(['surface-browser-frame-key', 'browser-frame-1', 'chord=meta+r']);
    expect(run('ViewReload', 'reloadKey', '')).toEqual(['reload']);
    expect(run('ViewReload', 'reloadWindow', 'browser-frame-1')).toEqual(['reload']);
    expect(run('ViewZoom', 'zoomKey', 'browser-frame-1,browser-frame-2', ['meta+=', 3])).toEqual(['local']);
    expect(asked.pop()).toEqual(['surface-browser-frame-key', 'browser-frame-1,browser-frame-2', 'chord=meta+=&step=3']);
    expect(run('ViewZoom', 'zoomKey', '', ['meta+-', 1])).toEqual(['zoom 1']);
  });
});
