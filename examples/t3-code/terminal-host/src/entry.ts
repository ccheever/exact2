// The terminal page (LLP 1020.001: a bundled page under `assets/terminal/` in an `iframe`). It runs T3 Code's own
// Ghostty surface (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts, copied unchanged
// apart from imports under ../vendor/ghostty) and opens its own server socket for the session (session.ts), as the
// reference's renderer holds the attach stream. It talks to the app by `postMessage` only, in JSON strings:
//
// - page → app: boot, ticket, focus, link, command, selection-ready, contextmenu, selection-dismiss, exited,
//   error, auth-error, auth-recovered, stats; each carries the identity the config named.
// - app → page: config (theme, font, visible, keybindings, closePending and the session), socket (a URL with a
//   fresh ticket, or none), transcript, system, focus, clearSelection, selectAll, scrollToBottom, paste,
//   pasteClipboard, stats.
//
// Mounting, the setup failure text and the theme fallbacks follow ThreadTerminalDrawer.tsx
// (:176-215 terminalThemeFromApp, :895-915 the failure text; the ANSI palette is 9c161462c9's,
// :159-203 and :307), and its beforeKey (:745-782) decides which keys the page keeps.
import type { GhosttyColor, GhosttyTheme } from "../vendor/ghostty/core";
import {
  GhosttyTerminalSurface,
  type GhosttyTerminalFont,
} from "../vendor/ghostty/surface";
import { observeSelectionActions, resolveSelectionActionPosition } from "../vendor/lib/selectionActions";
import { holdInlineHangul } from "./inlineHangul";
import { SocketRequests, TerminalSocket, crlf, type AttachTarget, type AuthTarget, type SocketFactory } from "./session";
import { terminalTranscriptUpdate } from "../../src/providers/provider-auth-terminal";

type Message = { readonly type: string; readonly [key: string]: unknown };
type Outbound = Record<string, unknown> & { readonly type: string };

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
 * `terminalFocus: true, terminalOpen: true` (the config sends them), so a match is a deliberate close. */
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

/** The composing state of the terminal's input: from its first composition event to the commit, once each.
 * Updates in between change nothing: the input method's text stays in the input until it commits, and the
 * surface sends what it commits (surface.ts onCompositionEnd). */
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

const OPTION_RETURN_LIFETIME_MS = 1000;
const DECLINED_KEY_WINDOW_MS = 50;

/** Whether a keydown is the input method's. WebKit sends a key the input method takes as "Process"
 * (keyCode 229), as Chromium does, and the reference's surface never encodes it (surface.ts
 * isTerminalCompositionKey) while its shortcuts (ThreadTerminalDrawer.handleBeforeKey) still run. A key the
 * input method declines (a ⌃ chord, ⌥Return) WebKit sends with its own code right after the commit it caused,
 * where Chromium still sends "Process": such a key counts as the input method's too, so ⌃C typed over ㅎ
 * never reaches the shell as ^C. */
export class CompositionTracker {
  private active = false;
  private endedAt = -Infinity;
  private optionReturnAt = -Infinity;

  constructor(private readonly now: () => number = () => performance.now()) {}

  set(composing: boolean): void {
    if (this.active && !composing) this.endedAt = this.now();
    this.active = composing;
  }

  composed(event: Pick<KeyboardEvent, "keyCode" | "code" | "metaKey" | "ctrlKey" | "altKey">): boolean {
    const optionReturn = event.altKey && !event.metaKey && !event.ctrlKey && (event.code === "Enter" || event.code === "NumpadEnter");
    if (event.keyCode === 229) {
      if (optionReturn) this.optionReturnAt = this.now();
      return true;
    }
    if (this.active) return true;
    return ((event.ctrlKey && !event.metaKey) || optionReturn) && this.now() - this.endedAt < DECLINED_KEY_WINDOW_MS;
  }

  /** ⌥Return went to the input method a moment ago: its Hanja conversion reads the committed text back. */
  converting(): boolean { return this.now() - this.optionReturnAt < OPTION_RETURN_LIFETIME_MS; }
}

