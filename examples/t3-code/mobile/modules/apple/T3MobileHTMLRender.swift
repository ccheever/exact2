#if os(iOS)
// Adapted from T3 Code (MIT), 365aa87982 HtmlRenderWebView.tsx.
// Dormant opt-in for t3-document-html; the app owns URL signing and route admission.
import UIKit
import WebKit
import CoreFoundation

fileprivate struct T3HTMLRenderStamp: Equatable {
    let identifier: String
    let attempt: Int
}

private struct T3HTMLRenderSource {
    let stamp: T3HTMLRenderStamp
    let active: Bool
    let title: String
    let url: String
    let nested: Bool
    let phase: String
    let theme: [String: Any]
    let themeJSON: String
    let background: UIColor
    let iconColor: UIColor
    let muted: UIColor
    let card: UIColor
    let border: UIColor

    init(_ text: String) throws {
        guard let value = try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              value["mode"] as? String == "html-render",
              let identifier = value["identifier"] as? String, !identifier.isEmpty,
              let number = value["attempt"] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
              number.doubleValue.isFinite, number.doubleValue >= 0,
              number.doubleValue <= 9_007_199_254_740_991, number.doubleValue.rounded(.down) == number.doubleValue,
              let active = Self.boolean(value["active"]), let nested = Self.boolean(value["nested"]),
              let title = value["title"] as? String, let url = value["url"] as? String,
              let iconColor = value["iconColor"] as? String,
              let phase = value["phase"] as? String, ["pending", "ready", "failed"].contains(phase),
              let theme = value["theme"] as? [String: Any],
              let appearance = theme["appearance"] as? String, ["light", "dark"].contains(appearance),
              let variables = theme["variables"] as? [String: String],
              let background = variables["--background"],
              let muted = variables["--muted-foreground"], let card = variables["--card"],
              let border = variables["--border"] else { throw ExactNativeRefusal("Invalid HTML render descriptor.") }
        if phase == "ready" {
            guard Self.remoteURL(url) != nil else { throw ExactNativeRefusal("Invalid HTML render URL.") }
        } else if !url.isEmpty { throw ExactNativeRefusal("An unsigned HTML render must not contain a URL.") }
        stamp = T3HTMLRenderStamp(identifier: identifier, attempt: number.intValue)
        self.active = active; self.title = title; self.url = url; self.nested = nested; self.phase = phase
        self.theme = ["appearance": appearance, "variables": variables]
        themeJSON = T3MobileHTMLRender.json(self.theme)
        self.background = try T3SymbolView.color(background); self.iconColor = try T3SymbolView.color(iconColor)
        self.muted = try T3SymbolView.color(muted); self.card = try T3SymbolView.color(card)
        self.border = try T3SymbolView.color(border)
    }
    fileprivate static func boolean(_ value: Any?) -> Bool? {
        guard let number = value as? NSNumber, CFGetTypeID(number) == CFBooleanGetTypeID() else { return nil }
        return number.boolValue
    }
    static func remoteURL(_ text: String) -> URL? {
        guard let url = URL(string: text), ["http", "https"].contains(url.scheme?.lowercased() ?? ""),
              url.host != nil, url.user == nil, url.password == nil else { return nil }
        return url
    }
}

private final class T3HTMLRenderRoot: UIView {
    var changed: (() -> Void)?
    override func didMoveToWindow() { super.didMoveToWindow(); changed?() }
    override func layoutSubviews() { super.layoutSubviews(); changed?() }
}

private final class T3HTMLRenderOpenButton: UIButton {
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        !isHidden && isEnabled && bounds.insetBy(dx: -8, dy: -8).contains(point)
    }
}

final class T3MobileHTMLRender: ExactNativeInstance {
    private let root = T3HTMLRenderRoot()
    private let spinner = UIActivityIndicatorView(style: .medium)
    private let open = T3HTMLRenderOpenButton(type: .system)
    private let retry = UIButton(type: .system)
    private let audioSession: T3MobileAudioSession
    private let mountID = UUID().uuidString
    private var audioLease: UUID?
    private var source: T3HTMLRenderSource?
    private var web: WKWebView?
    private var delegate: T3HTMLRenderDelegate?
    private var generation = 0
    private var alive = true
    private var mounted = false
    private var lastWidth: CGFloat?
    private var loaded = false
    private var loadFailed = false
    private var processRetried = false
    private var initialThemeJSON = ""
    private var shownThemeJSON = ""
    override var view: UIView { root }

