// ProjectSettingsPanel removeMembers (T3 Code 74541bab17): removing a whole project leaves Settings open on the
// Projects overview for the same machine, where it used to close Settings.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

test('a removed project returns to the Projects overview and keeps the machine', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const state: Env = { settingsOpen: true, settingsRoute: 'projects', settingsMachine: 'env-remote', settingsProjectKey: 'group-1', settingsProjectId: 'p1', settingsCheckout: 'p1' };
  root.call('projectRemoved', state, {}, []);
  expect(state).toEqual({ settingsOpen: true, settingsRoute: 'projects', settingsMachine: 'env-remote', settingsProjectKey: '', settingsProjectId: '', settingsCheckout: '' });
});
