// The window's last interaction (app.contract `interacted`: backgroundActivityReporter's and useLiveRefresh's window
// listeners) and the device tab's rename field, which keeps every key (RightPanelTabs.tsx: its keydown stops
// propagation; Escape restores the title and blurs).
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { APP } from '../support/app-files';

const contracts = () => (readdirSync(join(APP, 'src'), { recursive: true }) as string[]).filter(name => name.endsWith('.contract'))
  .map(name => ({ name, lines: readFileSync(join(APP, 'src', name), 'utf8').split('\n') })).concat([{ name: 'app.contract', lines: readFileSync(join(APP, 'app.contract'), 'utf8').split('\n') }]);

describe('the last interaction', () => {
  test('each new second goes to the data module; the first after six idle minutes is a resume', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const sent: string[] = [];
    let clock = 0;
    const scope: Env = { performanceNow: () => clock, wallTime: { epochAtZero: 1_000_000 },
      activityInteraction: (at: number) => sent.push(`interaction ${at}`), activityResumed: (at: number) => sent.push(`resumed ${at}`) };
    const state = { interactedAt: 0 };
    for (const at of [400, 1_200, 1_900, 2_300]) { clock = at; root.call('interacted', state, scope, []); }
    expect(sent).toEqual(['interaction 1001200', 'interaction 1002300']); // a move every frame costs one request a second
    clock = 2_300 + 6 * 60_000;
    root.call('interacted', state, scope, []);
    expect(sent.at(-1)).toBe(`resumed ${1_000_000 + clock}`);
    expect(state.interactedAt).toBe(clock);
  });

  test('the window root hears every pointer move, press, wheel and key', () => {
    const window = contractComponent('app-window.contract', 'T3Window');
    const marks: string[] = [];
    window.call('modifierDown', { focusEditable: false }, { modifierKey: () => {}, interacted: () => marks.push('key') }, ['a', { key: 'a' }]);
    expect(marks).toEqual(['key']);
    const root = readFileSync(join(APP, 'src/shell/app-window.contract'), 'utf8').split('\n').find(line => line.trimStart().startsWith('main navigationKey="t3-code"'))!;
    expect(root).toContain('pointermove=interacted');
    expect(root).toContain('wheel=interacted');
    expect(root).toContain('pointerdown=outsidePressDown');
  });

  // A hovering pointer's move goes only to the innermost node that hears `pointermove`, never on to the root.
  test('every node with its own pointermove marks the interaction too', () => {
    const files = contracts(), unmarked: string[] = [];
    const marks = (lines: string[], action: string) => {
      const start = lines.findIndex(line => new RegExp(`^  action ${action}(\\(|$)`).test(line));
      return start >= 0 && lines[start + 1]?.trim() === 'interacted()';
    };
    for (const { name, lines } of files) {
      lines.forEach(line => {
        const handler = /\bpointermove=(\w+)/.exec(line)?.[1];
        if (!handler || handler === 'interacted') return;
        // A prop action is marked where it is defined.
        if (!files.some(file => marks(file.lines, handler))) unmarked.push(`${name}: ${handler}`);
      });
    }
    expect(unmarked).toEqual([]);
  });
});

describe('the device tab rename field keeps its keys', () => {
  test('Escape restores the title and cancels before the blur commits; other keys stay the field\'s', () => {
    const field = contractComponent('panel.contract', 'PanelTabNameField');
    const calls: unknown[][] = [];
    const tab = { id: 'device:1', renameValue: 'iPhone 17' };
    const state = { name: 'iPhone 1' };
    const scope: Env = { tab, local: (...args: unknown[]) => calls.push(args), preventDefault: () => calls.push(['prevent']) };
    field.call('keys', state, scope, ['a', { key: 'a', metaKey: false }]);
    expect(calls).toEqual([]);
    field.call('keys', state, scope, ['Escape', { key: 'Escape', metaKey: false }]);
    expect(calls).toEqual([['prevent'], ['surface-rename-cancel', 'device:1', '']]);
    expect(state.name).toBe('iPhone 17');
  });

  // An `aria-keyshortcuts` button hears its chord before any field: the window's keybinding chords and the Escape
  // buttons that can show beside the right panel's tab strip stand aside while the field edits.
  test('the window\'s chords stand aside while a device tab is renamed', () => {
    const held = ['ShortcutButton', 'ShortcutEventButton', 'ModelTrigger', 'TraitsTrigger', 'RuntimeTrigger', 'OverflowTrigger', 'PrimaryActions',
      'ComposerFooter', 'ComposerDock', 'StashKey', 'SidebarToggle', 'PanelHeaderBar', 'SurfacePanel', 'ThreadDetailsPanel'];
    const unguarded: string[] = [];
    for (const { lines } of contracts()) {
      let component = '';
      for (const line of lines) {
        component = /^component (\w+)/.exec(line)?.[1] ?? component;
        if (held.includes(component) && line.includes('aria-keyshortcuts=') && !line.includes('aria-keyshortcuts=(renameHoldsKeys ? "" : ')) unguarded.push(component);
      }
    }
    expect(unguarded).toEqual([]);
    const app = readFileSync(join(APP, 'app.contract'), 'utf8');
    expect(app).toContain('derive renameHoldsKeys = shell.panel.open and shell.panel.tabs.filter((tab) => tab.renaming).length > 0');
  });
});