    // A present but invalid discriminator is refused, never sent to the file viewer.
    static func requested(_ props: [String: String]) throws -> Bool {
        guard let text = props["media-source"],
              let value = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              let mode = value["mode"] else { return false }
        guard mode as? String == "html-render" else { throw ExactNativeRefusal("Unsupported HTML render mode.") }
        return true
    }

    init(audioSession: T3MobileAudioSession, events: ExactNativeEvents) {
        self.audioSession = audioSession
        super.init(events: events)
        root.backgroundColor = .clear
        root.changed = { [weak self] in self?.reconcile() }
        spinner.hidesWhenStopped = true
        open.setImage(UIImage(systemName: "arrow.up.left.and.arrow.down.right", withConfiguration:
            UIImage.SymbolConfiguration(pointSize: 12, weight: .regular)), for: .normal)
        open.layer.cornerRadius = 14; open.layer.borderWidth = 1
        retry.setTitle("Page unavailable", for: .normal)
        retry.titleLabel?.font = .systemFont(ofSize: 14)
        root.addSubview(spinner); root.addSubview(retry); root.addSubview(open)
        open.isHidden = true; retry.isHidden = true
    }

    override func setProps(_ props: [String: String]) throws {
        let next: T3HTMLRenderSource
        do { next = try T3HTMLRenderSource(props["media-source"] ?? "") }
        catch { source = nil; retireWeb(); mounted = false; lastWidth = nil; renderControls(); throw error }
        let previous = source
        let ownerChanged = previous?.stamp != next.stamp
        let reload = ownerChanged || previous?.url != next.url || previous?.phase != next.phase || previous?.nested != next.nested
        source = next
        if ownerChanged { mounted = false; lastWidth = nil; processRetried = false }
        if reload { retireWeb(); loadFailed = false }
        if previous?.active != next.active { mounted = false; lastWidth = nil }
        open.accessibilityLabel = "Open \(next.title)"
        retry.accessibilityLabel = "Reload \(next.title)"
        open.removeAction(identifiedBy: UIAction.Identifier("open"), for: .touchUpInside)
        retry.removeAction(identifiedBy: UIAction.Identifier("retry"), for: .touchUpInside)
        let stamp = next.stamp
        open.addAction(UIAction(identifier: UIAction.Identifier("open")) { [weak self] _ in
            guard let self, self.source?.stamp == stamp, self.source?.phase == "ready", self.web != nil, !self.loadFailed else { return }
            self.emit("open", stamp: stamp)
        }, for: .touchUpInside)
        retry.addAction(UIAction(identifier: UIAction.Identifier("retry")) { [weak self] _ in
            guard let self, self.source?.stamp == stamp, self.source?.phase == "failed" else { return }
            self.emit("retry", stamp: stamp)
        }, for: .touchUpInside)
        web?.accessibilityLabel = next.title
        updateTheme()
        reconcile()
    }

    private var attached: Bool { root.window != nil && root.bounds.width > 0 && root.bounds.height > 0 }

    private func reconcile() {
        guard alive else { return }
        layoutContent()
        guard let source, source.active, attached else {
            mounted = false; lastWidth = nil; retireWeb(); renderControls(); return
        }
        let width = max(0, root.bounds.width - (source.nested ? 0 : 32))
        guard width > 0 else { renderControls(); return }
        if !mounted {
            mounted = true; lastWidth = width
            emit("mount", stamp: source.stamp, extra: ["width": Double(width)])
        } else if lastWidth != width {
            lastWidth = width; emit("width", stamp: source.stamp, extra: ["width": Double(width)])
        }
        if source.phase == "ready", web == nil, !loadFailed { createWeb(source) }
        renderControls()
    }

    private func layoutContent() {
        let gutter: CGFloat = source?.nested == false ? 16 : 0
        web?.frame = root.bounds.insetBy(dx: gutter, dy: 0)
        spinner.center = CGPoint(x: root.bounds.midX, y: root.bounds.midY)
        retry.frame = root.bounds
        open.frame = CGRect(x: max(0, root.bounds.width - 34), y: 6, width: 28, height: 28)
    }

    private func renderControls() {
        let visible = alive && source?.active == true && attached
        let failed = source?.phase == "failed"
        retry.isHidden = !visible || !failed
        open.isHidden = !visible || source?.nested != true || source?.phase != "ready" || web == nil || loadFailed
        if visible && !failed && !loaded { spinner.startAnimating() } else { spinner.stopAnimating() }
        if let source {
            root.backgroundColor = source.nested ? .clear : source.background
            spinner.color = source.muted
            retry.setTitleColor(source.muted, for: .normal)
            open.tintColor = source.iconColor
            open.backgroundColor = source.card.withAlphaComponent(source.card.cgColor.alpha * 0.8)
            open.layer.borderColor = source.border.withAlphaComponent(source.border.cgColor.alpha * 0.6).cgColor
        }
    }