/** What the app tells a page about its session (terminal-frames.ts): the terminal it shows, or a sign-in transcript. */
interface SessionConfig {
  readonly mode: "session" | "transcript";
  readonly identity: Record<string, string>;
  readonly attach?: AttachTarget;
  readonly auth?: AuthTarget;
  readonly readOnly: boolean;
  readonly canAddToChat: boolean;
}

function sessionFrom(value: unknown): SessionConfig {
  const record = (typeof value === "object" && value !== null ? value : {}) as Record<string, unknown>;
  const identity = Object.fromEntries(Object.entries((typeof record.identity === "object" && record.identity) || {})
    .filter((entry): entry is [string, string] => typeof entry[1] === "string"));
  return {
    mode: record.mode === "transcript" ? "transcript" : "session",
    identity,
    ...(typeof record.attach === "object" && record.attach ? { attach: record.attach as AttachTarget } : {}),
    ...(typeof record.auth === "object" && record.auth ? { auth: record.auth as AuthTarget } : {}),
    readOnly: record.readOnly === true,
    canAddToChat: record.canAddToChat === true,
  };
}

/** Bulk output since the last report that makes the page report again (a flood, a long listing; not an echo). */
const REPORT_BYTES = 64 * 1024;
const REPORT_IDLE_MS = 500;

/** The framed page's app, reached by `postMessage` only (LLP 1020.001 D5): its parent on the web; on Apple a
 * bundled page is its web view's own top document, and the host forwards what it posts to its own window. Either
 * way the receiver is the page's own origin (`/`). */
function post(message: Outbound): void {
  if (typeof window !== "undefined") window.parent.postMessage(JSON.stringify(message), "/");
}

/** A message the page takes: from its app (the parent, or on Apple the host through the page's own window), and on
 * the web only from the app's origin, which a bundled page shares. */
export function fromApp(event: Pick<MessageEvent, "source" | "origin" | "data">, view: { parent: unknown; location: { origin: string } }, self: unknown): boolean {
  return typeof event.data === "string" && event.source === view.parent && (view.parent === self || event.origin === view.location.origin);
}

class TerminalPage {
  private surface: GhosttyTerminalSurface | null = null;
  private session: SessionConfig | null = null;
  private socket: TerminalSocket | null = null;
  private readonly pending: Message[] = [];
  private bindings: CommandBinding[] = [];
  private readonly composition = new CompositionTracker();
  private readOnly = false;
  private closePending = false;
  private selectionActions: ReturnType<typeof observeSelectionActions> | null = null;
  private stopInlineHangul: (() => void) | null = null;
  private visible = true;
  private theme = "";
  private font = "";
  private lastError = "";
  private disposed = false;
  private starting = false;
  private transcriptFlow = "";
  private transcriptWritten = 0;
  private readonly sockets = new SocketRequests((serial) => this.send({ type: "ticket", serial }));
  /** UTF-16 units of output written, and the bulk output since the last report. */
  private received = 0;
  private burst = { units: 0, startedAt: 0, lastAt: 0 };
  private reportTimer: ReturnType<typeof setTimeout> | null = null;
  private typedAt = 0;
  private readonly echoes: number[] = [];

  constructor(private readonly mount: HTMLElement, private readonly connect: SocketFactory) {}

  /** `type` leads each message, so the frame can route a ticket request by its prefix (terminal-frame.contract). */
  private send(message: Outbound): void {
    const { type, ...rest } = message;
    post({ type, ...(this.session?.identity ?? {}), ...rest });
  }

  /** The app's `config`: the first starts the surface and the session; later ones change what changed. */
  private configure(message: Message): void {
    this.bindings = parseCommandBindings(message.keybindings);
    this.closePending = message.closePending === true;
    const session = sessionFrom(message.session);
    this.readOnly = session.readOnly;
    if (this.surface) this.surface.input.readOnly = this.readOnly;
    const theme = JSON.stringify(message.theme ?? null), font = JSON.stringify(message.font ?? null);
    const visible = message.visible !== false;
    if (!this.session) {
      this.session = session;
      this.theme = theme; this.font = font; this.visible = visible;
      void this.start(message);
      return;
    }
    this.session = { ...this.session, readOnly: session.readOnly, canAddToChat: session.canAddToChat };
    const surface = this.surface;
    if (!surface) return;
    if (theme !== this.theme) { this.theme = theme; this.applyTheme(message.theme); }
    if (font !== this.font) { this.font = font; void surface.setFont(fontFrom(message.font)); }
    if (visible !== this.visible) { this.visible = visible; surface.setVisible(visible); }
  }

