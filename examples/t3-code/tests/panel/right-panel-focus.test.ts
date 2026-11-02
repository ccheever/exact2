// T3 Code 74541bab17 (MIT; see LICENSE-T3), PreviewPanelShell (#14786): a right panel that closes with the keyboard
// focus in it gives the focus to the header's Toggle right panel. app-main.contract RightPanels, run with Contract's
// action semantics.
import { expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const panels = contractComponent('app-main.contract', 'RightPanels');
function run(steps: [string, unknown[]][]) {
  const state: Env = { focused: false, shown: false }, focused: string[] = [];
  // Entering also tells the window which workspace area holds the focus (shell-members.contract workspaceEntered).
  for (const [name, args] of steps) panels.call(name, state, { focus: (id: string) => focused.push(id), workspaceEntered: () => {} }, args);
  return focused;
}
const focusIn = { target: 'files-search', relatedTarget: null };

test('a panel closed with the focus inside hands it to the header toggle', () => {
  expect(run([['laidOut', [2, 1]], ['entered', [focusIn]], ['laidOut', [1, 1]]])).toEqual(['toggle-right-panel']);
  // Its removal blurs to nowhere: the focus counts as still inside.
  expect(run([['laidOut', [2, 1]], ['entered', [focusIn]], ['left', [{ target: 'files-search', relatedTarget: null }]], ['laidOut', [1, 1]]])).toEqual(['toggle-right-panel']);
});

test('a panel closed with the focus elsewhere leaves it there', () => {
  expect(run([['laidOut', [2, 1]], ['laidOut', [1, 1]]])).toEqual([]);
  expect(run([['laidOut', [2, 1]], ['entered', [focusIn]], ['left', [{ target: 'files-search', relatedTarget: 'composer' }]], ['laidOut', [1, 1]]])).toEqual([]);
});
