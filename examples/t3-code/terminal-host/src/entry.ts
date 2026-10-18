// The terminal page a `t3-terminal` view loads (T3TerminalView.swift). It runs T3 Code's own
// Ghostty surface (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts,
// copied unchanged apart from imports under ../vendor/ghostty) and joins it to the native view
// with a JSON bridge:
//
// - page → native (`webkit.messageHandlers.t3terminal`): ready, data, resize, selection, link, composition,
//   commitComposition, contextmenu, chord, focus, error.
// - native → page (`t3Terminal.receive(message)`): write, resetAndWrite, theme, font, visible,
//   focus, chords, compositionKey, clearSelection, readSelection, selectAll, scrollToBottom, paste,
//   pointerRelease, dispose; `t3Terminal.debug()` answers the agent's status (ready, grid, visible
//   text, selection, last error).
//
// Mounting, the setup failure text and the theme fallbacks follow ThreadTerminalDrawer.tsx
// (:176-215 terminalThemeFromApp, :895-915 the failure text; the ANSI palette is 9c161462c9's,
// :159-203 and :307). Which chords the app keeps is the
// app's decision (ThreadTerminalDrawer.tsx:745-782 beforeKey); the native view sends the list.
import type { GhosttyColor, GhosttyTheme } from "../vendor/ghostty/core";
import {
  GhosttyTerminalSurface,
  type GhosttyTerminalFont,
} from "../vendor/ghostty/surface";
import { observeSelectionActions, resolveSelectionActionPosition } from "../vendor/lib/selectionActions";
import { holdInlineHangul } from "./inlineHangul";

type Message = { readonly type: string; readonly [key: string]: unknown };
type Outbound = Record<string, unknown> & { readonly type: string };

interface Bridge {
  postMessage(message: Outbound): void;
}

/** Chords (`Meta+K`, `Control+Shift+Backquote`) the page must leave to the app. */
interface Chord {
  readonly key: string;
  readonly meta: boolean;
  readonly ctrl: boolean;
  readonly alt: boolean;
  readonly shift: boolean;
}

type CommandBinding = { readonly chord: string; readonly command: string; readonly keys: readonly Chord[] };

export function parseCommandBindings(value: unknown): CommandBinding[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((entry: unknown) => {
    if (typeof entry !== "object" || entry === null || !("chord" in entry) || !("command" in entry) ||
        typeof entry.chord !== "string" || typeof entry.command !== "string" || entry.command.length === 0) return [];
    const keys = parseChords([entry.chord]);
    return keys.length ? [{ chord: entry.chord, command: entry.command, keys }] : [];
  });
}

type CloseShortcutEvent = Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey" | "repeat" | "preventDefault">;

/** terminalCloseShortcut.ts preventTerminalCloseShortcut: `bindings` are the app's winners for
 * `terminalFocus: true, terminalOpen: true` (the native view sends them), so a match is a deliberate close. */
export function preventTerminalCloseShortcut(event: CloseShortcutEvent, bindings: readonly CommandBinding[]): boolean {
  if (bindings.find(({ keys }) => matchesChord(event, keys))?.command !== "terminal.close") return false;
  event.preventDefault();
  return true;
}

/** terminalCloseShortcut.ts preventRepeatedTerminalCloseShortcut: a held close never repeats into another close. */
export function preventRepeatedTerminalCloseShortcut(event: CloseShortcutEvent, bindings: readonly CommandBinding[]): boolean {
  if (!event.repeat) return false;
  return preventTerminalCloseShortcut(event, bindings);
}

/** ThreadTerminalDrawer.handleBeforeKey: readline navigation/delete/clear owned by the terminal.
 * Physical names also preserve these shortcuts under a non-Latin keyboard layout on macOS. */
export function terminalInputShortcutData(
  event: Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">,
): string | null {
  if (event.shiftKey) return null;
  const names = eventKeyNames(event);
  const onlyMeta = event.metaKey && !event.ctrlKey && !event.altKey;
  const onlyAlt = event.altKey && !event.metaKey && !event.ctrlKey;
  if (names.includes("arrowleft")) return onlyMeta ? "\u0001" : onlyAlt ? "\u001bb" : null;
  if (names.includes("arrowright")) return onlyMeta ? "\u0005" : onlyAlt ? "\u001bf" : null;
  if (onlyMeta && names.includes("backspace")) return "\u0015";
  if ((onlyMeta && names.includes("k")) || (event.ctrlKey && !event.metaKey && !event.altKey && names.includes("l"))) return "\u000c";
  return null;
}