  private applyTheme(value: unknown): GhosttyTheme {
    const { theme, dark } = themeFrom(value);
    document.documentElement.classList.toggle("dark", dark);
    document.body.style.background = `rgb(${theme.background.r}, ${theme.background.g}, ${theme.background.b})`;
    this.surface?.setTheme(theme);
    return theme;
  }

  private async start(config: Message): Promise<void> {
    if (this.starting) return;
    this.starting = true;
    const theme = this.applyTheme(config.theme);
    const transcript = this.session?.mode === "transcript";
    try {
      const surface = await GhosttyTerminalSurface.create(this.mount, {
        theme,
        font: fontFrom(config.font),
        visible: this.visible,
        onData: (data) => this.typed(data),
        onResize: (cols, rows) => this.resized(cols, rows),
        onSelectionChange: () => this.selectionChanged(),
        beforeKey: (event) => this.beforeKey(event),
        onLinkActivate: (text, event) =>
          this.send({ type: "link", text, metaKey: event.metaKey, ctrlKey: event.ctrlKey, shiftKey: event.shiftKey, altKey: event.altKey }),
        onContextMenu: (event) => {
          event.preventDefault();
          if (transcript) return;
          this.selectionActions?.cancel();
          this.send({ type: "contextmenu", x: event.clientX, y: event.clientY, readOnly: this.readOnly, ...this.selection() });
        },
      });
      if (this.disposed) { surface.dispose(); return; }
      this.surface = surface;
      surface.input.readOnly = this.readOnly;
      if (!transcript) this.selectionActions = observeSelectionActions({
        element: this.mount,
        onSelection: (pointer) => {
          const selection = this.selection();
          if (!selection.text || !selection.position) return;
          const point = resolveSelectionActionPosition({ bounds: this.mount.getBoundingClientRect(),
            selectionRect: selection.end, pointer, viewport: { width: innerWidth, height: innerHeight } });
          this.send({ type: "selection-ready", x: point.x, y: point.y, ...selection });
        },
        onDismiss: (reason) => this.send({ type: "selection-dismiss", reason }),
      });
      surface.input.addEventListener("focus", () => this.send({ type: "focus", focused: true }));
      surface.input.addEventListener("blur", () => this.send({ type: "focus", focused: false }));
      // After the surface's own listeners, so a commit's data goes out before the state ends.
      observeComposition(surface.input, (composing) => this.composition.set(composing));
      this.stopInlineHangul = holdInlineHangul(document, surface.input, (data) => this.typed(data), () => undefined);
      keepConversionContext(surface.input, () => this.composition.converting());
      for (const message of this.pending.splice(0)) this.receive(message);
      if (transcript) this.socket = this.openSocket();
      else if (this.session?.attach) this.socket = this.openSocket();
      if (this.socket) this.wantSocket();
      if (transcript) this.socket?.authInput("", { cols: surface.cols, rows: surface.rows });
      // A focus that reached the web view while the WASM loaded (TerminalSessionViewport's mount check).
      if (document.hasFocus() && this.visible) surface.focus();
    } catch (error) {
      this.fail(error instanceof Error ? error.message : "Unable to initialize libghostty-vt");
    }
  }

