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

const line = (file: string, needle: string) => {
  const found = views().find(v => v.name === file)!.lines.filter(l => l.includes(needle));
  expect(found).toHaveLength(1);
  return found[0]!;
};

test('the device tools section title is an h3 and the import menu label is no heading', () => {
  expect(line('r7-device.contract', 'text title font-size="0.75rem" font-weight=500')).toContain('role="heading" aria-level=3');
  expect(line('settings-projects.contract', 'text "Import from t3.json"')).toContain('role="presentation"');
});

// settings-headings: settingsLayout.tsx's SettingsSection title is an `<h2>` (`sr-only` with hideTitle) and its
// SettingsRow title an `<h3>`; Legacy features' trigger, a button, holds an `<h2>`. The reference's lists, page by page,
// are in the task record (20261010-settings-headings.md); these are the clone's views that draw them.
test('Settings section titles are h2 and row titles h3, as the reference', () => {
  // General and Appearance (CoreSections, CoreRowView); the Project page's Model row is a CoreRowView too.
  const sectionTitle = 'text section.title font-size="0.875rem" font-weight=400 line-height="1.25rem" color=pal.heading role="heading" aria-level=2';
  const rows = views().find(v => v.name === 'settings-rows.contract')!.lines;
  expect(rows.filter(l => l.includes(sectionTitle))).toHaveLength(2);
  // adopt-main-fixes-r10 RT-2: Legacy features' `<h2>` is inside its trigger, button › heading [level=2], since main
  // #451 (X74) keeps a heading inside a Mac button as its child; the sr-only heading beside the button is gone.
  const trigger = rows.findIndex(l => /^\s*button press=toggleLegacy .*aria-expanded=legacyOpen/.test(l));
  expect(trigger).toBeGreaterThan(0);
  expect(rows[trigger + 1]!.trim()).toBe(sectionTitle);
  expect(rows[trigger + 1]!.search(/\S/)).toBe(rows[trigger]!.search(/\S/) + 2);
  expect(rows.join('\n')).not.toContain('SettingsSrHeading');
  expect(line('settings-rows.contract', 'text row.title font-size="0.875rem" font-weight=500')).toContain('role="heading" aria-level=3');
  expect(line('settings-rows.contract', 'text "Version"')).toContain('role="heading" aria-level=3 aria-label=`Version ${row.value}`');
  expect(line('settings-rows.contract', 'text row.value font-family="ui-monospace"')).toContain('aria-hidden=true');
  // Appearance: "Colors & themes" (sr-only h2), "Color scheme" and "Themes" (h3).
  expect(line('settings-appearance.contract', 'SettingsSrHeading(title="Colors & themes"')).toContain('level=2');
  expect(line('settings-appearance.contract', 'text "Color scheme"')).toContain('role="heading" aria-level=3');
  expect(line('settings-appearance.contract', 'text "Themes"')).toContain('role="heading" aria-level=3');
  // Project: "Project" (sr-only h2). Scheduled Tasks: the environment's title is an h2, sr-only for one environment.
  expect(line('settings-projects.contract', 'SettingsSrHeading(title="Project"')).toContain('level=2');
  expect(line('settings-scheduled.contract', 'text section.label')).toContain('role="heading" aria-level=2');
  expect(line('settings-scheduled.contract', 'SettingsSrHeading(title=section.label')).toContain('level=2');
  // Connections: EmptyTitle is a `<div>`.
  expect(line('connections.contract', 'text "No saved remote environments"')).not.toContain('role="heading"');
  // The sr-only heading draws nothing and takes no space.
  expect(line('settings-kit.contract', 'text title role="heading" aria-level=level')).toContain('position="absolute" width=1 height=1 overflow="hidden" opacity=0');
});

// adopt-main-fixes-r9: main `a6a49a015` (LLP 1116 D7) gives a `text role="heading"` its text style's emphasized weight,
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
    // settings-rows.contract draws two: a section's title and, inside its trigger, Legacy features' (RT-2, r10).
    const found = all.find(v => v.name === file)!.lines.filter(l => l.includes(needle));
    expect(found).toHaveLength(file === 'settings-rows.contract' ? 2 : 1);
    for (const l of found) expect(l).toContain('role="heading"');
  }
});
