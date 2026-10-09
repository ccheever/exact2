#if os(macOS)
import AppKit
import WebKit

/// One Browser tab's page (browser-surface part 1; X1 path B, a `WKWebView` in the clone's module, user
/// decision 2026-10-08). MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/desktop/src/preview/Manager.ts (`computeNavStatus`, `failed`, `navigate`, `refresh`, `hardReload`,
/// `setWindowOpenHandler`, `previewWindowOpenAction`), BrowserSession.ts (the permission set, the native user
/// agent), apps/web/src/browser/webviewCrashRecovery.ts and HostedBrowserWebview.tsx (`recoverGuest`).
///
/// - The page state is the reference's: Idle until a page loads (`about:blank` included), Loading while it
///   loads, Success after, LoadFailed(code, description) when the main frame fails (kept until the next load
///   starts). A cancelled load (WebKit's -999, Chromium's -3) and a load a policy stopped (WebKit 102) are not
///   failures. The description is Chromium's error name where WebKit's error has one (`chromiumName`), so the
///   load-failed page words it as the reference does; WebKit's own code stays the code.
/// - Refresh is `reload()` also while loading ("Stop" in the reference only names the button: its
///   `PreviewManager.refresh` reloads); a reload during the first load asks for the pending URL again.
/// - Permissions (BrowserSession.ts ALLOWED_PREVIEW_PERMISSIONS: clipboard-read, clipboard-sanitized-write,
///   notifications, geolocation; the rest denied). WebKit asks the delegate only for the camera and the
///   microphone, which are denied. Clipboard writes need a user gesture and clipboard reads show WebKit's own
///   paste confirmation; WKWebView has no Notification API and asks no delegate for geolocation (this app
///   declares no location use), so those two are unavailable: declared in EXACT2-GAPS.md X1.
/// - Pop-ups (`createWebViewWith`): a scripted window (`window.open` with window features) to an http(s) URL
///   opens a real window that keeps its opener (OAuth sign-ins post back to it), whose own pop-ups are refused;
///   a `target=_blank` link, and any other window request, loads in the tab (`previewWindowOpenAction`).
/// - The page keeps WebKit's native user agent: nothing sets `customUserAgent` or `applicationNameForUserAgent`.
/// - A development build marks the view `isInspectable` (T3WebInspection.swift, #101); a release build never does.
/// - Part 3 (capture): the annotation overlay (T3BrowserAnnotate.swift) and the recording overlay and encoder
///   (T3BrowserRecorder.swift) share the page's configuration; a response the page cannot show, a
///   `Content-Disposition: attachment` and a `download` link become a download (T3BrowserDownloads); `pip` is the
///   separate preview window (T3BrowserCapture.swift).
final class T3BrowserSession: NSObject, WKNavigationDelegate, WKUIDelegate {
    /// The runtime tab id (`previewRuntimeTabId`: environment, thread, server epoch, tab).
    let id: String
    let profile: String
    let environment: String
    let web: T3BrowserWebView
    /// Called when the reported state changes (the registry coalesces it into one `t3.status`).
    var changed: (() -> Void)?
    private(set) var failure: (url: String, code: Int, description: String)?
    /// Failed loads so far: the data module reports each failure to the server once (`buildReportInput`).
    private(set) var failures = 0
    /// Alerts, confirms and the file chooser show; an agent run answers them at once instead.
    var dialogs = true
    /// The URL a load was asked for, while WebKit has not committed it (the reference's pending Loading url).
    private(set) var pending: URL?
    private(set) var crashed = false, crashes = 0
    private var crash = T3BrowserCrashRecovery()
    private var recovery: DispatchWorkItem?
    private var observations: [NSKeyValueObservation] = []
    let favicon: T3BrowserFavicon
    private(set) var popups: [T3BrowserPopup] = []
    /// Every refused navigation and permission request (the agent's status reads them).
    private(set) var refused: [String] = []
    private(set) var closed = false
    /// Part 3: Annotate's overlay, the recording's overlay and encoder, downloads and the separate window.
    let annotation: T3BrowserAnnotation
    let recording: T3BrowserRecording
    let downloads = T3BrowserDownloads()
    var pip: T3BrowserPictureInPicture?
    /// The file the encoder finished last, which `save` may move into the artifact directory (once).
    var encodedPath: String?
    private var grace: DispatchWorkItem?
    private let agent: Bool

