// browser-surface part 5. Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under its own names:
// apps/desktop/src/preview/PreviewKeyboard.test.ts (16 with its \`it.each\` rows), whole. Then the clone's rows for
// what the module sends: T3TerminalKeys' chord, the typed text, the editing expression and the DOM fallback.
import { describe, expect, it } from "bun:test";

import {
  makePreviewAutomationKeySequence,
  makePreviewAutomationNativeKeySequence,
  pressPlan,
  domKeyExpression,
} from "./browser-automation-keys";

describe("preview keyboard packets", () => {
  it("includes the Chromium virtual key code and Enter text", () => {
    expect(makePreviewAutomationKeySequence({ key: "Enter" })).toEqual({
      keyDown: {
        type: "keyDown",
        key: "Enter",
        code: "Enter",
        modifiers: 0,
        windowsVirtualKeyCode: 13,
        location: 0,
        isKeypad: false,
        text: "\r",
        unmodifiedText: "\r",
      },
      keyUp: {
        type: "keyUp",
        key: "Enter",
        code: "Enter",
        modifiers: 0,
        windowsVirtualKeyCode: 13,
        location: 0,
        isKeypad: false,
      },
      signal: { kind: "key", key: "Enter", code: "Enter" },
    });
  });

  it("dispatches printable keys as text key-down events", () => {
    const sequence = makePreviewAutomationKeySequence({ key: "z" });
    expect(sequence.keyDown).toMatchObject({
      type: "keyDown",
      key: "z",
      code: "KeyZ",
      windowsVirtualKeyCode: 90,
      text: "z",
    });
    expect(sequence.keyUp).not.toHaveProperty("text");
  });

  it("suppresses text and uses raw key-down for shortcuts", () => {
    expect(
      makePreviewAutomationKeySequence({ key: "a", modifiers: ["Meta"] }, { isMac: true }).keyDown,
    ).toEqual({
      type: "rawKeyDown",
      key: "a",
      code: "KeyA",
      modifiers: 4,
      windowsVirtualKeyCode: 65,
      location: 0,
      isKeypad: false,
      commands: ["selectAll"],
    });
  });

  it("maps common macOS editing shortcuts without changing other platforms", () => {
    expect(
      makePreviewAutomationKeySequence({ key: "z", modifiers: ["Shift", "Meta"] }, { isMac: true })
        .keyDown.commands,
    ).toEqual(["redo"]);
    expect(
      makePreviewAutomationKeySequence({ key: "a", modifiers: ["Meta"] }).keyDown,
    ).not.toHaveProperty("commands");
  });

  it("resolves shifted printable keys to their browser values", () => {
    const sequence = makePreviewAutomationKeySequence({ key: "1", modifiers: ["Shift"] });
    expect(sequence.keyDown).toMatchObject({
      key: "!",
      code: "Digit1",
      modifiers: 8,
      windowsVirtualKeyCode: 49,
      text: "!",
    });
    expect(sequence.signal).toEqual({ kind: "key", key: "!", code: "Digit1" });
  });

  it("keeps shifted key values while suppressing text for modified chords", () => {
    const sequence = makePreviewAutomationKeySequence({
      key: "1",
      modifiers: ["Control", "Shift"],
    });
    expect(sequence.keyDown).toEqual({
      type: "rawKeyDown",
      key: "!",
      code: "Digit1",
      modifiers: 10,
      windowsVirtualKeyCode: 49,
      location: 0,
      isKeypad: false,
    });
    expect(sequence.signal).toEqual({ kind: "key", key: "!", code: "Digit1" });
  });

  it.each([
    ["Enter", "\r"],
    ["z", "z"],
  ])("converts %s into native down, char, and up packets", (key, text) => {
    const sequence = makePreviewAutomationNativeKeySequence({ key });
    const shared = { keyCode: key, modifiers: [], skipIfUnhandled: true };
    expect(sequence.keyDown).toEqual({ type: "keyDown", ...shared });
    expect(sequence.char).toEqual({ type: "char", ...shared, keyCode: text });
    expect(sequence.keyUp).toEqual({ type: "keyUp", ...shared });
  });

  it("suppresses text for shortcuts and retains macOS editing commands", () => {
    const sequence = makePreviewAutomationNativeKeySequence(
      { key: "a", modifiers: ["Meta"] },
      { isMac: true },
    );
    expect(sequence.keyDown).toEqual({
      type: "keyDown",
      keyCode: "a",
      modifiers: ["meta"],
      skipIfUnhandled: true,
    });
    expect(sequence.char).toBeUndefined();
    expect(sequence.commands).toEqual(["selectAll"]);
  });

  it.each([
    ["ArrowLeft", "Left"],
    ["ArrowRight", "Right"],
    ["ArrowUp", "Up"],
    ["ArrowDown", "Down"],
  ])("maps %s to Electron's %s accelerator", (key, keyCode) => {
    const sequence = makePreviewAutomationNativeKeySequence({ key });
    expect(sequence.keyDown.keyCode).toBe(keyCode);
    expect(sequence.keyUp.keyCode).toBe(keyCode);
    expect(sequence.signal.key).toBe(key);
    expect(sequence.char).toBeUndefined();
  });

  it("matches native uppercase key signals without inventing shortcut modifiers", () => {
    const plain = makePreviewAutomationNativeKeySequence({ key: "X" });
    expect(plain.signal).toEqual({ kind: "key", key: "x", code: "KeyX" });
    expect(plain.char?.keyCode).toBe("X");
    const shortcut = makePreviewAutomationNativeKeySequence({ key: "A", modifiers: ["Control"] });
    expect(shortcut.signal).toEqual({ kind: "key", key: "a", code: "KeyA" });
    expect(shortcut.keyDown.modifiers).toEqual(["control"]);
    expect(shortcut.char).toBeUndefined();
    expect(
      makePreviewAutomationNativeKeySequence({ key: "X", modifiers: ["Shift"] }).signal,
    ).toEqual({
      kind: "key",
      key: "X",
      code: "KeyX",
    });
  });

  it("matches native signals for Unicode text and literal spaces", () => {
    const unicode = makePreviewAutomationNativeKeySequence({ key: "é" });
    expect(unicode.signal).toEqual({ kind: "key", key: "", code: "" });
    expect(unicode.char?.keyCode).toBe("é");
    expect(makePreviewAutomationNativeKeySequence({ key: " " }).signal).toEqual({
      kind: "key",
      key: " ",
      code: "Space",
    });
  });

  it("preserves text and editing commands for isolated child renderer targets", () => {
    const text = makePreviewAutomationKeySequence({ key: "é" });
    expect(text.keyDown).toMatchObject({ type: "keyDown", text: "é", key: "é" });
    expect(text.keyUp).toMatchObject({ type: "keyUp", key: "é" });
    const shortcut = makePreviewAutomationKeySequence(
      { key: "a", modifiers: ["Meta"] },
      { isMac: true },
    );
    expect(shortcut.keyDown).toMatchObject({
      type: "rawKeyDown",
      modifiers: 4,
      commands: ["selectAll"],
    });
    expect(shortcut.keyDown).not.toHaveProperty("text");
    expect(shortcut.keyDown).not.toHaveProperty("nativeVirtualKeyCode");
  });
});

