import { afterEach, expect, mock, test } from 'bun:test';

mock.module('@exact/language-model', () => Object.fromEntries([
  'abortLanguageModelResponse', 'createLanguageModel', 'destroyLanguageModel', 'languageModelAvailability', 'languageModelResponse', 'prompt', 'promptStreaming',
].map(name => [name, () => { throw new Error(`unexpected device call: ${name}`); }])));
const realFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = realFetch; });

test('stopping while the service accepts a turn drops its blank row and cancels the late job', async () => {
  const source = await import('./app.ts');
  const storage = { fs: { readFile: async () => { throw new Error('ENOENT'); }, atomicWriteFile: async () => {} } };
  const call = (name: string, ...args: unknown[]) => (source.answer as any)(name, args, {}, storage);
  let accept!: (r: Response) => void, started!: () => void;
  const pending = new Promise<Response>(resolve => { accept = resolve; });
  const posted = new Promise<void>(resolve => { started = resolve; });
  const deleted: string[] = [];
  globalThis.fetch = (async (url: any, init: any) => {
    if (init?.method === 'POST') { started(); return pending; }
    if (init?.method === 'DELETE') { deleted.push(String(url)); return Response.json({}); }
    return Response.json({ connected: true });
  }) as typeof fetch;
  const { id } = await call('command', 'new', '', source.FEATURED[0]!.id, 0);
  const send = call('command', 'send', id, 'Hello', 100);
  await posted;
  await call('command', 'stop', id, '', 200);
  accept(Response.json({ id: 'late' }));
  await send;
  const state = await call('snapshot', id, '', 0, 300, 0);
  expect(state.streaming).toBe(false);
  expect(state.messages.map((m: any) => m.body)).toEqual(['Hello']);
  expect(deleted).toEqual([`${source.SERVICE}/turns/late`]);
});