    init(id: String, profile: String, environment: String, store: WKWebsiteDataStore, agent: Bool, imageDirectory: URL? = nil) {
        self.id = id; self.profile = profile; self.environment = environment
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = store
        // Chromium's popup blocker: a script opens a window only from a user gesture.
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        // A hidden tab keeps running, throttled, as a background Chromium guest does.
        if #available(macOS 14.0, *) { configuration.preferences.inactiveSchedulingPolicy = .throttle }
        favicon = T3BrowserFavicon(configuration: configuration)
        // Both register their script message handlers on the configuration, so before the web view takes it.
        annotation = T3BrowserAnnotation(configuration: configuration, imageDirectory: imageDirectory)
        recording = T3BrowserRecording(configuration: configuration)
        self.agent = agent
        web = T3BrowserWebView(frame: NSRect(x: 0, y: 0, width: 640, height: 480), configuration: configuration)
        super.init()
        web.navigationDelegate = self
        web.uiDelegate = self
        web.allowsBackForwardNavigationGestures = false
        web.setAccessibilityLabel("Browser")
        T3WebInspection.mark(web, "browser") // Safari's Web Inspector in a development build (scope item 11)
        // Under the agent the window is often covered; WebKit would then stop painting the page (as for the terminal).
        let occlusion = Selector(("_setWindowOcclusionDetectionEnabled:"))
        if agent, web.responds(to: occlusion) { web.perform(occlusion, with: false) }
        favicon.page = web
        annotation.page = web
        recording.page = web
        downloads.session = self
        annotation.changed = { [weak self] in self?.changed?() }
        recording.changed = { [weak self] in self?.changed?() }
        downloads.changed = { [weak self] in self?.changed?() }
        favicon.captured = { [weak self] in self?.changed?() }
        favicon.currentURL = { [weak self] in self?.web.url }
        let publish: () -> Void = { [weak self] in self?.changed?() }
        observations = [
            web.observe(\.title, options: [.new]) { _, _ in publish() },
            web.observe(\.url, options: [.new]) { [weak self] _, _ in self?.urlChanged() },
            web.observe(\.canGoBack, options: [.new]) { _, _ in publish() },
            web.observe(\.canGoForward, options: [.new]) { _, _ in publish() },
            web.observe(\.isLoading, options: [.new]) { [weak self] web, _ in
                // A load that ends with no navigation in flight (a same-document jump, a stop) ends its pending URL too.
                if let self, !web.isLoading, !self.provisional { self.pending = nil }
                publish()
            },
        ]
    }

    // MARK: State (computeNavStatus)

    var navigation: (kind: String, url: String) {
        if let failure { return ("LoadFailed", failure.url) }
        let current = (pending ?? web.url)?.absoluteString ?? ""
        if current.isEmpty || current == "about:blank" { return ("Idle", "") }
        return (web.isLoading || pending != nil ? "Loading" : "Success", current)
    }

    var report: [String: Any] {
        let nav = navigation
        var value: [String: Any] = ["kind": nav.kind, "url": nav.url, "title": web.title ?? "", "canGoBack": web.canGoBack, "canGoForward": web.canGoForward,
                                    "profile": profile, "crashed": crashed, "crashes": crashes, "attached": web.window != nil, "inspectable": web.isInspectable,
                                    "popups": popups.count, "refused": Array(refused.suffix(8))]
        value["failures"] = failures
        value["pick"] = annotation.report
        value["recording"] = recording.report
        value["pip"] = pip != nil
        value["downloads"] = downloads.entries
        if let failure { value["code"] = failure.code; value["description"] = failure.description }
        if let icon = favicon.current { value["favicon"] = ["dataUrl": icon.dataUrl, "pageUrl": icon.pageUrl, "capturedAt": icon.capturedAt] }
        return value
    }

