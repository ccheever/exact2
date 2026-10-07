#if os(iOS)
// Source: examples/t3-code/modules/apple/T3Credentials.swift at 887b2491b182f851b11253655f6aa84fe2a26708.
// Mobile adaptations: LLP 1106.003, native identity and lifecycle. Shared source is unchanged.
import Foundation
import Security

/// Access tokens remain in the app's Keychain account, never in a Contract value.
/// One instance is shared by the focused transport and every background
/// environment transport (T3Fleet), so its in-memory agent store is locked.
final class T3Credentials: @unchecked Sendable {
    let persistent: Bool
    private var store: [String: String] = [:]
    private let lock = NSLock()
    private var volatile: [String: String] {
        get { lock.lock(); defer { lock.unlock() }; return store }
        set { lock.lock(); store = newValue; lock.unlock() }
    }
    init(persistent: Bool) { self.persistent = persistent }
    private let service = "com.exact.t3code.ios.access-token"

    func read(origin: String, environment: String) throws -> String? {
        guard persistent else { return volatile[Self.account(origin: origin, environment: environment)] }
        var query = self.query(origin, environment)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var value: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &value)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let bytes = value as? Data else {
            throw T3Failure(kind: "Keychain", message: "The saved server credential could not be read from Keychain.")
        }
        return String(data: bytes, encoding: .utf8)
    }

    func save(_ token: String, origin: String, environment: String) throws {
        guard persistent else { volatile[Self.account(origin: origin, environment: environment)] = token; return }
        let value = Data(token.utf8)
        let updated = SecItemUpdate(query(origin, environment) as CFDictionary, [kSecValueData as String: value] as CFDictionary)
        if updated == errSecSuccess { return }
        guard updated == errSecItemNotFound else {
            throw T3Failure(kind: "Keychain", message: "The server credential could not be saved in Keychain.")
        }
        var item = query(origin, environment)
        item[kSecValueData as String] = value
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        guard SecItemAdd(item as CFDictionary, nil) == errSecSuccess else {
            throw T3Failure(kind: "Keychain", message: "The server credential could not be saved in Keychain.")
        }
    }

    func forget(origin: String, environment: String) throws {
        guard persistent else { volatile.removeValue(forKey: Self.account(origin: origin, environment: environment)); return }
        let status = SecItemDelete(query(origin, environment) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw T3Failure(kind: "Keychain", message: "The saved credential could not be removed from Keychain.")
        }
    }

    static func account(origin: String, environment: String) -> String { origin + "\n" + environment }

    private func query(_ origin: String, _ environment: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
         kSecAttrAccount as String: Self.account(origin: origin, environment: environment)]
    }
}

/// Environments this client has paired with, in the order they were added
/// (rows never jump when one reconnects): one entry per environment id with its
/// identity, display facts, whether this device keeps it switched on, and its
/// ordered routes (lane environment-routes; T3 Code 1e2ecbd975 catalog.ts
/// alternateRoutes). A route is `{ id, origin, kind, learned, credential,
/// authorization?, ssh? }`: `credential` names the origin whose Keychain account
/// (`T3Credentials`, origin + environment) holds its token, so a learned route
/// borrows its source route's token. The entry's `origin` is its home: the first
/// address it was saved under, kept while that route exists, and the key the
/// Connections rows and the fleet use. Entries saved before routes (one per
/// origin) migrate to one entry per environment with one route each.
/// Agent runs keep the list in memory, as they keep credentials.
final class T3SavedEnvironments: @unchecked Sendable {
    let persistent: Bool
    private var memory: [[String: Any]] = []
    private let key = "t3.saved.environments"
    private let lock = NSLock()
    init(persistent: Bool) { self.persistent = persistent }