/** terminalInputShortcutData for a keydown, `composed` when it began while an input method composed.
 * That key matches by `KeyboardEvent.key` alone, as the reference's drawer reads it on the "Process" key
 * (keybindings.ts isTerminalClearShortcut): under Korean 2-Set ⌃L is "ㅣ", which ends the composition
 * and does not clear the screen. */
export function terminalKeyShortcutData(
  event: Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">,
  composed: boolean,
): string | null {
  return terminalInputShortcutData(composed ? { key: event.key, code: "", metaKey: event.metaKey, ctrlKey: event.ctrlKey,
    altKey: event.altKey, shiftKey: event.shiftKey } : event);
}

/** The composing state of the terminal's input, for the native view (`composition` messages): its key
 * path runs before the input method and must leave a composing key to it.
 * WebKit's own marked-text answer (`hasMarkedText`) waits on an editor state the page sends late, or,
 * in a window that is not key, never. Updates in between change nothing: the input method's text stays
 * in the input until it commits, and the surface sends what it commits (surface.ts onCompositionEnd). */
export function observeComposition(input: EventTarget, send: (composing: boolean) => void): () => void {
  let composing = false;
  const set = (value: boolean) => () => {
    if (value === composing) return;
    composing = value;
    send(value);
  };
  const start = set(true), end = set(false);
  input.addEventListener("compositionstart", start);
  input.addEventListener("compositionend", end);
  input.addEventListener("blur", end);
  return () => {
    input.removeEventListener("compositionstart", start);
    input.removeEventListener("compositionend", end);
    input.removeEventListener("blur", end);
  };
}

type KeyModifiers = Pick<KeyboardEvent, "metaKey" | "ctrlKey" | "altKey" | "shiftKey">;
type MarkedKey = KeyModifiers & { readonly code: string };

/** How long a mark waits for its keydown; WebKit sends it as soon as the input method answers. */
const COMPOSITION_KEY_LIFETIME_MS = 1000;

/** Keys that began while an input method composed text, which the native view marks on their way to
 * WebKit's keyDown (a ⌃ chord, ⌥Return). Chromium sends a keydown that begins while text is marked
 * as "Process" (keyCode 229), so the reference's surface never encodes it (surface.ts
 * isTerminalCompositionKey) while its shortcuts (ThreadTerminalDrawer.handleBeforeKey) still run.
 * WebKit sends the key's own code when the input method declines it, after any text the input method
 * committed for it, so the composition can be over by then: unmarked, ⌃C while ㅎ composes would
 * reach the shell as ^C.
 * A mark is taken by the next keydown with its modifiers and physical key (`code`; WebKit's "Process"
 * keydown, keyCode 229, matches any key); marks before it, and a mark whose keydown never came (a menu
 * took the key), lapse, so a second ⌃C is never mistaken for the first. */
export class CompositionKeys {
  private marks: { readonly key: MarkedKey; readonly at: number }[] = [];

  constructor(private readonly now: () => number = () => performance.now()) {}

  /** `code` is the key's `KeyboardEvent.code`, or "" when the native view has no name for it. */
  mark(key: MarkedKey): void {
    this.marks.push({ key, at: this.now() });
  }

  take(event: MarkedKey & Pick<KeyboardEvent, "keyCode">): boolean {
    const now = this.now();
    this.marks = this.marks.filter(({ at }) => now - at < COMPOSITION_KEY_LIFETIME_MS);
    const index = this.marks.findIndex(({ key }) => key.metaKey === event.metaKey && key.ctrlKey === event.ctrlKey &&
      key.altKey === event.altKey && key.shiftKey === event.shiftKey && (key.code === "" || key.code === event.code || event.keyCode === 229));
    if (index < 0) return false;
    this.marks.splice(0, index + 1);
    return true;
  }

  /** Whether a mark still waiting for its keydown is ⌥Return's, the Korean input method's Hanja key. */
  awaitsOptionReturn(): boolean {
    const now = this.now();
    return this.marks.some(({ key, at }) => now - at < COMPOSITION_KEY_LIFETIME_MS && key.altKey && !key.metaKey &&
      !key.ctrlKey && (key.code === "Enter" || key.code === "NumpadEnter"));
  }
}

type ConversionInput = EventTarget & { value: string };

