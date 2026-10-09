#if os(macOS)
import Foundation
import WebKit

/// Browser surface part 4 (profiles; MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/desktop/src/preview/BrowserSession.ts `clearCookies`/`clearCache`, BrowserImport/BrowserImport.ts `writeCookies`).
/// Every op acts on one environment's store for one profile (`store(environment:profile:)`: a persistent
/// `WKWebsiteDataStore(forIdentifier:)`, Incognito's in memory), never across profiles.
/// - `browserClearData` `what: cookies`: cookies, local storage, IndexedDB and service workers (the reference's
///   `clearStorageData` storages); `what: cache`: the HTTP caches (`session.clearCache()`).
/// - `browserImportCookies`: writes a batch through the store's `WKHTTPCookieStore` and reads the store back, so a
///   cookie WebKit refused (malformed, expired) counts as skipped, as a rejected `cookies.set` does there.
/// - `browserImportContext`, `browserImportIO`, `browserImportOpenSettings`: the import's reads (T3BrowserImportIO.swift);
///   `browserImportProgress`: a redraw while the wizard's command runs.
extension T3BrowserSessions {
    static let importIO = T3BrowserImportIO(policy: T3BrowserImportIO.resolve(env: ProcessInfo.processInfo.environment, packaged: T3LocalPolicy.packaged(resources: Bundle.main.resourceURL)))

    /// The part 4 ops; false for any other op.
    func performProfiles(_ request: [String: Any], reply: ExactReply) -> Bool {
        let generation = request["generation"] as? Int ?? 0
        let answer = { (value: [String: Any]) in reply.send(["ok": true, "generation": generation, "value": value]) }
        let environment = request["environment"] as? String ?? "", profile = (request["profile"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "default"
        switch request["op"] as? String {
        case "browserClearData":
            let what = request["what"] as? String ?? ""
            DispatchQueue.main.async { self.clearData(environment: environment, profile: profile, what: what) { answer(["cleared": $0]) } }
        case "browserImportCookies":
            let cookies = request["cookies"] as? [[String: Any]] ?? []
            DispatchQueue.main.async { self.importCookies(environment: environment, profile: profile, cookies: cookies) { answer(["results": $0]) } }
        case "browserImportContext":
            answer(Self.importIO.context)
        case "browserImportIO":
            Self.importIO.perform(request) { answer($0) }
        case "browserImportOpenSettings":
            DispatchQueue.main.async { answer(Self.importIO.openFullDiskAccessSettings()) }
        case "browserImportLog":
            // The listing's outcome per browser (`sources chrome=ready …`): a `t3.browser:` host line and the status log.
            let line = String((request["line"] as? String ?? "").prefix(400))
            DispatchQueue.main.async { self.note("import \(line)"); answer([:]) }
        case "browserImportProgress":
            // The wizard moved to a step of its own while its command runs ("Importing cookies", "Checking …"): one
            // `t3.status` so the page reads it now, not when the command ends.
            DispatchQueue.main.async { self.publish(); answer([:]) }
        default:
            return false
        }
        return true
    }

    static func dataTypes(_ what: String) -> Set<String> {
        switch what {
        case "cookies": return [WKWebsiteDataTypeCookies, WKWebsiteDataTypeLocalStorage, WKWebsiteDataTypeIndexedDBDatabases, WKWebsiteDataTypeServiceWorkerRegistrations]
        case "cache": return [WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache, WKWebsiteDataTypeFetchCache]
        default: return []
        }
    }

    func clearData(environment: String, profile: String, what: String, done: @escaping (Bool) -> Void) {
        let types = Self.dataTypes(what)
        guard !types.isEmpty, !environment.isEmpty else { return done(false) }
        let store = store(environment: environment, profile: profile)
        store.removeData(ofTypes: types, modifiedSince: .distantPast) { [weak self] in
            self?.note("clear-\(what) \(environment) \(profile)")
            done(true)
        }
    }

    func importCookies(environment: String, profile: String, cookies: [[String: Any]], done: @escaping ([Bool]) -> Void) {
        guard !environment.isEmpty else { return done(cookies.map { _ in false }) }
        let store = store(environment: environment, profile: profile).httpCookieStore
        let made = cookies.map(Self.httpCookie)
        let group = DispatchGroup()
        for cookie in made.compactMap({ $0 }) {
            group.enter()
            store.setCookie(cookie) { group.leave() }
        }
        group.notify(queue: .main) { [weak self] in
            store.getAllCookies { all in
                let present = Set(all.map(Self.cookieKey))
                let results = made.map { $0.map { present.contains(Self.cookieKey($0)) } ?? false }
                self?.note("import \(results.filter { $0 }.count)/\(results.count) cookies \(environment) \(profile)")
                done(results)
            }
        }
    }

    static func cookieKey(_ cookie: HTTPCookie) -> String { "\(cookie.name)\u{0}\(cookie.domain)\u{0}\(cookie.path)" }

    /// The reference's `cookies.set` input as an `HTTPCookie`: a domain cookie keeps its leading-dot domain; a host-only
    /// one takes its host from the URL and no domain, so it is never widened to subdomains. WebKit has no explicit
    /// `SameSite=None` property: None and unspecified both land without a policy, which WebKit treats as None.
    static func httpCookie(_ value: [String: Any]) -> HTTPCookie? {
        guard let urlString = value["url"] as? String, let url = URL(string: urlString), let host = url.host, !host.isEmpty,
              let name = value["name"] as? String, !name.isEmpty else { return nil }
        var properties: [HTTPCookiePropertyKey: Any] = [.name: name, .value: value["value"] as? String ?? "", .path: value["path"] as? String ?? "/", .originURL: url]
        properties[.domain] = (value["domain"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? host
        if value["secure"] as? Bool == true { properties[.secure] = "TRUE" }
        if value["httpOnly"] as? Bool == true { properties[HTTPCookiePropertyKey("HttpOnly")] = "TRUE" }
        if let seconds = value["expirationDate"] as? Double { properties[.expires] = Date(timeIntervalSince1970: seconds) } else { properties[.discard] = "TRUE" }
        switch value["sameSite"] as? String {
        case "lax": properties[.sameSitePolicy] = HTTPCookieStringPolicy.sameSiteLax
        case "strict": properties[.sameSitePolicy] = HTTPCookieStringPolicy.sameSiteStrict
        default: break
        }
        return HTTPCookie(properties: properties)
    }
}
#endif
