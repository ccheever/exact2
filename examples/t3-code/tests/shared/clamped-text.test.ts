// A text trimmed to its alphabetic baseline (`text-box: trim-both cap alphabetic`) has a box that ends at the
// baseline, and a clamp clips to that box, so every descender is cut: the work rows read "Thinkina". The
// reference's work line keeps its whole line box (WorkLog.tsx WorkLogLine: `leading-relaxed`, a `truncate` label).
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { appFiles, appPath } from '../support/app-files';

test('no clamped text is trimmed to its baseline, where the clamp would cut its descenders', () => {
  const found: string[] = [];
  for (const file of appFiles().filter(name => name.endsWith('.contract'))) {
    readFileSync(appPath(file), 'utf8').split('\n').forEach((line, index) => {
      if (!/^\s*text\b/.test(line) || !/\btext-box="trim-(both|end)\b[^"]*\balphabetic"/.test(line)) return;
      if (/\b(line-clamp|overflow)=/.test(line)) found.push(`${file}:${index + 1}`);
    });
  }
  expect(found).toEqual([]);
});

test('the work line, which every live row sweeps, clamps its label on its whole line box', () => {
  const work = readFileSync(appPath('timeline-work.contract'), 'utf8');
  const line = work.slice(work.indexOf('component WorkLine'), work.indexOf('component ThreadReadLine'));
  const labels = line.split('\n').filter(text => /^\s*text (label|answer) /.test(text));
  expect(labels).toHaveLength(3);
  for (const label of labels) {
    expect(label).toContain('line-height="1.421875rem" ');
    expect(label).toContain('line-clamp=1 ');
    expect(label).not.toContain('text-box=');
  }
});
