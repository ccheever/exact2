// A partitioned iframe: a top-level browsing context with its own website
// data store, the Navigation API's events seen from outside, and the four
// commands that move it (@ref LLP 1020.002 D1–D4). WebArm.swift builds the
// web view and forwards its navigation delegate calls here.
import CryptoKit
import Foundation
import WebKit

/// Partition names and their data stores (@ref LLP 1020.002 D1). A
/// `persist:` store is the app's, made once per name and kept for the
/// process; an in-memory one is its session's, let go when the session ends
/// (`exact_web_end_session`), as an Electron in-memory partition lives until
/// the app quits.
enum Partitions {
    struct Store { let name: String; let session: String; let store: WKWebsiteDataStore }
    /// By `app\0name` for a persistent store, `session\0name` for one in memory.
    nonisolated(unsafe) static var stores: [String: Store] = [:]

    /// `persist:` and 1 to 64 of `[A-Za-z0-9._-]`, or the same without it.
    static func valid(_ name: String) -> Bool {
        let body = name.hasPrefix("persist:") ? name.dropFirst("persist:".count) : Substring(name)
        return (1...64).contains(body.count) && body.unicodeScalars.allSatisfy {
            CharacterSet.alphanumerics.contains($0) && $0.isASCII || $0 == "." || $0 == "_" || $0 == "-"
        }
    }

    /// Under the agent every partition is in memory, so a drive never reads
    /// or writes a person's saved logins.
    static func store(_ name: String, agent: Bool, app: String, session: String) -> WKWebsiteDataStore {
        let persistent = name.hasPrefix("persist:") && !agent
        let key = persistent ? "\(app)\u{0}\(name)" : "\(session)\u{0}\(name)"
        if let found = stores[key] { return found.store }
        let store: WKWebsiteDataStore
        if persistent, #available(macOS 14.0, iOS 17.0, *) {
            store = WKWebsiteDataStore(forIdentifier: identifier(name, app: app))
        } else {
            store = .nonPersistent()
        }
        stores[key] = Store(name: name, session: persistent ? "" : session, store: store)
        return store
    }

    /// The session ended: its in-memory partitions go with it.
    static func end(session: String) {
        stores = stores.filter { $0.value.session.isEmpty || $0.value.session != session }
    }

    /// A UUID per app and name: SHA-256 of the app's id and the name, with
    /// RFC 9562's version 8 and variant bits, so two apps naming one
    /// partition, embedded in one process or not, never share its files.
    static func identifier(_ name: String, app: String) -> UUID {
        var bytes = Array(SHA256.hash(data: Data("\(app)\u{0}\(name)".utf8)).prefix(16))
        bytes[6] = (bytes[6] & 0x0f) | 0x80
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return UUID(uuid: (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                           bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]))
    }

    /// `clearSiteData(partition, types)` (@ref LLP 1020.002 D4): the Clear
    /// Site Data header's type names over a whole partition. `nil` is success;
    /// otherwise a DOMException name and message.
    /// Every live store of that name is cleared: a package call names no
    /// session, so two embedded sessions that name one partition clear both.
    static func clear(_ name: String, types: [String], done: @escaping ((String, String)?) -> Void) {
        let named = stores.values.filter { $0.name == name }.map(\.store)
        guard !named.isEmpty else {
            return done(("NotFoundError", "clearSiteData: this session has never named the partition \(name)"))
        }
        var kinds = Set<String>()
        for type in types {
            switch type {
            case "cookies": kinds.insert(WKWebsiteDataTypeCookies)
            case "storage":
                kinds.formUnion([WKWebsiteDataTypeLocalStorage, WKWebsiteDataTypeSessionStorage, WKWebsiteDataTypeIndexedDBDatabases,
                                 WKWebsiteDataTypeServiceWorkerRegistrations, WKWebsiteDataTypeFileSystem, WKWebsiteDataTypeWebSQLDatabases])
            case "cache":
                kinds.formUnion([WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache, WKWebsiteDataTypeFetchCache,
                                 WKWebsiteDataTypeOfflineWebApplicationCache])
            case "*": kinds.formUnion(WKWebsiteDataStore.allWebsiteDataTypes())
            default: return done(("SyntaxError", "clearSiteData: \(type) is not a type; the types are \"cookies\", \"storage\", \"cache\" and \"*\""))
            }
        }
        var left = named.count
        for store in named { store.removeData(ofTypes: kinds, modifiedSince: .distantPast) { left -= 1; if left == 0 { done(nil) } } }
    }
}

