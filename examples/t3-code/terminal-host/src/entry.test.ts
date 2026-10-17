import { describe, expect, test } from "bun:test";
import { CompositionKeys, eventKeyNames, keepConversionContext, matchesChord, observeComposition, parseChords, parseCommandBindings, preventRepeatedTerminalCloseShortcut, preventTerminalCloseShortcut,
  terminalInputShortcutData, terminalKeyShortcutData } from "./entry";

const key = (init: Partial<KeyboardEvent> & { key: string; code: string }) => ({
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...init,
});

describe("the chords the page leaves to the app", () => {
  test("parses web-named chords", () => {
    expect(parseChords(["Meta+K", "Control+Shift+Backquote", "", 3])).toEqual([
      { key: "k", meta: true, ctrl: false, alt: false, shift: false },
      { key: "backquote", meta: false, ctrl: true, alt: false, shift: true },
    ]);
    expect(parseChords("Meta+K")).toEqual([]);
  });

  test("matches by the physical key, so a Korean 2-Set ⌘K still matches", () => {
    const chords = parseChords(["Meta+K"]);
    expect(matchesChord(key({ key: "ㅏ", code: "KeyK", metaKey: true }), chords)).toBe(true);
    expect(matchesChord(key({ key: "k", code: "KeyK", metaKey: true }), chords)).toBe(true);
    expect(matchesChord(key({ key: "k", code: "KeyK", metaKey: true, shiftKey: true }), chords)).toBe(false);
    expect(matchesChord(key({ key: "k", code: "KeyK", ctrlKey: true }), chords)).toBe(false);
    expect(matchesChord(key({ key: "k", code: "KeyK" }), chords)).toBe(false);
    expect(matchesChord(key({ key: "e", code: "KeyK", metaKey: true }), chords)).toBe(false);
    expect(matchesChord(key({ key: "k", code: "KeyE", metaKey: true }), chords)).toBe(true);
  });

  test("names digits and named keys", () => {
    expect(eventKeyNames({ key: "!", code: "Digit1" })).toEqual(["1", "!"]);
    expect(eventKeyNames({ key: "`", code: "Backquote" })).toEqual(["`", "backquote"]);
    expect(matchesChord(key({ key: "`", code: "Backquote", ctrlKey: true, shiftKey: true }), parseChords(["Control+Shift+Backquote"]))).toBe(true);
  });

  test("accepts configured command bindings and literal plus keys", () => {
    const bindings = parseCommandBindings([{ chord: "Meta+Alt+T", command: "terminal.toggle" },
      { chord: "Meta+W", command: "terminal.close" }, {}, null, { chord: "Meta+K", command: 3 }]);
    expect(bindings.map(({ chord, command }) => ({ chord, command }))).toEqual([
      { chord: "Meta+Alt+T", command: "terminal.toggle" }, { chord: "Meta+W", command: "terminal.close" },
    ]);
    expect(matchesChord(key({ key: "+", code: "Equal", metaKey: true }), parseChords(["Meta++"]))).toBe(true);
    expect(matchesChord(key({ key: " ", code: "Space" }), parseChords(["Space"]))).toBe(true);
  });
});

