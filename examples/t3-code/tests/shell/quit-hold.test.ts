// The quit shortcut's hold (quit-hold.contract; app.contract quitHoldKey, quitWatchTick, quitApply). The step functions
// port T3 Code 74541bab17's apps/desktop/src/window/QuitHold.test.ts and the cases its earlier revisions held, with
// their names; the mode is read at the press, so the pending-read cases have no counterpart. The harness applies each
// step as the root does and fires the watchdog at its deadline, where the root's 20 ms tick fires it within 20 ms.
import { describe, expect, test } from 'bun:test';
import { contractFns } from '../support/contract-fn-fixture';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { readFileSync } from 'node:fs';
import { appPath } from '../support/app-files';

type State = Record<string, unknown> & { watchUntil: number; consumed: boolean; signal: string; quit: boolean; mode: string };
type Key = { key: string; code: string; metaKey: boolean; altKey: boolean; shiftKey: boolean; ctrlKey: boolean; repeat: boolean };
const fns = contractFns('src/shell/quit-hold.contract', ['QuitHoldState']) as unknown as {
  quitHoldIdle: () => State;
  quitKeyDown: (s: State, e: Key, now: number, setting: string) => State;
  quitKeyUp: (s: State, e: Key, now: number) => State;
  quitWatchFired: (s: State) => State;
};
const HOLD_MS = 1200, DOUBLE_MS = 500, GRACE_MS = 600;
const HOLD_DOWN = 'down:hold', DOUBLE_CLICK_DOWN = 'down:double-click', UP = 'up';

type Input = { type?: 'keyDown' | 'keyUp'; key?: string; code?: string; meta?: boolean; alt?: boolean; shift?: boolean; isAutoRepeat?: boolean };
/** A US-layout event unless `code` names the physical key (another layout's letter on it). */
const event = (input: Input): Key => {
  const key = input.key ?? 'q';
  const code = input.code ?? (key === 'q' || key === 'ㅂ' ? 'KeyQ' : key === 'Meta' ? 'MetaLeft' : key.length === 1 ? `Key${key.toUpperCase()}` : key);
  return { key, code, metaKey: input.meta ?? true, altKey: input.alt ?? false, shiftKey: input.shift ?? false, ctrlKey: false, repeat: input.isAutoRepeat ?? false };
};

class Harness {
  s = fns.quitHoldIdle();
  now = 100_000;
  notifications: string[] = [];
  concealed = 0;
  quits = 0;
  prevented = 0;
  constructor(public mode = 'hold') {}
  apply(next: State) {
    if (next.consumed) this.prevented += 1;
    if (next.signal === 'down') this.notifications.push(`down:${next.mode}`);
    else if (next.signal === 'up') this.notifications.push(UP);
    else if (next.signal === 'conceal') this.concealed += 1;
    if (next.quit) this.quits += 1;
    this.s = next;
  }
  send(input: Input = {}) {
    const e = event(input);
    this.apply(input.type === 'keyUp' ? fns.quitKeyUp(this.s, e, this.now) : fns.quitKeyDown(this.s, e, this.now, this.mode));
  }
  advance(ms: number) {
    const end = this.now + ms;
    while (this.s.watchUntil > 0 && this.s.watchUntil <= end) {
      this.now = Math.max(this.now, this.s.watchUntil);
      this.apply(fns.quitWatchFired(this.s));
    }
    this.now = end;
  }
  /** The OS auto-repeating the held shortcut every `interval` ms for `duration` ms. */
  holdFor(duration: number, interval = 100, input: Input = {}) {
    for (let elapsed = 0; elapsed < duration; elapsed += interval) {
      this.advance(interval);
      this.send({ ...input, isAutoRepeat: true });
    }
  }
}

