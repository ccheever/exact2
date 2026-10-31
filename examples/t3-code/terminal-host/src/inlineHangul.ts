// Hangul an input method edits in place, without marked text, held in the terminal's input until
// the input method is done with it.
//
// macOS's input method kit (IMKTextDocumentTextInputAdaptor, macOS 26) lets the Korean input method
// edit an app's text in place: for the first key it inserts the jamo unmarked (`insertText`, ㅎ), and
// for each further key it reads the syllable back before the caret and replaces it
// (`insertReplacementText`: 하, then 한). It does so in an app that offers document access, as
// WKWebView does, unless the bundle identifier is com.google.Chrome or com.apple.Terminal or the
// input method has found the app's text not holding what it inserted; from then on, for the life of
// the input method's process, it composes with marked text (`compositionstart` … `compositionend`),
// which is what the surface handles. The surface sends its input's text on every other `input` event
// and empties it (surface.ts onInput), so alone it sends the first syllable typed in an app the
// input method has not met as separate jamo (ㅎ ㅏ ㄴ). The reference's surface under Chromium does
// no better there (ㅎ 하 ㅏ ㄴ한, measured in Chrome for Testing under a fresh bundle identifier).
//
// So while the input method edits in place, its text stays in the input, as it inserted it, and a
// syllable goes to the shell once the input method starts the next one, as a composition commits;
// `composing` hears that a syllable composes meanwhile. Anything else (a space, a key the surface
// encodes, a composition, leaving the input) sends the syllable and gives the input back to the
// surface, which empties it.

const HANGUL = /^[ᄀ-ᇿㄱ-ㆎ가-힣]$/u;
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "CapsLock", "Fn", "AltGraph"]);

type Input = EventTarget & { value: string };

/** Listens on `root` in the capture phase, ahead of the surface's own listeners on `input`. `send`
 * takes text for the shell; `composing` reports whether a syllable is being edited in place. */
export function holdInlineHangul(root: EventTarget, input: Input, send: (data: string) => void,
  composing: (value: boolean) => void): () => void {
  // Characters at the start of the input that have gone to the shell; -1 while nothing is held.
  let sent = -1;
  // The input's text before the syllable a composition replaced (it went out when the composition started).
  let before: string | null = null;
  const sendHeld = (end: number) => {
    const text = input.value.slice(sent, end);
    if (text) send(text);
  };
  const release = (end: number, report: boolean) => {
    if (sent < 0) return;
    sendHeld(end);
    sent = -1;
    if (report) composing(false);
  };
  const own = (event: Event) => event.target === input;

  const onInput = (event: Event) => {
    if (!own(event)) return;
    const { inputType, data, isComposing } = event as InputEvent;
    if (isComposing) return;
    const length = data?.length ?? 0;
    if (inputType === "insertText" && data && HANGUL.test(data)) {
      // A new jamo: the syllable before it is done.
      const start = input.value.length - length;
      if (sent < 0) composing(true);
      else sendHeld(start);
      sent = start;
    } else if (sent < 0) {
      return;
    } else if (inputType !== "insertReplacementText" && inputType !== "deleteContentBackward") {
      release(input.value.length - length, true);
      input.value = data ?? "";
      return;
    }
    event.stopImmediatePropagation();
  };

  const onKeyDown = (event: Event) => {
    const key = event as KeyboardEvent;
    if (!own(event) || sent < 0 || key.keyCode === 229 || MODIFIER_KEYS.has(key.key)) return;
    // Backspace on a lone jamo: the input method lets the key delete it from the input, as a jamo
    // leaves a composition, so the shell never sees it.
    if (key.key === "Backspace" && !key.metaKey && !key.ctrlKey && !key.altKey && input.value.length > sent) {
      event.stopImmediatePropagation();
      return;
    }
    release(input.value.length, true);
    input.value = "";
  };

  const onCompositionStart = (event: Event) => {
    if (!own(event) || sent < 0) return;
    // The input method marks the syllable it edits (⌥Return's Hanja list): it goes out first, as a
    // committed syllable does, and the composition replaces it at the end of the input.
    const replaced = (event as CompositionEvent).data?.length ?? 0;
    before = input.value.slice(0, input.value.length - replaced);
    release(input.value.length, false);
  };

  const onCompositionEnd = (event: Event) => {
    if (!own(event) || before === null) return;
    if (input.value.startsWith(before)) input.value = input.value.slice(before.length);
    before = null;
  };

  const onBlur = () => {
    if (sent < 0) return;
    release(input.value.length, true);
    input.value = "";
  };

  root.addEventListener("input", onInput, true);
  root.addEventListener("keydown", onKeyDown, true);
  root.addEventListener("compositionstart", onCompositionStart, true);
  root.addEventListener("compositionend", onCompositionEnd, true);
  input.addEventListener("blur", onBlur);
  return () => {
    root.removeEventListener("input", onInput, true);
    root.removeEventListener("keydown", onKeyDown, true);
    root.removeEventListener("compositionstart", onCompositionStart, true);
    root.removeEventListener("compositionend", onCompositionEnd, true);
    input.removeEventListener("blur", onBlur);
  };
}