describe("the module's key (pressPlan)", () => {
  it("names the key as T3TerminalKeys reads it", () => {
    expect(pressPlan({ key: "Enter" })).toMatchObject({ chord: "Enter", text: "\r", commands: [] });
    expect(pressPlan({ key: " " })).toMatchObject({ chord: "Space", text: " " });
    expect(pressPlan({ key: "a", modifiers: ["Meta"] })).toMatchObject({ chord: "Meta+a", text: "", commands: ["selectAll"] });
    expect(pressPlan({ key: "z", modifiers: ["Shift", "Meta"] })).toMatchObject({ chord: "Shift+Meta+z", commands: ["redo"] });
    expect(pressPlan({ key: "!" })).toMatchObject({ chord: "Shift+1", text: "!" });
    expect(pressPlan({ key: "ArrowDown" })).toMatchObject({ chord: "ArrowDown", text: "" });
    expect(pressPlan({ key: "F5" })).toMatchObject({ chord: "F5" });
    expect(pressPlan({ key: "é" })).toMatchObject({ chord: "", text: "é" });
  });
  it("edits in the page for macOS editing shortcuts, with the clipboard put in by the module", () => {
    const paste = pressPlan({ key: "v", modifiers: ["Meta"] });
    expect(paste.commands).toEqual(["paste"]);
    expect(paste.editing).toContain("for (const { type, data } of __T3_PREVIEW_CLIPBOARD__)");
    expect(paste.editing).not.toContain("setHTML");
    expect(pressPlan({ key: "Enter" }).editing).toBe("");
  });
  it("falls back to DOM events with the key's default edit", () => {
    const enter = domKeyExpression({ key: "Enter" });
    expect(enter).toContain('new KeyboardEvent("keydown"');
    expect(enter).toContain("target.form.requestSubmit()");
    expect(domKeyExpression({ key: "x" })).toContain('"key":"x"');
  });
});
