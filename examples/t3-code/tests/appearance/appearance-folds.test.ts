// Settings › Appearance's Advanced folds and the search jumps into them, against T3 Code 7c3b3b3605ef (MIT, see
// LICENSE-T3): AppearanceSettingsPanel.tsx folds Panel animations, Environment identification and Composer context;
// TypographySettings.tsx folds Separate fonts (a row, no longer the header's switch) and Font smoothing; a Prompt or
// Terminal font result turns Separate fonts on (settingsSearch.ts revealSettingsSearchTarget), and a hidden-title
// section's jump centres its group (settingsLayout.tsx SettingsSection).
import { describe, expect, test } from 'bun:test';
import { appText } from '../support/app-files';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';

const settings = contractComponent('app-settings.contract', 'SettingsWindow');

describe('a separate font result turns Separate fonts on before the jump', () => {
  const run = (action: string, args: unknown[], advanced: boolean, firstTarget = '') => {
    const calls: unknown[][] = [];
    const record = (name: string) => (...values: unknown[]) => calls.push([name, ...values]);
    const scope: Env = { settingsCore: { typographyAdvanced: advanced }, settingsQuery: '', settingsNavigation: { firstRoute: 'appearance', firstTarget, firstId: firstTarget },
      coreChange: record('coreChange'), corePickRoot: record('corePickRoot'), coreSearchKeyRoot: record('coreSearchKeyRoot') };
    settings.call(action, { modelAdding: '', settingsLegacyOpen: false, settingsEscapeHeld: false }, scope, args);
    return calls;
  };
  test('a pick of Prompt font or Terminal font', () => {
    for (const target of ['prompt-font', 'terminal-font']) {
      expect(run('corePick', ['appearance', target, '', target], false)).toEqual([
        ['coreChange', 'separate-fonts', 'typographyAdvanced', 'true'], ['corePickRoot', 'appearance', target, '', target]]);
    }
  });
  test('Enter on a Terminal font result', () => {
    expect(run('coreSearchKey', ['Enter'], false, 'terminal-font')).toEqual([['coreChange', 'separate-fonts', 'typographyAdvanced', 'true'], ['coreSearchKeyRoot', 'Enter']]);
  });
  test('nothing to turn on: already on, or another row', () => {
    expect(run('corePick', ['appearance', 'prompt-font', '', 'prompt-font'], true)).toEqual([['corePickRoot', 'appearance', 'prompt-font', '', 'prompt-font']]);
    expect(run('corePick', ['appearance', 'code-font', '', 'code-font'], false)).toEqual([['corePickRoot', 'appearance', 'code-font', '', 'code-font']]);
    expect(run('coreSearchKey', ['ArrowDown'], false, 'prompt-font')).toEqual([['coreSearchKeyRoot', 'ArrowDown']]);
  });
});

test('a jump from elsewhere (⌘K) to a separate font row turns Separate fonts on (TypographySection\'s effect)', () => {
  const members = appText('shell-members.contract');
  const gate = /^  task separateFontsJump when (.+)$/m.exec(members)![1]!;
  const at = (settingsFocusTarget: string, typographyAdvanced = false, commandPending = false) =>
    value(gate, { settingsOpen: true, settingsFocusTarget, settingsCore: { typographyAdvanced }, commandPending });
  expect([at('prompt-font'), at('terminal-font'), at('code-font'), at('prompt-font', true), at('prompt-font', false, true)]).toEqual([true, true, false, false, false]);
  expect(members).toContain('settingsCommand("settings-core", `separate-fonts:typographyAdvanced|${settingsCore.scopeKey}`, "true")');
});

test('Typography draws no header switch: Separate fonts is its fold\'s row', () => {
  expect(appText('settings-rows.contract')).not.toContain('typography-advanced');
});

test('the Appearance section takes the jump focus and its group is what the jump centres', () => {
  const library = appText('settings-appearance.contract');
  expect(library).toContain('column width="100%" gap="0.75rem" id="appearance" tabindex=-1');
  expect(library).toContain('column width="100%" gap="0.75rem" id="appearance-scroll-target"');
  // SnapShots' rows take the jump focus too (SettingsRow's tabIndex -1 for a row with an id).
  expect(appText('snapshot.contract')).toContain('row id=rowId tabindex=-1');
});