  private openSocket(): TerminalSocket {
    const session = this.session!;
    return new TerminalSocket({
      write: (data) => this.surface?.write(data),
      resetAndWrite: (data) => { this.surface?.resetAndWrite(data); this.selectionActions?.cancel(); },
      system: (message) => this.system(message),
      exited: () => this.send({ type: "exited" }),
      wantSocket: () => this.wantSocket(),
      authFailed: (failed) => this.send({ type: failed ? "auth-error" : "auth-recovered" }),
      received: (units) => this.output(units),
      answered: () => this.sockets.reset(),
    }, this.connect, { ...(session.attach ? { attach: session.attach } : {}), ...(session.auth ? { auth: session.auth } : {}), readOnly: session.readOnly });
  }

  private wantSocket(): void { if (!this.disposed) this.sockets.want(); }

  private socketReady(url: string, serial: number): void {
    if (this.sockets.answered(url, serial)) this.socket?.start(url);
  }

  private typed(data: string): void {
    if (this.readOnly || !data) return;
    this.typedAt = performance.now();
    if (this.session?.mode === "transcript") this.socket?.authInput(data);
    else this.socket?.write(data);
  }

  private resized(cols: number, rows: number): void {
    if (this.session?.mode === "transcript") this.socket?.authInput("", { cols, rows });
    else this.socket?.resize(cols, rows);
  }

  /** Output reached the surface: the echo latency of the last typed key, and a report after bulk output. */
  private output(units: number): void {
    const now = performance.now();
    this.received += units;
    if (this.typedAt > 0) { this.echoes.push(Math.round((now - this.typedAt) * 10) / 10); this.typedAt = 0; if (this.echoes.length > 50) this.echoes.shift(); }
    if (this.burst.units === 0 || now - this.burst.lastAt > REPORT_IDLE_MS) this.burst = { units: 0, startedAt: now, lastAt: now };
    this.burst.units += units; this.burst.lastAt = now;
    if (this.burst.units < REPORT_BYTES) return;
    if (this.reportTimer) clearTimeout(this.reportTimer);
    this.reportTimer = setTimeout(() => { this.reportTimer = null; this.report(); }, REPORT_IDLE_MS);
  }

  /** The page's own view of itself, for the agent (the app keeps the latest as the pane's `stats`). */
  private report(): void {
    const surface = this.surface;
    const painted = (surface as unknown as { snapshot?: { rowData: ReadonlyArray<{ text: string }> } | null } | null)?.snapshot;
    this.send({ type: "stats", ready: surface !== null, cols: surface?.cols ?? 0, rows: surface?.rows ?? 0, received: this.received,
      chunks: this.socket?.chunks ?? 0, busy: Math.round(this.socket?.busy ?? 0), largest: this.socket?.largest ?? 0, fullWindows: this.socket?.fullWindows ?? 0, attached: this.socket?.attached ?? false, status: this.socket?.status ?? "",
      burst: { units: this.burst.units, ms: Math.round(this.burst.lastAt - this.burst.startedAt) }, echoes: this.echoes.slice(-20),
      text: painted ? painted.rowData.map((row) => row.text.replace(/\s+$/, "")).slice(-12) : [], error: this.lastError });
  }

  private system(message: string): void { this.surface?.write(`\r\n[terminal] ${crlf(message)}\r\n`); }

  private fail(message: string): void {
    this.lastError = message;
    this.mount.classList.add("failed");
    this.mount.textContent = `${message} — close and reopen the terminal to retry.`;
    this.send({ type: "error", message });
  }

  private beforeKey(event: KeyboardEvent): boolean {
    const composed = this.composition.composed(event);
    if (this.session?.mode === "transcript") return !composed && event.key !== "Tab";
    // A held ⌘W, or one while the close dialog is pending, is swallowed without another close.
    if (preventRepeatedTerminalCloseShortcut(event, this.bindings) || (this.closePending && preventTerminalCloseShortcut(event, this.bindings))) {
      event.stopPropagation();
      return false;
    }
    // The app's own shortcuts take their chords before the page sees them; a binding that reaches the page
    // still runs its command (terminal-drawer-view.ts terminalCommandAction).
    const binding = this.bindings.find(({ keys }) => matchesChord(event, keys));
    if (binding) {
      event.preventDefault();
      event.stopPropagation();
      this.send({ type: "command", command: binding.command, chord: binding.chord, repeat: event.repeat });
      return false;
    }
    const data = terminalKeyShortcutData(event, composed);
    if (data !== null) {
      event.preventDefault();
      event.stopPropagation();
      this.typed(data);
      return false;
    }
    return !composed;
  }

