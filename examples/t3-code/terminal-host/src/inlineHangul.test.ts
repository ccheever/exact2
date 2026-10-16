import { describe, expect, test } from "bun:test";
import { holdInlineHangul } from "./inlineHangul";

/** The terminal's input with the surface's own handlers after the hold's (surface.ts onInput, onKeyDown,
 * onCompositionEnd): what the shell receives, and the composing states the native view is told. */
function terminal() {
  const input = Object.assign(new EventTarget(), { value: "" });
  const shell: string[] = [], states: boolean[] = [];
  const stop = holdInlineHangul(input, input, (data) => shell.push(data), (composing) => states.push(composing));
  input.addEventListener("input", (event) => {
    const { data, isComposing } = event as InputEvent;
    if (isComposing) return;
    const text = input.value || data || "";
    if (text) shell.push(text);
    input.value = "";
  });
  input.addEventListener("keydown", (event) => {
    const { key, keyCode } = event as KeyboardEvent;
    if (keyCode === 229) return;
    if (key === "Enter") shell.push("\r");
    if (key === "Backspace") shell.push("\u007f");
  });
  input.addEventListener("compositionend", (event) => {
    const text = input.value || (event as CompositionEvent).data;
    if (text) shell.push(text);
    input.value = "";
  });
  // WebKit's order for one edit: the input method's text lands in the input, then its `input` event.
  const insert = (data: string) => {
    input.value += data;
    input.dispatchEvent(Object.assign(new Event("input"), { inputType: "insertText", data, isComposing: false }));
  };
  const replace = (data: string) => {
    input.value = input.value.slice(0, -1) + data;
    input.dispatchEvent(Object.assign(new Event("input"), { inputType: "insertReplacementText", data, isComposing: false }));
  };
  const key = (key: string, keyCode: number) => input.dispatchEvent(Object.assign(new Event("keydown"), { key, keyCode }));
  return { input, shell, states, stop, insert, replace, key };
}

describe("Hangul the input method edits in place", () => {
  test("the first syllable in an app the input method has not met goes out whole, not as jamo", () => {
    // Real 2-Set keys g k s g k s Space in a WKWebView, as the input method sent them (unmarked).
    const t = terminal();
    t.insert("ㅎ");
    t.key("ㅎ", 229);
    t.replace("하");
    t.key("ㅏ", 229);
    t.replace("한");
    t.key("ㄴ", 229);
    expect(t.shell).toEqual([]);
    expect(t.input.value).toBe("한");
    // g: 핞 (ㄶ); k: the ㅎ moves to the next syllable, so 한 is done.
    t.replace("핞");
    t.replace("한");
    t.insert("하");
    expect(t.shell).toEqual(["한"]);
    t.replace("한");
    // Space: the input method finishes 한 and inserts the space, which the surface sends and empties.
    t.replace("한");
    t.insert(" ");
    expect(t.shell).toEqual(["한", "한", " "]);
    expect(t.input.value).toBe("");
    expect(t.states).toEqual([true, false]);
  });

  test("a key the surface encodes sends the syllable first", () => {
    const t = terminal();
    t.insert("ㅎ");
    t.replace("하");
    t.replace("한");
    t.key("Shift", 16);
    expect(t.shell).toEqual([]);
    t.key("Enter", 13);
    expect(t.shell).toEqual(["한", "\r"]);
    expect(t.input.value).toBe("");
    expect(t.states).toEqual([true, false]);
  });

  test("Backspace deletes a lone jamo in the input, never in the shell", () => {
    const t = terminal();
    t.insert("ㅎ");
    t.replace("하");
    // The input method takes the vowel back itself; on the lone ㅎ it lets Backspace delete it.
    t.replace("ㅎ");
    t.key("Backspace", 229);
    t.replace("ㅎ");
    t.key("Backspace", 8);
    t.input.value = "";
    t.input.dispatchEvent(Object.assign(new Event("input"), { inputType: "deleteContentBackward", data: null, isComposing: false }));
    expect(t.shell).toEqual([]);
    // With nothing held, Backspace is the shell's.
    t.insert("ㅎ");
    t.replace("하");
    t.insert(" ");
    t.key("Backspace", 8);
    expect(t.shell).toEqual(["하", " ", "\u007f"]);
  });

  test("leaving the input, or a composition over the syllable, sends it once", () => {
    const t = terminal();
    t.insert("ㄱ");
    t.replace("글");
    t.input.dispatchEvent(new Event("blur"));
    expect(t.shell).toEqual(["글"]);
    expect(t.input.value).toBe("");
    // ⌥Return: the input method marks 한 to convert it; 한 goes out, then the pick replaces it.
    t.insert("ㅎ");
    t.replace("한");
    t.insert("ㄱ");
    t.replace("글");
    t.input.dispatchEvent(Object.assign(new Event("compositionstart"), { data: "글" }));
    expect(t.shell).toEqual(["글", "한", "글"]);
    t.input.value = "한契";
    t.input.dispatchEvent(Object.assign(new Event("compositionend"), { data: "契" }));
    expect(t.shell).toEqual(["글", "한", "글", "契"]);
  });

  test("text typed without an input method, and composed text, are the surface's", () => {
    const t = terminal();
    t.insert("g");
    t.input.value = "한";
    t.input.dispatchEvent(Object.assign(new Event("input"), { inputType: "insertCompositionText", data: "한", isComposing: true }));
    t.input.dispatchEvent(Object.assign(new Event("compositionend"), { data: "한" }));
    expect(t.shell).toEqual(["g", "한"]);
    expect(t.states).toEqual([]);
    t.stop();
    t.insert("ㅎ");
    expect(t.shell).toEqual(["g", "한", "ㅎ"]);
  });
});
