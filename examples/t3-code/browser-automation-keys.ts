// browser-surface part 5: `preview_press`'s key (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/preview/PreviewKeyboard.ts, ported whole with its tests). The reference builds CDP and
// Electron key packets; the clone's module sends the same key as an `NSEvent` to a page in a window
// (T3BrowserAutomationInput.swift reads `chord` and `text`), runs the macOS editing shortcuts in the page
// (`editing`, the reference's `previewAutomationEditingCommandExpression`), and for a page in no window
// dispatches DOM key events and their default edit (`dom`; untrusted, X1 path B).

export type PressInput = { key: string; modifiers?: ReadonlyArray<'Alt' | 'Control' | 'Meta' | 'Shift'> };

interface KeyDefinition {
  readonly code: string;
  readonly key: string;
  readonly keyCode: number;
  readonly text?: string;
  readonly location?: number;
  readonly shiftedKey?: string;
}

export interface PreviewAutomationKeyEvent {
  readonly [key: string]: unknown;
  readonly type: 'keyDown' | 'rawKeyDown' | 'keyUp';
  readonly key: string;
  readonly code: string;
  readonly modifiers: number;
  readonly windowsVirtualKeyCode: number;
  readonly location: number;
  readonly isKeypad: boolean;
  readonly text?: string;
  readonly unmodifiedText?: string;
  readonly commands?: ReadonlyArray<string>;
}

export interface PreviewAutomationKeySequence {
  readonly keyDown: PreviewAutomationKeyEvent;
  readonly keyUp: PreviewAutomationKeyEvent;
  readonly signal: { readonly kind: 'key'; readonly key: string; readonly code: string };
}

const NAMED_KEYS: Readonly<Record<string, KeyDefinition>> = {
  Escape: { code: 'Escape', key: 'Escape', keyCode: 27 },
  Backspace: { code: 'Backspace', key: 'Backspace', keyCode: 8 },
  Tab: { code: 'Tab', key: 'Tab', keyCode: 9 },
  Enter: { code: 'Enter', key: 'Enter', keyCode: 13, text: '\r' },
  Shift: { code: 'ShiftLeft', key: 'Shift', keyCode: 16, location: 1 },
  Control: { code: 'ControlLeft', key: 'Control', keyCode: 17, location: 1 },
  Alt: { code: 'AltLeft', key: 'Alt', keyCode: 18, location: 1 },
  Meta: { code: 'MetaLeft', key: 'Meta', keyCode: 91, location: 1 },
  CapsLock: { code: 'CapsLock', key: 'CapsLock', keyCode: 20 },
  Space: { code: 'Space', key: ' ', keyCode: 32, text: ' ' },
  PageUp: { code: 'PageUp', key: 'PageUp', keyCode: 33 },
  PageDown: { code: 'PageDown', key: 'PageDown', keyCode: 34 },
  End: { code: 'End', key: 'End', keyCode: 35 },
  Home: { code: 'Home', key: 'Home', keyCode: 36 },
  ArrowLeft: { code: 'ArrowLeft', key: 'ArrowLeft', keyCode: 37 },
  ArrowUp: { code: 'ArrowUp', key: 'ArrowUp', keyCode: 38 },
  ArrowRight: { code: 'ArrowRight', key: 'ArrowRight', keyCode: 39 },
  ArrowDown: { code: 'ArrowDown', key: 'ArrowDown', keyCode: 40 },
  Insert: { code: 'Insert', key: 'Insert', keyCode: 45 },
  Delete: { code: 'Delete', key: 'Delete', keyCode: 46 },
};

