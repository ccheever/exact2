// The window's last interaction (app.contract `interacted`: backgroundActivityReporter's and useLiveRefresh's window
// listeners) and the device tab's rename field, which keeps every key (RightPanelTabs.tsx: its keydown stops
// propagation; Escape restores the title and blurs).
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { contractComponent, contractFns, type Env } from '../support/contract-action-fixture';
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
    window.call('modifierDown', { focusEditable: false }, { shortcuts: [], modifierKey: () => {}, interacted: () => marks.push('key') }, ['a', { key: 'a' }]);
    expect(marks).toEqual(['key']);
    const root = readFileSync(join(APP, 'src/shell/app-window.contract'), 'utf8').split('\n').find(line => line.trimStart().startsWith('main navigationKey="t3-code"'))!;
    expect(root).toContain('pointermove=interacted');
    expect(root).toContain('wheel=interacted');
    expect(root).toContain('pointerdown=outsidePressDown');
  });

  // A hovering pointer's move goes only to the innermost node that hears `pointermove`, never on to the root. A handler
  // that is a component's action prop is followed to each use of the component, to the action bound there.
  test('every node with its own pointermove marks the interaction too', () => {
    const files = contracts(), unmarked: string[] = [];
    const componentOf = (lines: string[], at: number) => { for (let i = at; i >= 0; i--) { const m = /^component (\w+)/.exec(lines[i]!); if (m) return { name: m[1]!, from: i }; } return null; };
    const body = (lines: string[], from: number) => { const end = lines.findIndex((line, i) => i > from && line.startsWith('component ')); return lines.slice(from, end < 0 ? undefined : end); };
    const check = (lines: string[], at: number, handler: string, path: string, depth = 0): void => {
      const owner = componentOf(lines, at);
      if (!owner || depth > 4) { unmarked.push(path); return; }
      const own = body(lines, owner.from);
      const start = own.findIndex(line => new RegExp(`^  action ${handler}(\\(|$)`).test(line));
      if (start >= 0) { if (own[start + 1]?.trim() !== 'interacted()') unmarked.push(path); return; }
      if (!own.some(line => new RegExp(`^    ${handler}: action`).test(line))) { unmarked.push(`${path} (no action or prop)`); return; }
      // A prop: every use of the component binds it.
      let uses = 0;
      for (const file of files) file.lines.forEach((line, i) => {
        const bound = new RegExp(`\\b${owner.name}\\([^\\n]*\\b${handler}=(\\w+)`).exec(line)?.[1];
        if (!bound) return;
        uses++;
        if (bound !== 'interacted') check(file.lines, i, bound, `${path} <- ${file.name}:${i + 1} ${bound}`, depth + 1);
      });
      if (!uses) unmarked.push(`${path} (no use binds it)`);
    };
    for (const { name, lines } of files) lines.forEach((line, i) => {
      const handler = /\bpointermove=(\w+)/.exec(line)?.[1];
      if (handler && handler !== 'interacted') check(lines, i, handler, `${name}:${i + 1} ${handler}`);
    });
    expect(unmarked).toEqual([]);
  });

  test('the pull request page\'s first watcher counts as an interaction (watchInteraction)', async () => {
    const { holdPullRequestRefreshes, interactionResumes } = await import('../../src/pull-requests/pages-pr-refresh');
    const { liveRefreshDue, noteViewRefreshed } = await import('../../src/pull-requests/pages-pr-refresh');
    const client = { generation: 1, restAccess: () => ({ call: async () => ({ id: '1-1' }) }) } as never;
    noteViewRefreshed(client, 'v', 0);
    await holdPullRequestRefreshes(client, {} as never, 'list', true, 10 * 60_000);
    // Opened at ten minutes: the reader counts as here then, so the five-minute interval reads at fifteen.
    expect(liveRefreshDue(client, 'v', { visible: true, now: 15 * 60_000, arrival: false })).toBe(true);
    expect(interactionResumes(client)).toBe(0);
  });
});

