#if os(macOS)
import AppKit
import WebKit

/// Where a page's links and window requests go, and its HTML fullscreen (MIT reference, see LICENSE-T3, T3 Code
/// 9c161462c9: apps/desktop/src/preview/Manager.ts `previewWindowOpenAction`, `setWindowOpenHandler`, `emitOpenLink`;
/// BrowserSession.ts `NEVER_EXTERNAL_PROTOCOLS`, `externalProtocolPromptUrl`, `confirmOpenExternal`, the `fullscreen`
/// permission; DesktopWindow.ts `disableHtmlFullscreenWindowResize`).
///
/// - A window request follows Chromium's disposition: a scripted window with window features (or a ⇧-click) is a
///   `new-window`, a ⌘- or middle-click a `background-tab`, anything else a `foreground-tab`. A blank window is denied,
///   so `window.open()` answers null; an http(s) `new-window` gets a real window (T3BrowserPopup); an http(s) tab
///   disposition without a form body opens as another tab of the thread (`openLinks`, which the data module takes); the
///   rest load in the tab. A ⌘- or middle-click on a link with no target opens a tab the same way.
/// - A custom scheme (`slack:`, `zoommtg:`, `mailto:`) opens its app only after the person answers "Open" in a sheet
///   on the tab's window, one prompt at a time so a page cannot stack them by looping a deep link. The schemes in
///   `neverExternalSchemes` never leave. An agent run answers Cancel at once, as it answers `confirm()`.
/// - HTML fullscreen fills the tab, not the screen: WebKit's element fullscreen would take the whole display, so it
///   stays off and `fullscreenScript` gives the main frame the Fullscreen API over the page's own viewport (Chromium's
///   `:fullscreen` user-agent rules on the element, a black backdrop, Escape to leave).
final class T3BrowserLinks {
    /// Every page's requests share one counter, so a tab made again under the same id never repeats a serial.
    private static var serial = 0
    /// Links the page asked to open beside itself, oldest first, until the data module takes them.
    private(set) var requests: [(serial: Int, url: String, background: Bool)] = []
    /// A custom-scheme prompt is up.
    var prompting = false

    func request(_ url: String, background: Bool) {
        Self.serial += 1
        requests.append((Self.serial, url, background))
        if requests.count > 16 { requests.removeFirst(requests.count - 16) }
    }

    func take(through serial: Int) { requests.removeAll { $0.serial <= serial } }

    var report: [[String: Any]] { requests.map { ["serial": $0.serial, "url": $0.url, "background": $0.background] } }
}

extension T3BrowserSession {
    /// BrowserSession.ts NEVER_EXTERNAL_PROTOCOLS: schemes that read local files, run script, or are web pages that
    /// belong in the tab itself.
    static let neverExternalSchemes: Set<String> = ["about", "blob", "chrome", "data", "devtools", "file", "filesystem", "http", "https", "javascript", "view-source"]
    static let maxPromptURLLength = 300

    /// `externalProtocolPromptUrl`: the URL to offer the person for a custom scheme, nil when it stays denied.
    static func externalPromptURL(_ raw: String) -> String? {
        guard let url = URL(string: raw), let scheme = url.scheme?.lowercased(), !scheme.isEmpty, !neverExternalSchemes.contains(scheme) else { return nil }
        return url.absoluteString
    }

    /// The prompt's message and detail, as `confirmOpenExternal` words them.
    static func externalPrompt(_ url: String) -> (message: String, detail: String) {
        let scheme = URL(string: url)?.scheme ?? ""
        let shown = url.count > maxPromptURLLength ? "\(url.prefix(maxPromptURLLength))…" : url
        return ("Open this \(scheme) link?", "A page in the browser wants to open another application:\n\n\(shown)")
    }

    /// Chromium's WindowOpenDisposition for a WebKit window request or link click.
    static func disposition(scripted: Bool, features: Bool, modifiers: NSEvent.ModifierFlags, button: Int) -> String {
        if scripted { return features ? "new-window" : "foreground-tab" }
        let command = modifiers.contains(.command), shift = modifiers.contains(.shift)
        // WKNavigationAction.buttonNumber is a pressed-buttons mask (middle 1 << 2); 2 is the plain middle button number.
        if command && shift { return "foreground-tab" }
        if command || button == 4 || button == 2 { return "background-tab" }
        return shift ? "new-window" : "foreground-tab"
    }

    /// Manager.ts previewWindowOpenAction: a real window only for a `new-window` to a scheme whose window can be hardened;
    /// a tab disposition opens another tab unless it carries a form body (which a new tab could only reopen as a GET);
    /// a blank window is denied (MSAL and other SDKs read the null as a blocked pop-up and fall back); the rest navigate.
    static func windowOpenAction(url: String, disposition: String, postBody: Bool = false) -> String {
        if url.isEmpty || url == "about:blank" { return "deny" }
        guard let parsed = URL(string: url), ["http", "https"].contains(parsed.scheme?.lowercased() ?? "") else { return "navigate" }
        if disposition == "new-window" { return "popup" }
        return !postBody && (disposition == "foreground-tab" || disposition == "background-tab") ? "new-tab" : "navigate"
    }

    /// The tab disposition a click on a link with no target asks for (a ⌘- or middle-click), or nil to follow it in place.
    static func linkTabDisposition(_ action: WKNavigationAction) -> String? {
        guard action.navigationType == .linkActivated, action.modifierFlags.contains(.command) || action.buttonNumber == 4 || action.buttonNumber == 2 else { return nil }
        return disposition(scripted: false, features: false, modifiers: action.modifierFlags, button: action.buttonNumber)
    }