const PRINTABLE_KEYS: ReadonlyArray<KeyDefinition> = [
  { code: 'Backquote', key: '`', shiftedKey: '~', keyCode: 192 },
  { code: 'Digit1', key: '1', shiftedKey: '!', keyCode: 49 },
  { code: 'Digit2', key: '2', shiftedKey: '@', keyCode: 50 },
  { code: 'Digit3', key: '3', shiftedKey: '#', keyCode: 51 },
  { code: 'Digit4', key: '4', shiftedKey: '$', keyCode: 52 },
  { code: 'Digit5', key: '5', shiftedKey: '%', keyCode: 53 },
  { code: 'Digit6', key: '6', shiftedKey: '^', keyCode: 54 },
  { code: 'Digit7', key: '7', shiftedKey: '&', keyCode: 55 },
  { code: 'Digit8', key: '8', shiftedKey: '*', keyCode: 56 },
  { code: 'Digit9', key: '9', shiftedKey: '(', keyCode: 57 },
  { code: 'Digit0', key: '0', shiftedKey: ')', keyCode: 48 },
  { code: 'Minus', key: '-', shiftedKey: '_', keyCode: 189 },
  { code: 'Equal', key: '=', shiftedKey: '+', keyCode: 187 },
  { code: 'Backslash', key: '\\', shiftedKey: '|', keyCode: 220 },
  { code: 'BracketLeft', key: '[', shiftedKey: '{', keyCode: 219 },
  { code: 'BracketRight', key: ']', shiftedKey: '}', keyCode: 221 },
  { code: 'Semicolon', key: ';', shiftedKey: ':', keyCode: 186 },
  { code: 'Quote', key: "'", shiftedKey: '"', keyCode: 222 },
  { code: 'Comma', key: ',', shiftedKey: '<', keyCode: 188 },
  { code: 'Period', key: '.', shiftedKey: '>', keyCode: 190 },
  { code: 'Slash', key: '/', shiftedKey: '?', keyCode: 191 },
];

/** Chromium does not infer macOS editing commands from synthetic Meta chords (nor does a key sent to WebKit
 *  outside the window's key path); the common editing and navigation shortcuts stay explicit. */
const MAC_EDITING_COMMANDS: Readonly<Record<string, string>> = {
  'Meta+Backspace': 'deleteToBeginningOfLine',
  'Meta+ArrowUp': 'moveToBeginningOfDocument',
  'Meta+ArrowDown': 'moveToEndOfDocument',
  'Meta+ArrowLeft': 'moveToLeftEndOfLine',
  'Meta+ArrowRight': 'moveToRightEndOfLine',
  'Shift+Meta+ArrowUp': 'moveToBeginningOfDocumentAndModifySelection',
  'Shift+Meta+ArrowDown': 'moveToEndOfDocumentAndModifySelection',
  'Shift+Meta+ArrowLeft': 'moveToLeftEndOfLineAndModifySelection',
  'Shift+Meta+ArrowRight': 'moveToRightEndOfLineAndModifySelection',
  'Meta+KeyA': 'selectAll',
  'Meta+KeyC': 'copy',
  'Meta+KeyX': 'cut',
  'Meta+KeyV': 'paste',
  'Meta+KeyZ': 'undo',
  'Shift+Meta+KeyZ': 'redo',
};
const SHORTCUT_MODIFIER_ORDER = ['Shift', 'Control', 'Alt', 'Meta'] as const;

const macEditingCommands = (code: string, modifiers: PressInput['modifiers']): ReadonlyArray<string> => {
  const shortcut = [...SHORTCUT_MODIFIER_ORDER.filter(modifier => modifiers?.includes(modifier)), code].join('+');
  const command = MAC_EDITING_COMMANDS[shortcut];
  return command ? [command] : [];
};

const modifierMask = (modifiers: PressInput['modifiers']): number =>
  (modifiers ?? []).reduce((value, modifier) => value | (modifier === 'Alt' ? 1 : modifier === 'Control' ? 2 : modifier === 'Meta' ? 4 : 8), 0);

