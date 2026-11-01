// Settings › Appearance's Advanced folds and the search jumps into them, against T3 Code 7c3b3b3605ef (MIT, see
// LICENSE-T3): AppearanceSettingsPanel.tsx folds Panel animations, Environment identification and Composer context;
// TypographySettings.tsx folds Separate fonts (a row, no longer the header's switch) and Font smoothing; a Prompt or
// Terminal font result turns Separate fonts on before its jump lands (settingsSearch.ts revealSettingsSearchTarget),
// and a hidden-title section's jump centres its group (settingsLayout.tsx SettingsSection).
import { describe, expect, test } from 'bun:test';
import { appText } from '../support/app-files';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';

// The root's write of Separate fonts (shell-members.contract), run as Contract runs it.
const members = contractComponent('shell-members.contract', 'T3Code');
const membersText = appText('shell-members.contract');
const task = (name: string) => {
  const at = membersText.indexOf(`  task ${name} when `);
  const [head, body] = membersText.slice(at).split('\n');
  return { gate: head!.slice(`  task ${name} when `.length), body: body!.trim() };
};

describe('a jump to a separate font row turns Separate fonts on, whatever else is in flight', () => {
  const jump = task('separateFontsJump');
  const asked = (wanted: boolean, saving = false, open = true) =>
    value(jump.gate, { settingsOpen: open, settingsCore: { separateFontsWanted: wanted }, separateFontsSaving: saving });
  const run = (state: Env, scope: Env) => {
    const sent: unknown[][] = [];
    members.call('separateFontsOn', state, { command: (...args: unknown[]) => { sent.push(args); return {}; }, ...scope }, []);
    return sent;
  };

  test('the task arms when the page asks, and one tick later writes the setting on its own mutation', () => {
    expect([asked(true), asked(false), asked(true, true), asked(true, false, false)]).toEqual([true, false, false, false]);
    expect(jump.body).toBe('after(1, separateFontsOn)');
    expect(membersText).toContain('  mutation separateFontsSaved as shape Change queue refreshes data then separateFontsLanded');
  });

  test('a command in flight does not drop the write', () => {
    const state: Env = { separateFontsSaving: false };
    expect(run(state, { settingsOpen: true, settingsCore: { separateFontsWanted: true }, commandPending: true })).toEqual([['device-setting', 'typographyAdvanced', 'true', 0]]);
    expect(state.separateFontsSaving).toBe(true);
  });

  test('the write re-checks as it runs: one at a time, and none once the page reads the setting on or Settings has closed', () => {
    expect(run({ separateFontsSaving: true }, { settingsOpen: true, settingsCore: { separateFontsWanted: true } })).toEqual([]);
    expect(run({ separateFontsSaving: false }, { settingsOpen: true, settingsCore: { separateFontsWanted: false } })).toEqual([]);
    expect(run({ separateFontsSaving: false }, { settingsOpen: false, settingsCore: { separateFontsWanted: true } })).toEqual([]);
  });

  test('the page reads the setting again when the write lands, and the next write waits until it reads it on', () => {
    const state: Env = { settingsRefresh: 4, separateFontsSaving: true };
    members.call('separateFontsLanded', state, {}, []);
    expect(state.settingsRefresh).toBe(5);
    const read = task('separateFontsRead');
    expect(read.body).toBe('after(1, separateFontsDone)');
    const settled = (saving: boolean, wanted: boolean) => value(read.gate, { separateFontsSaving: saving, settingsCore: { separateFontsWanted: wanted } });
    expect([settled(true, true), settled(true, false), settled(false, false)]).toEqual([false, true, false]);
    members.call('separateFontsDone', state, {}, []);
    expect(state.separateFontsSaving).toBe(false);
  });

  test('the jump waits for it: it is ready only once the answer names its target (settings-core-view.ts holds it)', () => {
    expect(appText('settings-members.contract')).toContain('settingsCore.jumpTarget == settingsFocusTarget');
  });
});

test('Typography draws no header switch: Separate fonts is its fold\'s row', () => {
  expect(appText('settings-rows.contract')).not.toContain('typography-advanced');
});

test('the fold\'s trigger rounds its foot until it opens and rings a keyboard focus (AdvancedSettings.tsx)', () => {
  const fold = appText('settings-advanced.contract');
  expect(fold).toContain('border-bottom-left-radius=(open ? "0px" : "0.75rem") border-bottom-right-radius=(open ? "0px" : "0.75rem")');
  expect(fold).toContain(':focus-visible\n          box-shadow="inset 0 0 0 2px light-dark(#1b4ed8, #346bf1)"');
});

test('the Appearance section takes the jump focus and its group is what the jump centres', () => {
  const library = appText('settings-appearance.contract');
  expect(library).toContain('column width="100%" gap="0.75rem" id="appearance" tabindex=-1');
  expect(library).toContain('column width="100%" gap="0.75rem" id="appearance-scroll-target"');
  // SnapShots' rows take the jump focus too (SettingsRow's tabIndex -1 for a row with an id).
  expect(appText('snapshot.contract')).toContain('row id=rowId tabindex=-1');
});
