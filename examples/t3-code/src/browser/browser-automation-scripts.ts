// The page scripts of the previewAutomation host (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/preview/Manager.ts `captureAutomationSnapshot`, `resolveClickPoint`, `typeIntoAutomationTarget`,
// `performAutomationScroll`, `performAutomationWaitFor`, the diagnostics listeners; PlaywrightInjectedRuntime.ts
// `playwrightInjectedRuntimeInstallExpression`). The reference runs them over CDP (`Runtime.evaluate`); here each is a
// function body `@exact/browser-automation`'s `callFunction` runs in the host's sandbox (`SANDBOX`, a content world that
// shares the page's DOM and not its globals, so a page cannot see or replace them). `preview_evaluate` alone runs in the
// page's world, as `Runtime.evaluate` does. Console and network diagnostics come from a page-world script each document
// gets at its commit, in place of CDP's Runtime, Log and Network domains: console calls, uncaught errors, and failed
// `fetch`, XHR and element loads, kept in the page until the host reads them; a failed subresource that is none of those has no entry, and an
// element's failed load has no status.
import { PLAYWRIGHT_INJECTED } from './page-scripts';

/** The host's sandbox (Playwright's utility world). */
export const SANDBOX = 't3-preview-automation';
const json = (value: string): string => JSON.stringify(value);

// ── Playwright (PlaywrightInjectedRuntime.ts) ────────────────────────────────────────────────────
const PLAYWRIGHT_OPTIONS = '{"isUnderTest":false,"sdkLanguage":"javascript","testIdAttributeName":"data-testid","stableRafCount":1,"browserName":"webkit","shouldPrependErrorPrefix":false,"isUtilityWorld":false,"customEngines":[]}';
/** `playwrightInjectedRuntimeInstallExpression` for WebKit, as a function the sandbox runs once per document. */
export const PLAYWRIGHT_INSTALL = `() => {\n  if (globalThis.__t3PlaywrightInjected) return true;\n  const module = { exports: {} };\n${PLAYWRIGHT_INJECTED}\n  globalThis.__t3PlaywrightInjected = new (module.exports.InjectedScript())(globalThis, ${PLAYWRIGHT_OPTIONS});\n  return true;\n}`;
export const PLAYWRIGHT_INSTALLED = '() => Boolean(globalThis.__t3PlaywrightInjected)';

/** The element a locator names (`injected.querySelector(parsed, document, strict)`), as an expression. */
export function element(locator: string | null, strict = true): string {
  if (locator === null) return 'document.activeElement';
  return `(() => { const injected = globalThis.__t3PlaywrightInjected; return injected.querySelector(injected.parseSelector(${json(locator)}), document, ${strict}); })()`;
}

// ── Snapshot (captureAutomationSnapshot) ─────────────────────────────────────────────────────────
export const MAX_VISIBLE_TEXT = 20_000, MAX_ELEMENTS = 200, MAX_ELEMENT_NAME = 200;
export const SNAPSHOT = `
  const selectorFor = (element) => {
    if (element.id) return "#" + CSS.escape(element.id);
    for (const attribute of ["data-testid", "name"]) {
      const value = element.getAttribute(attribute);
      if (value) return element.tagName.toLowerCase() + "[" + attribute + "=" + JSON.stringify(value) + "]";
    }
    const buildParts = (current, parts = []) => {
      if (!current || current.nodeType !== Node.ELEMENT_NODE || parts.length >= 8) return parts;
      const parent = current.parentElement;
      const siblings = parent ? Array.from(parent.children).filter((child) => child.tagName === current.tagName) : [];
      const base = current.tagName.toLowerCase();
      const part = siblings.length > 1 ? base + ":nth-of-type(" + (siblings.indexOf(current) + 1) + ")" : base;
      return buildParts(parent, [part, ...parts]);
    };
    return buildParts(element).join(" > ");
  };
  const visible = (element) => {
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.visibility !== "hidden" && style.display !== "none" && rect.width > 0 && rect.height > 0;
  };
  const elements = Array.from(document.querySelectorAll("a[href],button,input,textarea,select,[role],[tabindex]"))
    .filter(visible).slice(0, ${MAX_ELEMENTS}).map((element) => {
      const rect = element.getBoundingClientRect();
      return {
        tag: element.tagName.toLowerCase(),
        role: element.getAttribute("role"),
        name: (element.getAttribute("aria-label") || element.innerText || element.getAttribute("name") || "").slice(0, ${MAX_ELEMENT_NAME}),
        selector: selectorFor(element),
        x: rect.x, y: rect.y, width: rect.width, height: rect.height
      };
    });
  let accessibilityTree = null;
  try {
    const injected = globalThis.__t3PlaywrightInjected;
    if (injected && document.body) accessibilityTree = { format: "playwright-aria-snapshot", snapshot: injected.ariaSnapshot(document.body, { mode: "ai" }) };
  } catch (error) { accessibilityTree = { format: "playwright-aria-snapshot", error: String(error) }; }
  return {
    url: location.href,
    title: document.title,
    loading: document.readyState !== "complete",
    visibleText: (document.body?.innerText || "").slice(0, ${MAX_VISIBLE_TEXT}),
    interactiveElements: elements,
    accessibilityTree,
    viewport: { width: window.innerWidth, height: window.innerHeight }
  };`;

