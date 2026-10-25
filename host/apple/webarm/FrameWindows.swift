// Every iframe's new windows, dialogs, file inputs, downloads, permissions
// and other apps' schemes on Apple (@ref LLP 1020.002 D5): the arm is each
// web view's `WKUIDelegate`. What the author leaves unsaid is the platform's:
// its sheets and panels, Safari's popup rules, and denial of what a guest
// asks for. Under the agent a sheet or panel is a request the drive answers
// instead (§10).
import Foundation
import WebKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

enum GuestScheme {
    /// Schemes that never open another app: web pages belong in the frame,
    /// and the rest read local files, run script or are the app's own (the
    /// reference's `NEVER_EXTERNAL_PROTOCOLS`, `BrowserSession.ts`, with
    /// `exact-app`).
    static let never: Set<String> = ["about", "blob", "chrome", "data", "devtools", "file", "filesystem", "http", "https",
                                     "javascript", "view-source", "exact-app"]

    static func web(_ url: URL?) -> Bool { ["http", "https"].contains(url?.scheme?.lowercased() ?? "") }

    /// Another app's URL (`mailto:`, `slack:`), which only the platform's
    /// confirmation opens.
    static func external(_ url: URL?) -> URL? {
        guard let url, let scheme = url.scheme?.lowercased(), !scheme.isEmpty, !never.contains(scheme) else { return nil }
        return url
    }

    /// What a window request or a link click asks for, by Chromium's
    /// `WindowOpenDisposition` as Electron names it: a scripted window with
    /// features (or a ⇧-click) is a `new-window`, a ⌘- or middle-click a
    /// `background-tab` (⌘⇧ a `foreground-tab`), anything else a
    /// `foreground-tab`.
    static func disposition(_ action: WKNavigationAction, features: WKWindowFeatures?) -> String {
        if action.navigationType != .linkActivated {
            let shaped = features.map { $0.width != nil || $0.height != nil || $0.x != nil || $0.y != nil
                || $0.toolbarsVisibility?.boolValue == false || $0.menuBarVisibility?.boolValue == false } ?? false
            return shaped ? "new-window" : "foreground-tab"
        }
        let (command, shift, middle) = keys(action)
        if command && shift { return "foreground-tab" }
        if command || middle { return "background-tab" }
        return shift ? "new-window" : "foreground-tab"
    }

    /// The tab a ⌘- or middle-click on a link with no target asks for; nil
    /// to follow it in the frame.
    static func linkTab(_ action: WKNavigationAction) -> String? {
        guard action.navigationType == .linkActivated else { return nil }
        let (command, _, middle) = keys(action)
        return command || middle ? disposition(action, features: nil) : nil
    }

    static func keys(_ action: WKNavigationAction) -> (command: Bool, shift: Bool, middle: Bool) {
        #if os(macOS)
        // `buttonNumber` is AppKit's button number, 2 the middle one; a pressed-buttons mask has it as 1 << 2.
        return (action.modifierFlags.contains(.command), action.modifierFlags.contains(.shift), action.buttonNumber == 2 || action.buttonNumber == 4)
        #else
        if #available(iOS 18.4, *) { return (action.modifierFlags.contains(.command), action.modifierFlags.contains(.shift), false) }
        return (false, false, false)
        #endif
    }

    /// How a dialog names who is speaking: the guest's host and port, so a
    /// person can tell the page's words from the app's.
    static func speaker(_ frame: WKFrameInfo?) -> String {
        guard let origin = frame?.securityOrigin, !origin.host.isEmpty else {
            return frame?.request.url?.scheme.map { "\($0):" } ?? "This page"
        }
        let port = origin.port == 0 || (origin.protocol == "http" && origin.port == 80) || (origin.protocol == "https" && origin.port == 443) ? "" : ":\(origin.port)"
        return origin.host + port
    }
}

/// The arm's `WKUIDelegate` and its policy for what leaves a frame. It holds
/// the frame's popups and downloads; the arm holds it.
class FrameUI: NSObject, WKUIDelegate {
    enum Policy { case proceed, cancel, download }

    /// What a person decided about a page's dialogs and other-app prompts,
    /// per web view (the frame's or a popup's), by the origin that asked. A
    /// reload or a page of the same origin keeps it, so a page cannot start
    /// over by reloading; it is cleared when the main document's origin
    /// changes or the app navigates the frame (`src`, `navigate`).
    struct Asked { var dialogs = 0; var recent: [Date] = []; var suppressed = false; var schemesRefused = false }
    struct PageState { var origin = ""; var asked: [String: Asked] = [:] }

