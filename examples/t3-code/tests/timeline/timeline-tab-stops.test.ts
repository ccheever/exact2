// The transcript's buttons are Tab stops with Keyboard Navigation off, as Chromium tabs to every `<button>` and the
// reference sets no tabIndex={-1} in its timeline (orders measured with CDP in "Edit awaiting approval", "Turn with file
// edits" and "Plan sample").
import { expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { appText } from '../support/app-files';

const files = ['timeline', 'media'].flatMap(dir =>
  readdirSync(join(import.meta.dir, '../../src', dir)).filter(name => name.endsWith('.contract')).map(name => `src/${dir}/${name}`));

test('every timeline button but a menu item and the table menu backdrop is a Tab stop', () => {
  const missing = files.flatMap(file => appText(file).split('\n')
    .filter(line => /^\s*button /.test(line) && !line.includes('tabindex') && !line.includes('role="menuitem')
      && !line.includes('table-copy-menu-backdrop'))
    .map(line => `${file}: ${line.trim().slice(0, 80)}`));
  expect(missing).toEqual([]);
});

test("the header's panel toggles come before the breadcrumb, so ⇧Tab reaches the title first", () => {
  const chat = appText('src/timeline/chat.contract');
  expect(chat.indexOf('testId="panel-layout-controls"')).toBeLessThan(chat.indexOf('aria-label="Thread breadcrumb"'));
});

test("an approval's detail and a tool group's list ring on a keyboard's focus", () => {
  expect(appText('src/timeline/timeline-approval.contract')).toMatch(/tabindex=0 focus=ring\(true\) blur=ring\(false\) outline="none" box-shadow=\(ringed/);
  expect(appText('src/timeline/timeline.contract')).toMatch(/tabindex=0 focus=ring\(true\)[^\n]*aria-label="Tool calls"/);
});