    private func createWeb(_ source: T3HTMLRenderSource) {
        guard let url = URL(string: source.url + Self.themeFragment(source.themeJSON)) else {
            loadFailed = true; emit("error", stamp: source.stamp, extra: ["message": "The page URL is invalid."]); return
        }
        generation += 1
        let delegate = T3HTMLRenderDelegate(owner: self, stamp: source.stamp, generation: generation)
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.allowsInlineMediaPlayback = true
        config.preferences.javaScriptCanOpenWindowsAutomatically = false
        if source.nested {
            config.userContentController.add(delegate, name: "htmlRender")
            config.userContentController.addUserScript(WKUserScript(source: Self.overflowScript, injectionTime: .atDocumentEnd, forMainFrameOnly: true))
        }
        let next = WKWebView(frame: .zero, configuration: config)
        next.navigationDelegate = delegate; next.uiDelegate = delegate
        next.accessibilityLabel = source.title
        next.isOpaque = false; next.backgroundColor = .clear; next.scrollView.backgroundColor = .clear
        next.scrollView.contentInsetAdjustmentBehavior = source.nested ? .never : .automatic
        next.scrollView.bounces = !source.nested
        next.scrollView.showsVerticalScrollIndicator = !source.nested
        next.scrollView.showsHorizontalScrollIndicator = !source.nested
        next.scrollView.isScrollEnabled = !source.nested
        self.delegate = delegate; web = next
        loaded = false; initialThemeJSON = source.themeJSON; shownThemeJSON = ""
        let lease = UUID(); audioLease = lease; audioSession.hold(lease)
        root.insertSubview(next, at: 0); layoutContent()
        next.load(URLRequest(url: url))
    }

    private func retireWeb() {
        generation += 1
        loaded = false; initialThemeJSON = ""; shownThemeJSON = ""
        delegate?.owner = nil; delegate = nil
        let lease = audioLease; audioLease = nil
        if let old = web {
            web = nil
            old.configuration.userContentController.removeScriptMessageHandler(forName: "htmlRender")
            old.navigationDelegate = nil; old.uiDelegate = nil; old.stopLoading(); old.removeFromSuperview()
            let session = audioSession
            old.setAllMediaPlaybackSuspended(true) { [old] in _ = old; if let lease { session.release(lease) } }
        } else if let lease { audioSession.release(lease) }
    }

    private func updateTheme() {
        guard let source, source.active, attached, loaded, let web, shownThemeJSON != source.themeJSON else { return }
        shownThemeJSON = source.themeJSON
        let message: [String: Any] = ["jsonrpc": "2.0", "method": "ui/notifications/host-context-changed",
            "params": ["theme": source.theme["appearance"]!, "styles": ["variables": source.theme["variables"]!]]]
        web.evaluateJavaScript("window.postMessage(\(Self.json(message)), '*'); true;")
    }

    private func emit(_ kind: String, stamp: T3HTMLRenderStamp, extra: [String: Any] = [:]) {
        guard alive, let source, source.stamp == stamp, source.active, attached else { return }
        var value = extra
        value["identifier"] = stamp.identifier; value["attempt"] = stamp.attempt
        value["mountId"] = mountID; value["kind"] = kind
        events.change(Self.json(value))
    }

    fileprivate func current(_ view: WKWebView, _ callback: T3HTMLRenderDelegate) -> Bool {
        alive && source?.active == true && source?.phase == "ready" && attached && view === web
            && source?.stamp == callback.stamp && generation == callback.generation
    }

    fileprivate func finished(_ view: WKWebView, _ callback: T3HTMLRenderDelegate) {
        guard current(view, callback) else { return }
        loaded = true; shownThemeJSON = initialThemeJSON
        updateTheme(); renderControls()
    }

    fileprivate func failed(_ view: WKWebView, _ callback: T3HTMLRenderDelegate, message: String) {
        guard current(view, callback), !loadFailed else { return }
        let stamp = callback.stamp
        loadFailed = true; retireWeb(); renderControls()
        emit("error", stamp: stamp, extra: ["message": message])
    }

    fileprivate func terminated(_ view: WKWebView, _ callback: T3HTMLRenderDelegate) {
        guard current(view, callback) else { return }
        if processRetried { failed(view, callback, message: "The page stopped responding."); return }
        processRetried = true; retireWeb(); reconcile()
    }