  private selection() {
    return { text: this.surface?.getSelection() ?? "", position: this.surface?.getSelectionPosition() ?? null,
      end: this.surface?.getSelectionEndClientRect() ?? null };
  }

  private selectionChanged(): void {
    const surface = this.surface;
    if (!surface || this.session?.mode === "transcript") return;
    if (!surface.hasSelection() && this.selectionActions?.pending) {
      this.selectionActions.cancel();
      this.send({ type: "selection-dismiss", reason: "interaction" });
    }
  }

  /** App → page. */
  receive(message: Message): void {
    if (this.disposed) return;
    if (message.type === "config") { this.configure(message); return; }
    if (message.type === "socket") { this.socketReady(typeof message.url === "string" ? message.url : "", Number(message.serial)); return; }
    const surface = this.surface;
    if (!surface) { this.pending.push(message); return; }
    switch (message.type) {
      case "transcript": {
        const flow = String(message.flow ?? ""), output = String(message.output ?? "");
        const offset = typeof message.offset === "number" ? message.offset : output.length;
        if (flow !== this.transcriptFlow) { this.transcriptFlow = flow; this.transcriptWritten = offset; surface.resetAndWrite(output); return; }
        const update = terminalTranscriptUpdate(this.transcriptWritten, output, offset);
        if (update.reset) surface.resetAndWrite(update.data);
        else if (update.data) surface.write(update.data);
        this.transcriptWritten = update.written;
        return;
      }
      case "system": this.system(String(message.message ?? "")); return;
      case "focus": surface.focus(); return;
      case "clearSelection": surface.clearSelection(); return;
      case "selectAll": surface.selectAll(); surface.focus(); return;
      case "scrollToBottom": surface.scrollToBottom(); surface.focus(); return;
      case "paste": {
        const text = String(message.text ?? "");
        if (!this.readOnly) void surface.pasteFromClipboard(() => Promise.resolve(text));
        surface.focus();
        return;
      }
      // The context menu's Paste: the page reads the clipboard itself, as the reference's renderer does.
      case "pasteClipboard":
        if (!this.readOnly) void surface.pasteFromClipboard(() => navigator.clipboard.readText())
          .then(() => surface.focus(), (error: unknown) => this.system(error instanceof Error ? error.message : "Unable to read the clipboard"));
        return;
      case "stats": this.report(); return;
      default: return;
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.sockets.dispose();
    if (this.reportTimer) clearTimeout(this.reportTimer);
    this.socket?.dispose();
    this.selectionActions?.dispose();
    this.stopInlineHangul?.();
    this.surface?.dispose();
    this.surface = null;
  }
}

/** The framed page: every app message is `receive`'s; it asks for its config once it listens. */
function boot(): void {
  const style = document.createElement("style");
  style.textContent = STYLE;
  document.head.append(style);
  const mount = document.getElementById("terminal") ?? document.body.appendChild(document.createElement("div"));
  mount.id = "terminal";
  const page = new TerminalPage(mount, (url) => new WebSocket(url));
  window.addEventListener("message", (event) => {
    if (!fromApp(event, window, window)) return;
    let message: Message;
    try { message = JSON.parse(event.data) as Message; } catch { return; }
    if (message && typeof message.type === "string") page.receive(message);
  });
  // The web view became the window's first responder (a click, the app's `focus`): the surface takes the keys.
  window.addEventListener("focus", () => page.receive({ type: "focus" }));
  window.addEventListener("pagehide", () => page.dispose());
  window.addEventListener("error", (event) => post({ type: "error", message: event.message }));
  window.addEventListener("unhandledrejection", (event) =>
    post({ type: "error", message: event.reason instanceof Error ? event.reason.message : String(event.reason) }),
  );
  post({ type: "boot" });
}

if (typeof document !== "undefined" && typeof window !== "undefined") boot();