describe("terminal readline key policy", () => {
  test("moves by word and line, deletes to line start, and clears without app commands", () => {
    for (const [name, code, modifiers, data] of [
      ["ArrowLeft", "ArrowLeft", { altKey: true }, "\u001bb"],
      ["ArrowRight", "ArrowRight", { altKey: true }, "\u001bf"],
      ["ArrowLeft", "ArrowLeft", { metaKey: true }, "\u0001"],
      ["ArrowRight", "ArrowRight", { metaKey: true }, "\u0005"],
      ["Backspace", "Backspace", { metaKey: true }, "\u0015"],
      ["k", "KeyK", { metaKey: true }, "\u000c"],
      ["l", "KeyL", { ctrlKey: true }, "\u000c"],
      ["ㅏ", "KeyK", { metaKey: true }, "\u000c"],
    ] as const) {
      expect(terminalInputShortcutData(key({ key: name, code, ...modifiers }))).toBe(data);
      expect(terminalInputShortcutData(key({ key: name, code, ...modifiers, shiftKey: true }))).toBeNull();
    }
    expect(terminalInputShortcutData(key({ key: "c", code: "KeyC", ctrlKey: true }))).toBeNull();
    expect(terminalInputShortcutData(key({ key: "ArrowLeft", code: "ArrowLeft", ctrlKey: true }))).toBeNull();
  });

  test("a key that began while text composed matches by its key alone, as the reference's Process key", () => {
    // Korean 2-Set: ⌃L is "ㅣ"; it ends the composition and leaves the screen.
    expect(terminalKeyShortcutData(key({ key: "ㅣ", code: "KeyL", ctrlKey: true }), true)).toBeNull();
    expect(terminalKeyShortcutData(key({ key: "ㅣ", code: "KeyL", ctrlKey: true }), false)).toBe("\u000c");
    // A Latin layout's input method (Kotoeri romaji): ⌃L is "l" and still clears.
    expect(terminalKeyShortcutData(key({ key: "l", code: "KeyL", ctrlKey: true }), true)).toBe("\u000c");
    expect(terminalKeyShortcutData(key({ key: "ArrowLeft", code: "ArrowLeft", altKey: true }), true)).toBe("\u001bb");
  });
});

// T3 Code 1e2ecbd975 lib/terminalCloseShortcut.test.ts (macOS chord; the page receives the winners for terminalFocus).
describe("terminal close shortcut guards", () => {
  const bindings = parseCommandBindings([{ chord: "Meta+W", command: "terminal.close" }]);
  const keyboardEvent = (overrides: Partial<KeyboardEvent> = {}) => {
    let defaultPrevented = false;
    return { key: "w", code: "KeyW", metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, repeat: false, ...overrides,
      preventDefault: () => { defaultPrevented = true; }, get defaultPrevented() { return defaultPrevented; } };
  };
  test("prevents the browser default for a deliberate terminal close", () => {
    const event = keyboardEvent();
    expect(preventTerminalCloseShortcut(event, bindings)).toBe(true);
    expect(event.defaultPrevented).toBe(true);
  });
  test("keeps held close repeats from closing the browser after the last terminal unmounts", () => {
    expect(preventTerminalCloseShortcut(keyboardEvent(), bindings)).toBe(true);
    let browserCloseCount = 0;
    for (const repeat of [true, true, true]) {
      const event = keyboardEvent({ repeat });
      preventRepeatedTerminalCloseShortcut(event, bindings);
      if (!event.defaultPrevented) browserCloseCount += 1;
    }
    expect(browserCloseCount).toBe(0);
    expect(preventRepeatedTerminalCloseShortcut(keyboardEvent(), bindings)).toBe(false);
  });
  test("leaves a non-repeated window close and unrelated repeats alone", () => {
    const deliberateWindowClose = keyboardEvent({ repeat: false });
    const unrelatedRepeat = keyboardEvent({ key: "q", code: "KeyQ", repeat: true });
    expect(preventRepeatedTerminalCloseShortcut(deliberateWindowClose, bindings)).toBe(false);
    expect(deliberateWindowClose.defaultPrevented).toBe(false);
    expect(preventRepeatedTerminalCloseShortcut(unrelatedRepeat, bindings)).toBe(false);
    expect(unrelatedRepeat.defaultPrevented).toBe(false);
  });
});