/// The schemes a partitioned frame shows, by `src` or `navigate`.
enum GuestURL {
    static func web(_ text: String) -> URL? {
        guard let url = URL(string: text), let scheme = url.scheme?.lowercased() else { return nil }
        if scheme == "http" || scheme == "https" { return url.host == nil ? nil : url }
        return text.lowercased() == "about:blank" ? url : nil
    }
}

/// The navigation state of one partitioned web view, reported as
/// `FrameNavigation` records (@ref LLP 1020.002 D2). WebKit's delegate calls
/// and KVO are the source; each navigation ends in `navigatesuccess` or
/// `navigateerror`.
final class FrameBrowser: NSObject, WKScriptMessageHandler {
    struct Flight {
        let id: Int
        var destination: String
        let type: String
        var navigation: WKNavigation?
        var committed = false
    }

    /// The icon reader's world: it reads the page's `<link rel~=icon>` and
    /// installs nothing the page can observe (@ref LLP 1020.002 D1).
    static let world = WKContentWorld.world(name: "exact.browser")
    static let iconScript = """
    (() => { const post = () => { try { window.webkit.messageHandlers.exactIcons.postMessage(Array.from(document.querySelectorAll('link[rel~="icon" i]'), l => l.href).filter(Boolean)); } catch {} };
    post(); new MutationObserver(post).observe(document.head || document.documentElement, {subtree: true, childList: true, attributes: true, attributeFilter: ['href', 'rel']}); })()
    """

    /// Crash recovery's delays (the clone's and the reference's: three
    /// attempts in 30 s, from 250 ms).
    static let recoveryDelays: [TimeInterval] = [0.25, 0.5, 1.0]

    weak var web: WKWebView?
    let emit: (String, [String: Any]) -> Void
    /// The source to load again after a crash when nothing committed.
    var source: () -> String?
    /// Another app's URL `navigate` was given: the platform's confirmation
    /// decides (@ref LLP 1020.002 D3, D5).
    var external: ((URL) -> Void)?
    var nextId = 0
    var flight: Flight?
    var lastId = 0
    var lastType = "push"
    var committed = ""
    var icons: [String] = []
    /// The main frame's last policy decision: its URL and navigation type.
    var policy: (url: String, type: String)?
    /// The type a command gives the navigation it starts.
    var commandType: String?
    var observations: [NSKeyValueObservation] = []
    var history: (back: Int, current: ObjectIdentifier?) = (0, nil)
    var lastSent: String?
    var stateScheduled = false
    var urlScheduled = false
    var deaths: [Date] = []
    var recovery: DispatchWorkItem?
    /// The content process died since the last commit: WebKit's own reload
    /// has no document to reload, so `reload` loads the URL again.
    var dead = false
    var closed = false

    init(emit: @escaping (String, [String: Any]) -> Void, source: @escaping () -> String?) {
        self.emit = emit
        self.source = source
    }

    /// Installs the reader on a configuration about to make a web view.
    func install(on controller: WKUserContentController) {
        controller.add(self, contentWorld: Self.world, name: "exactIcons")
        controller.addUserScript(WKUserScript(source: Self.iconScript, injectionTime: .atDocumentEnd, forMainFrameOnly: true, in: Self.world))
    }

    func attach(_ web: WKWebView) {
        self.web = web
        observations = [
            web.observe(\.title, options: [.new]) { [weak self] _, _ in self?.stateChanged() },
            web.observe(\.canGoBack, options: [.new]) { [weak self] _, _ in self?.stateChanged() },
            web.observe(\.canGoForward, options: [.new]) { [weak self] _, _ in self?.stateChanged() },
            web.observe(\.url, options: [.new]) { [weak self] _, _ in self?.urlChanged() },
            // A same-document traversal can move `url` while WebKit says it is loading: looked at again when it is not.
            web.observe(\.isLoading, options: [.new]) { [weak self] web, _ in if !web.isLoading { self?.urlChanged() } },
        ]
        rememberHistory()
    }