    private func urlChanged() {
        // A committed document of another origin drops the old icon (Manager.ts clearFavicon on a confirmed navigation).
        favicon.navigated(to: web.url)
        changed?()
    }

    // MARK: Commands (Manager.ts navigate / goBack / goForward / refresh / hardReload)

    func navigate(_ url: URL) {
        failure = nil
        crashed = false
        if web.url == url { web.reload() }
        else if let current = web.url, Self.sameDocument(url, current) { web.load(URLRequest(url: url)) } // a fragment jump: no load to wait for
        else { pending = url; web.load(URLRequest(url: url)) }
        changed?()
    }

    /// The same document at another fragment: WebKit scrolls to it without a provisional navigation.
    static func sameDocument(_ target: URL, _ current: URL) -> Bool {
        guard target.fragment != nil, var a = URLComponents(url: target, resolvingAgainstBaseURL: false), var b = URLComponents(url: current, resolvingAgainstBaseURL: false) else { return false }
        a.fragment = nil; b.fragment = nil
        return a.url == b.url
    }

    func command(_ name: String) -> Bool {
        switch name {
        case "back": if web.canGoBack { web.goBack() }
        case "forward": if web.canGoForward { web.goForward() }
        case "refresh":
            crashed = false
            // Electron's reload restarts the current entry, the pending one included; WebKit's reloads the committed one.
            // A failed load is retried at its own URL (Chromium's error page sits at the failed URL; WebKit stays on the last page).
            if let pending { web.load(URLRequest(url: pending)) } else if let last = failure.flatMap({ URL(string: $0.url) }) { navigate(last) } else if web.url != nil { web.reload() }
        case "hard-reload":
            if let target = pending ?? failure.flatMap({ URL(string: $0.url) }) { failure = nil; pending = target; web.load(URLRequest(url: target, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData)) } else { web.reloadFromOrigin() }
        default: return false
        }
        changed?()
        return true
    }

