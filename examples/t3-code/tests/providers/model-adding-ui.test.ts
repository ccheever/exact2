// Settings › Providers' "Add custom model" field: its open block and Escape owner are written in place by
// providerUiWindow and modelAddingUi (app-settings.contract), with no nested call, so the field's Escape
// (ProviderModels cancelAdd) stays under Contract's call size.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const settings = contractComponent('app-settings.contract', 'SettingsWindow');
// escapeFocusNext, read from app-settings.contract and run as JavaScript (audit-wave-followups.test.ts reads its fns so).
const next = /^fn escapeFocusNext\(current: string, field: string, focused: bool\): string = (.+)$/m.exec(appText('src/shell/app-settings.contract'))![1]!;
const escapeFocusNext = new Function('current', 'field', 'focused', `return ${next.replace(/ == /g, ' === ')};`);

test('model-adding opens and closes the block; slug-focus says whether the field owns Escape', () => {
  const state: Env = { modelAdding: '', modelAddingVisit: 0, escapeFocus: '' };
  const forwarded: unknown[][] = [];
  const scope: Env = { providerVisit: 3, providerUi: (...args: unknown[]) => forwarded.push(args), escapeFocusNext };
  settings.call('providerUiWindow', state, scope, ['model-adding', 'block-1']);
  expect(state).toMatchObject({ modelAdding: 'block-1', modelAddingVisit: 3, escapeFocus: '' });
  settings.call('modelAddingUi', state, scope, ['slug-focus', 'true']);
  expect(state.escapeFocus).toBe('custom-model-slug');
  settings.call('modelAddingUi', state, scope, ['model-adding', '']);
  expect(state).toMatchObject({ modelAdding: '', escapeFocus: '' });
  settings.call('providerUiWindow', state, scope, ['open', 'add']);
  expect(forwarded).toEqual([['open', 'add']]);
});
