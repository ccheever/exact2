import { describe, expect, test } from "bun:test";
import { eventKeyNames, matchesChord, parseChords, parseCommandBindings, preventRepeatedTerminalCloseShortcut, preventTerminalCloseShortcut,
  terminalInputShortcutData } from "./entry";

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