/** Leaves the text an input method commits for ⌥Return in the input until the input method has read it.
 * For ⌥Return the Korean input method commits the composing syllable, reads the text before the caret,
 * and replaces it with a marked Hanja candidate (韓 for 한) while its candidate list is open. The surface
 * empties its input on every commit (surface.ts onCompositionEnd); Chromium answers the read from the text
 * it last knew, so under the reference 한 goes out at the commit and the list opens on 韓. WebKit answers
 * from the page, where the input is empty by then: the input method finds nothing, declines the key, and
 * no list opens. So the committed text goes back into the input after the surface has sent it; the
 * candidate's composition replaces it, and any other key, or leaving the input, empties it again, so
 * it is never sent twice. */
export function keepConversionContext(input: ConversionInput, converting: () => boolean): () => void {
  let kept = false;
  const end = (event: Event) => {
    const data = (event as CompositionEvent).data;
    if (!data || !converting()) return;
    input.value = data;
    kept = true;
  };
  const start = () => { kept = false; };
  const release = () => {
    if (!kept) return;
    kept = false;
    input.value = "";
  };
  input.addEventListener("compositionend", end);
  input.addEventListener("compositionstart", start);
  input.addEventListener("keydown", release);
  input.addEventListener("blur", release);
  return () => {
    input.removeEventListener("compositionend", end);
    input.removeEventListener("compositionstart", start);
    input.removeEventListener("keydown", release);
    input.removeEventListener("blur", release);
  };
}

function hexToGhosttyColor(hex: string): GhosttyColor {
  const value = Number.parseInt(hex.slice(1), 16);
  return { r: (value >> 16) & 255, g: (value >> 8) & 255, b: value & 255 };
}

// VS Code's default ANSI colors.
export const TERMINAL_PALETTES = {
  light: [
    "#000000",
    "#cd3131",
    "#107c10",
    "#949800",
    "#0451a5",
    "#bc05bc",
    "#0598bc",
    "#555555",
    "#666666",
    "#cd3131",
    "#14ce14",
    "#b5ba00",
    "#0451a5",
    "#bc05bc",
    "#0598bc",
    "#a5a5a5",
  ].map(hexToGhosttyColor),
  dark: [
    "#000000",
    "#cd3131",
    "#0dbc79",
    "#e5e510",
    "#2472c8",
    "#bc3fbc",
    "#11a8cd",
    "#e5e5e5",
    "#666666",
    "#f14c4c",
    "#23d18b",
    "#f5f543",
    "#3b8eea",
    "#d670d6",
    "#29b8db",
    "#e5e5e5",
  ].map(hexToGhosttyColor),
};

const LIGHT: GhosttyTheme = {
  background: { r: 255, g: 255, b: 255 },
  foreground: { r: 28, g: 33, b: 41 },
  cursor: { r: 38, g: 56, b: 78 },
};
const DARK: GhosttyTheme = {
  background: { r: 14, g: 18, b: 24 },
  foreground: { r: 237, g: 241, b: 247 },
  cursor: { r: 180, g: 203, b: 255 },
};

const bridge: Bridge | undefined = (
  globalThis as { webkit?: { messageHandlers?: { t3terminal?: Bridge } } }
).webkit?.messageHandlers?.t3terminal;
const sent: Outbound[] = [];

function post(message: Outbound): void {
  if (bridge) bridge.postMessage(message);
  else sent.push(message);
}

function color(value: unknown, fallback: GhosttyColor): GhosttyColor {
  if (typeof value !== "object" || value === null) return fallback;
  const { r, g, b } = value as Record<string, unknown>;
  return typeof r === "number" && typeof g === "number" && typeof b === "number"
    ? { r, g, b }
    : fallback;
}

export function themeFrom(value: unknown): { theme: GhosttyTheme; dark: boolean } {
  const record = (typeof value === "object" && value !== null ? value : {}) as Record<string, unknown>;
  const dark = record.dark === true;
  const base = dark ? DARK : LIGHT;
  const selection = record.selectionBackground;
  return {
    dark,
    theme: {
      background: color(record.background, base.background),
      foreground: color(record.foreground, base.foreground),
      cursor: color(record.cursor, base.cursor),
      palette: TERMINAL_PALETTES[dark ? "dark" : "light"],
      ...(typeof selection === "string" ? { selectionBackground: selection } : {}),
    },
  };
}

function fontFrom(value: unknown): GhosttyTerminalFont {
  const record = (typeof value === "object" && value !== null ? value : {}) as Record<string, unknown>;
  return {
    ...(typeof record.family === "string" && record.family.trim() !== "" ? { family: record.family } : {}),
    ...(typeof record.size === "number" ? { size: record.size } : {}),
  };
}