    fileprivate func overflow(_ view: WKWebView, _ callback: T3HTMLRenderDelegate, message: WKScriptMessage) {
        guard current(view, callback), source?.nested == true, message.frameInfo.isMainFrame,
              let text = message.body as? String,
              let value = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              value["type"] as? String == "t3-html-render-overflow", let overflow = T3HTMLRenderSource.boolean(value["overflow"]) else { return }
        view.scrollView.isScrollEnabled = overflow
    }

    fileprivate func policy(_ view: WKWebView, _ callback: T3HTMLRenderDelegate, action: WKNavigationAction) -> WKNavigationActionPolicy {
        guard current(view, callback), let source, let url = action.request.url else { return .cancel }
        if action.targetFrame?.isMainFrame == false { return .allow }
        if action.targetFrame == nil { return T3HTMLRenderSource.remoteURL(url.absoluteString) == nil ? .cancel : .allow }
        return Self.withoutFragment(url.absoluteString) == Self.withoutFragment(source.url) ? .allow : .cancel
    }

    fileprivate func external(_ view: WKWebView, _ callback: T3HTMLRenderDelegate, action: WKNavigationAction) {
        guard current(view, callback), action.targetFrame == nil, let url = action.request.url,
              T3HTMLRenderSource.remoteURL(url.absoluteString) != nil else { return }
        emit("external", stamp: callback.stamp, extra: ["url": url.absoluteString])
    }

    override func destroy() {
        guard alive else { return }; alive = false
        root.changed = nil; source = nil; retireWeb(); spinner.stopAnimating()
        open.removeAction(identifiedBy: UIAction.Identifier("open"), for: .touchUpInside)
        retry.removeAction(identifiedBy: UIAction.Identifier("retry"), for: .touchUpInside)
    }

    fileprivate static func json(_ value: Any) -> String {
        let data = (try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])) ?? Data()
        return String(decoding: data, as: UTF8.self).replacingOccurrences(of: "<", with: "\\u003c")
            .replacingOccurrences(of: "\u{2028}", with: "\\u2028").replacingOccurrences(of: "\u{2029}", with: "\\u2029")
    }
    private static func withoutFragment(_ text: String) -> String { String(text.split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)[0]) }
    private static func themeFragment(_ text: String) -> String {
        let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.!~*'()")
        return "#t3-theme=" + (text.addingPercentEncoding(withAllowedCharacters: allowed) ?? "")
    }
    private static let overflowScript = """
    (function(){var last;function report(){var d=document.documentElement,b=document.body;var o=Math.max(d.scrollHeight,b?b.scrollHeight:0)>window.innerHeight+1||Math.max(d.scrollWidth,b?b.scrollWidth:0)>window.innerWidth+1;if(o===last)return;last=o;window.webkit.messageHandlers.htmlRender.postMessage(JSON.stringify({type:"t3-html-render-overflow",overflow:o}));}report();if(window.ResizeObserver){var r=new ResizeObserver(report);r.observe(document.documentElement);if(document.body)r.observe(document.body);}window.addEventListener("resize",report);})();true;
    """
}

fileprivate final class T3HTMLRenderDelegate: NSObject, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler {
    weak var owner: T3MobileHTMLRender?
    let stamp: T3HTMLRenderStamp
    let generation: Int
    init(owner: T3MobileHTMLRender, stamp: T3HTMLRenderStamp, generation: Int) {
        self.owner = owner; self.stamp = stamp; self.generation = generation
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { owner?.finished(webView, self) }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        if (error as NSError).code != NSURLErrorCancelled { owner?.failed(webView, self, message: "The page could not be loaded.") }
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        if (error as NSError).code != NSURLErrorCancelled { owner?.failed(webView, self, message: "The page could not be loaded.") }
    }
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        decisionHandler(owner?.policy(webView, self, action: action) ?? .cancel)
    }
    func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard let owner, owner.current(webView, self) else { decisionHandler(.cancel); return }
        if response.isForMainFrame, let http = response.response as? HTTPURLResponse, http.statusCode >= 400 {
            owner.failed(webView, self, message: "The page could not be loaded (HTTP \(http.statusCode)).")
            decisionHandler(.cancel)
        } else { decisionHandler(.allow) }
    }
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        owner?.external(webView, self, action: action); return nil
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { owner?.terminated(webView, self) }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let webView = message.webView else { return }; owner?.overflow(webView, self, message: message)
    }
}
#endif