    weak var arm: WebArm?
    var popups: [FramePopup] = []
    let downloads = FrameDownloads()
    /// A confirmation for another app's scheme is up: a second one is
    /// refused rather than stacked behind it.
    var prompting = false
    var pages: [ObjectIdentifier: PageState] = [:]
    /// The sheets and panels up now, each with how it ends as Cancel when
    /// its frame or popup goes first.
    private struct Shown { let id: Int; weak var web: WKWebView?; let end: () -> Void }
    private var shown: [Shown] = []
    private var nextShown = 0

    init(arm: WebArm) {
        self.arm = arm
        super.init()
        downloads.ui = self
    }

    func log(_ message: String) {
        FileHandle.standardError.write(Data("exact: iframe \(arm?.id ?? 0): \(message)\n".utf8))
    }

    func close() {
        endSheets()
        let open = popups
        popups = []
        open.forEach { $0.close() }
    }

    /// An origin as a key: scheme, host and port.
    static func origin(_ o: WKSecurityOrigin?) -> String {
        guard let o else { return "" }
        return "\(o.protocol)://\(o.host):\(o.port)"
    }

    static func origin(_ url: URL?) -> String {
        guard let url, let scheme = url.scheme?.lowercased() else { return "" }
        let port = url.port ?? (scheme == "https" ? 443 : scheme == "http" ? 80 : 0)
        return "\(scheme)://\(url.host?.lowercased() ?? ""):\(port == 80 && scheme == "http" || port == 443 && scheme == "https" ? 0 : port)"
    }

    /// What was decided for `origin`'s requests in `web`.
    func asked(_ web: WKWebView, _ origin: String) -> Asked { pages[ObjectIdentifier(web)]?.asked[origin] ?? Asked() }

    func record(_ web: WKWebView, _ origin: String, _ change: (inout Asked) -> Void) {
        change(&pages[ObjectIdentifier(web), default: PageState()].asked[origin, default: Asked()])
    }

    /// A web view committed a main-frame document: a new origin starts over.
    func committed(_ web: WKWebView) {
        let origin = Self.origin(web.url)
        guard pages[ObjectIdentifier(web)]?.origin != origin else { return }
        pages[ObjectIdentifier(web)] = PageState(origin: origin)
    }

    /// The app moved the frame (`src`, `navigate`), or the web view went:
    /// what a person decided about the old page no longer holds.
    func forget(_ web: WKWebView?) {
        if let web { pages[ObjectIdentifier(web)] = nil }
    }

    /// A sheet or panel went up on `web`; `end` dismisses it as Cancel.
    func showing(_ web: WKWebView, end: @escaping () -> Void) -> Int {
        nextShown += 1
        shown.append(Shown(id: nextShown, web: web, end: end))
        return nextShown
    }

    func ended(_ id: Int) { shown.removeAll { $0.id == id } }

    /// Ends, as Cancel, the sheets and panels up on `web`, or all of them.
    func endSheets(for web: WKWebView? = nil) {
        var kept: [Shown] = [], ending: [Shown] = []
        for sheet in shown {
            if web == nil || sheet.web == nil || sheet.web === web { ending.append(sheet) } else { kept.append(sheet) }
        }
        shown = kept
        for sheet in ending { sheet.end() }
    }

    /// `done` once: a sheet ended from outside and its own button cannot both answer.
    static func once<T>(_ done: @escaping (T) -> Void) -> (T) -> Void {
        var answered = false
        return { value in
            guard !answered else { return }
            answered = true
            done(value)
        }
    }

    // MARK: Policy for a navigation

    /// What a navigation in the frame does before WebKit loads it: another
    /// app's scheme asks first, a ⌘- or middle-click is a tab, and `<a
    /// download>` downloads (@ref LLP 1020.002 D5).
    func intercept(_ action: WKNavigationAction, in web: WKWebView) -> Policy {
        let url = action.request.url
        let main = action.targetFrame?.isMainFrame != false
        if let external = GuestScheme.external(url), main || action.navigationType == .linkActivated {
            // HTML lets a sandboxed frame open another app's scheme only with
            // one of these tokens (the `allow-top-navigation-to-custom-protocols` note).
            let tokens = Set((arm?.sandbox ?? "").split(whereSeparator: \.isWhitespace).map(String.init))
            if arm?.sandbox != nil, tokens.isDisjoint(with: ["allow-popups", "allow-top-navigation", "allow-top-navigation-by-user-activation",
                                                              "allow-top-navigation-to-custom-protocols"]) {
                log("\(external.scheme ?? ""): refused: the frame's sandbox does not allow another app's scheme")
                return .cancel
            }
            otherApp(external, action, from: web)
            return .cancel
        }
        if action.targetFrame != nil, let disposition = GuestScheme.linkTab(action), GuestScheme.web(url), let arm {
            guard arm.handlesNewWindow else {
                log("a \(disposition) for \(url!.absoluteString) is refused: the frame has no newwindow=")
                return .cancel
            }
            arm.emitRecord("newwindow", ["url": url!.absoluteString, "disposition": disposition])
            return .cancel
        }
        if action.shouldPerformDownload { return .download }
        return .proceed
    }