    /// `emitOpenLink`: the data module opens it as another tab of the thread, under this tab's profile.
    func openBeside(_ url: String, background: Bool) {
        links.request(url, background: background)
        changed?()
    }

    /// `confirmOpenExternal`.
    func confirmOpenExternal(_ raw: String) {
        guard let shown = Self.externalPromptURL(raw), let url = URL(string: shown), !links.prompting else { return refuse(raw) }
        guard dialogs, let window = web.window ?? NSApp.mainWindow else { return refuse(raw) }
        links.prompting = true
        let prompt = Self.externalPrompt(shown)
        askExternal(prompt.message, prompt.detail, window) { [weak self] granted in
            guard let self else { return }
            self.links.prompting = false
            if granted { self.openExternal(url) } else { self.refuse(raw) }
        }
    }

    private func refuse(_ url: String) {
        refuse(navigation: url)
        changed?()
    }

    /// The sheet `confirmOpenExternal` shows (ElectronDialog.showMessageBox with the host window as owner: a sheet on macOS).
    static func askExternalInSheet(_ message: String, _ detail: String, _ window: NSWindow, _ done: @escaping (Bool) -> Void) {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = message
        alert.informativeText = detail
        alert.addButton(withTitle: "Open")
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { done($0 == .alertFirstButtonReturn) }
    }

    // MARK: HTML fullscreen

    static func installFullscreen(on configuration: WKWebViewConfiguration) {
        configuration.userContentController.addUserScript(WKUserScript(source: fullscreenScript, injectionTime: .atDocumentStart, forMainFrameOnly: true))
    }

    static let fullscreenScript = """
    (() => {
      if (window.__t3codeFullscreen || document.fullscreenEnabled) return;
      window.__t3codeFullscreen = true;
      const ATTR = "data-t3code-fullscreen";
      const css = `[${ATTR}]{object-fit:contain;position:fixed!important;inset:0!important;margin:0!important;box-sizing:border-box!important;min-width:0!important;max-width:none!important;min-height:0!important;max-height:none!important;width:100%!important;height:100%!important;transform:none!important;z-index:2147483647!important}iframe[${ATTR}]{border:none!important;padding:0!important}[${ATTR}-backdrop]{position:fixed!important;inset:0!important;background:black!important;z-index:2147483646!important}`;
      let element = null, style = null, backdrop = null;
      const fire = (target) => {
        for (const type of ["fullscreenchange", "webkitfullscreenchange"]) {
          (target.isConnected ? target : document).dispatchEvent(new Event(type, { bubbles: true, composed: true }));
        }
      };
      const leave = () => {
        if (!element) return;
        const target = element;
        element.removeAttribute(ATTR);
        style?.remove(); backdrop?.remove();
        element = null; style = null; backdrop = null;
        fire(target);
      };
      const enter = function () {
        if (!(this instanceof Element) || !this.isConnected) return Promise.reject(new TypeError("Element is not connected"));
        if (navigator.userActivation && !navigator.userActivation.isActive && !element) {
          document.dispatchEvent(new Event("fullscreenerror", { bubbles: true }));
          return Promise.reject(new TypeError("Permissions check failed"));
        }
        if (element === this) return Promise.resolve();
        if (element) { element.removeAttribute(ATTR); }
        else {
          style = document.createElement("style"); style.textContent = css;
          backdrop = document.createElement("div"); backdrop.setAttribute(ATTR + "-backdrop", "");
          document.documentElement.append(style, backdrop);
        }
        element = this;
        if (this !== document.documentElement) this.setAttribute(ATTR, "");
        fire(this);
        return Promise.resolve();
      };
      const exit = () => {
        if (!element) return Promise.reject(new TypeError("Not in fullscreen"));
        leave();
        return Promise.resolve();
      };
      const define = (target, name, value) => Object.defineProperty(target, name, { configurable: true, writable: true, value });
      const getter = (target, name, get) => Object.defineProperty(target, name, { configurable: true, get });
      define(Element.prototype, "requestFullscreen", enter);
      define(Element.prototype, "webkitRequestFullscreen", enter);
      define(Element.prototype, "webkitRequestFullScreen", enter);
      define(Document.prototype, "exitFullscreen", exit);
      define(Document.prototype, "webkitExitFullscreen", exit);
      define(Document.prototype, "webkitCancelFullScreen", exit);
      getter(Document.prototype, "fullscreenEnabled", () => true);
      getter(Document.prototype, "webkitFullscreenEnabled", () => true);
      getter(Document.prototype, "fullscreenElement", () => element);
      getter(Document.prototype, "webkitFullscreenElement", () => element);
      getter(Document.prototype, "webkitCurrentFullScreenElement", () => element);
      getter(Document.prototype, "fullscreen", () => element !== null);
      getter(Document.prototype, "webkitIsFullScreen", () => element !== null);
      window.addEventListener("keydown", (event) => {
        if (!element || event.key !== "Escape") return;
        event.preventDefault(); event.stopImmediatePropagation();
        leave();
      }, true);
      window.addEventListener("pagehide", leave);
    })();
    """
}

extension T3BrowserSessions {
    /// `browserOpenLinks`: the data module took a tab's requests through `taken`; nil for any other op.
    func performLinks(_ request: [String: Any]) -> [String: Any]? {
        guard request["op"] as? String == "browserOpenLinks" else { return nil }
        let generation = request["generation"] as? Int ?? 0
        guard let session = sessions[request["tab"] as? String ?? ""] else { return ["ok": true, "generation": generation, "value": ["done": false]] }
        session.links.take(through: (request["taken"] as? NSNumber)?.intValue ?? 0)
        session.changed?()
        return ["ok": true, "generation": generation, "value": ["done": true]]
    }
}
#endif
