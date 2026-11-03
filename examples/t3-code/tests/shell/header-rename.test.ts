// The chat header's rename field ends against the title it began with (T3 Code 5d3d78ab93 ChatHeader.tsx commitRename,
// originalTitle `renamingTitle ?? activeThreadTitle`): ending an untouched rename writes nothing, so a title that
// changed meanwhile (a generated one) stays; an edited one is sent whatever the title became.
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';

const window = contractComponent('src/shell/app-window.contract', 'T3Window');

function header(title: string) {
  const sent: unknown[][] = [];
  const state: Env = {};
  const scope: Env = {
    data: { threadTitle: title, threadId: 't1', projectId: 'p1' }, shell: { serverThread: true, panel: {} },
    titleSend: (...args: unknown[]) => sent.push(args), focus: () => {}, frame: () => ({ x: 0, y: 0, height: 0 }),
  };
  const act = (...args: unknown[]) => window.call('titleUi', state, scope, args);
  return { sent, state, scope, act };
}

describe('ending the header rename', () => {
  for (const finish of ['blur', 'Enter']) {
    test(`an untouched rename that ends with ${finish} keeps a newer title`, () => {
      const h = header('New thread');
      h.act('rename', '');
      expect(h.state).toMatchObject({ renaming: true, renameText: 'New thread' });
      h.scope.data = { threadTitle: 'Fix login redirect', threadId: 't1', projectId: 'p1' };
      h.act('commit', '');
      expect(h.state.renaming).toBe(false);
      expect(h.sent).toEqual([]);
    });
  }

  test('an edited rename is sent though the title changed meanwhile', () => {
    const h = header('Fix login redirect');
    h.act('rename', '');
    h.act('edit', 'My edited title');
    h.scope.data = { threadTitle: 'Another concurrent title', threadId: 't1', projectId: 'p1' };
    h.act('commit', '');
    expect(h.sent).toEqual([['commit', 'My edited title']]);
  });

  test('the menu\'s Rename starts from the title too', () => {
    const h = header('New thread');
    window.call('titleMenuPick', h.state, { ...h.scope, titleMenuPickRoot: () => {} }, ['ui:rename', 't1', '', '', '', false]);
    h.scope.data = { threadTitle: 'Generated', threadId: 't1', projectId: 'p1' };
    h.act('commit', '');
    expect(h.sent).toEqual([]);
  });
});