export function parseChords(list: unknown): Chord[] {
  if (!Array.isArray(list)) return [];
  return list.flatMap((entry) => {
    if (typeof entry !== "string" || entry.length === 0) return [];
    const parts = entry.split("+");
    let key = parts.pop() ?? "";
    if (key === "" && parts[parts.length - 1] === "") { parts.pop(); key = "Plus"; }
    if (key === "") return [];
    const has = (name: string) => parts.includes(name);
    return [{ key: key.toLowerCase(), meta: has("Meta"), ctrl: has("Control"), alt: has("Alt"), shift: has("Shift") }];
  });
}

/** keybindings.resolveEventKeys: preserve Latin remaps; under Korean 2-Set ⌘K's key is "ㅏ". */
export function eventKeyNames(event: Pick<KeyboardEvent, "key" | "code">): string[] {
  const names = [event.key.toLowerCase()];
  const code = event.code;
  if (/^Key[A-Z]$/.test(code)) { if (!/^[a-z]$/.test(event.key.toLowerCase())) names.unshift(code.slice(3).toLowerCase()); }
  else if (/^Digit\d$/.test(code)) names.unshift(code.slice(5));
  else if (code !== "") names.push(code.toLowerCase());
  if (event.key === "+") names.push("plus");
  if (event.key === " ") names.push("space");
  return names;
}

export function matchesChord(
  event: Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">,
  chords: readonly Chord[],
): boolean {
  const names = eventKeyNames(event);
  return chords.some(
    (chord) =>
      chord.meta === event.metaKey &&
      chord.ctrl === event.ctrlKey &&
      chord.alt === event.altKey &&
      chord.shift === event.shiftKey &&
      names.includes(chord.key),
  );
}

const STYLE = `
:root { --app-scrollbar-width: 6px; --app-scrollbar-thumb: rgb(217 217 217); --app-scrollbar-thumb-hover: rgb(191 191 191); }
:root.dark { --app-scrollbar-thumb: rgb(255 255 255 / 8%); --app-scrollbar-thumb-hover: rgb(255 255 255 / 12%); }
html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; }
#terminal { position: relative; width: 100%; height: 100%; overflow: hidden; }
#terminal > canvas { display: block; width: 100%; height: 100%; cursor: text; }
#terminal > [role="scrollbar"]:not([hidden]) { position: absolute; top: 4px; right: 1px; bottom: 4px; z-index: 1; width: var(--app-scrollbar-width); cursor: default; touch-action: none; }
#terminal > [role="scrollbar"] > div { position: absolute; left: 1px; right: 1px; top: 0; border-radius: 3px; background-color: var(--app-scrollbar-thumb); transition: background-color 120ms ease-out; }
#terminal > [role="scrollbar"]:hover > div, #terminal > [role="scrollbar"]:focus-visible > div { background-color: var(--app-scrollbar-thumb-hover); }
#terminal.failed { box-sizing: border-box; padding: 8px; font: 12px -apple-system, BlinkMacSystemFont, sans-serif; color: rgb(113 113 122); }
`;

class TerminalPage {
  private surface: GhosttyTerminalSurface | null = null;
  private readonly pending: Message[] = [];
  private chords: Chord[] = [];
  private bindings: CommandBinding[] = [];
  private readonly compositionKeys = new CompositionKeys();
  private auth = false;
  private readOnly = false;
  private closePending = false;
  private selectionActions: ReturnType<typeof observeSelectionActions> | null = null;
  private stopInlineHangul: (() => void) | null = null;
  private visible = true;
  private lastError = "";
  private disposed = false;
  /** UTF-16 units of output received from the native view (write and resetAndWrite). */
  private received = 0;

  constructor(private readonly mount: HTMLElement) {}

