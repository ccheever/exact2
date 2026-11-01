import { expect, test } from 'bun:test';
import { appText } from '../support/app-files';

import { contractComponent } from '../support/contract-action-fixture';

// These messages can arrive in one load turn, and ACKs can overlap RPC replies.
// They must use the runner's ordered mutation rather than newest-answer wins.
test('document events and delivery acknowledgements share a dedicated ordered mutation', () => {
  const source = appText('app.contract');
  const route = /if op\.startsWith\("inline-doc-"\)\n\s+send (\w+) = command\(`chatlocal:\$\{op\}`, id, value, 0\)/.exec(source);
  expect(route).not.toBeNull();
  const name = route![1]!;
  expect(source).toMatch(new RegExp(`mutation ${name} as shape Change queue refreshes data(?:\\n|\\s*//)`));
  expect(source).toContain(`pending(${name})`);
  expect(name).not.toBe('localChanged');

  const component = contractComponent('app.contract', 'T3Code');
  const calls: unknown[][] = [];
  const state = { foldHold: false, rightPanelAt: '', restAtEnd: false, restOwner: '', composerOwner: 'thread' };
  const scope = { command: (...args: unknown[]) => calls.push(args), shell: { panel: { tabs: [], browser: { capture: { floating: false } } } } };
  for (const [op, value] of [['message', 'initialize'], ['measure', '728|340'], ['message', 'adapter-ready'], ['ack', 'reply-1'], ['permission', '1:allow']]) {
    component.call('chatLocal', state, scope, [`inline-doc-${op}`, 'document-owner', value]);
  }
  expect(calls).toEqual([
    ['chatlocal:inline-doc-message', 'document-owner', 'initialize', 0],
    ['chatlocal:inline-doc-measure', 'document-owner', '728|340', 0],
    ['chatlocal:inline-doc-message', 'document-owner', 'adapter-ready', 0],
    ['chatlocal:inline-doc-ack', 'document-owner', 'reply-1', 0],
    ['chatlocal:inline-doc-permission', 'document-owner', '1:allow', 0],
  ]);
});
