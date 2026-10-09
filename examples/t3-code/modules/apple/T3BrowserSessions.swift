#if os(macOS)
import AppKit
import CryptoKit
import WebKit

/// Every Browser tab's page, by runtime tab id (browser-surface part 1; MIT reference, see LICENSE-T3, T3 Code
/// 1e2ecbd975: apps/web/src/browser/ElectronBrowserHost.tsx, desktopTabLifetime.ts; apps/desktop/src/preview/
/// BrowserSession.ts partitions). The reference mounts a `<webview>` for every live preview session at the app
/// root, so a tab keeps its page while the panel is hidden or another thread is shown, and closes it when its
/// session ends. Here the module keeps one `T3BrowserSession` per session the data module lists (`browserSync`,
/// browser-surface.ts `liveSessions`), whether or not a `t3-browser` view shows it; the view only borrows the
/// page while it is mounted (T3BrowserView.swift). A session the list no longer names closes.
///
/// Storage: each environment's profile has its own persistent WebKit data store (the reference's
/// `persist:t3code-preview-<scope>` partition), identified by a UUID derived from the environment and the
/// profile, apart from the app's other web views; Incognito's store is in memory (part 4, which adds the named
/// profiles, clearing and the cookie import: T3BrowserSessions+Profiles.swift). Agent runs keep everything in memory.
///
/// One registry per module, so per session: two sessions in one process (the sample host's) keep their own
/// pages, and a session that ends closes only its own (T3Module `browserSessions`, `T3BrowserSessionOwner`).
final class T3BrowserSessions {
    let agent: Bool
    private let announce: (String) -> Void
    private(set) var sessions: [String: T3BrowserSession] = [:]
    private var stores: [String: WKWebsiteDataStore] = [:]
    private var scheduled = false
    /// The module's ops and syncs, newest last (the agent's status).
    private(set) var log: [String] = []
    /// A page made or closed (browser-surface part 5: the automation host's scripts go in before the first load).
    var created: ((T3BrowserSession) -> Void)?
    var ended: ((String) -> Void)?

    init(agent: Bool, changed: @escaping (String) -> Void) {
        self.agent = agent
        announce = changed
    }

    /// One `t3.status` per run-loop turn, however many page facts changed in it.
    func publish() {
        guard !scheduled else { return }
        scheduled = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.scheduled = false
            self.announce("t3.status")
        }
    }

    func store(environment: String, profile: String) -> WKWebsiteDataStore {
        let key = "\(environment)\u{0}\(profile)"
        if let store = stores[key] { return store }
        let store: WKWebsiteDataStore
        // Part 4: Incognito keeps its data in memory, gone when the app quits (one store per environment, as the
        // reference's ephemeral partition per scope); every other profile persists in its own store.
        if agent || profile == "incognito" { store = .nonPersistent() }
        else if #available(macOS 14.0, *) { store = WKWebsiteDataStore(forIdentifier: Self.storeIdentifier(environment: environment, profile: profile)) }
        else { store = .default() }
        stores[key] = store
        return store
    }

    /// A stable UUID for an environment's profile store (SHA-256 of the scope, as an RFC 4122 v8 UUID).
    static func storeIdentifier(environment: String, profile: String) -> UUID {
        var bytes = Array(SHA256.hash(data: Data("t3code-preview\u{0}\(environment)\u{0}\(profile)".utf8)).prefix(16))
        bytes[6] = (bytes[6] & 0x0f) | 0x80
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return UUID(uuid: (bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]))
    }

    /// The tab's page, made on first use (at its URL, if it has one).
    @discardableResult
    func ensure(id: String, url: String, profile: String, environment: String) -> T3BrowserSession {
        if let existing = sessions[id] { return existing }
        let session = T3BrowserSession(id: id, profile: profile.isEmpty ? "default" : profile, environment: environment,
                                       store: store(environment: environment, profile: profile.isEmpty ? "default" : profile), agent: agent)
        session.changed = { [weak self] in self?.publish() }
        session.dialogs = !agent
        sessions[id] = session
        created?(session)
        if let target = URL(string: url), ["http", "https"].contains(target.scheme?.lowercased() ?? "") { session.navigate(target) }
        note("open \(id)")
        publish()
        return session
    }

    /// `browserSync`: a page for each live session, and no other.
    func sync(_ live: [[String: Any]]) {
        var keep = Set<String>()
        for entry in live {
            guard let id = entry["id"] as? String, !id.isEmpty else { continue }
            keep.insert(id)
            ensure(id: id, url: entry["url"] as? String ?? "", profile: entry["profile"] as? String ?? "default", environment: entry["environment"] as? String ?? "")
        }
        for id in sessions.keys where !keep.contains(id) { close(id) }
    }

    func close(_ id: String) {
        guard let session = sessions.removeValue(forKey: id) else { return }
        session.close()
        ended?(id)
        note("close \(id)")
        publish()
    }

    func navigate(id: String, url: String, profile: String, environment: String) -> Bool {
        guard let target = URL(string: url), ["http", "https"].contains(target.scheme?.lowercased() ?? "") else { return false }
        ensure(id: id, url: "", profile: profile, environment: environment).navigate(target)
        note("navigate \(url)")
        return true
    }

    func command(id: String, name: String) -> Bool {
        guard let session = sessions[id] else { return false }
        note("\(name) \(id)")
        return session.command(name)
    }

    /// The registry's own record, and a `t3.browser:` host line (the agent's `logs`).
    func note(_ line: String) {
        log.append(String(line.prefix(200)))
        if log.count > 24 { log.removeFirst(log.count - 24) }
        FileHandle.standardError.write(Data("t3.browser: \(line.prefix(300))\n".utf8))
    }

    /// The module's status (`presentation.browserTabs`, `presentation.browserLog`).
    var status: [String: Any] {
        ["browserTabs": sessions.mapValues { $0.report.merging($0.navigationReport) { first, _ in first } }, "browserLog": log] // part 2: zoom, appearance
    }

    /// The data module's ops (T3Module+Browser.swift); nil for any other op.
    func perform(_ request: [String: Any]) -> [String: Any]? {
        let generation = request["generation"] as? Int ?? 0
        let answer = { (value: [String: Any]) -> [String: Any] in ["ok": true, "generation": generation, "value": value] }
        switch request["op"] as? String {
        case "browserSync":
            sync(request["tabs"] as? [[String: Any]] ?? [])
            return answer(["tabs": sessions.count])
        case "browserNavigate":
            let started = navigate(id: request["tab"] as? String ?? "", url: request["url"] as? String ?? "", profile: request["profile"] as? String ?? "default", environment: request["environment"] as? String ?? "")
            return answer(["started": started])
        case "browserCommand":
            return answer(["done": command(id: request["tab"] as? String ?? "", name: request["command"] as? String ?? "")])
        case "browserClose":
            close(request["tab"] as? String ?? "")
            return answer([:])
        default:
            return nil
        }
    }
}

/// The module that owns a session's Browser pages (T3Module+Browser.swift); a view finds its registry through it.
protocol T3BrowserSessionOwner: AnyObject {
    var browserSessions: T3BrowserSessions { get }
}
#endif
