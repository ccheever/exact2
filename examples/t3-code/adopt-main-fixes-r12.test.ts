// adopt-main-fixes-r12: what round 12 adapts to main and the workarounds its fixes end. Each reads the clone's sources;
// the behaviour is proven by the record's agent drives (tasks/20261013-adopt-main-fixes-r12.md).
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const contracts = async () => Promise.all(readdirSync(dir).filter(name => name.endsWith('.contract'))
  .map(async name => ({ name, lines: (await Bun.file(new URL(name, dir)).text()).split('\n') })));
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const code = (line: string) => !line.trimStart().startsWith('//');

describe('AD-1: the Mac\'s own presentations main gives what the clone writes stay the reference\'s', () => {
  test('every vertical list opts out of the Mac\'s table (LLP 1115.002 D5): the reference draws its rows as divs', async () => {
    const lists = (await contracts()).flatMap(({ name, lines }) => lines.filter(line => code(line) && /^\s*list\s/.test(line)).map(line => ({ name, line })));
    expect(lists.map(({ name }) => name).sort()).toEqual(['app-main.contract', 'connections-routes.contract', 'diff.contract', 'legacy-sidebar.contract', 'pages-pr-code.contract']);
    for (const { line } of lists) expect(line).toMatch(/^\s*list appearance="none" /);
  });

  test('every popover keeps the painted layer (LLP 1115.002 D7.3): an authored fallback row, `none`, so no NSPopover and no flip', async () => {
    const popovers = (await contracts()).flatMap(({ lines }) => lines.filter(line => code(line) && /\bpopover="/.test(line)));
    expect(popovers.length).toBeGreaterThanOrEqual(62);
    for (const line of popovers) expect(line).toContain('popover="auto" position-try-fallbacks="none"');
  });
});

describe('RT-3: workarounds main\'s fixes end', () => {
  test('X70 (main 9074a745b): the update pill is aria-disabled, as SidebarUpdatePill.tsx, so Tab reaches it', async () => {
    const pill = (await source('sidebar-icons.contract')).split('\n').find(line => line.includes('aria-label="Check for updates"'))!;
    expect(pill).toContain('button aria-disabled=true aria-label="Check for updates" cursor="not-allowed"');
    expect(/(?<![-\w])disabled=/.test(pill)).toBe(false); // HTML disabled takes no focus
  });

  test('#287 (main ff2251b0c): the "+N" toggle\'s underline is dotted, as ConnectionsSettings.tsx\'s border-b border-dotted', async () => {
    const toggle = (await source('connections-network.contract')).split('\n').find(line => line.includes('testId="network-access-endpoints-toggle"'))!;
    expect(toggle).toContain('border-bottom-width=1 border-bottom-style="dotted"');
  });

  // Review round 1: the reference's other `border-dashed` boxes the clone draws (each was `solid`). The reference's
  // unresolved chip and pending review card keep the clone's stroked rects (listed in EXACT2-GAPS.md's #287 row).
  const dashed: [file: string, anchor: string, reference: string][] = [
    ['settings-appearance-import.contract', 'testId="add-theme-empty"', 'ThemeSearchSection.tsx:359'],
    ['settings-appearance-import.contract', 'testId="add-theme-file"', 'ThemeImportDialog.tsx:520'],
    ['providers-wizard.contract', 'text "No compatible agents found"', 'AcpRegistrySearchStep.tsx:198'],
    ['settings-a-telemetry.contract', 'text "Desktop host signals not connected"', 'ResourceTelemetryDiagnostics.tsx:1151'],
    ['settings-keybindings.contract', 'if view.lines.length == 0 and view.error == ""', 'KeybindingsSettings.tsx:687'],
  ];
  for (const [file, anchor, reference] of dashed) {
    test(`#287: the box at ${reference} is dashed, as the reference's border-dashed (${file})`, async () => {
      const lines = (await source(file)).split('\n');
      const at = lines.findIndex(line => line.includes(anchor));
      expect(at).toBeGreaterThanOrEqual(0);
      // The anchor is the box itself (a testId) or the line right before (its `when`) or after (its first text) it.
      const box = anchor.startsWith('testId=') ? lines[at] : anchor.startsWith('if ') ? lines[at + 1] : lines[at - 1];
      expect(box).toContain('border-style="dashed"');
      expect(box).not.toContain('border-style="solid"');
    });
  }
});

describe('Main\'s new warning lower-width-padding (LLP 1116.009 D5)', () => {
  test('the pull request list\'s error notes (width 100%, 0.75rem padding each side) are border-box, as Tailwind\'s', async () => {
    const lines = (await source('pages-prs.contract')).split('\n');
    const at = lines.findIndex(line => line.includes('for note in page.errors key=note.key'));
    expect(at).toBeGreaterThan(0);
    const box = lines[at - 1];
    expect(box).toContain('width="100%"');
    expect(box).toContain('padding-left="0.75rem" padding-right="0.75rem"');
    expect(box).toContain('box-sizing="border-box"');
  });
});