    func close(controller: WKUserContentController?) {
        guard !closed else { return }
        closed = true
        recovery?.cancel()
        observations = []
        controller?.removeScriptMessageHandler(forName: "exactIcons", contentWorld: Self.world)
    }

    // MARK: Records

    /// The state now; `ending` is the navigation an end event reports, no
    /// longer in flight.
    func record(ending: Flight? = nil, error: String = "", message: String = "", recovering: Bool = false) -> [String: Any] {
        let flight = ending ?? self.flight
        return [
            "url": committed,
            "destination": ending == nil ? flight.map { $0.committed ? "" : $0.destination } ?? "" : "",
            "navigationId": flight?.id ?? lastId,
            "navigationType": flight?.type ?? lastType,
            "title": web?.title ?? "",
            "icons": iconList(),
            "canGoBack": web?.canGoBack ?? false,
            "canGoForward": web?.canGoForward ?? false,
            "loading": ending == nil && flight != nil,
            "partitioned": true,
            "controller": "person",
            "error": error,
            "message": message,
            "recovering": recovering,
        ]
    }

    /// The page's icons in document order, then its origin's `/favicon.ico`.
    func iconList() -> [String] {
        var out = icons
        if let url = URL(string: committed), let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https",
           var parts = URLComponents(url: url, resolvingAgainstBaseURL: false) {
            parts.path = "/favicon.ico"; parts.query = nil; parts.fragment = nil; parts.user = nil; parts.password = nil
            if let fallback = parts.url?.absoluteString, !out.contains(fallback) { out.append(fallback) }
        }
        return out
    }

    func send(_ event: String, ending: Flight? = nil, error: String = "", message: String = "", recovering: Bool = false) {
        guard !closed else { return }
        let value = record(ending: ending, error: error, message: message, recovering: recovering)
        if event == "currententrychange" || event == "navigatesuccess" { lastSent = Self.stateKey(value) }
        emit(event, value)
    }

    /// What a `currententrychange` from KVO compares, so one change is one event.
    static func stateKey(_ record: [String: Any]) -> String {
        "\(record["url"] ?? "")\u{0}\(record["title"] ?? "")\u{0}\(record["icons"] ?? "")\u{0}\(record["canGoBack"] ?? "")\u{0}\(record["canGoForward"] ?? "")"
    }

    // MARK: Navigations

    func begin(_ destination: String, type: String, navigation: WKNavigation?) {
        if let old = flight {
            // A navigation replacing one in flight ends the old one first, as
            // the Navigation API aborts a superseded navigation.
            flight = nil
            emitEnd(old, "navigateerror", error: "AbortError", message: "superseded by a new navigation")
        }
        nextId += 1
        flight = Flight(id: nextId, destination: destination, type: type, navigation: navigation)
        lastId = nextId
        lastType = type
        send("navigate")
    }

    func emitEnd(_ ended: Flight, _ event: String, error: String = "", message: String = "", recovering: Bool = false) {
        send(event, ending: ended, error: error, message: message, recovering: recovering)
    }

    func decided(_ action: WKNavigationAction) {
        guard action.targetFrame?.isMainFrame == true, let url = action.request.url?.absoluteString else { return }
        let type: String
        switch action.navigationType {
        case .backForward: type = "traverse"
        case .reload: type = "reload"
        default: type = "push"
        }
        policy = (url, type)
    }

    func started(_ navigation: WKNavigation?) {
        // WebKit's own navigation type wins, but for a reload by URL (crash
        // recovery, the reload of a load that failed), which it calls `other`;
        // so a stale command type never colours a link.
        let type: String
        switch (policy?.type, commandType) {
        case ("push"?, "reload"?): type = "reload"
        case (let decided?, _): type = decided
        case (nil, let command?): type = command
        default: type = "push"
        }
        commandType = nil
        let destination = policy?.url ?? web?.url?.absoluteString ?? ""
        policy = nil
        begin(destination, type: type, navigation: navigation)
    }

