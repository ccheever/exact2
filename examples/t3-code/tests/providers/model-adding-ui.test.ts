// Settings › Providers' "Add custom model" field: its open block and Escape owner go through modelAddingUi alone
// (app-settings.contract), so the field's Escape (ProviderModels cancelAdd) stays under Contract's call size.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const settings = contractComponent('app-settings.contract', 'SettingsWindow');

test('model-adding opens and closes the block; slug-focus says whether the field owns Escape', () => {
  const state: Env = { modelAdding: '', modelAddingVisit: 0, escapeFocus: '' };
  const forwarded: unknown[][] = [];
  const scope: Env = { providerVisit: 3, providerUi: (...args: unknown[]) => forwarded.push(args) };
  scope.escapeFocusSet = (...args: unknown[]) => settings.call('escapeFocusSet', state, scope, args);
  scope.modelAddingUi = (...args: unknown[]) => settings.call('modelAddingUi', state, scope, args);
  settings.call('providerUiWindow', state, scope, ['model-adding', 'block-1']);
  expect(state).toMatchObject({ modelAdding: 'block-1', modelAddingVisit: 3, escapeFocus: '' });
  settings.call('modelAddingUi', state, scope, ['slug-focus', 'true']);
  expect(state.escapeFocus).toBe('custom-model-slug');
  settings.call('modelAddingUi', state, scope, ['model-adding', '']);
  expect(state).toMatchObject({ modelAdding: '', escapeFocus: '' });
  settings.call('providerUiWindow', state, scope, ['open', 'add']);
  expect(forwarded).toEqual([['open', 'add']]);
});