describe('makeQuitShortcutHandler', () => {
  test('shows the hint on a tap without quitting even when the release is never seen', () => {
    const h = new Harness();
    h.send();
    expect(h.notifications).toEqual([HOLD_DOWN]);
    expect(h.prevented).toBe(1);
    h.advance(HOLD_MS + GRACE_MS + 100);
    expect(h.quits).toBe(0);
    expect(h.notifications).toEqual([HOLD_DOWN, UP]);
  });
  test('conceals a completed hold then quits after release', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS + 100);
    expect(h.concealed).toBe(1);
    expect(h.quits).toBe(0);
    h.send({ type: 'keyUp' });
    expect(h.quits).toBe(1);
  });
  test('keeps a concealed hold committed when another key is pressed', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS + 100);
    h.send({ key: 'a' });
    expect(h.prevented).toBeGreaterThan(0);
    h.advance(GRACE_MS);
    expect(h.quits).toBe(1);
  });
  test('keeps a concealed hold committed through a fresh Cmd+Q press', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS + 100);
    h.send();
    h.send({ type: 'keyUp' });
    expect(h.quits).toBe(1);
  });
  test('quits when a completed hold goes quiet without release events', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS + 100);
    h.advance(GRACE_MS - 50);
    expect(h.quits).toBe(0);
    h.advance(100);
    expect(h.quits).toBe(1);
  });
  test('waits for slow repeats to stop before quitting', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS + 400, 400);
    expect(h.concealed).toBe(1);
    // Two observed 400 ms cadences: 800 ms, past the 600 ms grace.
    h.advance(700);
    expect(h.quits).toBe(0);
    h.advance(200);
    expect(h.quits).toBe(1);
  });
  test('waits for Q release when Cmd is released first', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS + 100);
    h.send({ type: 'keyUp', key: 'Meta', meta: false });
    expect(h.quits).toBe(0);
    h.send({ type: 'keyUp' });
    expect(h.quits).toBe(1);
  });
  test('does not quit when the hold stops before the duration', () => {
    const h = new Harness();
    h.send();
    h.holdFor(HOLD_MS - 300);
    h.advance(HOLD_MS + GRACE_MS);
    expect(h.quits).toBe(0);
    expect(h.notifications).toEqual([HOLD_DOWN, UP]);
  });
  test('cancels the hold when the modifier is released first', () => {
    const h = new Harness();
    h.send();
    h.send({ type: 'keyUp', key: 'Meta', meta: false });
    expect(h.notifications).toEqual([HOLD_DOWN, UP]);
    h.holdFor(HOLD_MS + 100, 100, { meta: false });
    expect(h.quits).toBe(0);
  });
  test('quits without showing a hint in direct mode', () => {
    const h = new Harness('direct');
    h.send();
    expect(h.quits).toBe(1);
    expect(h.notifications).toEqual([]);
  });
  test('quits on a quick second press in hold mode', () => {
    const h = new Harness();
    h.send();
    h.send({ type: 'keyUp' });
    h.advance(DOUBLE_MS - 100);
    h.send();
    expect(h.quits).toBe(1);
  });
  test('quits on a quick second press in hold mode when the first release is unseen', () => {
    const h = new Harness();
    h.send();
    h.advance(DOUBLE_MS - 100);
    h.send();
    expect(h.quits).toBe(1);
  });
  test('quits on a quick double press in double-click mode when the first release is unseen', () => {
    const h = new Harness('double-click');
    h.send();
    expect(h.notifications).toEqual([DOUBLE_CLICK_DOWN]);
    h.advance(DOUBLE_MS - 100);
    h.send();
    expect(h.quits).toBe(1);
  });
  test('keeps the double press hint visible after key release until the window ends', () => {
    const h = new Harness('double-click');
    h.send();
    h.send({ type: 'keyUp' });
    expect(h.notifications).toEqual([DOUBLE_CLICK_DOWN]);
    h.advance(DOUBLE_MS);
    expect(h.notifications).toEqual([DOUBLE_CLICK_DOWN, UP]);
    expect(h.quits).toBe(0);
  });
  test('accepts a second full shortcut after the modifier is released and pressed again', () => {
    const h = new Harness('double-click');
    h.send();
    h.send({ type: 'keyUp' });
    h.send({ type: 'keyUp', key: 'Meta', meta: false });
    h.advance(100);
    h.send({ key: 'Meta' });
    h.send();
    expect(h.quits).toBe(1);
  });
  test('treats two slow presses as separate attempts in double-click mode', () => {
    const h = new Harness('double-click');
    h.send();
    h.send({ type: 'keyUp' });
    h.advance(DOUBLE_MS + 100);
    h.send();
    expect(h.quits).toBe(0);
    expect(h.notifications).toEqual([DOUBLE_CLICK_DOWN, UP, DOUBLE_CLICK_DOWN]);
  });
  test('does not count auto-repeat as a second press', () => {
    const h = new Harness('double-click');
    h.send();
    h.holdFor(300);
    expect(h.quits).toBe(0);
  });
  test('cancels the hold when another key interrupts it', () => {
    const h = new Harness();
    h.send();
    h.send({ key: 'a' });
    expect(h.notifications).toEqual([HOLD_DOWN, UP]);
    h.holdFor(HOLD_MS + 100);
    expect(h.quits).toBe(0);
  });
  test('does not count an interrupted press toward a double press', () => {
    const h = new Harness('double-click');
    h.send();
    h.send({ shift: true });
    h.send();
    expect(h.quits).toBe(0);
    expect(h.notifications).toEqual([DOUBLE_CLICK_DOWN, UP, DOUBLE_CLICK_DOWN]);
  });
  test('a modifier pressed mid-hold cancels it like another key', () => {
    const h = new Harness();
    h.send();
    h.send({ key: 'Shift', shift: true });
    expect(h.notifications).toEqual([HOLD_DOWN, UP]);
  });
  test('ignores other shortcuts', () => {
    const h = new Harness();
    for (const input of [{ key: 'w' }, { alt: true }, { shift: true }, { meta: false }]) h.send(input);
    expect(h.prevented).toBe(0);
    expect(h.quits).toBe(0);
    expect(h.notifications).toEqual([]);
  });
  test('Q\'s physical key is the shortcut under a non-Latin input source', () => {
    const h = new Harness();
    h.send({ key: 'ㅂ' });
    expect(h.notifications).toEqual([HOLD_DOWN]);
  });
  test('another layout\'s letter on Q\'s key is not the shortcut (AZERTY\'s ⌘A selects all, Dvorak\'s ⌘\' does not quit)', () => {
    for (const mode of ['hold', 'double-click', 'direct']) {
      const azerty = new Harness(mode);
      azerty.send({ key: 'a', code: 'KeyQ' });
      azerty.send({ type: 'keyUp', key: 'a', code: 'KeyQ' });
      azerty.send({ key: 'a', code: 'KeyQ' });
      expect([azerty.prevented, azerty.quits, azerty.notifications]).toEqual([0, 0, []]);
      const dvorak = new Harness(mode);
      dvorak.send({ key: "'", code: 'KeyQ' });
      expect([dvorak.prevented, dvorak.quits, dvorak.notifications]).toEqual([0, 0, []]);
    }
    // AZERTY's Q is on A's key: its key, not its code, is the shortcut.
    const azertyQ = new Harness();
    azertyQ.send({ key: 'q', code: 'KeyA' });
    expect(azertyQ.notifications).toEqual([HOLD_DOWN]);
  });
  test('the hint says the reference\'s words', () => {
    // Read from the source: the fixture's word operators would rewrite the strings' "or".
    expect(readFileSync(appPath('src/shell/quit-hold.contract'), 'utf8')).toContain('fn quitHint(mode: string): string = mode == "hold" ? "Hold ⌘Q or press twice to quit" : "Press ⌘Q again to quit"');
  });
});

