import { expect, test } from 'bun:test';

// A picker still open across a dev restart answers into a runner that reuses
// view ids: its choice is dropped, not dispatched to the new view.
test('a picker answer that outlives its generation is dropped', async () => {
  globalThis.exact = {};
  const showDirectoryPicker = globalThis.showDirectoryPicker;
  try {
    await import(`./documents-glue.js?generation=${Date.now()}`);
    let generation = 1, open;
    const sent = [];
    globalThis.showDirectoryPicker = () => new Promise(resolve => { open = resolve; });
    const host = globalThis.exact.documents.install({ generation: () => generation, dispatch: (...args) => sent.push(args), log() {}, openFile: () => false });
    const shown = host.show({ present: true, view: 7 }, 'showDirectoryPicker');
    await new Promise(resolve => setTimeout(resolve, 0));
    generation = 2;
    open({ kind: 'directory', name: 'chosen' });
    await shown;
    expect(sent).toEqual([]);
    const again = host.show({ present: true, view: 7 }, 'showDirectoryPicker');
    await new Promise(resolve => setTimeout(resolve, 0));
    open({ kind: 'directory', name: 'chosen' });
    await again;
    expect(sent.map(([view, kind]) => [view, kind])).toEqual([[7, 1]]);
  } finally {
    globalThis.showDirectoryPicker = showDirectoryPicker;
    delete globalThis.exact;
  }
});
