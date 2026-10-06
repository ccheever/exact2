import { describe, expect, test } from "bun:test";
import { eventKeyNames, matchesChord, parseChords, parseCommandBindings, terminalInputShortcutData } from "./entry";

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
