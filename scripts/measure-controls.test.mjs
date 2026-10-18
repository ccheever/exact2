// The control-role table (kernel/tables/schema.json `controls`) and its
// measurements (docs/control-roles-measured.md) say the same, cell for
// cell (LLP 1121 D9, stage 8 C); and the measure's judge as its doc says.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { deltaE00, judge, lab, ROWS } from './measure-controls.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const HOSTS = ['ios', 'macos', 'web', 'linux', 'windows', 'terminal', 'android'];
const PROVENANCE = ['measured', 'inferred: same painter, not measured', 'none by construction', 'inferred: agent capture is an inactive window'];

test('every cell of the table is recorded with its measurement, and no other', () => {
  const { controls } = JSON.parse(readFileSync(resolve(ROOT, 'kernel/tables/schema.json'), 'utf8'));
  const page = readFileSync(resolve(ROOT, 'docs/control-roles-measured.md'), 'utf8');
  const recorded = new Map();
  for (const line of page.split('\n').filter(l => /^\| [a-z]/.test(l) && !l.startsWith('| control'))) {
    const [control, parameter, host, fidelity, , , provenance] = line.split('|').slice(1, -1).map(c => c.trim());
    expect(PROVENANCE, line).toContain(provenance);
    const key = `${control} ${parameter} ${host}`;
    expect(recorded.has(key), `${key} twice`).toBe(false);
    recorded.set(key, fidelity);
  }
  let cells = 0;
  for (const [control, parameter, ...hosts] of controls) {
    HOSTS.forEach((host, i) => {
      cells++;
      expect(recorded.get(`${control} ${parameter} ${host}`), `${control} ${parameter} on ${host}`).toBe(hosts[i].split(': ')[0]);
    });
  }
  expect(recorded.size).toBe(cells);
  // The fixture measures every row the table has.
  expect(controls.map(([c, p]) => `${c} ${p}`).sort()).toEqual(ROWS.map(([c, p]) => `${c} ${p}`).sort());
});

test('the judge: exact within ΔE00 3, best-effort nearer the authored colour and of its hue, else none', () => {
  const px = rgb => [lab(rgb), rgb];
  const all = rgb => Array(20).fill(px(rgb));
  const grey = all([128, 128, 128]);
  expect(judge(all([224, 33, 138]), grey, '#e0218a').fidelity).toBe('exact');
  // A platform colour already the authored one is not the author's.
  expect(judge(all([224, 33, 138]), all([224, 33, 138]), '#e0218a').fidelity).toBe('none');
  expect(judge(all([200, 60, 140]), grey, '#e0218a').fidelity).toBe('best-effort');
  // Nearer the authored colour than the platform's, of its hue (the review's case): best-effort.
  expect(judge(all([150, 110, 130]), grey, '#e0218a').fidelity).toBe('best-effort');
  // Further than the platform's, or a grey: none.
  expect(judge(all([0, 0, 64]), grey, '#e0218a').fidelity).toBe('none');
  expect(judge(grey, grey, '#e0218a').fidelity).toBe('none');
  // Another asked-for colour's antialiasing over the ground is not this one's.
  const white = all([255, 255, 255]);
  expect(judge(all([134, 196, 156]), white, '#fff1a8', ['#0b8a3a']).fidelity).toBe('none');
  // An authored grey has no hue: nearer it is enough (theme2's 08:42 ruling).
  expect(judge(all([90, 90, 90]), all([200, 200, 200]), '#404040').fidelity).toBe('best-effort');
  // Too few pixels to say: unmeasured, never none.
  expect(judge(Array(4).fill(px([224, 33, 138])), Array(4).fill(px([128, 128, 128])), '#e0218a').fidelity).toBe('unmeasured');
  expect(deltaE00([50, 2.6772, -79.7751], [50, 0, -82.7485])).toBeCloseTo(2.0425, 3);
});