describe('the device tab rename field keeps its keys', () => {
  test('Escape restores the title and cancels before the blur commits; other keys stay the field\'s', () => {
    const field = contractComponent('panel.contract', 'PanelTabNameField');
    const calls: unknown[][] = [];
    const tab = { id: 'device:1', renameValue: 'iPhone 17' };
    const state = { name: 'iPhone 1' };
    const scope: Env = { tab, local: (...args: unknown[]) => calls.push(args), preventDefault: () => calls.push(['prevent']), renameFocused: (on: boolean) => calls.push(['held', on]) };
    field.call('keys', state, scope, ['a', { key: 'a', metaKey: false }]);
    expect(calls).toEqual([]);
    field.call('keys', state, scope, ['Escape', { key: 'Escape', metaKey: false }]);
    expect(calls).toEqual([['prevent'], ['surface-rename-cancel', 'device:1', '']]);
    expect(state.name).toBe('iPhone 17');
  });

  // Leaving the thread mid-rename removes the field (its blur), and coming back remounts it: the chords are held only
  // while it has the focus.
  test('the window\'s chords are held while the field has the focus, through leaving the thread and coming back', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const field = contractComponent('panel.contract', 'PanelTabNameField');
    const rootState = { renameHoldsKeys: false };
    const scope: Env = { tab: { id: 'device', renameValue: 'Device' }, local: () => {}, renameFocused: (on: boolean) => root.call('renameFocused', rootState, {}, [on]) };
    field.call('focused', { name: 'Device' }, scope, []);
    expect(rootState.renameHoldsKeys).toBe(true);
    field.call('blurred', { name: 'Device' }, scope, []); // the thread changed: the field leaves the tree
    expect(rootState.renameHoldsKeys).toBe(false);
    // Back on the thread the field is drawn again, unfocused until it takes the focus.
    expect(rootState.renameHoldsKeys).toBe(false);
    field.call('focused', { name: 'Device' }, scope, []);
    expect(rootState.renameHoldsKeys).toBe(true);
  });

  // An `aria-keyshortcuts` button hears its chord before any field. The reference's bubble-phase listeners lose the key to
  // the field (stopPropagation); its capture-phase ones still hear it.
  test('only the bubble-phase chords and the Escape buttons stand aside while a device tab is renamed', () => {
    const { renameHeldCommand, editingChord } = contractFns('settings-shortcuts.contract');
    for (const command of ['commandPalette.toggle', 'thread.next', 'thread.jump.3', 'navigation.back', 'chat.new', 'preview.toggle', 'editor.openFavorite']) expect(renameHeldCommand!(command)).toBe(true);
    for (const command of ['sidebar.toggle', 'modelPicker.toggle', 'composer.stash', 'thread.undo', 'view.reopenClosed', 'terminal.toggle', 'diff.toggle', 'thread.stop', 'settings.open']) expect(renameHeldCommand!(command)).toBe(false);
    expect(editingChord!('Meta+Z')).toBe(true);
    expect(editingChord!('Meta+K')).toBe(false);
    const button = contractComponent('settings-shortcuts.contract', 'ShortcutButton');
    expect(button.derives({}, { renameHoldsKeys: true, renameHeldCommand, item: { command: 'commandPalette.toggle' } }).held).toBe(true);
    expect(button.derives({}, { renameHoldsKeys: true, renameHeldCommand, item: { command: 'sidebar.toggle' } }).held).toBe(false);
    const held = ['ComposerDock', 'PanelHeaderBar', 'SurfacePanel', 'ThreadDetailsPanel'];
    const free = ['ModelTrigger', 'TraitsTrigger', 'RuntimeTrigger', 'OverflowTrigger', 'StashKey', 'SidebarToggle'];
    const unguarded: string[] = [], guarded: string[] = [];
    for (const { lines } of contracts()) {
      let component = '';
      for (const line of lines) {
        component = /^component (\w+)/.exec(line)?.[1] ?? component;
        if (!line.includes('aria-keyshortcuts=')) continue;
        const guard = line.includes('aria-keyshortcuts=(renameHoldsKeys ? "" : ');
        if (held.includes(component) && !guard) unguarded.push(component);
        if (free.includes(component) && guard) guarded.push(component);
      }
    }
    expect(unguarded).toEqual([]);
    expect(guarded).toEqual([]);
  });
});