  async start(init: Record<string, unknown>): Promise<void> {
    this.chords = parseChords(init.chords);
    this.bindings = parseCommandBindings(init.keybindings);
    this.auth = init.auth === true;
    this.readOnly = init.readOnly === true;
    this.closePending = init.closePending === true;
    this.visible = init.visible !== false;
    const { theme, dark } = themeFrom(init.theme);
    document.documentElement.classList.toggle("dark", dark);
    document.body.style.background = `rgb(${theme.background.r}, ${theme.background.g}, ${theme.background.b})`;
    try {
      const surface = await GhosttyTerminalSurface.create(this.mount, {
        theme,
        font: fontFrom(init.font),
        visible: this.visible,
        onData: (data) => { if (!this.readOnly) post({ type: "data", data }); },
        onResize: (cols, rows) => post({ type: "resize", cols, rows }),
        onSelectionChange: () => this.postSelection(),
        beforeKey: (event) => this.beforeKey(event),
        onLinkActivate: (text, event) =>
          post({ type: "link", text, metaKey: event.metaKey, ctrlKey: event.ctrlKey, shiftKey: event.shiftKey, altKey: event.altKey }),
        onContextMenu: (event) => {
          event.preventDefault();
          if (this.auth) return;
          this.selectionActions?.cancel();
          post({ type: "contextmenu", x: event.clientX, y: event.clientY, ...this.selection() });
        },
      });
      if (this.disposed) {
        surface.dispose();
        return;
      }
      this.surface = surface;
      if (!this.auth) this.selectionActions = observeSelectionActions({
        element: this.mount,
        onSelection: (pointer) => {
          const selection = this.selection();
          if (!selection.text || !selection.position) return;
          const point = resolveSelectionActionPosition({ bounds: this.mount.getBoundingClientRect(),
            selectionRect: selection.end, pointer, viewport: { width: innerWidth, height: innerHeight } });
          post({ type: "selection-ready", x: point.x, y: point.y, ...selection });
        },
        onDismiss: (reason) => post({ type: "selection-dismiss", reason }),
      });
      surface.input.addEventListener("focus", () => post({ type: "focus", focused: true }));
      surface.input.addEventListener("blur", () => post({ type: "focus", focused: false }));
      // After the surface's own listeners, so a commit's data goes out before the state ends.
      observeComposition(surface.input, (composing) => post({ type: "composition", composing }));
      this.stopInlineHangul = holdInlineHangul(document, surface.input, (data) => { if (!this.readOnly) post({ type: "data", data }); },
        (composing) => post({ type: "composition", composing }));
      keepConversionContext(surface.input, () => this.compositionKeys.awaitsOptionReturn());
      for (const message of this.pending.splice(0)) this.receive(message);
      post({ type: "ready", cols: surface.cols, rows: surface.rows });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unable to initialize libghostty-vt";
      this.fail(message);
    }
  }

  private fail(message: string): void {
    this.lastError = message;
    this.mount.classList.add("failed");
    this.mount.textContent = `${message} — close and reopen the terminal to retry.`;
    post({ type: "error", message });
  }

  private beforeKey(event: KeyboardEvent): boolean {
    // The input method's key: its shortcuts below still run, as on the reference's "Process" key, and
    // the surface never encodes it. One the input method declined does nothing else: no keyboard
    // layout command (⌥Return's line break) and no resend to the app. Chromium's input method ends a
    // composition on a key it declines; when one keeps composing, the native view commits it as it
    // stands.
    const composed = this.compositionKeys.take(event);
    if (composed && event.keyCode !== 229) {
      event.preventDefault();
      post({ type: "commitComposition" });
    }
    if (this.auth) return !composed && event.key !== "Tab";
    // A held ⌘W, or one while the close dialog is pending, is swallowed without another close.
    if (preventRepeatedTerminalCloseShortcut(event, this.bindings) || (this.closePending && preventTerminalCloseShortcut(event, this.bindings))) {
      event.stopPropagation();
      return false;
    }
    const binding = this.bindings.find(({ keys }) => matchesChord(event, keys));
    if (binding) {
      event.preventDefault();
      event.stopPropagation();
      post({ type: "command", command: binding.command, chord: binding.chord, repeat: event.repeat });
      return false;
    }
    // The old `chords` prop is the surface harness's passthrough seam, not a command dispatch.
    if (matchesChord(event, this.chords)) {
      post({ type: "chord", key: event.key, code: event.code, metaKey: event.metaKey, ctrlKey: event.ctrlKey, altKey: event.altKey, shiftKey: event.shiftKey });
      return false;
    }
    const data = terminalKeyShortcutData(event, composed);
    if (data !== null) {
      event.preventDefault();
      event.stopPropagation();
      if (!this.readOnly) post({ type: "data", data });
      return false;
    }
    return !composed;
  }

  private selection() {
    return { text: this.surface?.getSelection() ?? "", position: this.surface?.getSelectionPosition() ?? null,
      end: this.surface?.getSelectionEndClientRect() ?? null };
  }

  private postSelection(): void {
    const surface = this.surface;
    if (!surface || this.auth) return;
    post({ type: "selection", ...this.selection() });
    if (!surface.hasSelection() && this.selectionActions?.pending) {
      this.selectionActions.cancel();
      post({ type: "selection-dismiss", reason: "interaction" });
    }
  }