function resolveKeyDefinition(input: PressInput): KeyDefinition {
  const named = NAMED_KEYS[input.key === ' ' ? 'Space' : input.key];
  if (named) return named;
  const functionKey = /^F([1-9]|1[0-2])$/.exec(input.key);
  if (functionKey) return { code: input.key, key: input.key, keyCode: 111 + Number(functionKey[1]) };
  if (/^[a-z]$/i.test(input.key)) {
    const upper = input.key.toUpperCase();
    const shifted = input.modifiers?.includes('Shift') ?? false;
    const key = shifted || input.key === upper ? upper : input.key;
    return { code: `Key${upper}`, key, keyCode: upper.charCodeAt(0), text: key };
  }
  const printable = PRINTABLE_KEYS.find(definition => definition.key === input.key || definition.shiftedKey === input.key);
  if (printable) {
    const shifted = input.modifiers?.includes('Shift') ?? false;
    const key = printable.shiftedKey && (shifted || input.key === printable.shiftedKey) ? printable.shiftedKey : printable.key;
    return { ...printable, key, text: key };
  }
  return { code: input.key.length > 1 ? input.key : '', key: input.key, keyCode: 0, ...(input.key.length === 1 ? { text: input.key } : {}) };
}

/** Chromium CDP key packets with the required fields and down-event choice of Playwright's pinned keyboard. */
export function makePreviewAutomationKeySequence(input: PressInput, options?: { readonly isMac?: boolean }): PreviewAutomationKeySequence {
  const definition = resolveKeyDefinition(input);
  const modifiers = modifierMask(input.modifiers);
  const suppressText = input.modifiers?.some(modifier => modifier !== 'Shift') ?? false;
  const text = suppressText ? '' : (definition.text ?? '');
  const location = definition.location ?? 0;
  const commands = options?.isMac ? macEditingCommands(definition.code, input.modifiers) : [];
  const shared = { key: definition.key, code: definition.code, modifiers, windowsVirtualKeyCode: definition.keyCode, location, isKeypad: location === 3 };
  return {
    keyDown: { type: text ? 'keyDown' : 'rawKeyDown', ...shared, ...(text ? { text, unmodifiedText: text } : {}), ...(commands.length > 0 ? { commands } : {}) },
    keyUp: { type: 'keyUp', ...shared },
    signal: { kind: 'key', key: definition.key, code: definition.code },
  };
}

/** Root CDP input can retarget the embedder; native packets address the guest widget. */
export function makePreviewAutomationNativeKeySequence(input: PressInput, options?: { readonly isMac?: boolean }) {
  const { keyDown, signal } = makePreviewAutomationKeySequence(input, options);
  const modifiers = ([[1, 'alt'], [2, 'control'], [4, 'meta'], [8, 'shift']] as const).filter(([mask]) => keyDown.modifiers & mask).map(([, modifier]) => modifier);
  const shared = { keyCode: keyDown.key.startsWith('Arrow') ? keyDown.key.slice(5) : keyDown.key, modifiers, skipIfUnhandled: true as const };
  // Electron lowercases unshifted letters and reports no key for Unicode accelerators.
  const key = keyDown.windowsVirtualKeyCode === 0 && keyDown.key.length === 1 ? '' : /^[A-Z]$/.test(keyDown.key) && !modifiers.includes('shift') ? keyDown.key.toLowerCase() : keyDown.key;
  return {
    keyDown: { type: 'keyDown' as const, ...shared },
    ...(keyDown.text ? { char: { type: 'char' as const, ...shared, keyCode: keyDown.text } } : {}),
    keyUp: { type: 'keyUp' as const, ...shared },
    ...(keyDown.commands ? { commands: keyDown.commands } : {}),
    signal: { ...signal, key },
  };
}

/** T3BrowserAutomation.swift `clipboardPlaceholder`: the module puts the system clipboard's formats here. */
export const CLIPBOARD_PLACEHOLDER = '__T3_PREVIEW_CLIPBOARD__';

/** Keep macOS editing shortcuts inside the target page without native focus. The paste command reads the
 *  clipboard formats the module substitutes for the placeholder; WebKit has no `setHTML`, so pasted HTML is
 *  parsed in an inert template and stripped of scripts and handlers instead. */