    /// RECORDING_ARM_GRACE_MS (10 s): an armed recording that never begins lets go of its lease; nil cancels the wait.
    func armGrace(_ expired: (() -> Void)?) {
        grace?.cancel(); grace = nil
        guard let expired else { return }
        let work = DispatchWorkItem(block: expired)
        grace = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 10, execute: work)
    }

    /// Occlusion detection off while the page is parked off screen (T3BrowserParking), so WebKit keeps painting it.
    func keepPainting(_ on: Bool) {
        guard !agent else { return } // agent runs keep it off for every page
        let selector = Selector(("_setWindowOcclusionDetectionEnabled:"))
        guard web.responds(to: selector), let method = web.method(for: selector) else { return }
        typealias SetFlag = @convention(c) (AnyObject, Selector, Bool) -> Void
        unsafeBitCast(method, to: SetFlag.self)(web, selector, !on)
    }

    func close() {
        guard !closed else { return }
        closed = true
        grace?.cancel(); grace = nil
        annotation.close()
        recording.close()
        pip?.close(); pip = nil
        recovery?.cancel()
        observations.removeAll()
        favicon.cancel()
        for popup in popups { popup.close() }
        popups.removeAll()
        web.stopLoading()
        web.navigationDelegate = nil; web.uiDelegate = nil
        web.removeFromSuperview()
    }

    // MARK: WKNavigationDelegate

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let scheme = navigationAction.request.url?.scheme?.lowercased() ?? ""
        // The tab browses the web. A subframe may hold what a page embeds; the main frame refuses other schemes
        // (mailto:, an app's custom scheme) instead of handing them to another app.
        if navigationAction.targetFrame?.isMainFrame != false, !Self.mainFrameSchemes.contains(scheme) {
            refused.append(navigationAction.request.url?.absoluteString ?? "")
            return decisionHandler(.cancel)
        }
        if navigationAction.shouldPerformDownload { return decisionHandler(.download) } // an `<a download>` (part 3)
        decisionHandler(.allow)
    }
    static let mainFrameSchemes: Set<String> = ["http", "https", "about", "data", "blob"]

    func webView(_ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        // A response the page cannot show, or one sent as an attachment, downloads (Chromium's will-download; part 3).
        let disposition = (navigationResponse.response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Disposition")?.lowercased() ?? ""
        if !navigationResponse.canShowMIMEType || disposition.hasPrefix("attachment") { return decisionHandler(.download) }
        decisionHandler(.allow)
    }

    func webView(_ webView: WKWebView, navigationAction: WKNavigationAction, didBecome download: WKDownload) { downloads.adopt(download) }
    func webView(_ webView: WKWebView, navigationResponse: WKNavigationResponse, didBecome download: WKDownload) { downloads.adopt(download) }

    /// A main-frame navigation has started and not yet committed or failed.
    private(set) var provisional = false

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        annotation.navigated() // Manager onNavigated: a main-frame navigation settles an annotation in progress
        failure = nil // the reference keeps a failure until a new load actually starts
        provisional = true
        if let url = webView.url, url.absoluteString != "about:blank" { pending = url }
        changed?()
    }

    func webView(_ webView: WKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: WKNavigation!) {
        if let url = webView.url { pending = url } // the pending URL follows the redirect
        changed?()
    }

    func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
        provisional = false
        pending = nil
        recording.documentReady() // restoreRecordingCursor: the new document gets the recording overlay again
        changed?()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        pending = nil
        recording.documentReady()
        favicon.collect(in: webView)
        changed?()
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { failed(error) }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { failed(error) }

    private func failed(_ error: Error) {
        let failingURL = ((error as NSError).userInfo[NSURLErrorFailingURLErrorKey] as? URL)?.absoluteString ?? pending?.absoluteString ?? web.url?.absoluteString ?? ""
        // A load superseded by the next one (cancelled) leaves that one's pending URL alone; a load stopped for
        // itself (a download refused by policy, after a redirect too) ends its own.
        guard let failure = Self.loadFailure(error, url: failingURL) else {
            let error = error as NSError
            if !(error.domain == NSURLErrorDomain && error.code == NSURLErrorCancelled) || pending?.absoluteString == failingURL { pending = nil; provisional = false }
            return changed?() ?? ()
        }
        pending = nil
        provisional = false
        failures += 1
        self.failure = failure
        changed?()
    }

    /// Manager.ts `failed`: a cancelled load (Chromium -3, WebKit -999) and a load a policy stopped are not failures.
    static func loadFailure(_ error: Error, url: String) -> (url: String, code: Int, description: String)? {
        let error = error as NSError
        if error.domain == NSURLErrorDomain, error.code == NSURLErrorCancelled { return nil }
        if error.domain == WKErrorDomain || error.domain == "WebKitErrorDomain", error.code == 102 || error.code == 204 { return nil } // frame load interrupted; plug-in handled load
        return (url, error.code, chromiumName(error) ?? error.localizedDescription)
    }

    /// The Chromium net error a WebKit network error stands for (net_error_list.h), where there is one.
    static func chromiumName(_ error: NSError) -> String? {
        guard error.domain == NSURLErrorDomain else { return nil }
        switch error.code {
        case NSURLErrorCannotFindHost: return "ERR_NAME_NOT_RESOLVED"
        case NSURLErrorDNSLookupFailed: return "ERR_NAME_RESOLUTION_FAILED"
        case NSURLErrorCannotConnectToHost: return "ERR_CONNECTION_REFUSED"
        case NSURLErrorNetworkConnectionLost: return "ERR_CONNECTION_CLOSED"
        case NSURLErrorTimedOut: return "ERR_TIMED_OUT"
        case NSURLErrorNotConnectedToInternet: return "ERR_INTERNET_DISCONNECTED"
        case NSURLErrorServerCertificateUntrusted, NSURLErrorServerCertificateHasUnknownRoot: return "ERR_CERT_AUTHORITY_INVALID"
        case NSURLErrorServerCertificateHasBadDate, NSURLErrorServerCertificateNotYetValid: return "ERR_CERT_DATE_INVALID"
        case NSURLErrorSecureConnectionFailed: return "ERR_SSL_PROTOCOL_ERROR"
        case NSURLErrorClientCertificateRequired: return "ERR_SSL_CLIENT_AUTH_CERT_NEEDED"
        case NSURLErrorHTTPTooManyRedirects: return "ERR_TOO_MANY_REDIRECTS"
        case NSURLErrorUnsupportedURL: return "ERR_UNKNOWN_URL_SCHEME"
        case NSURLErrorBadURL: return "ERR_INVALID_URL"
        case NSURLErrorBadServerResponse: return "ERR_INVALID_RESPONSE"
        case NSURLErrorZeroByteResource: return "ERR_EMPTY_RESPONSE"
        default: return nil
        }
    }

    /// webviewCrashRecovery: up to three reloads 250, 500 and 1,000 ms after each crash in a 30 s window, at the last URL.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        crashes += 1
        if let scheduled = recovery, !scheduled.isCancelled { return changed?() ?? () } // one recovery at a time (recoverGuest)
        guard let plan = crash.plan(now: Date().timeIntervalSince1970 * 1000) else { crashed = true; changed?(); return } // three in 30 s: the tab stays down until Refresh
        crash = plan.state
        let target = failure.flatMap { URL(string: $0.url) } ?? pending ?? webView.url
        let work = DispatchWorkItem { [weak self] in
            guard let self, !self.closed else { return }
            self.recovery = nil; self.crashed = false
            if let target { self.web.load(URLRequest(url: target)) } else { self.web.reload() }
            self.changed?()
        }
        recovery = work
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(plan.delayMs), execute: work)
        changed?()
    }

    // MARK: WKUIDelegate

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        let url = navigationAction.request.url
        let scripted = navigationAction.navigationType != .linkActivated
        let features = windowFeatures.width != nil || windowFeatures.height != nil || windowFeatures.x != nil || windowFeatures.y != nil
            || windowFeatures.toolbarsVisibility?.boolValue == false || windowFeatures.menuBarVisibility?.boolValue == false
        if Self.windowOpenAction(url: url?.absoluteString ?? "", newWindow: scripted && features) == "popup" {
            let popup = T3BrowserPopup(configuration: configuration, features: windowFeatures, parent: webView.window)
            popup.closed = { [weak self, weak popup] in self?.popups.removeAll { $0 === popup }; self?.changed?() }
            popups.append(popup)
            changed?()
            return popup.web
        }
        // A tab disposition (a target=_blank link, a featureless window.open) loads in this tab.
        if let url, ["http", "https"].contains(url.scheme?.lowercased() ?? "") { navigate(url) } else { refused.append("window:" + (url?.absoluteString ?? "")) }
        return nil
    }

    /// previewWindowOpenAction: a real window only for a scripted pop-up (Chromium's `new-window` disposition) to
    /// a scheme whose window can be hardened; everything else navigates the tab.
    static func windowOpenAction(url: String, newWindow: Bool) -> String {
        guard newWindow, let parsed = URL(string: url), ["http", "https"].contains(parsed.scheme?.lowercased() ?? "") else { return "navigate" }
        return "popup"
    }

    @available(macOS 12.0, *)
    func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        // The camera and the microphone are not in ALLOWED_PREVIEW_PERMISSIONS.
        refused.append("permission:\(type == .camera ? "camera" : type == .microphone ? "microphone" : "camera+microphone")")
        decisionHandler(.deny)
        changed?()
    }

    func webView(_ webView: WKWebView, runOpenPanelWith parameters: WKOpenPanelParameters, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping ([URL]?) -> Void) {
        // A file input opens Chromium's chooser; under the agent nothing modal opens.
        guard dialogs else { return completionHandler(nil) }
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = parameters.allowsMultipleSelection
        panel.canChooseDirectories = parameters.allowsDirectories
        panel.canChooseFiles = true
        if let window = webView.window { panel.beginSheetModal(for: window) { completionHandler($0 == .OK ? panel.urls : nil) } }
        else { completionHandler(panel.runModal() == .OK ? panel.urls : nil) }
    }

    func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        guard let window = webView.window, dialogs else { return completionHandler() }
        let alert = NSAlert()
        alert.messageText = message
        alert.addButton(withTitle: "OK")
        alert.beginSheetModal(for: window) { _ in completionHandler() }
    }

    func webView(_ webView: WKWebView, runJavaScriptConfirmPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping (Bool) -> Void) {
        guard let window = webView.window, dialogs else { return completionHandler(false) }
        let alert = NSAlert()
        alert.messageText = message
        alert.addButton(withTitle: "OK")
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { completionHandler($0 == .alertFirstButtonReturn) }
    }
    // `prompt()` answers null, as Electron's does (no runJavaScriptTextInputPanel).
}

