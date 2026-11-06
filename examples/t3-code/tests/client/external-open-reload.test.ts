// The root's external open (app.contract externalOpen): the data module outlives View › Reload, which boots the plan
// again, so the serial standing at the first answer is the boot's baseline and the last link does not open again.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { answerExternalOpen, awaitExternalOpenAnswersForTests, externalOpenRequest, openExternal } from '../../src/client/external-open';


test('a reload takes the standing serial as its baseline; only a newer one opens', () => {
  const root = contractComponent('app.contract', 'T3Code');
  const opened: string[][] = [], told: string[][] = [];
  const state: Env = { externalOpenFrom: -1 };
  const scope = (serial: number, url: string): Env => ({ data: { externalOpen: { serial, url } }, openURL: (...args: string[]) => opened.push(args),
    chatLocal: (...args: string[]) => told.push(args) });
  root.call('externalOpenBaselineTaken', state, scope(4, 'https://example.com/last'), []);
  expect(state.externalOpenFrom).toBe(4);
  // An open still waiting on the baseline's serial hears `opened`, as if it had.
  expect(told).toEqual([['external-open-answer', '4', 'opened']]);
  root.call('openExternalUrl', state, scope(5, 'https://example.com/next'), []);
  // The window's hidden input hears whether it opened (app-window.contract externalOpenAnswered).
  expect(opened).toEqual([['https://example.com/next', 'external-open-answer']]);
  const source = appText('app.contract');
  expect(source).toContain('task externalOpenBaseline when externalOpenFrom < 0 and data.revision > 0');
  expect(source).toContain('task externalOpen when externalOpenFrom >= 0 and data.externalOpen.serial > externalOpenFrom key=data.externalOpen.serial');
});

// ElectronShell.openExternal resolves whether the system opened the URL: here, `openURL(url, id)`'s answer.
test('an open waits for the plan\'s answer; a refused URL is false at once', async () => {
  awaitExternalOpenAnswersForTests(true);
  try {
    expect(await openExternal('file:///etc/hosts')).toBe(false);
    const first = openExternal('https://example.com/a'), firstSerial = externalOpenRequest().serial;
    const second = openExternal('zed://ssh/devbox/p'), secondSerial = externalOpenRequest().serial;
    answerExternalOpen(firstSerial, 'opened');
    expect(await first).toBe(true);
    answerExternalOpen(secondSerial, 'failed');
    expect(await second).toBe(false);
  } finally { awaitExternalOpenAnswersForTests(false); }
});
