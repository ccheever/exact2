// Edit › Copy and Cut over a server Browser tab (T3 Code 2359643cff, ServerBrowserSurface.tsx handleClipboard): the menu
// bar's item fires no key, so the page is sent the shortcut the server copies on; ⌘C and ⌘X already reached the page as
// their key, and their copy or cut does not send them again. The field's own copy never runs.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const page = contractComponent('server-browser.contract', 'ServerBrowserPage');
const key = (k: string, meta: boolean) => ({ key: k, metaKey: meta, ctrlKey: false, altKey: false, shiftKey: false, isComposing: false });

function mounted(controlled = true) {
  const calls: unknown[][] = [];
  const scope = {
    runtimeId: 'run-1', mini: false, pdfUrl: '', frame: { controlled }, serverFrame: { controlled }, serverMiniFrame: { controlled },
    streamControl: (...args: unknown[]) => calls.push(['control', ...args]), streamKey: (...args: unknown[]) => calls.push(['key', args[0], args[2]]),
    preventDefault: () => calls.push(['preventDefault']), stopPropagation: () => calls.push(['stopPropagation']), blur: () => {},
  };
  return { state: page.state(scope), scope, calls };
}

describe('server browser Copy and Cut', () => {
  test('the menu bar\'s Copy and Cut send Control+C and Control+X to the page it controls', () => {
    const { state, scope, calls } = mounted();
    page.call('clip', state, scope, ['copy']);
    page.call('clip', state, scope, ['cut']);
    expect(calls).toEqual([['preventDefault'], ['control', 'run-1', 'copy'], ['preventDefault'], ['control', 'run-1', 'cut']]);
  });
  test('⌘C reached the page as its key, so the copy after it sends nothing more', () => {
    const { state, scope, calls } = mounted();
    page.call('key', state, scope, ['down', 'c', key('c', true)]);
    page.call('clip', state, scope, ['copy']);
    page.call('key', state, scope, ['up', 'c', key('c', true)]);
    expect(calls).toEqual([['preventDefault'], ['key', 'run-1', 'down'], ['preventDefault'], ['preventDefault'], ['key', 'run-1', 'up']]);
    calls.length = 0;
    page.call('clip', state, scope, ['copy']);
    expect(calls).toEqual([['preventDefault'], ['control', 'run-1', 'copy']]);
  });
  test('without control, a copy only keeps the field\'s buffer off the clipboard', () => {
    const { state, scope, calls } = mounted(false);
    page.call('clip', state, scope, ['copy']);
    expect(calls).toEqual([['preventDefault']]);
  });
  // ServerBrowserSurface handleMouseUp (#22522): a mouse's Back and Forward step the tab at the release, with or without
  // control, and never reach the page or the window's own Back and Forward.
  test('the thumb buttons step the tab\'s history at the release', () => {
    for (const controlled of [true, false]) {
      const { state, scope, calls } = mounted(controlled);
      const streamMouse = (...args: unknown[]) => calls.push(['mouse', ...args.slice(2, 3)]), at = { ...scope, streamMouse, interacted: () => {}, focus: () => {} };
      page.call('mouse', state, at, ['down', { button: 3 }]);
      page.call('mouse', state, at, ['up', { button: 3 }]);
      page.call('mouse', state, at, ['up', { button: 4 }]);
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['control', 'run-1', 'back'], ['preventDefault'], ['stopPropagation'], ['control', 'run-1', 'forward']]);
    }
  });
});