    var all: [[String: Any]] {
        lock.lock(); defer { lock.unlock() }
        return read()
    }
    private func read() -> [[String: Any]] {
        let raw = persistent ? ((UserDefaults.standard.array(forKey: key) as? [[String: Any]]) ?? []) : memory
        let migrated = Self.migrated(raw)
        if migrated.changed { store(migrated.list) }
        return migrated.list
    }
    private func store(_ list: [[String: Any]]) {
        if persistent { UserDefaults.standard.set(list, forKey: key) } else { memory = list }
    }
    static func trimmed(_ origin: String) -> String {
        var text = origin.trimmingCharacters(in: .whitespacesAndNewlines)
        while text.hasSuffix("/") { text.removeLast() }
        return text
    }
    /// A paired route: its id and its credential are its own origin.
    static func route(origin: String) -> [String: Any] {
        let text = trimmed(origin)
        return ["id": text, "origin": text, "kind": "", "learned": false, "credential": text]
    }
    /// One entry per environment id, each with at least one route. Duplicate
    /// entries (one per origin, before routes) join the first one's routes.
    static func migrated(_ raw: [[String: Any]]) -> (list: [[String: Any]], changed: Bool) {
        var list: [[String: Any]] = [], index: [String: Int] = [:], changed = false
        for var entry in raw {
            let environment = entry["environmentId"] as? String ?? ""
            var routes = (entry["routes"] as? [[String: Any]] ?? []).filter { !($0["id"] as? String ?? "").isEmpty }
            if routes.isEmpty, let origin = entry["origin"] as? String, !origin.isEmpty { routes = [route(origin: origin)]; changed = true }
            if !environment.isEmpty, let at = index[environment] {
                var merged = list[at]["routes"] as? [[String: Any]] ?? []
                for route in routes where !merged.contains(where: { $0["id"] as? String == route["id"] as? String }) { merged.append(route) }
                list[at]["routes"] = merged
                changed = true
                continue
            }
            entry["routes"] = routes
            if !environment.isEmpty { index[environment] = list.count }
            list.append(entry)
        }
        return (list, changed)
    }
    private static func has(_ entry: [String: Any], origin: String) -> Bool {
        let wanted = trimmed(origin)
        if trimmed(entry["origin"] as? String ?? "") == wanted { return true }
        return (entry["routes"] as? [[String: Any]] ?? []).contains { trimmed($0["origin"] as? String ?? "") == wanted }
    }

    /// Refreshes an entry's display facts in place, or appends a new one
    /// switched on. A reconnect never reorders the list or switches it back on.
    /// An address this environment was not saved under joins its routes, last
    /// (the app then places it by kind): pairing the same machine at a second
    /// address adds a route, not a row.
    func remember(origin: String, descriptor: [String: Any]) {
        guard let environment = descriptor["environmentId"] as? String, !environment.isEmpty else { return }
        let platform = descriptor["platform"] as? [String: Any] ?? [:]
        lock.lock(); defer { lock.unlock() }
        var list = read()
        var entry: [String: Any] = ["origin": Self.trimmed(origin), "environmentId": environment, "label": descriptor["label"] as? String ?? "",
                                    "machine": platform["machine"] as? String ?? "", "os": platform["os"] as? String ?? "",
                                    "serverVersion": descriptor["serverVersion"] as? String ?? "", "enabled": true,
                                    "pairedAt": ISO8601DateFormatter().string(from: Date()), "routes": [Self.route(origin: origin)]]
        if let index = list.firstIndex(where: { $0["environmentId"] as? String == environment }) {
            let previous = list[index]
            entry["origin"] = previous["origin"] ?? entry["origin"]
            entry["enabled"] = previous["enabled"] as? Bool ?? true
            entry["pairedAt"] = previous["pairedAt"] ?? entry["pairedAt"]
            if let label = previous["mobileLabel"] { entry["mobileLabel"] = label }
            var routes = previous["routes"] as? [[String: Any]] ?? []
            if !Self.has(previous, origin: origin) { routes.append(Self.route(origin: origin)) }
            entry["routes"] = routes
            list[index] = entry
        } else {
            list.append(entry)
        }
        store(Array(list.suffix(32)))
    }

    // upstream 365aa87982 onboarding.ts updateBearer: edit the first direct route, retaining its identity.
    func updateMobileBearer(environment: String, label: String, origin: String) throws {
        lock.lock(); defer { lock.unlock() }
        var list = read()
        guard let at = list.firstIndex(where: { $0["environmentId"] as? String == environment }) else {
            throw T3Failure(kind: "Missing", message: "That environment is no longer saved on this device.")
        }
        var routes = list[at]["routes"] as? [[String: Any]] ?? []
        guard let route = routes.firstIndex(where: { !["ssh", "relay"].contains($0["kind"] as? String ?? "") && $0["authorization"] as? String != "t3-connect" }) else {
            throw T3Failure(kind: "Arguments", message: "Only saved bearer environments can be edited.")
        }
        let previous = Self.trimmed(routes[route]["origin"] as? String ?? "")
        routes[route]["origin"] = Self.trimmed(origin)
        // id and credential still name the original connection; no token is copied into the catalog.
        list[at]["routes"] = routes
        list[at]["mobileLabel"] = label
        if Self.trimmed(list[at]["origin"] as? String ?? "") == previous { list[at]["origin"] = Self.trimmed(origin) }
        store(list)
    }

    /// The environment saved with this id.
    func entry(environment: String) -> [String: Any]? {
        lock.lock(); defer { lock.unlock() }
        return read().first { $0["environmentId"] as? String == environment }
    }
    /// The environment one of whose routes (or whose home) is this origin.
    func entry(origin: String) -> [String: Any]? {
        lock.lock(); defer { lock.unlock() }
        let list = read()
        return list.first { Self.trimmed($0["origin"] as? String ?? "") == Self.trimmed(origin) } ?? list.first { Self.has($0, origin: origin) }
    }