    /// A response WebKit cannot show, or one sent as an attachment, is a
    /// download, for web content only: the app's own `app:/` and bundled
    /// files keep showing as they did.
    static func downloads(_ response: WKNavigationResponse) -> Bool {
        guard ["http", "https", "blob", "data"].contains(response.response.url?.scheme?.lowercased() ?? "") else { return false }
        let disposition = (response.response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Disposition")?.lowercased() ?? ""
        return !response.canShowMIMEType || disposition.hasPrefix("attachment")
    }

    // MARK: New windows

    /// D5's popup table, rows in order; the first that matches decides. A
    /// sandbox without `allow-popups`, and a scripted window with no
    /// person's gesture (`javaScriptCanOpenWindowsAutomatically` is false),
    /// never reach this.
    func webView(_ web: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction,
                 windowFeatures: WKWindowFeatures) -> WKWebView? {
        guard let arm, !arm.invalidated, web === arm.built else { return nil }
        let url = action.request.url, raw = url?.absoluteString ?? ""
        if raw.isEmpty || raw == "about:blank" {
            log("a window for about:blank is refused")
            return nil
        }
        guard GuestScheme.web(url) else {
            if let external = GuestScheme.external(url) { confirmExternal(external, from: web, asker: Self.origin(action.sourceFrame.securityOrigin)) } else { log("a window for \(url?.scheme ?? "this"): is refused") }
            return nil
        }
        let disposition = GuestScheme.disposition(action, features: windowFeatures)
        if disposition == "new-window" {
            guard arm.handlesNewWindow else { log("a new-window for \(raw) is refused: the frame has no newwindow="); return nil }
            let popup = FramePopup(configuration: configuration, features: windowFeatures, opener: web, ui: self)
            popups.append(popup)
            arm.emitRecord("newwindow", ["url": raw, "disposition": disposition])
            return popup.web
        }
        if action.request.httpMethod?.uppercased() == "POST" {
            // A new tab could only ask again with GET: the form loads here.
            guard arm.direct else { log("a POST to a new tab is refused: this frame's guest is inside a wrapper document"); return nil }
            web.load(action.request)
            return nil
        }
        guard arm.handlesNewWindow else { log("a \(disposition) for \(raw) is refused: the frame has no newwindow="); return nil }
        arm.emitRecord("newwindow", ["url": raw, "disposition": disposition])
        return nil
    }

    func popupClosed(_ popup: FramePopup) { popups.removeAll { $0 === popup } }

    // MARK: Dialogs

    func webView(_ web: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping () -> Void) {
        dialog(web, frame, type: "alert", message, defaultText: nil) { _ in completionHandler() }
    }

    func webView(_ web: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping (Bool) -> Void) {
        dialog(web, frame, type: "confirm", message, defaultText: nil) { completionHandler($0 != nil) }
    }

    func webView(_ web: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String, defaultText: String?, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping (String?) -> Void) {
        dialog(web, frame, type: "prompt", prompt, defaultText: defaultText ?? "") { completionHandler($0) }
    }

    /// `alert`, `confirm` or `prompt`: the platform's own, titled with the
    /// guest's origin. `done` gets nil for Cancel, else the text (`""`
    /// for an alert's or a confirm's OK).
    func dialog(_ web: WKWebView, _ frame: WKFrameInfo, type: String, _ message: String, defaultText: String?, done: @escaping (String?) -> Void) {
        let speaker = GuestScheme.speaker(frame)
        if let arm, arm.agent {
            var args: [String: Any] = ["type": type, "message": message, "origin": speaker]
            if let defaultText { args["defaultText"] = defaultText }
            return arm.ask("dialog", args, from: web) { answer in
                guard let answer else { return done(nil) }
                done(answer["text"] as? String ?? defaultText ?? "")
            }
        }
        let origin = Self.origin(frame.securityOrigin)
        guard !asked(web, origin).suppressed else { return done(nil) }
        #if os(iOS)
        // iOS offers no suppression box: past three dialogs in ten seconds
        // from one origin, the rest answer Cancel unshown, so a loop cannot
        // hold the app.
        let now = Date()
        let recent = asked(web, origin).recent.filter { now.timeIntervalSince($0) < 10 }
        guard recent.count < 3 else {
            log("\(type) answered as Cancel unshown: more than three dialogs from \(speaker) in ten seconds")
            return done(nil)
        }
        record(web, origin) { $0.recent = recent + [now] }
        #endif
        record(web, origin) { $0.dialogs += 1 }
        #if os(macOS)
        guard let window = web.window else { log("\(type) answered as Cancel: the frame is in no window"); return done(nil) }
        let alert = NSAlert()
        alert.messageText = speaker
        alert.informativeText = message
        alert.addButton(withTitle: "OK")
        // From a page's second dialog on, a person can stop the rest, as
        // Safari and Chrome let one: a page cannot hold the window with a loop.
        if asked(web, origin).dialogs > 1 {
            alert.showsSuppressionButton = true
            alert.suppressionButton?.title = "Prevent this page from creating additional dialogs"
        }
        let field = defaultText.map { text -> NSTextField in
            let field = NSTextField(string: text)
            field.frame = NSRect(x: 0, y: 0, width: 260, height: 22)
            alert.accessoryView = field
            alert.window.initialFirstResponder = field
            return field
        }
        if type != "alert" { alert.addButton(withTitle: "Cancel") }
        let id = showing(web) { [weak window, weak alert] in if let alert { window?.endSheet(alert.window, returnCode: .cancel) } }
        alert.beginSheetModal(for: window) { [weak self] response in
            self?.ended(id)
            if alert.suppressionButton?.state == .on { self?.record(web, origin) { $0.suppressed = true } }
            done(response == .alertFirstButtonReturn ? field?.stringValue ?? "" : nil)
        }
        #else
        guard let presenter = FrameUI.presenter(web) else { log("\(type) answered as Cancel: the frame is in no window"); return done(nil) }
        let alert = UIAlertController(title: speaker, message: message, preferredStyle: .alert)
        var id = 0
        let answer = FrameUI.once { [weak self] (text: String?) in self?.ended(id); done(text) }
        if let defaultText { alert.addTextField { $0.text = defaultText } }
        if type != "alert" { alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in answer(nil) }) }
        alert.addAction(UIAlertAction(title: "OK", style: .default) { [weak alert] _ in answer(alert?.textFields?.first?.text ?? "") })
        id = showing(web) { [weak alert] in alert?.dismiss(animated: false); answer(nil) }
        presenter.present(alert, animated: true)
        #endif
    }

    #if os(iOS)
    /// The view controller to present over: the frame window's topmost.
    static func presenter(_ web: WKWebView) -> UIViewController? {
        var top = web.window?.rootViewController
        while let next = top?.presentedViewController, !next.isBeingDismissed { top = next }
        return top
    }
    #endif

    // MARK: File inputs

    #if os(macOS)
    func webView(_ web: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping ([URL]?) -> Void) {
        openPanel(web, parameters, completionHandler)
    }
    #endif

    /// A file input's chooser: the platform's open panel, or the agent's
    /// `open-file` request.
    @available(macOS 10.12, iOS 18.4, *)
    func openPanel(_ web: WKWebView, _ parameters: WKOpenPanelParameters, _ done: @escaping ([URL]?) -> Void) {
        if let arm, arm.agent {
            return arm.ask("open-file", ["multiple": parameters.allowsMultipleSelection, "directories": parameters.allowsDirectories], from: web) { answer in
                let paths = answer?["paths"] as? [String] ?? []
                done(paths.isEmpty ? nil : paths.map { URL(fileURLWithPath: $0) })
            }
        }
        #if os(macOS)
        guard let window = web.window else { log("a file input's panel answered as Cancel: the frame is in no window"); return done(nil) }
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = parameters.allowsMultipleSelection
        panel.canChooseDirectories = parameters.allowsDirectories
        panel.canChooseFiles = true
        let id = showing(web) { [weak window, weak panel] in if let panel { window?.endSheet(panel, returnCode: .cancel) } }
        panel.beginSheetModal(for: window) { [weak self] response in
            self?.ended(id)
            done(response == .OK ? panel.urls : nil)
        }
        #else
        done(nil)
        #endif
    }

    // MARK: Permissions, denied (§12 Q8)

    func webView(_ web: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 type: WKMediaCaptureType, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        log("\(GuestScheme.speaker(frame)) asked for the \(type == .camera ? "camera" : type == .microphone ? "microphone" : "camera and microphone"): denied")
        decisionHandler(.deny)
    }

    @available(macOS 27.0, iOS 27.0, *)
    func webView(_ web: WKWebView, requestGeolocationPermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        log("\(GuestScheme.speaker(frame)) asked for its location: denied")
        decisionHandler(.deny)
    }

    #if os(iOS)
    func webView(_ web: WKWebView, requestDeviceOrientationAndMotionPermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        log("\(GuestScheme.speaker(frame)) asked for device motion and orientation: denied")
        decisionHandler(.deny)
    }
    #endif

    // MARK: Another app's scheme

    /// A navigation to another app's scheme asks only when a person followed
    /// a link or sent a form: a script's `location` on a timer would raise
    /// the confirmation again and again.
    func otherApp(_ url: URL, _ action: WKNavigationAction, from web: WKWebView) {
        guard action.navigationType == .linkActivated || action.navigationType == .formSubmitted else {
            return log("\(url.scheme ?? ""): refused: only a link a person follows asks to open another app")
        }
        confirmExternal(url, from: web, asker: Self.origin(action.sourceFrame.securityOrigin))
    }

    /// The platform's confirmation, then the app that opens the URL (Safari's
    /// prompt). Under the agent no app opens: it answers Cancel and says so.
    /// `byApp`: the app's own `navigate`, which a page's refusals never stop.
    /// `asker`: the origin of the frame that asked, whose Cancel refuses its
    /// further asks.
    func confirmExternal(_ url: URL, from web: WKWebView, byApp: Bool = false, asker: String = "") {
        let scheme = url.scheme ?? ""
        guard !prompting else { return log("\(scheme): refused: a confirmation is already up") }
        guard byApp || !asked(web, asker).schemesRefused else { return log("\(scheme): refused: the person cancelled one from this page") }
        guard let arm, !arm.agent else { return log("\(scheme): would ask to open another app; the agent answers Cancel") }
        let shown = url.absoluteString.count > 300 ? "\(url.absoluteString.prefix(300))…" : url.absoluteString
        #if os(macOS)
        guard let app = NSWorkspace.shared.urlForApplication(toOpen: url) else { return log("\(scheme): refused: no app opens it") }
        guard let window = web.window else { return log("\(scheme): refused: the frame is in no window") }
        let name = FileManager.default.displayName(atPath: app.path).replacingOccurrences(of: ".app", with: "")
        let alert = NSAlert()
        alert.messageText = "Do you want to allow this page to open “\(name)”?"
        alert.informativeText = shown
        alert.addButton(withTitle: "Allow")
        alert.addButton(withTitle: "Cancel")
        prompting = true
        let id = showing(web) { [weak window, weak alert] in if let alert { window?.endSheet(alert.window, returnCode: .cancel) } }
        alert.beginSheetModal(for: window) { [weak self] response in
            self?.ended(id)
            self?.prompting = false
            if response == .alertFirstButtonReturn { _ = NSWorkspace.shared.open(url); return }
            if !byApp { self?.record(web, asker) { $0.schemesRefused = true } }
            self?.log("\(scheme): the person answered Cancel")
        }
        #else
        guard let presenter = FrameUI.presenter(web) else { return log("\(scheme): refused: the frame is in no window") }
        let alert = UIAlertController(title: "Open this page in another app?", message: shown, preferredStyle: .alert)
        var id = 0
        let answer = FrameUI.once { [weak self] (open: Bool) in
            guard let self else { return }
            self.ended(id)
            self.prompting = false
            guard open else {
                if !byApp { self.record(web, asker) { $0.schemesRefused = true } }
                return self.log("\(scheme): the person answered Cancel")
            }
            UIApplication.shared.open(url) { [weak self] opened in if !opened { self?.log("\(scheme): no app opened it") } }
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in answer(false) })
        alert.addAction(UIAlertAction(title: "Open", style: .default) { _ in answer(true) })
        prompting = true
        id = showing(web) { [weak alert] in alert?.dismiss(animated: false); answer(false) }
        presenter.present(alert, animated: true)
        #endif
    }
}

