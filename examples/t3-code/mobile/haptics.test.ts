import { describe, expect, test } from 'bun:test';
import { mobileStreamingAssistant, mobileStreamingHaptic, type MobileStreamingHapticState } from './haptics';

const message = (id: string, textLength: number) => ({ id, textLength });
const row = (id: string, text: string, streaming = true, type = 'assistant_message') => ({
  sourceThreadId: 'thread', sourceItemId: `item-${id}`,
  item: { id: `item-${id}`, type, messageId: id, text, streaming },
});

describe('mobile streaming haptics', () => {
  test('projects the latest streaming assistant using message identity and UTF-16 length', () => {
    expect(mobileStreamingAssistant([row('a', 'first'), row('b', '😀'), row('u', 'user', true, 'user_message'),
      row('c', 'finished', false), { item: { type: 'tool_call' } }])).toEqual(message('b', 2));
    expect(mobileStreamingAssistant([row('a', 'older'), row('empty', '')])).toEqual(message('empty', 0));
    expect(mobileStreamingAssistant([row('done', 'finished', false)])).toBeNull();
    expect(mobileStreamingAssistant(undefined)).toBeNull();
  });
  test('initial hydration is silent, even when history already contains a live answer', () => {
    const result = mobileStreamingHaptic(null, 'env/thread', message('a', 10), 10_000);
    expect(result.haptic).toBe('');
    expect(mobileStreamingHaptic(result.state, 'env/thread', message('a', 11), 10_001).haptic).toBe('selection');
  });
  test('an initially empty feed ticks on a new empty stream, which bypasses growth throttling', () => {
    const hydrated = mobileStreamingHaptic(null, 'env/thread', null, 0).state;
    const first = mobileStreamingHaptic(hydrated, 'env/thread', message('a', 0), 1);
    const second = mobileStreamingHaptic(first.state, 'env/thread', message('b', 0), 2);
    expect(first.haptic).toBe('selection');
    expect(second.haptic).toBe('selection');
    expect(second.state.lastAt).toBe(2);
  });
  test('only actual growth ticks at 320 ms; throttled growth is consumed without a delayed tick', () => {
    const initial: MobileStreamingHapticState = { owner: 'thread', latest: message('a', 1), lastAt: 1_000 };
    const throttled = mobileStreamingHaptic(initial, 'thread', message('a', 2), 1_319);
    expect(throttled.haptic).toBe('');
    expect(throttled.state.latest?.textLength).toBe(2);
    const clockOnly = mobileStreamingHaptic(throttled.state, 'thread', message('a', 2), 1_320);
    expect(clockOnly.haptic).toBe('');
    const growth = mobileStreamingHaptic(clockOnly.state, 'thread', message('a', 3), 1_320);
    expect(growth.haptic).toBe('selection');
    expect(growth.state.lastAt).toBe(1_320);
    expect(initial.latest?.textLength).toBe(1);
  });
  test('same-length edits and shrink are silent but become the next growth baseline', () => {
    let state = mobileStreamingHaptic(null, 'thread', message('a', 5), 10_000).state;
    for (const length of [5, 4, 0]) {
      const result = mobileStreamingHaptic(state, 'thread', message('a', length), 10_000);
      expect(result.haptic).toBe(''); state = result.state;
    }
    expect(mobileStreamingHaptic(state, 'thread', message('a', 1), 10_000).haptic).toBe('selection');
  });
  test('stream completion clears message identity and restarting that id is a new stream', () => {
    const previous: MobileStreamingHapticState = { owner: 'thread', latest: message('a', 10), lastAt: 1_000 };
    const ended = mobileStreamingHaptic(previous, 'thread', null, 1_001);
    expect(ended).toEqual({ state: { owner: 'thread', latest: null, lastAt: 1_000 }, haptic: '' });
    expect(mobileStreamingHaptic(ended.state, 'thread', message('a', 10), 1_002).haptic).toBe('selection');
  });
  test('thread/environment changes hydrate silently while retaining the source throttle clock', () => {
    const previous: MobileStreamingHapticState = { owner: 'env-a/thread', latest: message('a', 1), lastAt: 1_000 };
    const switched = mobileStreamingHaptic(previous, 'env-b/thread', message('a', 100), 1_001);
    expect(switched.haptic).toBe('');
    expect(switched.state.lastAt).toBe(1_000);
    expect(mobileStreamingHaptic(switched.state, 'env-b/thread', message('a', 101), 1_002).haptic).toBe('');
    expect(mobileStreamingHaptic(switched.state, 'env-b/thread', message('a', 101), 1_320).haptic).toBe('selection');
    expect(mobileStreamingHaptic(null, 'env-b/thread', message('a', 101), 1_321).haptic).toBe('');
  });
  test('a backwards wall clock suppresses growth but never suppresses a new stream', () => {
    const state: MobileStreamingHapticState = { owner: 'thread', latest: message('a', 1), lastAt: 1_000 };
    expect(mobileStreamingHaptic(state, 'thread', message('a', 2), 500).haptic).toBe('');
    expect(mobileStreamingHaptic(state, 'thread', message('b', 0), 500).haptic).toBe('selection');
  });
});