// ── The window root's actions ───────────────────────────────────────────────

const root = contractComponent('app.contract', 'T3Code');
const rootState = (mode = 'hold'): Env => ({
  quitHold: fns.quitHoldIdle(), quitHintMode: '', quitHintPressed: 'hold', quitHintLingers: false, quitHintSerial: 0,
  settingsCore: { quitMode: mode }, shortcutModifiers: { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false },
  speedModifierState: { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false }, undoHeldKey: '', composerFocused: false,
});

describe('the window root takes ⌘Q ahead of the menu', () => {
  const scope = (calls: string[], clock: { now: number }): Env => {
    const env: Env = {
      ...fns, preventDefault: () => calls.push('preventDefault'), stopPropagation: () => {}, close: () => calls.push('close'),
      performanceNow: () => clock.now, releaseUndoClaim: () => {},
      ...contractFns('src/shell/shortcut-modifiers.contract', ['ShortcutModifierState']),
      ShortcutModifierState: (record: unknown) => record,
    };
    return env;
  };
  const run = (state: Env, env: Env, name: string, args: unknown[]) => root.call(name, state, env, args);
  test('a press shows the hold hint, a completed hold closes the window at Q\'s release, and the hint goes', () => {
    const calls: string[] = [], clock = { now: 1000 }, state = rootState(), env = scope(calls, clock);
    const apply = (next: unknown) => run(state, env, 'quitApply', [next]);
    env.quitApply = apply;
    env.quitHoldKey = (down: boolean, e: Key) => run(state, env, 'quitHoldKey', [down, e]);
    run(state, env, 'modifierKey', [true, event({}), false]);
    expect(calls).toEqual(['preventDefault']);
    expect(state.quitHintMode).toBe('hold');
    for (let t = 100; t <= HOLD_MS + 100; t += 100) { clock.now = 1000 + t; run(state, env, 'modifierKey', [true, event({ isAutoRepeat: true }), false]); }
    expect(state.quitHintMode).toBe('');
    expect(calls.filter(call => call === 'close')).toEqual([]);
    run(state, env, 'modifierKey', [false, event({ type: 'keyUp' }), false]);
    expect(calls.at(-1)).toBe('close');
  });
  test('a released hold\'s hint lingers; the watchdog ends the hold', () => {
    const calls: string[] = [], clock = { now: 1000 }, state = rootState(), env = scope(calls, clock);
    env.quitApply = (next: unknown) => run(state, env, 'quitApply', [next]);
    run(state, env, 'quitHoldKey', [true, event({})]);
    clock.now = 1000 + HOLD_MS + GRACE_MS;
    run(state, env, 'quitWatchTick', []);
    expect(state.quitHintMode).toBe('hold');
    expect(state.quitHintLingers).toBe(true);
    run(state, env, 'quitHintHide', []);
    expect(state.quitHintMode).toBe('');
    expect(calls).toEqual(['preventDefault']);
  });
  test('direct mode closes at the press; a held ⌘W\'s repeats are taken, its press is not', () => {
    const calls: string[] = [], clock = { now: 1000 }, state = rootState('direct'), env = scope(calls, clock);
    env.quitApply = (next: unknown) => run(state, env, 'quitApply', [next]);
    env.quitHoldKey = (down: boolean, e: Key) => run(state, env, 'quitHoldKey', [down, e]);
    run(state, env, 'modifierKey', [true, event({}), false]);
    expect(calls).toEqual(['preventDefault', 'close']);
    calls.length = 0;
    run(state, env, 'modifierKey', [true, event({ key: 'w' }), false]);
    expect(calls).toEqual([]);
    run(state, env, 'modifierKey', [true, event({ key: 'w', isAutoRepeat: true }), false]);
    expect(calls).toEqual(['preventDefault']);
  });
});