#if os(iOS)
/// Under the agent on iOS, a file input is the drive's `open-file`
/// request. Without this delegate method WebKit shows its own picker, as
/// Safari does (`WKUIDelegate.h`), which a person outside the agent gets.
final class FrameUIAgent: FrameUI {
    @available(iOS 18.4, *)
    @objc(webView:runOpenPanelWithParameters:initiatedByFrame:completionHandler:)
    func webView(_ web: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping ([URL]?) -> Void) {
        openPanel(web, parameters, completionHandler)
    }
}
#endif

/// A scripted window (`new-window`): the web view WebKit configured, in a
/// window (macOS) or a sheet (iOS). It shares its opener's partition and
/// cookies, so `window.opener` and its `postMessage` work (OAuth); it gets
/// none of its opener's scripts and no app asset; its own popups are
/// refused, one level as the reference has it; it closes on `window.close()`.
final class FramePopup: NSObject, WKUIDelegate, WKNavigationDelegate {
    let web: WKWebView
    weak var ui: FrameUI?
    private var title: NSKeyValueObservation?
    private var finished = false
    #if os(iOS)
    private var agentPanel: FramePopupAgentPanel?
    #endif
    #if os(macOS)
    let window: NSWindow
    #else
    let controller: UINavigationController
    #endif

