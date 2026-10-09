#if os(macOS)
import Foundation
import WebKit

/// The page scripts of the previewAutomation host (browser-surface part 5; MIT reference, see LICENSE-T3, T3 Code
/// 1e2ecbd975: apps/desktop/src/preview/Manager.ts `captureAutomationSnapshot`, `resolveClickPoint`,
/// `typeIntoAutomationTarget`, `performAutomationScroll`, `performAutomationWaitFor`, the diagnostics listeners;
/// PlaywrightInjectedRuntime.ts `playwrightInjectedRuntimeInstallExpression`). The reference runs them over CDP
/// (`Runtime.evaluate`); WebKit runs them with `callAsyncJavaScript` in the host's own content world, which shares
/// the page's DOM and not its globals, so a page cannot see or replace them. `preview_evaluate` alone runs in the
/// page's world, as `Runtime.evaluate` does. Console and network diagnostics come from a page-world user script
/// in place of CDP's Runtime, Log and Network domains (X1 path B): console calls, uncaught errors, and failed
/// `fetch`, XHR and element loads; a failed subresource that is none of those has no entry, and an element's
/// failed load has no status.
enum T3BrowserAutomationScripts {
    /// The host's content world (Playwright's utility world).
    static let world = WKContentWorld.world(name: "t3-preview-automation")
    static let diagnosticsHandler = "t3PreviewDiagnostics"
    static let mediaHandler = "t3PreviewMedia"

    // MARK: Playwright (PlaywrightInjectedRuntime.ts)

    /// The vendored injected script (assets/vendor/playwright, Apache-2.0), read once.
    static let playwrightSource: String? = {
        guard let url = T3TerminalAssets.fileURL("vendor/playwright/injected-script.js") ?? bundledPlaywright() else { return nil }
        return try? String(contentsOf: url, encoding: .utf8)
    }()
    /// A test binary runs beside the sources: the example's own assets.
    private static func bundledPlaywright() -> URL? {
        guard let app = ProcessInfo.processInfo.environment["T3_APP_DIR"] else { return nil }
        let url = URL(fileURLWithPath: app).appendingPathComponent("assets/vendor/playwright/injected-script.js")
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }
    /// `playwrightInjectedRuntimeInstallExpression`, for WebKit (the injected script's WebKit workarounds).
    static func playwrightInstall(_ source: String) -> String {
        let options = #"{"isUnderTest":false,"sdkLanguage":"javascript","testIdAttributeName":"data-testid","stableRafCount":1,"browserName":"webkit","shouldPrependErrorPrefix":false,"isUtilityWorld":false,"customEngines":[]}"#
        return "(() => {\n  if (globalThis.__t3PlaywrightInjected) return true;\n  const module = { exports: {} };\n" + source
            + "\n  globalThis.__t3PlaywrightInjected = new (module.exports.InjectedScript())(globalThis, \(options));\n  return true;\n})()"
    }
    static let playwrightInstalled = "return Boolean(globalThis.__t3PlaywrightInjected);"

    /// The element a locator names (`injected.querySelector(parsed, document, strict)`), as an expression.
    static func element(_ locator: String?, strict: Bool = true) -> String {
        guard let locator else { return "document.activeElement" }
        return "(() => { const injected = globalThis.__t3PlaywrightInjected; return injected.querySelector(injected.parseSelector(\(json(locator))), document, \(strict)); })()"
    }

    // MARK: Snapshot (captureAutomationSnapshot)

