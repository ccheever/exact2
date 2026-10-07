#if os(iOS)
// @ref llp/1106.009-mobile-settings.decision.md#information-sources
import UIKit
import WebKit

final class T3MobileInformationLegal {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class ViewRef { weak var value: T3MobileLegalView?; init(_ value: T3MobileLegalView) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var views: [String: ViewRef] = [:]
    private let agent: Bool, audioSession: T3MobileAudioSession
    init(agent: Bool, audioSession: T3MobileAudioSession) { self.agent = agent; self.audioSession = audioSession }
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let view = T3MobileLegalView(owner: self, agent: agent, audioSession: audioSession, events: events); try view.setProps(props); return view
    }
    func configure(_ route: ExactRoute) { routes[route.key] = RouteRef(route); views[route.key]?.value?.attach(route) }
    func end(_ route: ExactRoute) { routes.removeValue(forKey: route.key); views[route.key]?.value?.detach() }
    fileprivate func bind(_ view: T3MobileLegalView, key: String) { views[key] = ViewRef(view); if let route = routes[key]?.value { view.attach(route) } }
    fileprivate func unbind(_ view: T3MobileLegalView, key: String) { if views[key]?.value === view { views.removeValue(forKey: key) } }
}

private struct T3LegalConfiguration: Decodable {
    let routeKey: String; let closeActionID: String; let url: String; let allowed: [String]; let colors: [String: String]
}
private final class T3LegalNavigation: NSObject, WKNavigationDelegate {
    weak var owner: T3MobileLegalView?
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let owner, owner.current(webView) else { decisionHandler(.cancel); return }; owner.policy(action, decisionHandler)
    }
    func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard let owner, owner.current(webView) else { decisionHandler(.cancel); return }; owner.response(response, decisionHandler)
    }
    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) { if owner?.current(webView) == true { owner?.loading() } }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { if owner?.current(webView) == true { owner?.finished(webView.url) } }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { if owner?.current(webView) == true { owner?.failed(error) } }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { if owner?.current(webView) == true { owner?.failed(error) } }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { if owner?.current(webView) == true { owner?.showError("The page could not be loaded.") } }
}
private final class T3MobileLegalView: ExactNativeInstance {
    private weak var owner: T3MobileInformationLegal?
    private weak var route: ExactRoute?
    private let root = UIView(), delegate = T3LegalNavigation()
    private var web: WKWebView, loadGeneration = 0
    private let spinner = UIActivityIndicatorView(style: .medium), progress = UIProgressView(progressViewStyle: .bar)
    private let errorPanel = UIStackView(), errorTitle = UILabel(), errorDetail = UITextView(), warning = UIImageView(image: UIImage(systemName: "exclamationmark.triangle"))
    private let retry = UIButton(type: .system), browser = UIButton(type: .system)
    private var observation: NSKeyValueObservation?, config: T3LegalConfiguration?, externalURL: URL?, alive = true, hasError = false
    private let agent: Bool, audioSession: T3MobileAudioSession
    private var audioLease = UUID()
    override var view: UIView { root }
    init(owner: T3MobileInformationLegal, agent: Bool, audioSession: T3MobileAudioSession, events: ExactNativeEvents) {
        self.owner = owner; self.agent = agent; self.audioSession = audioSession
        web = Self.makeWeb()
        super.init(events: events)
        audioSession.hold(audioLease)
        delegate.owner = self; attachWeb()
        spinner.translatesAutoresizingMaskIntoConstraints = false; root.addSubview(spinner)
        progress.translatesAutoresizingMaskIntoConstraints = false; root.addSubview(progress)
        errorPanel.axis = .vertical; errorPanel.spacing = 14; errorPanel.alignment = .fill; errorPanel.translatesAutoresizingMaskIntoConstraints = false
        warning.contentMode = .scaleAspectFit; warning.heightAnchor.constraint(equalToConstant: 32).isActive = true
        errorTitle.text = "Couldn't load the legal"; errorTitle.font = UIFont(name: "DMSans-Bold", size: 17.5) ?? .boldSystemFont(ofSize: 17.5); errorTitle.textAlignment = .center
        errorDetail.isEditable = false; errorDetail.isSelectable = true; errorDetail.isScrollEnabled = false; errorDetail.backgroundColor = .clear
        errorDetail.font = UIFont(name: "DMSans-Regular", size: 12.25) ?? .systemFont(ofSize: 12.25); errorDetail.textAlignment = .center
        retry.setTitle("Try Again", for: .normal); retry.titleLabel?.font = UIFont(name: "DMSans-Bold", size: 14) ?? .boldSystemFont(ofSize: 14); retry.layer.cornerRadius = 12
        retry.heightAnchor.constraint(greaterThanOrEqualToConstant: 42).isActive = true
        browser.setTitle("Open in Browser", for: .normal); browser.titleLabel?.font = UIFont(name: "DMSans-Medium", size: 14) ?? .systemFont(ofSize: 14, weight: .medium)
        browser.heightAnchor.constraint(greaterThanOrEqualToConstant: 42).isActive = true
        retry.addAction(UIAction { [weak self] _ in self?.load() }, for: .touchUpInside)
        browser.addAction(UIAction { [weak self] _ in guard let self, let url = config.flatMap({ URL(string: $0.url) }) else { return }; open(url) }, for: .touchUpInside)
        [warning, errorTitle, errorDetail, retry, browser].forEach { errorPanel.addArrangedSubview($0) }; root.addSubview(errorPanel)
        NSLayoutConstraint.activate([spinner.centerXAnchor.constraint(equalTo: root.centerXAnchor), spinner.centerYAnchor.constraint(equalTo: root.centerYAnchor),
            progress.leadingAnchor.constraint(equalTo: root.leadingAnchor), progress.trailingAnchor.constraint(equalTo: root.trailingAnchor), progress.topAnchor.constraint(equalTo: root.topAnchor),
            errorPanel.centerXAnchor.constraint(equalTo: root.centerXAnchor), errorPanel.centerYAnchor.constraint(equalTo: root.centerYAnchor),
            errorPanel.widthAnchor.constraint(lessThanOrEqualToConstant: 320), errorPanel.leadingAnchor.constraint(greaterThanOrEqualTo: root.leadingAnchor, constant: 28), errorPanel.trailingAnchor.constraint(lessThanOrEqualTo: root.trailingAnchor, constant: -28)])
        errorPanel.isHidden = true
    }
    private static func makeWeb() -> WKWebView {
        let configuration = WKWebViewConfiguration(); configuration.websiteDataStore = .nonPersistent()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        return WKWebView(frame: .zero, configuration: configuration)
    }
    private func attachWeb() {
        web.navigationDelegate = delegate; web.allowsBackForwardNavigationGestures = true; web.isOpaque = false; web.backgroundColor = .clear
        web.frame = root.bounds; web.autoresizingMask = [.flexibleWidth, .flexibleHeight]; root.insertSubview(web, at: 0)
        if let sheet = config?.colors["sheet"] { web.underPageBackgroundColor = Self.color(sheet) }
        observation = web.observe(\.estimatedProgress, options: [.new]) { [weak self] web, _ in
            guard let self, current(web), !hasError else { return }; progress.progress = Float(web.estimatedProgress); progress.isHidden = web.estimatedProgress >= 1
        }
    }
    fileprivate func current(_ view: WKWebView) -> Bool { alive && view === web }
    override func setProps(_ props: [String: String]) throws {
        guard let data = props["configuration"]?.data(using: .utf8) else { throw ExactNativeRefusal("Missing legal configuration.") }
        let next = try JSONDecoder().decode(T3LegalConfiguration.self, from: data)
        guard Self.identity(next.url) == "https://t3.codes/legal", Set(next.allowed.compactMap(Self.identity)) == Self.allowed else { throw ExactNativeRefusal("Invalid legal document URLs.") }
        let changed = config?.url != next.url, oldKey = config?.routeKey
        if oldKey != next.routeKey { if let oldKey { owner?.unbind(self, key: oldKey) }; detach() }
        config = next
        let foreground = Self.color(next.colors["foreground"]), muted = Self.color(next.colors["muted"]), sheet = Self.color(next.colors["sheet"])
        root.backgroundColor = sheet; web.underPageBackgroundColor = sheet; warning.tintColor = foreground; errorTitle.textColor = foreground; errorDetail.textColor = muted
        retry.backgroundColor = foreground; retry.setTitleColor(sheet, for: .normal); browser.setTitleColor(muted, for: .normal); spinner.color = foreground; progress.progressTintColor = foreground
        if oldKey != next.routeKey { owner?.bind(self, key: next.routeKey) }
        if changed { externalURL = URL(string: next.url); load() }
    }
    fileprivate func attach(_ route: ExactRoute) {
        self.route = route; route.controller.navigationItem.hidesBackButton = true
        let close = UIBarButtonItem(image: UIImage(systemName: "xmark"), primaryAction: UIAction { [weak self, weak route] _ in
            guard let id = self?.config?.closeActionID else { return }; route?.element(id)?.click()
        }); close.accessibilityLabel = "Close legal document"; route.controller.navigationItem.leftBarButtonItem = close
        let external = UIBarButtonItem(image: UIImage(systemName: "safari"), primaryAction: UIAction { [weak self] _ in guard let self, let url = externalURL else { return }; open(url) })
        external.accessibilityLabel = "Open legal documents in external browser"; route.controller.navigationItem.rightBarButtonItem = external
    }
    fileprivate func detach() { route?.controller.navigationItem.leftBarButtonItem = nil; route?.controller.navigationItem.rightBarButtonItem = nil; route?.controller.navigationItem.hidesBackButton = false; route = nil }
    private func load() {
        guard alive, let url = config.flatMap({ URL(string: $0.url) }) else { return }
        loadGeneration += 1; observation = nil
        let old = web, oldLease = audioLease
        audioLease = UUID(); audioSession.hold(audioLease)
        retire(old, lease: oldLease)
        web = Self.makeWeb(); attachWeb(); loading(); web.load(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData))
    }
    fileprivate func loading() { guard alive else { return }; hasError = false; web.isHidden = false; errorPanel.isHidden = true; spinner.startAnimating(); progress.progress = 0.05; progress.isHidden = false }
    fileprivate func finished(_ url: URL?) { guard alive, !hasError else { return }; spinner.stopAnimating(); progress.isHidden = true; if let url, Self.permitted(url) { externalURL = url } }
    fileprivate func failed(_ error: Error) { if (error as NSError).code != NSURLErrorCancelled { showError(error.localizedDescription) } }
    fileprivate func showError(_ message: String) { guard alive else { return }; hasError = true; spinner.stopAnimating(); progress.isHidden = true; web.isHidden = true; errorPanel.isHidden = false; errorDetail.text = message }
    fileprivate func policy(_ action: WKNavigationAction, _ reply: @escaping (WKNavigationActionPolicy) -> Void) {
        guard alive, let url = action.request.url else { reply(.cancel); return }
        if Self.permitted(url) {
            if action.targetFrame == nil || action.request.cachePolicy != .reloadIgnoringLocalAndRemoteCacheData {
                var request = action.request; request.cachePolicy = .reloadIgnoringLocalAndRemoteCacheData
                reply(.cancel); web.load(request)
            } else { reply(.allow) }
        } else { reply(.cancel); open(url) }
    }
    fileprivate func response(_ response: WKNavigationResponse, _ reply: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard alive else { reply(.cancel); return }
        if response.isForMainFrame, let http = response.response as? HTTPURLResponse, http.statusCode >= 400 { showError("The server returned status \(http.statusCode)."); reply(.cancel) } else { reply(.allow) }
    }
    private func open(_ url: URL) {
        guard alive else { return }; guard !agent else { showError("External browser opening requires direct interaction."); return }
        guard ["http", "https", "mailto"].contains(url.scheme?.lowercased() ?? "") else { showError("This link cannot be opened."); return }
        let generation = loadGeneration
        UIApplication.shared.open(url, options: [:]) { [weak self] opened in
            guard let self, alive, generation == loadGeneration else { return }; if !opened { showError("The link could not be opened.") }
        }
    }
    static let allowed: Set<String> = Set(["legal", "privacy-policy", "terms-of-service", "security-policy"].map { "https://t3.codes/\($0)" })
    static func identity(_ value: String) -> String? {
        guard let url = URLComponents(string: value), ["http", "https"].contains(url.scheme?.lowercased() ?? ""), let host = url.host?.lowercased() else { return nil }
        let scheme = url.scheme!.lowercased(), port = url.port.flatMap { ($0 == 443 && scheme == "https") || ($0 == 80 && scheme == "http") ? nil : ":\($0)" } ?? ""
        var path = url.percentEncodedPath; while path.hasSuffix("/") { path.removeLast() }; return "\(scheme)://\(host)\(port)\(path.isEmpty ? "/" : path)"
    }
    static func permitted(_ url: URL) -> Bool { identity(url.absoluteString).map { allowed.contains($0) } ?? false }
    static func color(_ text: String?) -> UIColor {
        guard let text, text.hasPrefix("#"), let n = UInt64(text.dropFirst(), radix: 16) else { return .label }
        let alpha = text.count == 9 ? CGFloat(n & 255) / 255 : 1, rgb = text.count == 9 ? n >> 8 : n
        return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: alpha)
    }
    private func retire(_ old: WKWebView, lease: UUID) {
        old.navigationDelegate = nil; old.stopLoading(); old.removeFromSuperview()
        // Each WK instance keeps its own lease until its media has stopped. A retry's
        // old completion cannot release a newer WK lease or retain an Exact handle.
        old.setAllMediaPlaybackSuspended(true) { [audioSession, lease, old] in _ = old; audioSession.release(lease) }
    }
    override func destroy() {
        guard alive else { return }; alive = false; observation = nil; delegate.owner = nil
        retire(web, lease: audioLease)
        if let key = config?.routeKey { owner?.unbind(self, key: key) }; detach()
    }
}
#endif