/// The tab's web view: the first click into an inactive window reaches the page, as Chromium's view takes it.
final class T3BrowserWebView: WKWebView {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

/// webviewCrashRecovery.ts `planWebviewCrashRecovery`.
struct T3BrowserCrashRecovery {
    static let windowMs: Double = 30_000
    static let maxAttempts = 3
    static let baseDelayMs = 250
    var attempts = 0
    var windowStartedAt: Double?
    func plan(now: Double) -> (delayMs: Int, state: T3BrowserCrashRecovery)? {
        let startsNewWindow = windowStartedAt == nil || now - windowStartedAt! >= Self.windowMs
        let attempts = startsNewWindow ? 0 : self.attempts
        if attempts >= Self.maxAttempts { return nil }
        return (Self.baseDelayMs << attempts, T3BrowserCrashRecovery(attempts: attempts + 1, windowStartedAt: startsNewWindow ? now : windowStartedAt))
    }
}

/// A scripted pop-up's window (Manager.ts POPUP_WINDOW_OPTIONS, `windowCreated`): the page's own data store and
/// opener; its own pop-ups are refused, so the chain stops at the first one; it closes when the page closes it.
final class T3BrowserPopup: NSObject, WKUIDelegate, NSWindowDelegate {
    let web: T3BrowserWebView
    let window: NSWindow
    var closed: (() -> Void)?
    private var title: NSKeyValueObservation?