    static let maxVisibleText = 20_000, maxElements = 200, maxElementName = 200
    static let snapshot = #"""
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
          .filter(visible).slice(0, MAX_ELEMENTS).map((element) => {
            const rect = element.getBoundingClientRect();
            return {
              tag: element.tagName.toLowerCase(),
              role: element.getAttribute("role"),
              name: (element.getAttribute("aria-label") || element.innerText || element.getAttribute("name") || "").slice(0, MAX_NAME),
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
          visibleText: (document.body?.innerText || "").slice(0, MAX_TEXT),
          interactiveElements: elements,
          accessibilityTree,
          viewport: { width: window.innerWidth, height: window.innerHeight }
        };
        """#
        .replacingOccurrences(of: "MAX_ELEMENTS", with: String(maxElements))
        .replacingOccurrences(of: "MAX_NAME", with: String(maxElementName))
        .replacingOccurrences(of: "MAX_TEXT", with: String(maxVisibleText))

    static let viewport = "return { width: window.innerWidth, height: window.innerHeight };"
    static let readyState = "return document.readyState;"

    // MARK: Click (resolveClickPoint)

    static func clickPoint(_ locator: String) -> String {
        """
        try {
          const injected = globalThis.__t3PlaywrightInjected;
          const parsed = injected.parseSelector(\(json(locator)));
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
        }
        """
    }

    /// A click as DOM events at a viewport point, for a page that is in no window (no native event reaches it):
    /// the pointer and mouse sequence a press makes, the focus it moves, then `click` (untrusted, X1 path B).
    static func syntheticClick(x: Double, y: Double) -> String {
        """
        const x = \(x), y = \(y);
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
        return { ok: true };
        """
    }

    /// A receipt for native input: settles once the page has handled the event (its listeners in every world ran;
    /// a `click` follows its `mouseup` in the same task), or false after 5 s or when the document goes.
    static func armReceipt(_ event: String) -> String {
        """
        globalThis.__t3InputReceipt = new Promise((resolve) => {
          const done = (value) => { clearTimeout(timer); removeEventListener("pagehide", gone, true); resolve(value); };
          const gone = () => done(false);
          const timer = setTimeout(() => done(false), 5000);
          addEventListener("pagehide", gone, true);
          document.addEventListener(\(json(event)), () => setTimeout(() => done(true), 0), { capture: true, once: true });
        });
        return true;
        """
    }
    static let awaitReceipt = "return await globalThis.__t3InputReceipt;"

    // MARK: Type (typeIntoAutomationTarget)

    static func type(locator: String?, text: String, clear: Bool) -> String {
        """
        try {
          const element = \(element(locator));
          if (!element) return { notFound: true };
          const textControl =
            element instanceof HTMLTextAreaElement ||
            (element instanceof HTMLInputElement &&
              !new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"]).has(element.type));
          const editable = textControl || element.isContentEditable;
          if (!editable || element.disabled || element.readOnly) return { notEditable: true };
          element.focus();
          if (document.activeElement !== element) return { notEditable: true };
          const clear = \(clear);
          if (clear) {
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
          const text = \(json(text));
          let inserted = true;
          if (text.length > 0) {
            inserted = document.execCommand("insertText", false, text);
          } else if (clear) {
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
        }
        """
    }

    // MARK: Scroll (performAutomationScroll)

    static func scroll(locator: String?, deltaX: Double, deltaY: Double) -> String {
        """
        try {
          const target = \(locator.map { element($0) } ?? "window");
          if (!target) return { notFound: true };
          target.scrollBy({ left: \(deltaX), top: \(deltaY), behavior: "instant" });
          return { ok: true };
        } catch (error) {
          return { invalidSelector: true, message: String(error) };
        }
        """
    }

    // MARK: Wait for (performAutomationWaitFor)

    static func waitFor(locator: String?, text: String?, urlIncludes: String?) -> String {
        """
        try {
          const selectorMatched = \(locator.map { "(() => { const injected = globalThis.__t3PlaywrightInjected; return injected.querySelector(injected.parseSelector(\(json($0))), document, false) !== null; })()" } ?? "true");
          const textMatched = \(text.map { "(document.body?.innerText || \"\").includes(\(json($0)))" } ?? "true");
          const urlMatched = \(urlIncludes.map { "location.href.includes(\(json($0)))" } ?? "true");
          return { matched: selectorMatched && textMatched && urlMatched };
        } catch (error) {
          return { invalidSelector: true, message: String(error) };
        }
        """
    }

    // MARK: Evaluate (performAutomationEvaluate)

    /// The expression in the page's world, awaited as `Runtime.evaluate`'s `awaitPromise`, returned by value: JSON
    /// in the page (a DOM node is `{}`, as CDP's `returnByValue`), `undefined` marked apart from `null`.
    static func evaluate(_ expression: String, awaitPromise: Bool) -> String {
        "const __t3Value = \(awaitPromise ? "await " : "")(\n\(expression)\n);\nreturn __t3Value === undefined ? \"\(undefinedMark)\" : JSON.stringify(__t3Value);"
    }
    static let undefinedMark = "__t3_preview_undefined__"
    /// A script that is no expression (`const a = 1; a + 1`) runs as a script: its completion value, not awaited.
    static func evaluateScript(_ expression: String) -> String {
        "(() => { const __t3Value = eval(\(json(expression))); return __t3Value === undefined ? \"\(undefinedMark)\" : JSON.stringify(__t3Value); })()"
    }

    // MARK: Diagnostics (the page's world, at document start)

    static let diagnostics = #"""
        (() => {
          if (window.__t3PreviewDiagnostics) return;
          window.__t3PreviewDiagnostics = true;
          const handler = window.webkit?.messageHandlers?.t3PreviewDiagnostics;
          if (!handler) return;
          const post = (kind, entry) => { try { handler.postMessage({ kind, entry: { ...entry, timestamp: new Date().toISOString() } }); } catch {} };
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
        })();
        """#

    // MARK: Media (the host's world, at document start): Mute and the audible indicator

    /// WebKit has no public page mute (`_setPageMuted:` is SPI) and no audible signal (`_isPlayingAudio` is SPI):
    /// this mutes and listens to the document's media elements (Web Audio is not covered: X1 path B). Audible as
    /// Chromium's tab audio state: a media element playing with sound the page asked for, muted by us or not.
    /// WebKit pauses a muted element while its page is out of the window (it cannot be heard) and plays it again
    /// when the page is shown; such an element still counts, as a muted background tab still plays in Chromium.
    static let media = #"""
        (() => {
          if (globalThis.__t3PreviewMedia) return;
          const handler = globalThis.webkit?.messageHandlers?.t3PreviewMedia;
          const wanted = new WeakMap(), suspended = new Set();
          let muted = false, last = null;
          const media = () => Array.from(document.querySelectorAll("audio, video"));
          const pageMuted = (element) => wanted.has(element) ? wanted.get(element) : element.muted;
          const playing = (element) => (!element.paused || suspended.has(element)) && !element.ended;
          const report = () => {
            const audible = media().some((element) => playing(element) && !pageMuted(element) && element.volume > 0);
            if (audible === last) return;
            last = audible;
            try { handler?.postMessage({ audible }); } catch {}
          };
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
              report();
            }, true);
          }
          // Shown again: WebKit plays what it paused; anything still paused was paused by the page.
          document.addEventListener("visibilitychange", () => {
            if (document.visibilityState !== "visible" || suspended.size === 0) return;
            setTimeout(() => { for (const element of suspended) if (element.paused) suspended.delete(element); report(); }, 1500);
          });
          globalThis.__t3PreviewMedia = {
            setMuted(next) { muted = next; if (!muted) suspended.clear(); for (const element of media()) apply(element); report(); return muted; },
            report() { last = null; report(); },
          };
        })();
        """#

    static func json(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: [value]), let text = String(data: data, encoding: .utf8) else { return "\"\"" }
        return String(text.dropFirst().dropLast())
    }
}
#endif
