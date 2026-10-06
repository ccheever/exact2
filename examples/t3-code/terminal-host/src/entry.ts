// The terminal page a `t3-terminal` view loads (T3TerminalView.swift). It runs T3 Code's own
// Ghostty surface (MIT reference, see LICENSE-T3: apps/web/src/terminal/ghostty/surface.ts,
// copied unchanged apart from imports under ../vendor/ghostty) and joins it to the native view
// with a JSON bridge:
//
// - page → native (`webkit.messageHandlers.t3terminal`): ready, data, resize, selection, link,
//   contextmenu, chord, focus, error.
// - native → page (`t3Terminal.receive(message)`): write, resetAndWrite, theme, font, visible,
//   focus, chords, clearSelection, readSelection, paste, dispose; `t3Terminal.debug()` answers
//   the agent's status (ready, grid, visible text, selection, last error).
//
// Mounting, the setup failure text and the theme fallbacks follow ThreadTerminalDrawer.tsx
// (:176-215 terminalThemeFromApp, :895-915 the failure text). Which chords the app keeps is the
// app's decision (ThreadTerminalDrawer.tsx:745-782 beforeKey); the native view sends the list.
import type { GhosttyColor, GhosttyTheme } from "../vendor/ghostty/core";
import {
  GhosttyTerminalSurface,
  type GhosttyTerminalFont,
} from "../vendor/ghostty/surface";

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

function themeFrom(value: unknown): { theme: GhosttyTheme; dark: boolean } {
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
    const key = parts.pop() ?? "";
    if (key === "") return [];
    const has = (name: string) => parts.includes(name);
    return [{ key: key.toLowerCase(), meta: has("Meta"), ctrl: has("Control"), alt: has("Alt"), shift: has("Shift") }];
  });
}

/** The key a chord names, by the physical key first: under Korean 2-Set ⌘K's `key` is "ㅏ". */
export function eventKeyNames(event: Pick<KeyboardEvent, "key" | "code">): string[] {
  const names = [event.key.toLowerCase()];
  const code = event.code;
  if (/^Key[A-Z]$/.test(code)) names.unshift(code.slice(3).toLowerCase());
  else if (/^Digit\d$/.test(code)) names.unshift(code.slice(5));
  else if (code !== "") names.push(code.toLowerCase());
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
  private visible = true;
  private lastError = "";
  private disposed = false;
  /** UTF-16 units of output received from the native view (write and resetAndWrite). */
  private received = 0;

  constructor(private readonly mount: HTMLElement) {}

  async start(init: Record<string, unknown>): Promise<void> {
    this.chords = parseChords(init.chords);
    this.visible = init.visible !== false;
    const { theme, dark } = themeFrom(init.theme);
    document.documentElement.classList.toggle("dark", dark);
    document.body.style.background = `rgb(${theme.background.r}, ${theme.background.g}, ${theme.background.b})`;
    try {
      const surface = await GhosttyTerminalSurface.create(this.mount, {
        theme,
        font: fontFrom(init.font),
        visible: this.visible,
        onData: (data) => post({ type: "data", data }),
        onResize: (cols, rows) => post({ type: "resize", cols, rows }),
        onSelectionChange: () => this.postSelection(),
        beforeKey: (event) => this.beforeKey(event),
        onLinkActivate: (text, event) =>
          post({ type: "link", text, metaKey: event.metaKey, ctrlKey: event.ctrlKey, shiftKey: event.shiftKey, altKey: event.altKey }),
        onContextMenu: (event) => {
          event.preventDefault();
          post({ type: "contextmenu", x: event.clientX, y: event.clientY, selection: this.surface?.getSelection() ?? "" });
        },
      });
      if (this.disposed) {
        surface.dispose();
        return;
      }
      this.surface = surface;
      surface.input.addEventListener("focus", () => post({ type: "focus", focused: true }));
      surface.input.addEventListener("blur", () => post({ type: "focus", focused: false }));
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
    if (!matchesChord(event, this.chords)) return true;
    post({ type: "chord", key: event.key, code: event.code, metaKey: event.metaKey, ctrlKey: event.ctrlKey, altKey: event.altKey, shiftKey: event.shiftKey });
    return false;
  }

  private postSelection(): void {
    const surface = this.surface;
    if (!surface) return;
    post({
      type: "selection",
      text: surface.getSelection(),
      position: surface.getSelectionPosition(),
      end: surface.getSelectionEndClientRect(),
    });
  }

  /** Native → page. Returns a value for the requests that read (readSelection, debug). */
  receive(message: Message): unknown {
    if (this.disposed) return null;
    const surface = this.surface;
    if (message.type === "chords") {
      this.chords = parseChords(message.chords);
      return null;
    }
    if (message.type === "dispose") {
      this.disposed = true;
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