    func redirected(_ navigation: WKNavigation?) {
        guard var current = flight, current.navigation === navigation || current.navigation == nil else { return }
        current.destination = policy?.url ?? web?.url?.absoluteString ?? current.destination
        policy = nil
        flight = current
        send("navigate")
    }

    func committed(_ navigation: WKNavigation?) {
        guard var current = flight, current.navigation === navigation || current.navigation == nil else { return }
        current.committed = true
        flight = current
        dead = false
        committed = web?.url?.absoluteString ?? current.destination
        icons = []
        rememberHistory()
        send("currententrychange")
    }

    func finished(_ navigation: WKNavigation?) {
        guard let current = flight, current.navigation === navigation || current.navigation == nil else { return }
        flight = nil
        committed = web?.url?.absoluteString ?? committed
        rememberHistory()
        emitEnd(current, "navigatesuccess")
    }

    func failed(_ navigation: WKNavigation?, _ error: Error) {
        guard let current = flight, current.navigation === navigation || current.navigation == nil else { return }
        flight = nil
        let ns = error as NSError
        // Cancelled (a new load, a stop) or interrupted by policy (a response
        // WebKit cannot show) is the Navigation API's AbortError.
        let aborted = (ns.domain == NSURLErrorDomain && ns.code == NSURLErrorCancelled) || (ns.domain == "WebKitErrorDomain" && ns.code == 102)
        emitEnd(current, "navigateerror", error: aborted ? "AbortError" : "NetworkError", message: "\(ns.domain) \(ns.code): \(ns.localizedDescription)")
    }

    // MARK: Same-document navigations and state

