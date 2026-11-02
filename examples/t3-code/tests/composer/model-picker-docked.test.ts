// The composer's model control at 2359643cff (MIT, see LICENSE-T3): the model and its effort in one trigger, and the
// picker docking the effort column and a footer (ComposerModelEffort.tsx, ProviderModelPicker.tsx,
// ModelPickerContent.tsx); with the composer's notices, Compact chip and context strip as v0.1.6 draws them.
import { describe, expect, test } from 'bun:test';
import { dockedNotices } from '../../src/composer/composer-presentation';
import { effortColumn, effortFocus, type EffortInput } from '../../src/composer/composer-model-effort';
import { setLayoutFrames } from '../support/layout-frames';
import { opened, effort, fast } from '../support/composer-controls-fixture';
import { contractComponent } from '../support/contract-action-fixture';

const input = (descriptors: object[], selections: object[], extra: Partial<EffortInput> = {}): EffortInput => ({ driver: 'claudeAgent', descriptors, selections,
  selection: null, reported: null, ultra: { primaryId: '', controlled: false, inBody: false }, unavailable: false, labels: {}, ...extra });

describe('the effort column', () => {
  test('a model pick focuses the checked level, composer.effort falls back to the other options', () => {
    const levels = effortColumn(input([effort, fast], [{ id: 'reasoningEffort', value: 'high' }])).rows;
    expect([effortFocus(levels, 'levels'), effortFocus(levels, 'any')]).toEqual([2, 2]);
    const toggles = effortColumn(input([{ id: 'webSearch', label: 'Web search', type: 'boolean', currentValue: true }], [])).rows;
    expect([effortFocus(toggles, 'levels'), effortFocus(toggles, 'any')]).toEqual([-1, 0]);
  });
  test("an unavailable model's options read as values, and the column names it in its header", () => {
    const column = effortColumn(input([effort], [], { unavailable: true }));
    expect(column.groups.map(group => [group.kind, group.label, group.value])).toEqual([['value', 'Reasoning', 'Medium']]);
    expect(column.rows).toEqual([]);
  });
});

describe('the docked picker', () => {
  const picker = contractComponent('src/composer/model-picker.contract', 'ModelPicker');
  const scope = (extra: object = {}) => ({ data: { running: false, composer: { toolbarTop: 600, effortHeight: 193, effortFocusAny: 1, effortGroups: [] } },
    catalog: { providers: [], models: [], count: 1, legacyCount: 0, searching: false, firstEnabled: 0, tallest: 0, setup: [], updateRequired: [], chatgptSharing: false,
      restCount: 1, restLegacyCount: 0, twoLineCount: 0, legacyTwoLineCount: 0, restTwoLineCount: 0, restLegacyTwoLineCount: 0, legacyDefault: false, selectedIndex: 0, selectedLegacy: false },
    viewport: { width: 1280, height: 840 }, query: '', left: 300, bottom: 260, session: 1, anchor: { id: '', top: 0, bottom: 0, left: 0 },
    modelAnchorX: 763, modelAnchorWidth: 143, modelOpenTarget: '', rem: 16, max: Math.max, min: Math.min, ...extra });
  test('beside the list the column and under both the footer: 554 wide, end-aligned to its trigger when the start would overflow', () => {
    const derived = picker.derives(picker.state(), scope());
    expect([derived.docked, derived.width, derived.rowHeight, derived.natural]).toEqual([true, 554, 193, 193 + 45]);
    expect(derived.x).toBe(763 + 143 - 554);
    // A Settings row's picker keeps the list alone.
    expect(picker.derives(picker.state(), scope({ anchor: { id: 'row', top: 100, bottom: 128, left: 40 } })).width).toBe(362);
  });
  test('Return that confirms an input method\'s candidate picks nothing', () => {
    const picks: unknown[] = [];
    const state = picker.state();
    const env = scope({ select: (...args: unknown[]) => picks.push(args), preventDefault: () => {}, focus: () => {} });
    picker.call('searchKey', state, env, ['Enter', { isComposing: true, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false }]);
    expect(picks).toEqual([]);
  });
  test('Return on a focused effort level picks it, as Space does; the arrows still move', () => {
    const level = contractComponent('src/composer/model-picker.contract', 'EffortLevel');
    const picks: unknown[] = [], moves: unknown[] = [];
    const key = (name: string, extra: object = {}, row: object = {}) => level.call('rowKey', level.state(), {
      row: { index: 2, descriptor: 'reasoningEffort', value: 'high', disabled: false, ...row }, rem: 16,
      pick: (...args: unknown[]) => picks.push(args), move: (...args: unknown[]) => moves.push(args.slice(0, 2)), preventDefault: () => {},
    }, [name, { isComposing: false, repeat: false, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, ...extra }]);
    key('Enter');
    key('Enter', { isComposing: true });
    key('Enter', {}, { disabled: true });
    key('ArrowDown');
    expect(picks).toEqual([['model-option', 'reasoningEffort', 'high']]);
    // A Return that confirms an input method's candidate is not a pick.
    expect(moves).toEqual([[2, 'Enter'], [2, 'ArrowDown']]);
  });
});

