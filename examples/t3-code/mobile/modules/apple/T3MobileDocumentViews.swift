#if os(iOS)
// AttachmentFileScreen native menu and inline HTML document, pinned365aa87982.
// @ref llp/1109.005-composer-and-transcript.decision.md#media-presentation
import UIKit
import WebKit

/// The document's native menu follows the public route and native-element lifetimes.
final class T3MobileDocumentMenus {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class MenuRef { weak var value: T3MobileDocumentMenu?; init(_ value: T3MobileDocumentMenu) { self.value = value } }
    private var routes: [String: RouteRef] = [:], menus: [String: MenuRef] = [:]
    private var alive = true
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The document session has ended.") }
        let menu = T3MobileDocumentMenu(owner: self, events: events); try menu.setProps(props); return menu
    }
    func configure(_ route: ExactRoute) {
        guard alive else { return }; routes[route.key] = RouteRef(route); menus[route.key]?.value?.attach(route)
    }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        menus[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ menu: T3MobileDocumentMenu, key: String) {
        guard alive else { return }
        if let old = menus[key]?.value, old !== menu { old.detach() }
        menus[key] = MenuRef(menu)
        if let route = routes[key]?.value { menu.attach(route) }
    }
    fileprivate func unbind(_ menu: T3MobileDocumentMenu, key: String) {
        if menus[key]?.value === menu { menus.removeValue(forKey: key) }
    }
    func destroy() { alive = false; for ref in menus.values { ref.value?.detach() }; menus.removeAll(); routes.removeAll() }
}

final class T3MobileDocumentMenu: ExactNativeInstance {
    private weak var owner: T3MobileDocumentMenus?
    private weak var route: ExactRoute?
    private var routeKey = ""
    private let button = UIButton(type: .system)
    private var group: UIBarButtonItemGroup?
    private var alive = true
    private var revision = UUID()
    private var configuration = "", identifier = "", title = "", rendered = ""
    private var rows: [[String: Any]] = []
    private var tint: UIColor?, renderedTint: UIColor?
    override var view: UIView { button }
    init(owner: T3MobileDocumentMenus, events: ExactNativeEvents) {
        self.owner = owner
        super.init(events: events)
        button.setImage(UIImage(systemName: "ellipsis"), for: .normal)
        button.showsMenuAsPrimaryAction = true
        button.accessibilityLabel = "File actions"
    }
    override func setProps(_ props: [String: String]) throws {
        guard alive, let raw = props["document-menu"], let data = raw.data(using: .utf8),
              let config = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let identifier = config["identifier"] as? String else { throw ExactNativeRefusal("Invalid file menu.") }
        let key = props["route-key"] ?? ""
        guard props["route-key"] == nil || !key.isEmpty else { throw ExactNativeRefusal("Invalid file menu route.") }
        let nextTint = try props["document-tint"].map(T3SymbolView.color)
        if key != routeKey { detach(); owner?.unbind(self, key: routeKey); routeKey = key }
        configuration = raw; self.identifier = identifier; title = config["title"] as? String ?? ""
        rows = config["actions"] as? [[String: Any]] ?? []; tint = nextTint
        button.isEnabled = !identifier.isEmpty; button.tintColor = tint
        button.isUserInteractionEnabled = key.isEmpty; button.accessibilityElementsHidden = !key.isEmpty
        if !key.isEmpty { owner?.bind(self, key: key) }
        refresh()
    }
    fileprivate func attach(_ route: ExactRoute) {
        if self.route !== route { detach(); self.route = route }
        refresh()
    }
    private func refresh() {
        guard alive else { return }
        if routeKey.isEmpty { removeItem(); button.menu = makeMenu(); return }
        guard let route, route.isLive else { return }
        if let group, rendered == configuration, renderedTint == tint {
            if !route.controller.navigationItem.trailingItemGroups.contains(where: { $0 === group }) {
                route.controller.navigationItem.trailingItemGroups.append(group)
            }
            return
        }
        removeItem()
        button.menu = makeMenu()
        let item = UIBarButtonItem(image: UIImage(systemName: "ellipsis"), menu: button.menu)
        item.accessibilityLabel = "File actions"; item.accessibilityIdentifier = "document-actions"
        item.isEnabled = button.isEnabled; item.tintColor = tint
        let next = UIBarButtonItemGroup.fixedGroup(representativeItem: nil, items: [item])
        group = next; rendered = configuration; renderedTint = tint
        route.controller.navigationItem.trailingItemGroups.append(next)
    }
    private func makeMenu() -> UIMenu {
        let token = revision, raw = configuration, identifier = identifier
        var modes: [UIMenuElement] = [], actions: [UIMenuElement] = []
        for row in rows {
            guard let id = row["id"] as? String, let title = row["title"] as? String else { continue }
            var attributes: UIMenuElement.Attributes = id == "remove" ? .destructive : []
            if row["disabled"] as? Bool == true { attributes.insert(.disabled) }
            let action = UIAction(title: title, image: UIImage(systemName: row["icon"] as? String ?? ""),
                            attributes: attributes,
                            state: row["selected"] as? Bool == true ? .on : .off) { [weak self] _ in
                guard let self, alive, revision == token, configuration == raw else { return }
                if !routeKey.isEmpty {
                    guard let route = self.route, route.isLive, route.key == routeKey,
                          route.controller.navigationController?.topViewController === route.controller,
                          route.controller.viewIfLoaded?.window != nil, route.controller.presentedViewController == nil else { return }
                }
                if let value = row["copyValue"] as? String {
                    UIPasteboard.general.string = value
                    UISelectionFeedbackGenerator().selectionChanged()
                    return
                }
                if let value = row["eventValue"] as? String { events.change(value); return }
                guard let value = try? JSONSerialization.data(withJSONObject: ["identifier": identifier, "operation": id]) else { return }
                events.change(String(decoding: value, as: UTF8.self))
            }
            if row["inline"] as? Bool == true { modes.append(action) } else { actions.append(action) }
        }
        if !modes.isEmpty { actions.insert(UIMenu(options: .displayInline, children: modes), at: 0) }
        return UIMenu(title: title, children: actions)
    }
    private func removeItem() {
        revision = UUID()
        if let item = route?.controller.navigationItem, let group {
            item.trailingItemGroups = item.trailingItemGroups.filter { $0 !== group }
        }
        group = nil; rendered = ""; renderedTint = nil
    }
    fileprivate func detach() { removeItem(); route = nil }
    override func destroy() {
        alive = false; configuration = ""; rows = []; button.menu = nil; detach(); owner?.unbind(self, key: routeKey)
    }
}