export const VIEWPORT = '({ width: window.innerWidth, height: window.innerHeight })';
export const READY_STATE = 'document.readyState';

// ── Click (resolveClickPoint) ────────────────────────────────────────────────────────────────────
export function clickPoint(locator: string): string {
  return `
  try {
    const injected = globalThis.__t3PlaywrightInjected;
    const parsed = injected.parseSelector(${json(locator)});
    const element = injected.querySelector(parsed, document, true);
    if (!element) return { notFound: true };
    const visible = injected.elementState(element, "visible");
    const enabled = injected.elementState(element, "enabled");
    if (!visible.matches || !enabled.matches) return { notFound: true };
    element.scrollIntoView({ block: "center", inline: "center" });
    const rect = element.getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  } catch (error) {
    return { invalidSelector: true, message: String(error) };
  }`;
}

/** A click as DOM events at a viewport point, for a page whose native input failed: the pointer and mouse sequence a
 *  press makes, the focus it moves, then `click` (untrusted). Runs in the page's world. */
export function syntheticClick(x: number, y: number): string {
  return `
  const x = ${x}, y = ${y};
  const target = document.elementFromPoint(x, y);
  if (!target) return { missed: true };
  const init = { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, screenX: x, screenY: y, button: 0, view: window };
  target.dispatchEvent(new PointerEvent("pointerdown", { ...init, pointerId: 1, pointerType: "mouse", isPrimary: true, buttons: 1 }));
  const pressed = target.dispatchEvent(new MouseEvent("mousedown", { ...init, buttons: 1, detail: 1 }));
  if (pressed) {
    let focusable = target;
    while (focusable && !(focusable.tabIndex >= 0 || focusable.isContentEditable)) focusable = focusable.parentElement;
    if (focusable && typeof focusable.focus === "function") focusable.focus({ preventScroll: true });
    else if (document.activeElement && document.activeElement !== document.body) document.activeElement.blur();
  }
  target.dispatchEvent(new PointerEvent("pointerup", { ...init, pointerId: 1, pointerType: "mouse", isPrimary: true, buttons: 0 }));
  target.dispatchEvent(new MouseEvent("mouseup", { ...init, buttons: 0, detail: 1 }));
  target.dispatchEvent(new MouseEvent("click", { ...init, buttons: 0, detail: 1 }));
  return { ok: true };`;
}

// ── Type (typeIntoAutomationTarget) ──────────────────────────────────────────────────────────────
export function typeInto(locator: string | null, text: string, clear: boolean): string {
  return `
  try {
    const element = ${element(locator)};
    if (!element) return { notFound: true };
    const textControl =
      element instanceof HTMLTextAreaElement ||
      (element instanceof HTMLInputElement &&
        !new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"]).has(element.type));
    const editable = textControl || element.isContentEditable;
    if (!editable || element.disabled || element.readOnly) return { notEditable: true };
    element.focus();
    if (document.activeElement !== element) return { notEditable: true };
    if (${clear}) {
      if (textControl) {
        element.select();
      } else {
        const range = document.createRange();
        range.selectNodeContents(element);
        const selection = document.getSelection();
        selection?.removeAllRanges();
        selection?.addRange(range);
      }
    }
    const text = ${json(text)};
    let inserted = true;
    if (text.length > 0) {
      inserted = document.execCommand("insertText", false, text);
    } else if (${clear}) {
      document.execCommand("delete", false);
      const cleared = textControl ? element.value.length === 0 : (element.textContent ?? "").length === 0;
      if (!cleared) {
        if (textControl) {
          const prototype = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
          const valueSetter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
          if (valueSetter) valueSetter.call(element, "");
          else element.value = "";
        } else {
          element.replaceChildren();
        }
        element.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "deleteContentBackward" }));
      }
    }
    if (!inserted) return { notEditable: true };
    element.dispatchEvent(new Event("change", { bubbles: true }));
    return { ok: true };
  } catch (error) {
    return { invalidSelector: true, message: String(error) };
  }`;
}

