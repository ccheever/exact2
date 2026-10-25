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

    weak var arm: WebArm?
    var popups: [FramePopup] = []
    let downloads = FrameDownloads()
    /// A confirmation for another app's scheme is up: one at a time, so a
    /// page cannot stack them by looping a deep link.
    var prompting = false

    init(arm: WebArm) {
        self.arm = arm
        super.init()
        downloads.ui = self
    }

    func log(_ message: String) {
        FileHandle.standardError.write(Data("exact: iframe \(arm?.id ?? 0): \(message)\n".utf8))
    }

    func close() {
        let open = popups
        popups = []
        open.forEach { $0.close() }
    }

    // MARK: Policy for a navigation

    /// What a navigation in the frame does before WebKit loads it: another
    /// app's scheme asks first, a ⌘- or middle-click is a tab, and `<a
    /// download>` downloads (@ref LLP 1020.002 D5).
    func intercept(_ action: WKNavigationAction, in web: WKWebView) -> Policy {
        let url = action.request.url
        let main = action.targetFrame?.isMainFrame != false
        if let external = GuestScheme.external(url), main || action.navigationType == .linkActivated {
            confirmExternal(external, from: web)
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
            if let external = GuestScheme.external(url) { confirmExternal(external, from: web) } else { log("a window for \(url?.scheme ?? "this"): is refused") }
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
            return arm.ask("dialog", args) { answer in
                guard let answer else { return done(nil) }
                done(answer["text"] as? String ?? defaultText ?? "")
            }
        }
        #if os(macOS)
        guard let window = web.window else { log("\(type) answered as Cancel: the frame is in no window"); return done(nil) }
        let alert = NSAlert()
        alert.messageText = speaker
        alert.informativeText = message
        alert.addButton(withTitle: "OK")
        let field = defaultText.map { text -> NSTextField in
            let field = NSTextField(string: text)
            field.frame = NSRect(x: 0, y: 0, width: 260, height: 22)
            alert.accessoryView = field
            alert.window.initialFirstResponder = field
            return field
        }
        if type != "alert" { alert.addButton(withTitle: "Cancel") }
        alert.beginSheetModal(for: window) { response in
            done(response == .alertFirstButtonReturn ? field?.stringValue ?? "" : nil)
        }
        #else
        guard let presenter = FrameUI.presenter(web) else { log("\(type) answered as Cancel: the frame is in no window"); return done(nil) }
        let alert = UIAlertController(title: speaker, message: message, preferredStyle: .alert)
        if let defaultText { alert.addTextField { $0.text = defaultText } }
        if type != "alert" { alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { _ in done(nil) }) }
        alert.addAction(UIAlertAction(title: "OK", style: .default) { [weak alert] _ in done(alert?.textFields?.first?.text ?? "") })
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
            return arm.ask("open-file", ["multiple": parameters.allowsMultipleSelection, "directories": parameters.allowsDirectories]) { answer in
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
        panel.beginSheetModal(for: window) { done($0 == .OK ? panel.urls : nil) }
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

    /// The platform's confirmation, then the app that opens the URL (Safari's
    /// prompt). Under the agent no app opens: it answers Cancel and says so.
    func confirmExternal(_ url: URL, from web: WKWebView) {
        let scheme = url.scheme ?? ""
        guard !prompting else { return log("\(scheme): refused: a confirmation is already up") }
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
        alert.beginSheetModal(for: window) { [weak self] response in
            self?.prompting = false
            if response == .alertFirstButtonReturn { NSWorkspace.shared.open(url) } else { self?.log("\(scheme): the person answered Cancel") }
        }
        #else
        guard let presenter = FrameUI.presenter(web) else { return log("\(scheme): refused: the frame is in no window") }
        let alert = UIAlertController(title: "Open this page in another app?", message: shown, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel) { [weak self] _ in self?.prompting = false })
        alert.addAction(UIAlertAction(title: "Open", style: .default) { [weak self] _ in
            self?.prompting = false
            UIApplication.shared.open(url) { opened in if !opened { self?.log("\(scheme): no app opened it") } }
        })
        prompting = true
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
            ui?.confirmExternal(external, from: web)
            return decisionHandler(.cancel)
        }
        if main, !["http", "https", "about", "data", "blob"].contains(url?.scheme?.lowercased() ?? "") {
            ui?.log("a popup's navigation to \(url?.scheme ?? "this"): is refused")
            return decisionHandler(.cancel)
        }
        decisionHandler(action.shouldPerformDownload ? .download : .allow)
    }

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
extension FramePopup: UIAdaptivePresentationControllerDelegate {
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) { finish() }
}
#endif

