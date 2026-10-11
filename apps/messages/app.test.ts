import { afterEach, expect, test } from 'bun:test';
import { fixtures, presentation, reveal } from './model';

const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; });
let moduleId = 0;
async function app(saved?: Uint8Array) {
  let bytes = saved;
  const storage = { fs: {
    readFile: async () => { if (!bytes) throw new Error('ENOENT'); return bytes; },
    atomicWriteFile: async (_path: string, next: Uint8Array) => { bytes = next.slice(); },
  } };
  const source = await import(`./app.ts?case=${++moduleId}`);
  return { call: (name: string, ...args: unknown[]) => source.answer(name, args, {}, storage), saved: () => bytes };
}
test('reference runs end at a receipt or before a reaction, and the drag resists', () => {
  const t = fixtures()[0];
  t.messages[2].from = 'me';
  t.messages[2].status = 'Delivered';
  let rows = presentation(t, 0);
  expect(rows[1].tail).toBe(false);
  t.messages[2].reaction = '❤️';
  rows = presentation(t, 0);
  expect(rows[1].tail).toBe(true);
  expect(reveal(-20)).toBe(0);
  expect(reveal(66)).toBe(33);
  expect(reveal(400)).toBeLessThan(66);
});
test('drafts and reactions survive a new module without leaking between conversations', async () => {
  const a = await app();
  await a.call('command', 'draft', 'ai-1', 'Only Claude gets this draft', 100);
  await a.call('command', 'reaction', 'demo', 'demo-1|❤️', 100);
  const b = await app(a.saved());
  globalThis.fetch = (async () => Response.json({ connected: true })) as typeof fetch;
  await b.call('pulse', 200);
  expect((await b.call('snapshot', '', 'ai-1', 1, 200)).draft).toBe('Only Claude gets this draft');
  expect((await b.call('snapshot', '', 'ai-2', 1, 200)).draft).toBe('');
  expect((await b.call('snapshot', '', 'demo', 1, 200)).messages[0].reaction).toBe('❤️');
});
test('stream snapshots replace text, preserve context, and keep simultaneous models separate', async () => {
  const requests: any[] = [];
  let chunk = 'Hello';
  globalThis.fetch = (async (url: any, init: any) => {
    if (init?.method === 'POST') { const body = JSON.parse(init.body); requests.push(body); return Response.json({ id: body.model.includes('claude') ? 'claude' : 'gemini' }); }
    if (String(url).includes('/turns/')) return Response.json({ content: chunk, done: chunk.endsWith('!'), error: '' });
    return Response.json({ connected: true });
  }) as typeof fetch;
  const a = await app();
  await a.call('sendMessage', 'ai-1', 'Remember marmalade', '', 100);
  await a.call('sendMessage', 'ai-3', 'Remember violet', '', 100);
  await a.call('pulse', 400);
  expect((await a.call('snapshot', '', 'ai-1', 1, 400)).messages[1].body).toBe('Hello');
  chunk = 'Hello world!';
  await a.call('pulse', 700);
  const state = await a.call('snapshot', '', 'ai-1', 1, 700);
  expect(state.messages).toHaveLength(2);
  expect(state.messages[1].body).toBe('Hello world!');
  expect(state.busy).toBe(false);
  await a.call('sendMessage', 'ai-1', 'What did I say?', 'Remember marmalade', 2000);
  expect(requests[2].messages.map((m: any) => m.content)).toEqual(['Remember marmalade', 'Hello world!', 'What did I say?']);
  expect(requests[2].messages.some((m: any) => m.content.includes('violet'))).toBe(false);
  const reply = await a.call('snapshot', '', 'ai-1', 1, 2000);
  expect(reply.messages[2].reply).toBe('Remember marmalade');
});
test('a service error keeps the outgoing message and retry does not send a duplicate', async () => {
  let fail = true, posts = 0;
  globalThis.fetch = (async (_url: any, init: any) => {
    if (init?.method === 'POST') { posts++; return Response.json(fail ? { error: 'Insufficient credits' } : { id: 'retry' }, { status: fail ? 402 : 200 }); }
    return Response.json({ connected: true });
  }) as typeof fetch;
  const a = await app();
  await a.call('sendMessage', 'ai-1', 'Hello', '', 100);
  const state = await a.call('snapshot', '', 'ai-1', 1, 100);
  expect(state.messages).toHaveLength(1);
  expect(state.error).toBe('Insufficient credits');
  fail = false;
  await a.call('command', 'retry', 'ai-1', '', 500);
  expect(posts).toBe(2);
  expect((await a.call('snapshot', '', 'ai-1', 1, 500)).messages).toHaveLength(1);
});
test('stop aborts the job and ignores a poll already in flight', async () => {
  let complete: (value: Response) => void = () => {}, cancelled = false;
  globalThis.fetch = (async (url: any, init: any) => {
    if (init?.method === 'POST') return Response.json({ id: 'cancel' });
    if (init?.method === 'DELETE') { cancelled = true; return Response.json({}); }
    if (String(url).includes('/turns/')) return new Promise<Response>(resolve => { complete = resolve; });
    return Response.json({ connected: true });
  }) as typeof fetch;
  const a = await app();
  await a.call('sendMessage', 'ai-1', 'Write a story', '', 100);
  const polling = a.call('pulse', 500);
  await new Promise(resolve => setTimeout(resolve, 0));
  await a.call('command', 'stop', 'ai-1', '', 600);
  complete(Response.json({ content: 'A late result', done: true, error: '' }));
  await polling;
  const state = await a.call('snapshot', '', 'ai-1', 1, 700);
  expect(cancelled).toBe(true);
  expect(state.busy).toBe(false);
  expect(state.messages).toHaveLength(1);
});
test('a sent message shows from the send, once, until the answer has it', async () => {
  const a = await app();
  const source = await import(`./app.ts?case=${moduleId}`);
  const before = await a.call('snapshot', '', 'ai-1', 1, 100);
  let kept = 0;
  const write = (reply?: { id: string }, answered = false) => ({ id: 7, mutation: 'changed', source: 'sendMessage', args: ['ai-1', ' Hello ', '', 100], reply, answered, keep: () => { kept++; } });
  const shown = source.overlay('snapshot', ['', 'ai-1', 1, 100, 0], before, [write()]);
  expect(shown.messages.map((m: any) => m.body)).toEqual([...before.messages.map((m: any) => m.body), 'Hello']);
  expect([shown.count, shown.messages.at(-1).outgoing]).toEqual([before.messages.length + 1, true]);
  // Another conversation's answer, or one the service ignored, shows nothing.
  expect(source.overlay('snapshot', ['', 'ai-2', 1, 100, 0], await a.call('snapshot', '', 'ai-2', 1, 100), [write()])).toBeUndefined();
  expect(source.overlay('snapshot', ['', 'ai-1', 1, 100, 0], before, [write({ id: '' })])).toBeUndefined();
  // An answer asked after the landing that lacks it keeps it; one that has it shows as it is, never twice.
  expect(source.overlay('snapshot', ['', 'ai-1', 1, 100, 0], before, [write({ id: 'm-1' }, true)]).count).toBe(before.messages.length + 1);
  expect(kept).toBe(1);
  globalThis.fetch = (async () => Response.json({ id: 'turn' })) as typeof fetch;
  await a.call('sendMessage', 'ai-1', ' Hello ', '', 100);
  const after = await a.call('snapshot', '', 'ai-1', 1, 100);
  expect(source.overlay('snapshot', ['', 'ai-1', 1, 100, 0], after, [write()])).toBeUndefined();
});