private final class T3MobileHTMLNavigation: NSObject, WKNavigationDelegate {
    weak var owner: T3MobileDocumentHTML?
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        owner?.webView(webView, didFail: navigation, withError: error)
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        owner?.webView(webView, didFailProvisionalNavigation: navigation, withError: error)
    }
    func webView(_ webView: WKWebView, decidePolicyFor response: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard let owner else { decisionHandler(.cancel); return }
        owner.webView(webView, decidePolicyFor: response, decisionHandler: decisionHandler)
    }
}
final class T3MobileDocumentHTML: ExactNativeInstance {
    private let navigation = T3MobileHTMLNavigation()
    private let root = UIView()
    private var web: WKWebView
    private let files: T3MobileMediaFiles
    private let audioSession: T3MobileAudioSession
    private var audioOwner = UUID()
    private var identifier = ""
    private var opening: Task<Void, Never>?
    private var lease: URL?
    private var alive = true
    override var view: UIView { root }
    init(dataRoot: URL, audioSession: T3MobileAudioSession, events: ExactNativeEvents) {
        self.audioSession = audioSession
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        web = WKWebView(frame: .zero, configuration: configuration)
        files = T3MobileMediaFiles(dataRoot: dataRoot)
        super.init(events: events)
        attachWeb()
        audioSession.hold(audioOwner)
    }
    private func attachWeb() {
        navigation.owner = self
        web.navigationDelegate = navigation
        web.allowsBackForwardNavigationGestures = true
        web.isOpaque = false; web.backgroundColor = .clear
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.frame = root.bounds; web.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        root.addSubview(web)
    }
    override func setProps(_ props: [String: String]) throws {
        let source = try T3MobileMediaSource(props["media-source"] ?? "")
        guard source.identifier != identifier else { return }
        opening?.cancel()
        let retired = web, retiredOwner = audioOwner, session = audioSession
        audioOwner = UUID(); audioSession.hold(audioOwner)
        retired.navigationDelegate = nil; retired.stopLoading(); retired.removeFromSuperview()
        retired.setAllMediaPlaybackSuspended(true) { _ = retired; session.release(retiredOwner) }
        T3MobileMediaFiles.release(lease); lease = nil
        let configuration = WKWebViewConfiguration(); configuration.websiteDataStore = .nonPersistent()
        web = WKWebView(frame: root.bounds, configuration: configuration); attachWeb()
        identifier = source.identifier
        if let url = source.url { web.load(URLRequest(url: url)); return }
        opening = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let file = try await files.prepare(source)
                guard alive, !Task.isCancelled, identifier == source.identifier else { T3MobileMediaFiles.release(file); return }
                lease = file
                web.loadFileURL(file, allowingReadAccessTo: file.deletingLastPathComponent())
            } catch {
                guard alive, !Task.isCancelled, identifier == source.identifier,
                      let value = try? JSONSerialization.data(withJSONObject: ["identifier": source.identifier, "message": error.localizedDescription]) else { return }
                events.change(String(decoding: value, as: UTF8.self))
            }
        }
    }
    private func failed(_ message: String) {
        guard alive, let value = try? JSONSerialization.data(withJSONObject: ["identifier": identifier, "message": message]) else { return }
        events.change(String(decoding: value, as: UTF8.self))
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        if webView === web, (error as NSError).code != NSURLErrorCancelled { failed(error.localizedDescription) }
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        if webView === web, (error as NSError).code != NSURLErrorCancelled { failed(error.localizedDescription) }
    }
    func webView(_ webView: WKWebView, decidePolicyFor navigationResponse: WKNavigationResponse, decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void) {
        guard webView === web, alive else { decisionHandler(.cancel); return }
        if navigationResponse.isForMainFrame, let response = navigationResponse.response as? HTTPURLResponse, response.statusCode >= 400 {
            failed("The file could not be loaded (HTTP \(response.statusCode)).")
            decisionHandler(.cancel)
        } else { decisionHandler(.allow) }
    }
    override func destroy() {
        web.navigationDelegate = nil
        guard alive else { return }; alive = false; web.removeFromSuperview()
        opening?.cancel(); opening = nil; web.stopLoading()
        let session = audioSession, token = audioOwner, retired = web
        retired.setAllMediaPlaybackSuspended(true) { _ = retired; session.release(token) }
        T3MobileMediaFiles.release(lease); lease = nil
    }
}
#endif