  /** Native → page. Returns a value for the requests that read (readSelection, debug). */
  receive(message: Message): unknown {
    if (this.disposed) return null;
    const surface = this.surface;
    if (message.type === "chords") {
      this.chords = parseChords(message.chords);
      return null;
    }
    if (message.type === "compositionKey") {
      this.compositionKeys.mark({ metaKey: message.metaKey === true, ctrlKey: message.ctrlKey === true,
        altKey: message.altKey === true, shiftKey: message.shiftKey === true, code: typeof message.code === "string" ? message.code : "" });
      return null;
    }
    if (message.type === "keyPolicy") {
      this.bindings = parseCommandBindings(message.keybindings);
      this.readOnly = message.readOnly === true;
      this.closePending = message.closePending === true;
      return null;
    }
    if (message.type === "dispose") {
      this.disposed = true;
      this.selectionActions?.dispose();
      this.stopInlineHangul?.();
      surface?.dispose();
      this.surface = null;
      return null;
    }
    if (message.type === "write" || message.type === "resetAndWrite") this.received += String(message.data ?? "").length;
    if (!surface) {
      if (message.type !== "readSelection") this.pending.push(message);
      return message.type === "readSelection" ? "" : null;
    }
    switch (message.type) {
      case "write":
        surface.write(String(message.data ?? ""));
        return null;
      case "resetAndWrite":
        surface.resetAndWrite(String(message.data ?? ""));
        return null;
      case "theme": {
        const { theme, dark } = themeFrom(message.theme);
        document.documentElement.classList.toggle("dark", dark);
        document.body.style.background = `rgb(${theme.background.r}, ${theme.background.g}, ${theme.background.b})`;
        surface.setTheme(theme);
        return null;
      }
      case "font":
        void surface.setFont(fontFrom(message.font));
        return null;
      case "visible":
        this.visible = message.visible !== false;
        surface.setVisible(this.visible);
        return null;
      case "focus":
        surface.focus();
        return null;
      case "clearSelection":
        surface.clearSelection();
        return null;
      case "readSelection":
        return surface.getSelection();
      case "selectAll":
        surface.selectAll();
        return null;
      case "scrollToBottom":
        surface.scrollToBottom();
        return null;
      // A press that ended outside the page: the reference's dividers are in the same document, so
      // their release reaches the surface's window listener (surface.ts onWindowPointerRelease).
      case "pointerRelease":
        window.dispatchEvent(new Event("pointerup"));
        return null;
      case "paste": {
        const text = String(message.text ?? "");
        void surface.pasteFromClipboard(() => Promise.resolve(text));
        return null;
      }
      default:
        return null;
    }
  }

  /** The agent's view of this terminal: what the canvas last painted, as text. */
  debug(): Record<string, unknown> {
    const surface = this.surface;
    const painted = (surface as unknown as { snapshot?: { rowData: ReadonlyArray<{ text: string }> } | null } | null)
      ?.snapshot;
    return {
      ready: surface !== null,
      cols: surface?.cols ?? 0,
      rows: surface?.rows ?? 0,
      visible: this.visible,
      focused: document.activeElement === surface?.input,
      text: painted ? painted.rowData.map((row) => row.text.replace(/\s+$/, "")) : [],
      selection: surface?.getSelection() ?? "",
      atBottom: surface?.isAtBottom() ?? true,
      error: this.lastError,
      received: this.received,
      unsent: sent.length,
      visibility: document.visibilityState,
    };
  }
}

function boot(): void {
  const style = document.createElement("style");
  style.textContent = STYLE;
  document.head.append(style);
  const mount = document.getElementById("terminal") ?? document.body.appendChild(document.createElement("div"));
  mount.id = "terminal";
  const page = new TerminalPage(mount);
  const api = {
    receive: (message: Message) => page.receive(message),
    debug: () => page.debug(),
  };
  (globalThis as { t3Terminal?: typeof api }).t3Terminal = api;
  window.addEventListener("error", (event) => post({ type: "error", message: event.message }));
  window.addEventListener("unhandledrejection", (event) =>
    post({ type: "error", message: event.reason instanceof Error ? event.reason.message : String(event.reason) }),
  );
  const init = (globalThis as { t3TerminalInit?: Record<string, unknown> }).t3TerminalInit ?? {};
  void page.start(init);
}

if (typeof document !== "undefined" && typeof window !== "undefined" && "webkit" in window) boot();
