// adopt-main-fixes-r8: main `f9dcdb1af` (LLP 1115) writes ARIA's default level, 2, on a `role="heading"` that has no
// `aria-level`, and the Mac host now exposes it as AXHeading level 2 (TextInteraction.swift `headingLevel`; before,
// such a text was AXStaticText). The reference's headings are `<h1>`–`<h6>`, so each clone heading says the level its
// reference element has, and a reference label that is no heading says so: DeviceToolsPanel.tsx `Section`'s `<h3>` is
// R7DevSection's `aria-level=3`; ProjectActionsSettings.tsx's `MenuGroupLabel` "Import from t3.json" is Base UI's
// `role="presentation"` (as browser-defaults.contract's viewport group labels already are).
import { expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';

const dir = import.meta.dir;
const views = () => readdirSync(dir).filter(name => name.endsWith('.contract')).sort()
  .map(name => ({ name, lines: readFileSync(`${dir}/${name}`, 'utf8').split('\n') }));

test('every role="heading" says its aria-level', () => {
  const headings: string[] = [];
  const unlevelled: string[] = [];
  for (const { name, lines } of views()) {
    lines.forEach((line, i) => {
      if (!/\brole=(?:"heading"|\([^)]*"heading")/.test(line)) return;
      headings.push(`${name}:${i + 1}`);
      if (!/\baria-level=/.test(line)) unlevelled.push(`${name}:${i + 1}`);
    });
  }
  expect(headings.length).toBeGreaterThan(150);
  expect(unlevelled).toEqual([]);
});

test('the device tools section title is an h3 and the import menu label is no heading', () => {
  const line = (file: string, needle: string) => {
    const found = views().find(v => v.name === file)!.lines.filter(l => l.includes(needle));
    expect(found).toHaveLength(1);
    return found[0]!;
  };
  expect(line('r7-device.contract', 'text title font-size="0.75rem" font-weight=500')).toContain('role="heading" aria-level=3');
  expect(line('settings-projects.contract', 'text "Import from t3.json"')).toContain('role="presentation"');
});
