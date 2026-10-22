// View › Actual Size, Zoom In and Zoom Out (settings-shortcuts.contract ViewZoom, app.contract rootFontApply): the
// reference's half Chromium zoom levels (DesktopWindow.ts zoomWindow: setZoomLevel(level ± 0.5), a factor of 1.2^level),
// as the root font size.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractFns } from '../support/contract-fn-fixture';
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
    expect(source).toMatch(/button -exact-menu="none" press=zoom\(min\(17, step \+ 1\)\) aria-keyshortcuts=\(taken.includes\("meta\+shift\+="\) \? "" : "Meta\+Shift\+="\)/);
    const root = readFileSync(appPath('app.contract'), 'utf8');
    expect(root).toContain('setRootFontSize((data.look.fontSize > 0 ? data.look.fontSize : 16) * zoomFactor(zoomStep))');
    expect(root).toContain('task rootFont key=`${data.look.fontSize}|${zoomStep}`');
  });
});