export function previewAutomationEditingCommandExpression(input: PressInput, commands: ReadonlyArray<string>): string {
  const definition = resolveKeyDefinition(input);
  const event = {
    key: definition.key, code: definition.code, keyCode: definition.keyCode, which: definition.keyCode, location: definition.location ?? 0,
    altKey: input.modifiers?.includes('Alt') ?? false, ctrlKey: input.modifiers?.includes('Control') ?? false,
    metaKey: input.modifiers?.includes('Meta') ?? false, shiftKey: input.modifiers?.includes('Shift') ?? false,
    bubbles: true, cancelable: true, composed: true,
  };
  return `(() => {
    let element = document.activeElement;
    while (element?.shadowRoot?.activeElement) element = element.shadowRoot.activeElement;
    if (!element) return;
    const event = ${JSON.stringify(event)};
    const sanitize = (html) => {
      const template = document.createElement("template");
      template.innerHTML = html;
      for (const node of template.content.querySelectorAll("script, style, iframe, object, embed")) node.remove();
      for (const node of template.content.querySelectorAll("*")) for (const attribute of [...node.attributes]) {
        if (/^on/i.test(attribute.name) || /^\\s*javascript:/i.test(attribute.value)) node.removeAttribute(attribute.name);
      }
      return template.innerHTML;
    };
    try {
      if (!element.dispatchEvent(new KeyboardEvent("keydown", event))) return;
      for (const command of ${JSON.stringify(commands)}) {
        if (command === "paste") {
          const transfer = new DataTransfer();
          for (const { type, data } of ${CLIPBOARD_PLACEHOLDER}) {
            if (type === "text/html") transfer.setData(type, sanitize(data));
            else if (type.startsWith("text/")) transfer.setData(type, data);
          }
          if (!element.dispatchEvent(new ClipboardEvent("paste", { clipboardData: transfer, bubbles: true, cancelable: true, composed: true }))) continue;
          const text = transfer.getData("text/plain");
          if (!element.dispatchEvent(new InputEvent("beforeinput", { inputType: "insertFromPaste", data: text, dataTransfer: transfer, bubbles: true, cancelable: true, composed: true }))) continue;
          const html = element.isContentEditable ? transfer.getData("text/html") : "";
          document.execCommand(html ? "insertHTML" : "insertText", false, html || text);
          continue;
        }
        const inputType = command === "deleteToBeginningOfLine" ? "deleteSoftLineBackward" : command === "undo" ? "historyUndo" : command === "redo" ? "historyRedo" : null;
        if (inputType && !element.dispatchEvent(new InputEvent("beforeinput", { inputType, bubbles: true, cancelable: true, composed: true }))) continue;
        const selection = document.getSelection();
        if (command === "deleteToBeginningOfLine") {
          const collapsed = typeof element.selectionStart === "number" ? element.selectionStart === element.selectionEnd : selection?.isCollapsed;
          if (collapsed) selection?.modify("extend", "backward", "lineboundary");
          document.execCommand("delete");
        } else if (command.startsWith("moveTo")) {
          const selectionElement = selection?.anchorNode?.nodeType === Node.ELEMENT_NODE ? selection.anchorNode : selection?.anchorNode?.parentElement;
          const editable = element.isContentEditable || selectionElement?.isContentEditable ||
            (((element instanceof HTMLInputElement && element.selectionStart !== null) || element instanceof HTMLTextAreaElement) && !element.readOnly && !element.disabled);
          if (!editable && (command === "moveToBeginningOfDocument" || command === "moveToEndOfDocument")) {
            let scrollable = element === document.body ? selectionElement ?? element : element;
            while (scrollable && !(scrollable.scrollHeight > scrollable.clientHeight && /^(auto|scroll|overlay)$/.test(getComputedStyle(scrollable).overflowY))) {
              scrollable = scrollable.parentElement ?? scrollable.getRootNode().host;
            }
            scrollable ??= document.scrollingElement;
            if (scrollable) scrollable.scrollTop = command === "moveToBeginningOfDocument" ? 0 : scrollable.scrollHeight;
            continue;
          }
          const direction = command.includes("Beginning") ? "backward" : command.includes("Left") ? "left" : command.includes("Right") ? "right" : "forward";
          selection?.modify(command.endsWith("AndModifySelection") ? "extend" : "move", direction, command.includes("Document") ? "documentboundary" : "lineboundary");
          if (element instanceof HTMLInputElement && element.selectionStart !== null) {
            if (command.includes("Left") || command.includes("Beginning")) element.scrollLeft = 0;
            else element.scrollLeft = element.scrollWidth;
          }
          if (editable && command.includes("Document")) {
            const beginning = command.includes("Beginning");
            if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) element.scrollTop = beginning ? 0 : element.scrollHeight;
            else {
              const caretElement = selection?.focusNode?.nodeType === Node.ELEMENT_NODE ? selection.focusNode : selection?.focusNode?.parentElement;
              caretElement?.scrollIntoView({ block: beginning ? "start" : "end", inline: "nearest" });
            }
          }
        } else {
          document.execCommand(command);
        }
      }
    } finally {
      element.dispatchEvent(new KeyboardEvent("keyup", event));
    }
  })()`;
}