describe("the picker's focus after it opens", () => {
  // ProviderModelPicker's handleInstanceModelChange moves the focus to the levels only after a pick; opening on a
  // composer leaves it on the search. A side chat's picker counts the side chat's model, not the window's.
  const root = contractComponent('app.contract', 'T3Code');
  const window = { providerId: 'claude', modelId: 'opus', composer: { effortFocusLevel: 1 } };
  const side = { providerId: 'standin', modelId: 'default', composer: { effortFocusLevel: 2 } };
  test("a side chat's picker opens on its search, and a pick there focuses the side chat's checked level", () => {
    const state: Record<string, unknown> = { modelAnchorX: -1, modelAnchorWidth: 0, modelOpenTarget: '', modelFocusedPick: 'claude:opus' };
    root.call('sideModelMeasure', state, { frame: () => ({ unavailable: false, x: 900, width: 140 }), sideChat: side }, []);
    expect(state.modelFocusedPick).toBe('standin:default');
    const focused: string[] = [];
    const scope = { data: window, sideChat: side, composerTargetData: side, focus: (id: string) => focused.push(id) };
    root.call('modelEffortFocused', state, scope, []);
    expect(focused).toEqual([]);
    root.call('modelEffortFocused', state, { ...scope, composerTargetData: { ...side, modelId: 'other' } }, []);
    expect([focused, state.modelFocusedPick]).toEqual([['model-effort-row-2'], 'standin:other']);
  });
});

describe('the composer around it', () => {
  test("a new thread's notices stand apart under its centred composer; a thread's dock above it", () => {
    expect(dockedNotices('', ['reconnecting'])).toEqual({ notices: [], detachedNotices: ['reconnecting'] });
    expect(dockedNotices('t1', ['reconnecting'])).toEqual({ notices: ['reconnecting'], detachedNotices: [] });
  });
  test('the Compact chip drops its word on a card under 480pt', async () => {
    const { client } = await opened();
    setLayoutFrames(client, { card: [0, 0, 479, 120] });
    const chip = contractComponent('src/composer/composer-controls.contract', 'CompactChip');
    expect(chip.derives(chip.state(), { data: { composer: { compactLabel: false, compactKeepFull: false, compactTokens: '312k' } } }).label).toBe('');
    expect(chip.derives(chip.state(), { data: { composer: { compactLabel: true, compactKeepFull: true, compactTokens: '312k' } } }).label).toBe('Keep');
  });
  test("stacked notices' actions line up: at least 3rem, 3.5rem under 640pt", () => {
    const row = contractComponent('src/composer/composer-controls.contract', 'NoticeRow');
    const notice = { id: 'n', action: 'su:reconnect', action2: '', dismiss: '', lines: [], usage: [], machines: [], segments: [], code: '', tip: '', redact: '', description: '', details: false };
    const width = (viewport: number) => row.attribute('testId=`notice-actions-', 'min-width', row.state(), { notice, attached: true, stacked: false, inlineDocumentViewportWidth: viewport });
    expect([width(1280), width(600)]).toEqual(['3rem', '3.5rem']);
  });
  test("the strip's labels are 14pt under 640pt, and its glyphs keep no inline margins", () => {
    const control = contractComponent('src/composer/composer-controls.contract', 'StripControl');
    const size = (viewport: number) => control.attribute('data-anchor=`sl:', 'font-size', control.state(), { inlineDocumentViewportWidth: viewport, over: false, open: false });
    expect([size(1280), size(600)]).toEqual(['0.75rem', '0.875rem']);
    const strip = contractComponent('src/composer/composer-controls.contract', 'StripControl');
    expect(() => strip.attribute('CnMachineIcon(kind=machine', 'margin-left', strip.state(), {})).toThrow();
  });
});