    /// `pushState`, `replaceState` and a fragment change move `url` with no
    /// provisional navigation: they are a navigation's three events at once.
    func urlChanged() {
        guard !urlScheduled else { return }
        urlScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.closed else { return }
            self.urlScheduled = false
            guard let web = self.web, self.flight == nil, !web.isLoading, let now = web.url?.absoluteString, !self.committed.isEmpty else { return }
            let back = web.backForwardList.backList.count, current = web.backForwardList.currentItem.map(ObjectIdentifier.init)
            // `pushState` of the same URL, or a traversal to an entry of the
            // same URL, moves the history and not `url`.
            let moved = current != self.history.current
            guard now != self.committed || moved else {
                // Nothing moved: a command's type must not outlive it into the next navigation.
                self.commandType = nil
                return
            }
            let type: String
            if let command = self.commandType {
                // `traverseHistory` to an entry of this document starts no provisional navigation.
                type = command
            } else if let policy = self.policy, policy.url == now {
                type = policy.type
            } else if back < self.history.back || !web.backForwardList.forwardList.isEmpty && current != self.history.current {
                type = "traverse"
            } else {
                type = back > self.history.back || (current != self.history.current && back == self.history.back + 1) ? "push" : "replace"
            }
            self.commandType = nil
            self.policy = nil
            self.begin(now, type: type, navigation: nil)
            self.committed = now
            self.flight?.committed = true
            self.rememberHistory()
            self.send("currententrychange")
            if let ended = self.flight { self.flight = nil; self.emitEnd(ended, "navigatesuccess") }
        }
    }

    func rememberHistory() {
        guard let web else { return }
        history = (web.backForwardList.backList.count, web.backForwardList.currentItem.map(ObjectIdentifier.init))
    }

    /// A title, icon or history change with no navigation in flight.
    func stateChanged() {
        guard !stateScheduled else { return }
        stateScheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.closed else { return }
            self.stateScheduled = false
            // A same-document navigation that moved only the history is one, not a state change.
            if self.flight == nil, let web = self.web, !self.committed.isEmpty,
               web.backForwardList.currentItem.map(ObjectIdentifier.init) != self.history.current { return self.urlChanged() }
            // A command whose navigation never came (WebKit says nothing of a
            // `pushState` that keeps the URL, nor of a traversal back to it)
            // must not outlive it into the next navigation.
            if self.flight == nil, self.web?.isLoading == false { self.commandType = nil }
            guard self.flight == nil, !self.committed.isEmpty, Self.stateKey(self.record()) != self.lastSent else { return }
            self.send("currententrychange")
        }
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.name == "exactIcons", message.webView === web, message.frameInfo.isMainFrame,
              let hrefs = message.body as? [String] else { return }
        // Bounded as the reference bounds its candidates: eight, each an
        // http(s) or inline image URL of at most 2,048 characters.
        let list = Array(hrefs.filter { href in
            href.count <= 2048 && ["http:", "https:", "data:image/"].contains { href.lowercased().hasPrefix($0) }
        }.prefix(8))
        guard list != icons else { return }
        icons = list
        stateChanged()
    }

    // MARK: Commands (@ref LLP 1020.002 D3)

    func refuse(_ error: String, _ message: String) {
        FileHandle.standardError.write(Data("exact: iframe: \(message)\n".utf8))
        send("navigateerror", error: error, message: message)
    }

    func command(_ op: [String: Any]) {
        guard let web, !closed else { return }
        switch op["op"] as? String {
        case "navigate":
            let text = op["url"] as? String ?? ""
            guard let url = GuestURL.web(text) else {
                if let other = GuestScheme.external(URL(string: text)) {
                    external?(other)
                    return
                }
                let scheme = URL(string: text)?.scheme.map { "\($0):" } ?? "this URL"
                return refuse("NotSupportedError", "navigate: \(scheme) is not a web page a partitioned frame shows (http, https or about:blank)")
            }
            web.load(URLRequest(url: url))
        case "traverseHistory":
            let delta = (op["delta"] as? NSNumber)?.intValue ?? 0
            if delta == 0 { return reload(ignoreCache: false) }
            guard let item = web.backForwardList.item(at: delta) else {
                return send("navigateerror", error: "InvalidStateError", message: "traverseHistory: no history entry at \(delta)")
            }
            commandType = "traverse"
            web.go(to: item)
        case "reload":
            reload(ignoreCache: op["ignoreCache"] as? Bool ?? false)
        case "stopLoading":
            guard let current = flight else { return }
            flight = nil
            web.stopLoading()
            emitEnd(current, "navigateerror", error: "AbortError", message: "stopLoading")
        default:
            break
        }
    }

    /// `reload(id)`: the committed document again, or what failed or was
    /// pending, or `src` when nothing ever committed. It also lifts a crash's
    /// last refusal.
    func reload(ignoreCache: Bool) {
        guard let web else { return }
        deaths = []
        recovery?.cancel()
        commandType = "reload"
        if let target = flight?.destination ?? (committed.isEmpty ? source() : dead ? committed : nil), let url = GuestURL.web(target) {
            web.load(URLRequest(url: url, cachePolicy: ignoreCache ? .reloadIgnoringLocalAndRemoteCacheData : .useProtocolCachePolicy))
        } else if web.url != nil {
            if ignoreCache { web.reloadFromOrigin() } else { web.reload() }
        } else {
            commandType = nil
        }
    }

    // MARK: Crashes

    /// The content process died: `navigateerror` "crashed", then a reload
    /// after 250, 500 and 1,000 ms; a fourth death within 30 s ends it until
    /// `reload` (@ref LLP 1020.002 D2).
    func crashed() {
        guard let web, !closed else { return }
        let now = Date()
        dead = true
        deaths = deaths.filter { now.timeIntervalSince($0) < 30 } + [now]
        let target = flight?.destination ?? (committed.isEmpty ? source() : committed)
        let ending = flight ?? { nextId += 1; lastId = nextId; return Flight(id: nextId, destination: "", type: "reload") }()
        flight = nil
        let attempt = deaths.count
        guard attempt <= Self.recoveryDelays.count else {
            return emitEnd(ending, "navigateerror", error: "crashed", message: "the web content process ended \(attempt) times in 30 s", recovering: false)
        }
        emitEnd(ending, "navigateerror", error: "crashed", message: "the web content process ended", recovering: true)
        let work = DispatchWorkItem { [weak self, weak web] in
            guard let self, let web, !self.closed else { return }
            self.commandType = "reload"
            if let target, let url = GuestURL.web(target) { web.load(URLRequest(url: url)) } else { web.reload() }
        }
        recovery?.cancel()
        recovery = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.recoveryDelays[attempt - 1], execute: work)
    }
}