// T3 Code 1e2ecbd975 keybindings.test.ts:1076-1200, macOS cases. The page's one terminalInputShortcutData
// answers all three; it only sees keydown (surface.ts onKeyDown), so the non-keydown cases are n/a.
describe("isTerminalClearShortcut", () => {
  test("matches Ctrl+L on all platforms", () => {
    expect(terminalInputShortcutData(key({ key: "l", code: "KeyL", ctrlKey: true }))).toBe("\u000c");
  });
  test("matches Cmd+K on macOS", () => {
    expect(terminalInputShortcutData(key({ key: "k", code: "KeyK", metaKey: true }))).toBe("\u000c");
  });
});
describe("terminalDeleteShortcutData", () => {
  test("maps Cmd+Backspace on macOS to delete-to-line-start", () => {
    expect(terminalInputShortcutData(key({ key: "Backspace", code: "Backspace", metaKey: true }))).toBe("\u0015");
  });
  test("ignores non-macOS platforms and modified variants", () => {
    expect(terminalInputShortcutData(key({ key: "Backspace", code: "Backspace", metaKey: true, altKey: true }))).toBeNull();
  });
});
describe("terminalNavigationShortcutData", () => {
  test("maps Option+Arrow on macOS to word movement", () => {
    expect(terminalInputShortcutData(key({ key: "ArrowLeft", code: "ArrowLeft", altKey: true }))).toBe("\u001bb");
    expect(terminalInputShortcutData(key({ key: "ArrowRight", code: "ArrowRight", altKey: true }))).toBe("\u001bf");
  });
  test("maps Cmd+Arrow on macOS to line movement", () => {
    expect(terminalInputShortcutData(key({ key: "ArrowLeft", code: "ArrowLeft", metaKey: true }))).toBe("\u0001");
    expect(terminalInputShortcutData(key({ key: "ArrowRight", code: "ArrowRight", metaKey: true }))).toBe("\u0005");
  });
  test("rejects unsupported combinations", () => {
    expect(terminalInputShortcutData(key({ key: "ArrowLeft", code: "ArrowLeft", shiftKey: true, altKey: true }))).toBeNull();
    expect(terminalInputShortcutData(key({ key: "a", code: "KeyA", altKey: true }))).toBeNull();
  });
});

describe("the input's composing state", () => {
  test("goes out as states: composing from the first update to the commit, once each, whatever the updates", () => {
    const input = new EventTarget(), states: boolean[] = [];
    const stop = observeComposition(input, (composing) => states.push(composing));
    // 2-Set Korean: g, k, s, then r starts the next syllable (한 commits, ㄱ composes), then Space commits ㄱ.
    for (const type of ["compositionstart", "compositionupdate", "compositionupdate", "compositionupdate", "compositionend",
      "compositionstart", "compositionupdate", "compositionend"]) input.dispatchEvent(new Event(type));
    expect(states).toEqual([true, false, true, false]);
    // A repeated start (an update that restarts) changes nothing; leaving the input ends it.
    input.dispatchEvent(new Event("compositionstart"));
    input.dispatchEvent(new Event("compositionstart"));
    input.dispatchEvent(new Event("blur"));
    input.dispatchEvent(new Event("compositionend"));
    expect(states).toEqual([true, false, true, false, true, false]);
    stop();
    input.dispatchEvent(new Event("compositionstart"));
    expect(states).toHaveLength(6);
  });
});

describe("keys that began while text composed", () => {
  const key = (code: string, init: Partial<{ metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; keyCode: number }> = {}) =>
    ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, keyCode: 0, code, ...init });

  test("each mark is taken once, by the next keydown with its modifiers and key", () => {
    let now = 0;
    const keys = new CompositionKeys(() => now);
    keys.mark(key("KeyC", { ctrlKey: true }));
    // A composing key WebKit still had queued (plain) does not take a ⌃ chord's mark.
    expect(keys.take(key("KeyG", { keyCode: 229 }))).toBe(false);
    expect(keys.take(key("KeyC", { ctrlKey: true, keyCode: 67 }))).toBe(true);
    // The second ⌃C, after the commit, is the shell's.
    expect(keys.take(key("KeyC", { ctrlKey: true, keyCode: 67 }))).toBe(false);
    keys.mark(key("Enter", { altKey: true }));
    expect(keys.take(key("Enter", { altKey: true, shiftKey: true }))).toBe(false);
    expect(keys.take(key("Enter", { altKey: true, keyCode: 13 }))).toBe(true);
  });

  test("a mark is taken by its own key, or by the input method's Process keydown", () => {
    const keys = new CompositionKeys(() => 0);
    keys.mark(key("KeyC", { ctrlKey: true }));
    // A ⌃ key whose mark never reached the page (the key went elsewhere) is not taken for it.
    expect(keys.take(key("KeyD", { ctrlKey: true, keyCode: 68 }))).toBe(false);
    expect(keys.take(key("KeyC", { ctrlKey: true, keyCode: 229 }))).toBe(true);
    keys.mark(key("KeyK", { ctrlKey: true }));
    expect(keys.take(key("", { ctrlKey: true, keyCode: 229 }))).toBe(true);
    // A key the native view has no name for is matched by its modifiers.
    keys.mark(key("", { ctrlKey: true }));
    expect(keys.take(key("F13", { ctrlKey: true, keyCode: 124 }))).toBe(true);
  });

  test("a mark whose keydown never came lapses, and so do marks older than the one taken", () => {
    let now = 0;
    const keys = new CompositionKeys(() => now);
    keys.mark(key("KeyC", { ctrlKey: true }));
    now = 1000;
    expect(keys.take(key("KeyC", { ctrlKey: true, keyCode: 67 }))).toBe(false);
    keys.mark(key("KeyC", { ctrlKey: true }));
    keys.mark(key("Enter", { altKey: true }));
    expect(keys.take(key("Enter", { altKey: true, keyCode: 13 }))).toBe(true);
    expect(keys.take(key("KeyC", { ctrlKey: true, keyCode: 67 }))).toBe(false);
  });
});