// ── Scroll (performAutomationScroll) ─────────────────────────────────────────────────────────────
export function scrollBy(locator: string | null, deltaX: number, deltaY: number): string {
  return `
  try {
    const target = ${locator === null ? 'window' : element(locator)};
    if (!target) return { notFound: true };
    target.scrollBy({ left: ${deltaX}, top: ${deltaY}, behavior: "instant" });
    return { ok: true };
  } catch (error) {
    return { invalidSelector: true, message: String(error) };
  }`;
}

// ── Wait for (performAutomationWaitFor) ──────────────────────────────────────────────────────────
export function waitFor(locator: string | null, text: string | null, urlIncludes: string | null): string {
  const selector = locator === null ? 'true'
    : `(() => { const injected = globalThis.__t3PlaywrightInjected; return injected.querySelector(injected.parseSelector(${json(locator)}), document, false) !== null; })()`;
  return `
  try {
    const selectorMatched = ${selector};
    const textMatched = ${text === null ? 'true' : `(document.body?.innerText || "").includes(${json(text)})`};
    const urlMatched = ${urlIncludes === null ? 'true' : `location.href.includes(${json(urlIncludes)})`};
    return { matched: selectorMatched && textMatched && urlMatched };
  } catch (error) {
    return { invalidSelector: true, message: String(error) };
  }`;
}

// ── Evaluate (performAutomationEvaluate) ─────────────────────────────────────────────────────────
/** `undefined` marked apart from `null` in an evaluation's JSON. */
export const UNDEFINED_MARK = '__t3_preview_undefined__';
/** The expression in the page's world, awaited as `Runtime.evaluate`'s `awaitPromise`, returned by value: JSON in the
 *  page (a DOM node is `{}`, as CDP's `returnByValue`). */
export function evaluateExpression(expression: string, awaitPromise: boolean): string {
  return `async () => {\nconst __t3Value = ${awaitPromise ? 'await ' : ''}(\n${expression}\n);\nreturn __t3Value === undefined ? ${json(UNDEFINED_MARK)} : JSON.stringify(__t3Value);\n}`;
}
/** A script that is no expression (`const a = 1; a + 1`) runs as a script: its completion value, not awaited. */
export function evaluateScript(expression: string): string {
  return `() => { const __t3Value = eval(${json(expression)}); return __t3Value === undefined ? ${json(UNDEFINED_MARK)} : JSON.stringify(__t3Value); }`;
}

