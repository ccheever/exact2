// Main `f9dcdb1af` (LLP 1115) writes ARIA's default level, 2, on a `role="heading"` that has no
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

// settingsLayout.tsx's SettingsSection title is an `<h2>` (`sr-only` with hideTitle) and its
// SettingsRow title an `<h3>`; Legacy features' trigger, a button, holds an `<h2>`. The reference's lists, page by page,
// follow from those two components; the tests below name the clone's views that draw them.

// Main `a6a49a015` (LLP 1116 D7) gives a `text role="heading"` its text style's emphasized weight,
// bold (700) for a title (levels 1-3), where until now an unwritten weight was the title's own regular (400); a written
// `font-weight` still wins. The reference's headings say their weight (settingsLayout.tsx's SettingsSection title and
// ThemeSettings.tsx's "Color scheme" / "Themes" are `font-normal`), so every drawn clone heading writes one, on its
// line or in its `style` class; the visually hidden ones (`opacity=0`) draw nothing.
test('every drawn text heading writes its font-weight (main\'s D7 bolds an unsaid one)', () => {
  const all = views();
  const weighted = new Set<string>();
  for (const { lines } of all) lines.forEach((l, i) => {
    const style = /^style (\w+)/.exec(l);
    if (style && /\bfont-weight=/.test(lines[i + 1] ?? '')) weighted.add(style[1]!);
  });
  const unweighted: string[] = [];
  let texts = 0;
  for (const { name, lines } of all) lines.forEach((l, i) => {
    if (!/^\s*text\b.*\brole="heading"/.test(l) || /\bopacity=0\b/.test(l)) return;
    texts++;
    const cls = /\bclass=(\w+)/.exec(l)?.[1];
    // a text whose runs write their weight (markdown's ChatRuns) is weighted by its children
    const runs = /^\s*ChatRuns\(.*\bweight=\d+/.test(lines[i + 1] ?? '');
    if (!/\bfont-weight=/.test(l) && !(cls && weighted.has(cls)) && !runs) unweighted.push(`${name}:${i + 1}`);
  });
  expect(texts).toBeGreaterThan(100);
  expect(unweighted).toEqual([]);
  // The seven that relied on the old regular weight now say it, as the reference's `font-normal`.
  for (const [file, needle] of [
    ['providers.contract', 'text editor.title font-size="0.875rem" font-weight=400'],
    ['settings-appearance.contract', 'text "Color scheme" font-size="0.875rem" font-weight=400'],
    ['settings-appearance.contract', 'text "Themes" font-size="0.875rem" font-weight=400'],
    ['settings-archive.contract', 'text group.title font-size="0.875rem" font-weight=400'],
    ['settings-rows.contract', 'text section.title font-size="0.875rem" font-weight=400'],
    ['settings-scheduled.contract', 'text "Scheduled tasks" font-size="0.875rem" font-weight=400'],
    ['settings-licenses.contract', 'text "Third-party notices" font-size="0.875rem" font-weight=400'],
  ] as const) {
    // settings-rows.contract draws two: a section's title and, inside its trigger, Legacy features'.
    const found = all.find(v => v.name === file)!.lines.filter(l => l.includes(needle));
    expect(found).toHaveLength(file === 'settings-rows.contract' ? 2 : 1);
    for (const l of found) expect(l).toContain('role="heading"');
  }
});