    init(configuration: WKWebViewConfiguration, features: WKWindowFeatures, opener: WKWebView, ui: FrameUI) {
        // WebKit requires the given configuration (`WKUIDelegate.h`); its
        // content controller is replaced before the view is made, so the
        // popup's own `window.postMessage` reaches its own listeners.
        configuration.userContentController = WKUserContentController()
        let width = CGFloat(features.width?.doubleValue ?? 800), height = CGFloat(features.height?.doubleValue ?? 600)
        web = WKWebView(frame: CGRect(x: 0, y: 0, width: width, height: height), configuration: configuration)
        self.ui = ui
        #if os(macOS)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: width, height: height), styleMask: [.titled, .closable, .resizable, .miniaturizable],
                          backing: .buffered, defer: false)
        #else
        let content = UIViewController()
        content.view = web
        controller = UINavigationController(rootViewController: content)
        #endif
        super.init()
        web.uiDelegate = self
        #if os(iOS)
        if ui.arm?.agent == true {
            agentPanel = FramePopupAgentPanel(self)
            web.uiDelegate = agentPanel
        }
        #endif
        web.navigationDelegate = self
        #if EXACT_DEVELOPMENT
        web.isInspectable = true
        #endif
        #if os(macOS)
        window.isReleasedWhenClosed = false
        window.contentView = web
        NotificationCenter.default.addObserver(self, selector: #selector(windowWillClose), name: NSWindow.willCloseNotification, object: window)
        title = web.observe(\.title, options: [.initial, .new]) { [weak self] web, _ in self?.window.title = web.title ?? "" }
        if let parent = opener.window {
            let frame = window.frameRect(forContentRect: NSRect(x: 0, y: 0, width: width, height: height))
            if let x = features.x?.doubleValue, let y = features.y?.doubleValue, let screen = parent.screen ?? NSScreen.main {
                // A window feature's y is from the screen's top; AppKit's from its bottom.
                window.setFrameOrigin(NSPoint(x: screen.frame.minX + x, y: screen.frame.maxY - y - frame.height))
            } else {
                window.setFrameOrigin(NSPoint(x: parent.frame.midX - frame.width / 2, y: parent.frame.midY - frame.height / 2))
            }
        } else {
            window.center()
        }
        window.makeKeyAndOrderFront(nil)
        #else
        let shown = controller.viewControllers[0]
        shown.navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in self?.close() })
        title = web.observe(\.title, options: [.initial, .new]) { [weak shown] web, _ in shown?.title = web.title }
        controller.modalPresentationStyle = .pageSheet
        controller.presentationController?.delegate = self
        FrameUI.presenter(opener)?.present(controller, animated: true)
        #endif
    }

    func close() {
        #if os(macOS)
        // A sheet on the popup ends as Cancel before its window goes.
        ui?.endSheets(for: web)
        window.close()
        #else
        controller.presentingViewController?.dismiss(animated: true)
        finish()
        #endif
    }

    #if os(macOS)
    @objc private func windowWillClose(_ note: Notification) { finish() }
    #endif

    private func finish() {
        guard !finished else { return }
        finished = true
        ui?.endSheets(for: web)
        ui?.arm?.endAsked(for: web)
        ui?.forget(web)
        title = nil
        web.stopLoading()
        web.uiDelegate = nil
        web.navigationDelegate = nil
        ui?.popupClosed(self)
    }

    func webView(_ web: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction,
                 windowFeatures: WKWindowFeatures) -> WKWebView? {
        ui?.log("a popup's own window for \(action.request.url?.absoluteString ?? "") is refused")
        return nil
    }

    func webViewDidClose(_ web: WKWebView) { close() }

    /// A popup shows web pages: another app's scheme asks first, as in its
    /// opener, and the app's `exact-app` origin, a file or any other scheme
    /// is refused in its main frame.
    func webView(_ web: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let url = action.request.url
        let main = action.targetFrame?.isMainFrame != false
        if let external = GuestScheme.external(url), main || action.navigationType == .linkActivated {
            ui?.otherApp(external, action, from: web)
            return decisionHandler(.cancel)
        }
        if main, !["http", "https", "about", "data", "blob"].contains(url?.scheme?.lowercased() ?? "") {
            ui?.log("a popup's navigation to \(url?.scheme ?? "this"): is refused")
            return decisionHandler(.cancel)
        }
        decisionHandler(action.shouldPerformDownload ? .download : .allow)
    }

    func webView(_ web: WKWebView, didCommit navigation: WKNavigation!) { ui?.committed(web) }

    func webView(_ web: WKWebView, decidePolicyFor response: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        decisionHandler(FrameUI.downloads(response) ? .download : .allow)
    }

    func webView(_ web: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) { ui?.downloads.adopt(download, from: web) }
    func webView(_ web: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) { ui?.downloads.adopt(download, from: web) }

    // Dialogs, file inputs and permissions are answered as the opener's.
    func webView(_ web: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping () -> Void) {
        guard let ui else { return completionHandler() }
        ui.dialog(web, frame, type: "alert", message, defaultText: nil) { _ in completionHandler() }
    }

    func webView(_ web: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping (Bool) -> Void) {
        guard let ui else { return completionHandler(false) }
        ui.dialog(web, frame, type: "confirm", message, defaultText: nil) { completionHandler($0 != nil) }
    }

    func webView(_ web: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String, defaultText: String?, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping (String?) -> Void) {
        guard let ui else { return completionHandler(nil) }
        ui.dialog(web, frame, type: "prompt", prompt, defaultText: defaultText ?? "", done: completionHandler)
    }

    #if os(macOS)
    func webView(_ web: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping ([URL]?) -> Void) {
        guard let ui else { return completionHandler(nil) }
        ui.openPanel(web, parameters, completionHandler)
    }
    #endif

    func webView(_ web: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 type: WKMediaCaptureType, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        decisionHandler(.deny)
    }

    @available(macOS 27.0, iOS 27.0, *)
    func webView(_ web: WKWebView, requestGeolocationPermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        decisionHandler(.deny)
    }

    #if os(iOS)
    func webView(_ web: WKWebView, requestDeviceOrientationAndMotionPermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo,
                 decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        decisionHandler(.deny)
    }
    #endif
}