describe('the window takes rightPanel.close\'s ⌘W ahead of File › Close Window', () => {
  const window = contractComponent('src/shell/app-window.contract', 'T3Window');
  const latin = contractFns('src/shell/quit-hold.contract', ['QuitHoldState']).quitTypesLatin;
  const keys = contractFns('src/composer/composer-keys.contract');
  const { windowChord } = contractFns('src/shell/app-window.contract', [], { quitTypesLatin: latin, composerChordKey: keys.composerChordKey });
  const run = (chord: string, kind: string, input: Input) => {
    const calls: string[] = [];
    const scope: Env = {
      shortcuts: chord ? [{ command: 'rightPanel.close', chord, kind, target: 'files' }] : [], windowChord,
      preventDefault: () => calls.push('preventDefault'), panelUi: (what: string, id: string) => calls.push(`panel ${what} ${id}`),
      prClose: () => calls.push('pr close'), modifierKey: () => {}, interacted: () => {},
    };
    scope.closeRightPanel = (k: string, t: string) => window.call('closeRightPanel', {}, scope, [k, t]);
    window.call('modifierDown', { focusEditable: false }, scope, [event(input).key, event(input)]);
    return calls;
  };
  test('⌘W closes the active surface, a pull request or the panel, and keeps it from the menu', () => {
    expect(run('Meta+W', 'close-surface', { key: 'w' })).toEqual(['preventDefault', 'panel tab-close files']);
    expect(run('Meta+W', 'pr-close', { key: 'w' })).toEqual(['preventDefault', 'pr close']);
    expect(run('Meta+W', 'close-diff', { key: 'w' })).toEqual(['preventDefault', 'panel close ']);
  });
  test('no open panel, a repeat, or another layout\'s letter on W\'s key leaves ⌘W to the menu', () => {
    expect(run('', '', { key: 'w' })).toEqual([]);
    expect(run('Meta+W', 'close-surface', { key: 'w', isAutoRepeat: true })).toEqual([]);
    expect(run('Meta+W', 'close-surface', { key: 'z', code: 'KeyW' })).toEqual([]);
    // Korean 2-Set types ㅈ on W's key: its physical key is the chord's.
    expect(run('Meta+W', 'close-surface', { key: 'ㅈ', code: 'KeyW' })).toEqual(['preventDefault', 'panel tab-close files']);
  });
});
