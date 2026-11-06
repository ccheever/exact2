// A bare `box` is `display: block` (LLP 1115: layout stays CSS), where `align-items` and `justify-content` do
// nothing. A box that sets either must make itself a flex or grid container, or its children sit at the
// top-left: the subagent avatars' marks and the group's "+N" sat off-centre in their 24pt circles.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appFiles, appPath } from '../support/app-files';

test('every box that aligns its children is a flex or grid container', () => {
  const found: string[] = [];
  for (const file of appFiles().filter(name => name.endsWith('.contract'))) {
    readFileSync(appPath(file), 'utf8').split('\n').forEach((line, index) => {
      if (!/^\s*box\b/.test(line) || !/\b(align-items|justify-content)=/.test(line)) return;
      if (!/\bdisplay=("(inline-)?(flex|grid)"|\()/.test(line)) found.push(`${file}:${index + 1}`);
    });
  }
  expect(found).toEqual([]);
});