    init(configuration: WKWebViewConfiguration, features: WKWindowFeatures, parent: NSWindow?) {
        let width = CGFloat(features.width?.doubleValue ?? 800), height = CGFloat(features.height?.doubleValue ?? 600)
        web = T3BrowserWebView(frame: NSRect(x: 0, y: 0, width: width, height: height), configuration: configuration)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: width, height: height), styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false)
        super.init()
        window.isReleasedWhenClosed = false
        window.contentView = web
        window.delegate = self
        web.uiDelegate = self
        T3WebInspection.mark(web, "browser pop-up")
        title = web.observe(\.title, options: [.initial, .new]) { [weak self] web, _ in self?.window.title = web.title ?? "" }
        if let x = features.x?.doubleValue, let y = features.y?.doubleValue { window.setFrameOrigin(NSPoint(x: x, y: y)) }
        else if let parent { window.setFrameOrigin(NSPoint(x: parent.frame.midX - width / 2, y: parent.frame.midY - height / 2)) }
        else { window.center() }
        window.makeKeyAndOrderFront(nil)
    }

    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? { nil }
    func webViewDidClose(_ webView: WKWebView) { close() }
    func windowWillClose(_ notification: Notification) { finish() }

    func close() { window.close() }
    private func finish() {
        title = nil
        web.stopLoading(); web.uiDelegate = nil
        let done = closed; closed = nil
        done?()
    }
}
#endif