// ── Diagnostics (the page's world, at document start) ───────────────────────────────────────────
/** Kept in the page (`__t3PreviewDiagnosticsLog`, at most 400) until the host drains it (`DRAIN_DIAGNOSTICS`). */
export const DIAGNOSTICS = `() => {
  if (window.__t3PreviewDiagnostics) return;
  window.__t3PreviewDiagnostics = true;
  const log = window.__t3PreviewDiagnosticsLog = window.__t3PreviewDiagnosticsLog || [];
  const post = (kind, entry) => { log.push({ kind, entry: { ...entry, timestamp: new Date().toISOString() } }); if (log.length > 400) log.splice(0, log.length - 400); };
  const format = (value) => {
    if (typeof value === "string") return value;
    if (value instanceof Error) return String(value.stack || value);
    try { const text = JSON.stringify(value); return text === undefined ? String(value) : text; } catch { return String(value); }
  };
  const levels = { log: "log", info: "info", warn: "warning", error: "error", debug: "debug", trace: "trace", assert: "assert" };
  for (const [method, level] of Object.entries(levels)) {
    const original = console[method];
    if (typeof original !== "function") continue;
    console[method] = function (...args) {
      if (method !== "assert" || !args[0]) post("console", { level, text: (method === "assert" ? args.slice(1) : args).map(format).join(" "), source: "console" });
      return original.apply(this, args);
    };
  }
  window.addEventListener("error", (event) => {
    const target = event.target;
    if (target && target !== window && target instanceof Element) {
      const url = target.currentSrc || target.src || target.href || "";
      if (url) post("network", { url: String(url), method: "GET", status: null, failed: true, errorText: "Load failed" });
      return;
    }
    post("console", { level: "error", text: String(event.message || "Uncaught exception"), source: "exception" });
  }, true);
  window.addEventListener("unhandledrejection", (event) => post("console", { level: "error", text: "Uncaught (in promise) " + format(event.reason), source: "exception" }));
  const originalFetch = window.fetch;
  if (typeof originalFetch === "function") {
    window.fetch = function (input, init) {
      const request = input instanceof Request ? input : null;
      const url = String(request ? request.url : input);
      const method = String(init?.method || request?.method || "GET").toUpperCase();
      return originalFetch.apply(this, arguments).then((response) => {
        if (response.status >= 400) post("network", { url: response.url || url, method, status: response.status, failed: true });
        return response;
      }, (error) => {
        post("network", { url, method, status: null, failed: true, errorText: String(error?.message || error || "Network request failed") });
        throw error;
      });
    };
  }
  const open = XMLHttpRequest.prototype.open, send = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (method, url) { this.__t3Request = { method: String(method || "GET").toUpperCase(), url: String(url) }; return open.apply(this, arguments); };
  XMLHttpRequest.prototype.send = function () {
    const request = this.__t3Request;
    if (request) {
      this.addEventListener("loadend", () => {
        if (this.status >= 400) post("network", { url: this.responseURL || request.url, method: request.method, status: this.status, failed: true });
        else if (this.status === 0) post("network", { url: request.url, method: request.method, status: null, failed: true, errorText: "Network request failed" });
      });
    }
    return send.apply(this, arguments);
  };
}`;
export const DRAIN_DIAGNOSTICS = '(window.__t3PreviewDiagnosticsLog || []).splice(0)';

// ── Media (the sandbox, at document start): Mute and the audible indicator ──────────────────────
/** WebKit has no public page mute and no audible signal: this mutes and listens to the document's media elements (Web
 *  Audio is not covered). Audible as Chromium's tab audio state: a media element playing with sound the page asked for,
 *  muted by us or not. WebKit pauses a muted element while its page is out of the window and plays it again when the
 *  page is shown; such an element still counts, as a muted background tab still plays in Chromium. The host reads
 *  `state()` and sets `setMuted`. */
export const MEDIA = `() => {
  if (globalThis.__t3PreviewMedia) return;
  const wanted = new WeakMap(), suspended = new Set();
  let muted = false;
  const media = () => Array.from(document.querySelectorAll("audio, video"));
  const pageMuted = (element) => wanted.has(element) ? wanted.get(element) : element.muted;
  const playing = (element) => (!element.paused || suspended.has(element)) && !element.ended;
  const audible = () => media().some((element) => playing(element) && !pageMuted(element) && element.volume > 0);
  const apply = (element) => {
    if (muted) { if (!wanted.has(element)) wanted.set(element, element.muted); if (!element.muted) element.muted = true; }
    else if (wanted.has(element)) { const value = wanted.get(element); wanted.delete(element); element.muted = value; }
  };
  for (const type of ["play", "playing", "pause", "ended", "volumechange", "emptied", "loadeddata"]) {
    document.addEventListener(type, (event) => {
      const element = event.target;
      if (!(element instanceof HTMLMediaElement)) return;
      if (type === "volumechange" && muted && wanted.has(element) && !element.muted) { wanted.set(element, false); element.muted = true; }
      else apply(element);
      if (type === "pause" && muted && wanted.has(element) && document.visibilityState === "hidden") suspended.add(element);
      else if (type !== "volumechange") suspended.delete(element);
    }, true);
  }
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState !== "visible" || suspended.size === 0) return;
    setTimeout(() => { for (const element of suspended) if (element.paused) suspended.delete(element); }, 1500);
  });
  globalThis.__t3PreviewMedia = {
    setMuted(next) { muted = next; if (!muted) suspended.clear(); for (const element of media()) apply(element); return muted; },
    state() { return { audible: audible(), muted }; },
  };
}`;
export const MEDIA_STATE = 'globalThis.__t3PreviewMedia ? globalThis.__t3PreviewMedia.state() : null';
export const setMuted = (muted: boolean): string => `globalThis.__t3PreviewMedia ? globalThis.__t3PreviewMedia.setMuted(${muted}) : null`;
