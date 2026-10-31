// The root's external open (app.contract externalOpen): the data module outlives View › Reload, which boots the plan
// again, so the serial standing at the first answer is the boot's baseline and the last link does not open again.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

test('a reload takes the standing serial as its baseline; only a newer one opens', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const opened: string[] = [];
  const state: Env = { externalOpenFrom: -1 };
  const scope = (serial: number, url: string): Env => ({ data: { externalOpen: { serial, url } }, openURL: (u: string) => opened.push(u) });
  root.call('externalOpenBaselineTaken', state, scope(4, 'https://example.com/last'), []);
  expect(state.externalOpenFrom).toBe(4);
  root.call('openExternalUrl', state, scope(5, 'https://example.com/next'), []);
  expect(opened).toEqual(['https://example.com/next']);
  const source = appText('app.contract');
  expect(source).toContain('task externalOpenBaseline when externalOpenFrom < 0 and data.revision > 0');
  expect(source).toContain('task externalOpen when externalOpenFrom >= 0 and data.externalOpen.serial > externalOpenFrom key=data.externalOpen.serial');
});