#if os(iOS)
/// A popup's UI delegate under the agent on iOS: its file input is the
/// drive's `open-file` request, as its opener's is (`FrameUIAgent`). It adds
/// that one method and hands every other to the popup.
final class FramePopupAgentPanel: NSObject, WKUIDelegate {
    unowned let popup: FramePopup
    init(_ popup: FramePopup) { self.popup = popup }
    override func responds(to selector: Selector!) -> Bool { super.responds(to: selector) || popup.responds(to: selector) }
    override func forwardingTarget(for selector: Selector!) -> Any? { popup.responds(to: selector) ? popup : nil }

    @available(iOS 18.4, *)
    func webView(_ web: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters, initiatedByFrame frame: WKFrameInfo,
                 completionHandler: @escaping ([URL]?) -> Void) {
        guard let ui = popup.ui else { return completionHandler(nil) }
        ui.openPanel(web, parameters, completionHandler)
    }
}

extension FramePopup: UIAdaptivePresentationControllerDelegate {
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { finish() }
}
#endif

/// The frame's downloads: unsaid, the platform asks where (an `NSSavePanel`
/// sheet on macOS; on iOS the export picker once the file is in), and the
/// frame fires `download` as each starts and ends, with Electron's
/// `DownloadItem` states.
final class FrameDownloads: NSObject, WKDownloadDelegate {
    /// `temp` is where WebKit writes, beside `path`, until the download is
    /// in and is renamed over it.
    struct Item { var url: String; var name = ""; var path: URL?; var temp: URL?; var shown = ""; weak var web: WKWebView? }
    weak var ui: FrameUI?
    var items: [ObjectIdentifier: Item] = [:]
    #if os(iOS)
    /// The export picker's downloads, by picker.
    var exporting: [ObjectIdentifier: ObjectIdentifier] = [:]
    var exportSheets: [ObjectIdentifier: Int] = [:]
    #endif

