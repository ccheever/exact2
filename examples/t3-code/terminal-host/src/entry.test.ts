import { describe, expect, test } from "bun:test";
import { eventKeyNames, matchesChord, parseChords } from "./entry";

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
  });

  test("names digits and named keys", () => {
    expect(eventKeyNames({ key: "!", code: "Digit1" })).toEqual(["1", "!"]);
    expect(eventKeyNames({ key: "`", code: "Backquote" })).toEqual(["`", "backquote"]);
    expect(matchesChord(key({ key: "`", code: "Backquote", ctrlKey: true, shiftKey: true }), parseChords(["Control+Shift+Backquote"]))).toBe(true);
  });
});
