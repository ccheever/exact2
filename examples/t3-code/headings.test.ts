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
  expect(line('settings-rows.contract', 'text section.title font-size="0.875rem" line-height="1.25rem" color=pal.heading role=')).toContain('role="heading" aria-level=2');
  expect(line('settings-rows.contract', 'SettingsSrHeading(title=section.title')).toContain('level=2');
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