    func adopt(_ download: WKDownload, from web: WKWebView) {
        download.delegate = self
        items[ObjectIdentifier(download)] = Item(url: download.originalRequest?.url?.absoluteString ?? "", web: web)
    }

    func report(_ item: Item, _ state: String, path: String = "") {
        ui?.arm?.emitRecord("download", ["url": item.url, "suggestedFilename": item.name, "state": state, "path": path])
    }

    func download(_ download: WKDownload, decideDestinationUsing response: URLResponse, suggestedFilename: String,
                  completionHandler: @escaping (URL?) -> Void) {
        let key = ObjectIdentifier(download)
        guard var item = items[key] else { return completionHandler(nil) }
        let base = (suggestedFilename as NSString).lastPathComponent
        item.name = base.isEmpty || base == "." || base == ".." ? "download" : base
        if item.url.isEmpty { item.url = response.url?.absoluteString ?? "" }
        items[key] = item
        report(item, "progressing")
        let accept: (URL?, String?) -> Void = { [weak self] target, shown in
            guard let self, var item = self.items[key] else { return completionHandler(nil) }
            var isFolder: ObjCBool = false
            let exists = target.map { FileManager.default.fileExists(atPath: $0.path, isDirectory: &isFolder) } ?? false
            // A folder is never a download's destination, nor replaced by one:
            // an agent's typed `/Users/me/Downloads` must not delete it.
            let folder = (exists && isFolder.boolValue) || shown?.hasSuffix("/") == true
            if let target, folder { self.ui?.log("a download of \(item.name) is cancelled: \(target.path) is a folder, not a file") }
            guard let target, !folder else {
                self.items[key] = nil
                self.report(item, "cancelled")
                return completionHandler(nil)
            }
            // WebKit writes beside the place and the finished file is renamed
            // over it (`rename(2)`, atomic): a file there, chosen knowing it is
            // there (the save panel asked; the agent named it), is replaced only
            // by a whole download, and kept when the download fails.
            let temp = target.deletingLastPathComponent().appendingPathComponent(".\(target.lastPathComponent).\(UUID().uuidString).download")
            try? FileManager.default.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
            item.path = target
            item.temp = temp
            item.shown = shown ?? target.path
            self.items[key] = item
            completionHandler(temp)
        }
        if let arm = ui?.arm, arm.agent {
            return arm.ask("save-file", ["suggestedFilename": item.name], from: item.web) { answer in
                accept((answer?["path"] as? String).map { URL(fileURLWithPath: $0) }, answer?["shown"] as? String)
            }
        }
        #if os(macOS)
        guard let web = item.web, let window = web.window else { ui?.log("a download of \(item.name) is cancelled: the frame is in no window"); return accept(nil, nil) }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = item.name
        panel.directoryURL = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        let id = ui?.showing(web) { [weak window, weak panel] in if let panel { window?.endSheet(panel, returnCode: .cancel) } }
        panel.beginSheetModal(for: window) { [weak self] response in
            if let id { self?.ui?.ended(id) }
            accept(response == .OK ? panel.url : nil, nil)
        }
        #else
        // The file comes in first, then the person chooses where it goes.
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("exact-downloads/\(UUID().uuidString)", isDirectory: true)
        accept(folder.appendingPathComponent(item.name), nil)
        #endif
    }