describe("the text the input method converts for ⌥Return", () => {
  const key = (code: string, init: Partial<{ metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean }> = {}) =>
    ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, code, ...init });

  test("only a waiting ⌥Return mark asks for it", () => {
    let now = 0;
    const keys = new CompositionKeys(() => now);
    keys.mark(key("KeyC", { ctrlKey: true }));
    expect(keys.awaitsOptionReturn()).toBe(false);
    keys.mark(key("Enter", { altKey: true }));
    expect(keys.awaitsOptionReturn()).toBe(true);
    expect(keys.take({ ...key("Enter", { altKey: true }), keyCode: 229 })).toBe(true);
    expect(keys.awaitsOptionReturn()).toBe(false);
    keys.mark(key("NumpadEnter", { altKey: true, shiftKey: true }));
    expect(keys.awaitsOptionReturn()).toBe(true);
    now = 1000;
    expect(keys.awaitsOptionReturn()).toBe(false);
  });

  /** An input that empties itself on every commit, as the surface's does (surface.ts onCompositionEnd). */
  const surfaceInput = () => {
    const input = Object.assign(new EventTarget(), { value: "" }), sent: string[] = [];
    input.addEventListener("compositionend", (event) => {
      sent.push(input.value || (event as CompositionEvent).data);
      input.value = "";
    });
    return { input, sent };
  };
  const end = (data: string) => Object.assign(new Event("compositionend"), { data });

  test("the committed syllable stays in the input until the input method's candidate replaces it", () => {
    const { input, sent } = surfaceInput();
    let converting = true;
    keepConversionContext(input, () => converting);
    // 2-Set, ⌥Return with 한 composing: the input method commits 한, which goes out, and finds it before the caret.
    input.value = "한";
    input.dispatchEvent(end("한"));
    expect(sent).toEqual(["한"]);
    expect(input.value).toBe("한");
    // It replaces 한 with 韓 while its list is open; the key's keydown follows and leaves the composition alone.
    input.dispatchEvent(new Event("compositionstart"));
    input.value = "韓";
    input.dispatchEvent(new Event("keydown"));
    expect(input.value).toBe("韓");
    // Picking 韓 commits it, as under the reference: the shell gets 한 then 韓.
    converting = false;
    input.dispatchEvent(end("韓"));
    expect(sent).toEqual(["한", "韓"]);
    expect(input.value).toBe("");
  });

  test("a declined key or leaving the input empties it, so the syllable never goes out twice", () => {
    const { input, sent } = surfaceInput();
    let converting = true;
    keepConversionContext(input, () => converting);
    input.value = "한";
    input.dispatchEvent(end("한"));
    converting = false;
    input.dispatchEvent(new Event("keydown"));
    expect(input.value).toBe("");
    converting = true;
    input.value = "글";
    input.dispatchEvent(end("글"));
    input.dispatchEvent(new Event("blur"));
    expect(input.value).toBe("");
    expect(sent).toEqual(["한", "글"]);
    // Any other commit leaves the input as the surface left it.
    converting = false;
    input.value = "말";
    input.dispatchEvent(end("말"));
    expect(input.value).toBe("");
  });
});
