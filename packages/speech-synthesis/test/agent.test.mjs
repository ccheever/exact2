import { test, expect } from 'bun:test';
import { implementation, wordMs } from '../web/index.js';

// The web implementation's agent path on a session clock it is handed (LLP
// 1133.012 §9): 170 words a minute at rate 1, the queue its own.
function context() {
  const clock = { now: 0 }, claims = [];
  const ctx = {
    agent: true, now: () => clock.now, log: () => {}, changed: () => {}, foreground: () => true,
    audioSession: { claim: ({ type }) => { const c = { type, released: false, release() { c.released = true; } }; claims.push(c); return c; } },
  };
  return { ctx, clock, claims };
}

test('at clock +1000 the third word is spoken, and a pause stands the word clock still', async () => {
  const { ctx, clock } = context(), impl = implementation(ctx);
  const { id } = await impl.speak({ text: 'one two three four five' });
  expect(wordMs(1)).toBeCloseTo(352.94, 1);
  clock.now = 1000;
  let state = await impl.speechSynthesisState();
  expect(state).toMatchObject({ utterance: id, speaking: true, charIndex: 8, charLength: 5, name: 'word' });
  await impl.pauseSpeech();
  clock.now = 5000;
  state = await impl.speechSynthesisState();
  expect(state).toMatchObject({ paused: true, charIndex: 8 });
  await impl.resumeSpeech();
  clock.now = 5000 + 2 * wordMs(1);
  expect((await impl.speechSynthesisState()).charIndex).toBe(19); // "five": the word clock stood still while paused
  clock.now = 10000;
  expect(await impl.speechUtterance({ utterance: id })).toMatchObject({ state: 'ended', outcome: 'end', error: null });
});

test('cancel ends the speaking utterance interrupted and the queued ones canceled', async () => {
  const { ctx, clock } = context(), impl = implementation(ctx);
  const a = await impl.speak({ text: 'one two three' }), b = await impl.speak({ text: 'four five' });
  clock.now = 400;
  const state = await impl.cancelSpeech();
  expect(state).toMatchObject({ speaking: false, pending: false, queue: [] });
  expect(await impl.speechUtterance({ utterance: a.id })).toMatchObject({ outcome: 'error', error: 'interrupted' });
  expect(await impl.speechUtterance({ utterance: b.id })).toMatchObject({ outcome: 'error', error: 'canceled' });
});

test('a playback queue claims playback while it speaks, and an utterance whose reply was dropped is canceled', async () => {
  const { ctx, claims } = context(), impl = implementation(ctx);
  const a = await impl.speak({ text: 'one two', audioSessionType: 'playback' });
  expect(claims.map((c) => c.type)).toEqual(['playback']);
  impl.dispose([a.id]);
  expect(await impl.speechUtterance({ utterance: a.id })).toMatchObject({ state: 'ended', outcome: 'error' });
  expect(claims.every((c) => c.released)).toBe(true);
});

test('fail makes the next utterance to begin end with that error', async () => {
  const { ctx, clock } = context(), impl = implementation(ctx);
  const a = await impl.speak({ text: 'one two three' }), b = await impl.speak({ text: 'four' });
  clock.now = 500;
  impl.preferred({ fail: 'synthesis-failed' });
  impl.preferred({ fail: 'synthesis-failed' });
  clock.now = 3000;
  expect(await impl.speechUtterance({ utterance: a.id })).toMatchObject({ outcome: 'end' });
  expect(await impl.speechUtterance({ utterance: b.id })).toMatchObject({ outcome: 'error', error: 'synthesis-failed' });
  const c = await impl.speak({ text: 'five' });
  clock.now = 6000;
  expect(await impl.speechUtterance({ utterance: c.id })).toMatchObject({ outcome: 'end' });
});