    func downloadDidFinish(_ download: WKDownload) {
        let key = ObjectIdentifier(download)
        guard let item = items[key] else { return }
        if let temp = item.temp, let path = item.path, rename(temp.path, path.path) != 0 {
            // A folder put there meanwhile fails with EISDIR, and stays.
            let reason = String(cString: strerror(errno))
            try? FileManager.default.removeItem(at: temp)
            items[key] = nil
            ui?.log("a download of \(item.name) was interrupted: it could not be put at \(path.path): \(reason)")
            return report(item, "interrupted")
        }
        #if os(iOS)
        if !(ui?.arm?.agent ?? true), let path = item.path, let web = item.web, let presenter = FrameUI.presenter(web) {
            let picker = UIDocumentPickerViewController(forExporting: [path], asCopy: false)
            picker.delegate = self
            exporting[ObjectIdentifier(picker)] = key
            exportSheets[ObjectIdentifier(picker)] = ui?.showing(web) { [weak self, weak picker] in
                guard let self, let picker else { return }
                picker.dismiss(animated: false)
                self.documentPickerWasCancelled(picker)
            }
            presenter.present(picker, animated: true)
            return
        }
        #endif
        items[key] = nil
        report(item, "completed", path: item.shown)
    }

    func download(_ download: WKDownload, didFailWithError error: Error, resumeData: Data?) {
        guard let item = items.removeValue(forKey: ObjectIdentifier(download)) else { return }
        if let temp = item.temp { try? FileManager.default.removeItem(at: temp) }
        ui?.log("a download of \(item.name) was interrupted: \((error as NSError).localizedDescription)")
        report(item, "interrupted")
    }
}

#if os(iOS)
extension FrameDownloads: UIDocumentPickerDelegate {
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        if let id = exportSheets.removeValue(forKey: ObjectIdentifier(controller)) { ui?.ended(id) }
        guard let key = exporting.removeValue(forKey: ObjectIdentifier(controller)), let item = items.removeValue(forKey: key) else { return }
        report(item, "completed", path: urls.first?.path ?? "")
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        if let id = exportSheets.removeValue(forKey: ObjectIdentifier(controller)) { ui?.ended(id) }
        guard let key = exporting.removeValue(forKey: ObjectIdentifier(controller)), let item = items.removeValue(forKey: key) else { return }
        if let path = item.path { try? FileManager.default.removeItem(at: path.deletingLastPathComponent()) }
        report(item, "cancelled")
    }
}
#endif