    /// The app's route edits (reorder, remove, learned routes, a new route placed by kind).
    /// Returns the routes that left the list, so their credentials can be forgotten.
    @discardableResult
    func setRoutes(environment: String, routes: [[String: Any]]) throws -> [[String: Any]] {
        guard !routes.isEmpty, routes.count <= 32 else { throw T3Failure(kind: "Arguments", message: "A saved environment needs one to 32 routes.") }
        var ids = Set<String>(), cleaned: [[String: Any]] = []
        for route in routes {
            guard let id = route["id"] as? String, !id.isEmpty, id.count <= 2048, ids.insert(id).inserted,
                  let origin = route["origin"] as? String, let url = URLComponents(string: origin), ["http", "https"].contains(url.scheme ?? ""), url.host?.isEmpty == false,
                  let credential = route["credential"] as? String, !credential.isEmpty else {
                throw T3Failure(kind: "Arguments", message: "Each route needs a unique id, an http(s) origin and a credential owner.")
            }
            let kind = route["kind"] as? String ?? ""
            guard kind == "" || kind == "ssh" else { throw T3Failure(kind: "Arguments", message: "Unknown route kind.") }
            var stored: [String: Any] = ["id": id, "origin": Self.trimmed(origin), "kind": kind, "learned": route["learned"] as? Bool == true, "credential": Self.trimmed(credential)]
            if let ssh = route["ssh"] as? [String: Any] { stored["ssh"] = ssh }
            if route["authorization"] as? String == "t3-connect" { stored["authorization"] = "t3-connect" }
            cleaned.append(stored)
        }
        lock.lock(); defer { lock.unlock() }
        var list = read()
        guard let index = list.firstIndex(where: { $0["environmentId"] as? String == environment }) else {
            throw T3Failure(kind: "Missing", message: "That environment is no longer saved on this device.")
        }
        let previous = list[index]["routes"] as? [[String: Any]] ?? []
        list[index]["routes"] = cleaned
        // The home stays while its route does; otherwise the preferred route's address becomes the home.
        if !cleaned.contains(where: { $0["origin"] as? String == Self.trimmed(list[index]["origin"] as? String ?? "") && $0["learned"] as? Bool != true }) {
            list[index]["origin"] = (cleaned.first { $0["learned"] as? Bool != true } ?? cleaned[0])["origin"]
        }
        store(list)
        return previous.filter { old in !ids.contains(old["id"] as? String ?? "") }
    }

    /// The Environments switch: whether this device keeps a connection open.
    @discardableResult
    func setEnabled(origin: String, environment: String, enabled: Bool) -> Bool {
        lock.lock(); defer { lock.unlock() }
        var list = read()
        guard let index = list.firstIndex(where: { $0["environmentId"] as? String == environment }) else { return false }
        list[index]["enabled"] = enabled
        store(list)
        return true
    }

    /// Forgets the environment with every route; returns the forgotten entry.
    @discardableResult
    func forget(origin: String, environment: String) -> [String: Any]? {
        lock.lock(); defer { lock.unlock() }
        let list = read()
        let gone = list.first { $0["environmentId"] as? String == environment }
        store(list.filter { $0["environmentId"] as? String != environment })
        return gone
    }
    /// The Keychain origins an entry's routes own (learned routes borrow, never own).
    static func credentialOrigins(_ entry: [String: Any]?) -> [String] {
        guard let entry else { return [] }
        var origins = [trimmed(entry["origin"] as? String ?? "")]
        for route in entry["routes"] as? [[String: Any]] ?? [] where route["learned"] as? Bool != true {
            origins.append(trimmed(route["credential"] as? String ?? route["origin"] as? String ?? ""))
        }
        var seen = Set<String>()
        return origins.filter { !$0.isEmpty && seen.insert($0).inserted }
    }

    /// Device-local Connections preferences beside the catalog (load balancing
    /// and GitHub sharing): one JSON object, at most 64 KB.
    private var memoryPreferences = "{}"
    private let preferencesKey = "t3.connection.preferences"
    var preferences: String {
        lock.lock(); defer { lock.unlock() }
        return persistent ? (UserDefaults.standard.string(forKey: preferencesKey) ?? "{}") : memoryPreferences
    }
    func setPreferences(_ text: String) throws {
        guard text.utf8.count <= 65_536, let data = text.data(using: .utf8),
              (try? JSONSerialization.jsonObject(with: data)) is [String: Any] else {
            throw T3Failure(kind: "Arguments", message: "Connection preferences must be a JSON object up to 64 KB.")
        }
        lock.lock(); defer { lock.unlock() }
        if persistent { UserDefaults.standard.set(text, forKey: preferencesKey) } else { memoryPreferences = text }
    }
}

#endif