/// The frame's downloads: unsaid, the platform asks where (an `NSSavePanel`
/// sheet on macOS; on iOS the export picker once the file is in), and the
/// frame fires `download` as each starts and ends, with Electron's
/// `DownloadItem` states.
final class FrameDownloads: NSObject, WKDownloadDelegate {
    struct Item { var url: String; var name = ""; var path: URL?; var shown = ""; weak var web: WKWebView? }
    weak var ui: FrameUI?
    var items: [ObjectIdentifier: Item] = [:]
    #if os(iOS)
    /// The export picker's downloads, by picker.
    var exporting: [ObjectIdentifier: ObjectIdentifier] = [:]
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
            guard let target else {
                self.items[key] = nil
                self.report(item, "cancelled")
                return completionHandler(nil)
            }
            // A save panel already confirmed replacing what is there; WebKit refuses a file that exists.
            try? FileManager.default.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: target)
            item.path = target
            item.shown = shown ?? target.path
            self.items[key] = item
            completionHandler(target)
        }
        if let arm = ui?.arm, arm.agent {
            return arm.ask("save-file", ["suggestedFilename": item.name]) { answer in
                accept((answer?["path"] as? String).map { URL(fileURLWithPath: $0) }, answer?["shown"] as? String)
            }
        }
        #if os(macOS)
        guard let window = item.web?.window else { ui?.log("a download of \(item.name) is cancelled: the frame is in no window"); return accept(nil, nil) }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = item.name
        panel.directoryURL = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask).first
        panel.beginSheetModal(for: window) { accept($0 == .OK ? panel.url : nil, nil) }
        #else
        // The file comes in first, then the person chooses where it goes.
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("exact-downloads/\(UUID().uuidString)", isDirectory: true)
        accept(folder.appendingPathComponent(item.name), nil)
        #endif
    }

    func downloadDidFinish(_ download: WKDownload) {
        let key = ObjectIdentifier(download)
        guard let item = items[key] else { return }
        #if os(iOS)
        if !(ui?.arm?.agent ?? true), let path = item.path, let web = item.web, let presenter = FrameUI.presenter(web) {
            let picker = UIDocumentPickerViewController(forExporting: [path], asCopy: false)
            picker.delegate = self
            exporting[ObjectIdentifier(picker)] = key
            presenter.present(picker, animated: true)
            return
        }
        #endif
        items[key] = nil
        report(item, "completed", path: item.shown)
    }

    func download(_ download: WKDownload, didFailWithError error: Error, resumeData: Data?) {
        guard let item = items.removeValue(forKey: ObjectIdentifier(download)) else { return }
        ui?.log("a download of \(item.name) was interrupted: \((error as NSError).localizedDescription)")
        report(item, "interrupted")
    }
}

#if os(iOS)
extension FrameDownloads: UIDocumentPickerDelegate {
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let key = exporting.removeValue(forKey: ObjectIdentifier(controller)), let item = items.removeValue(forKey: key) else { return }
        report(item, "completed", path: urls.first?.path ?? "")
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        guard let key = exporting.removeValue(forKey: ObjectIdentifier(controller)), let item = items.removeValue(forKey: key) else { return }
        if let path = item.path { try? FileManager.default.removeItem(at: path.deletingLastPathComponent()) }
        report(item, "cancelled")
    }
}
#endif