/** A page in no window has no native key path: the key as DOM events and its default edit (untrusted). */
export function domKeyExpression(input: PressInput): string {
  const sequence = makePreviewAutomationKeySequence(input, { isMac: true });
  const event = {
    key: sequence.keyDown.key, code: sequence.keyDown.code, keyCode: sequence.keyDown.windowsVirtualKeyCode, which: sequence.keyDown.windowsVirtualKeyCode,
    altKey: input.modifiers?.includes('Alt') ?? false, ctrlKey: input.modifiers?.includes('Control') ?? false,
    metaKey: input.modifiers?.includes('Meta') ?? false, shiftKey: input.modifiers?.includes('Shift') ?? false,
    bubbles: true, cancelable: true, composed: true,
  };
  return `
    let element = document.activeElement;
    while (element?.shadowRoot?.activeElement) element = element.shadowRoot.activeElement;
    const target = element ?? document.body;
    const event = ${JSON.stringify(event)};
    if (target.dispatchEvent(new KeyboardEvent("keydown", event))) {
      const text = ${JSON.stringify(sequence.keyDown.text ?? '')};
      const editable = target.isContentEditable || target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.selectionStart !== null);
      if (event.key === "Enter" && target instanceof HTMLInputElement && target.form) target.form.requestSubmit();
      else if (event.key === "Enter" && editable) document.execCommand("insertLineBreak");
      else if (event.key === "Backspace" && editable) document.execCommand("delete");
      else if (event.key === "Delete" && editable) document.execCommand("forwardDelete");
      else if (text && editable && text !== "\\r") document.execCommand("insertText", false, text);
    }
    target.dispatchEvent(new KeyboardEvent("keyup", event));
    return true;`;
}

const CHORD_KEYS = new Set(['Enter', 'Tab', 'Escape', 'Backspace', 'Space', 'Delete', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown']);
/** What the module needs for one key: T3TerminalKeys' chord ("Shift+Meta+z", "Enter"), the typed text, the editing
 *  commands and their expression, and the DOM fallback. */
export function pressPlan(input: PressInput): { chord: string; text: string; commands: string[]; editing: string; dom: string } {
  const sequence = makePreviewAutomationKeySequence(input, { isMac: true });
  const definition = resolveKeyDefinition(input);
  const named = CHORD_KEYS.has(input.key === ' ' ? 'Space' : input.key) || /^F([1-9]|1[0-2])$/.test(input.key) ? (input.key === ' ' ? 'Space' : input.key) : '';
  const printable = PRINTABLE_KEYS.find(entry => entry.key === definition.key || entry.shiftedKey === definition.key);
  const base = named || (/^Key[A-Z]$/.test(definition.code) ? definition.code.slice(3).toLowerCase() : printable?.key ?? '');
  const modifiers = SHORTCUT_MODIFIER_ORDER.filter(modifier => input.modifiers?.includes(modifier) || (modifier === 'Shift' && !!printable && definition.key === printable.shiftedKey))
    .map(modifier => modifier === 'Control' ? 'Control' : modifier);
  const chord = base ? [...modifiers, base].join('+') : '';
  const commands = [...(sequence.keyDown.commands ?? [])];
  return { chord, text: sequence.keyDown.text ?? '', commands, editing: commands.length ? previewAutomationEditingCommandExpression(input, commands) : '', dom: domKeyExpression(input) };
}
