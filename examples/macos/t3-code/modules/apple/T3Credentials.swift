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
    private let service = "com.exact.t3code.macos.access-token"

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
/// (rows never jump when one reconnects): origin, identity, display facts and
/// whether this device keeps it switched on. Their access tokens stay in
/// `T3Credentials`. Agent runs keep the list in memory, as they keep credentials.
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
        guard persistent else { return memory }
        return (UserDefaults.standard.array(forKey: key) as? [[String: Any]]) ?? []
    }
    private func store(_ list: [[String: Any]]) {
        if persistent { UserDefaults.standard.set(list, forKey: key) } else { memory = list }
    }
    private static func matches(_ entry: [String: Any], _ origin: String, _ environment: String) -> Bool {
        entry["origin"] as? String == origin && entry["environmentId"] as? String == environment
    }

    /// Refreshes an entry's display facts in place, or appends a new one
    /// switched on. A reconnect never reorders the list or switches it back on.
    func remember(origin: String, descriptor: [String: Any]) {
        guard let environment = descriptor["environmentId"] as? String, !environment.isEmpty else { return }
        let platform = descriptor["platform"] as? [String: Any] ?? [:]
        lock.lock(); defer { lock.unlock() }
        var list = read()
        var entry: [String: Any] = ["origin": origin, "environmentId": environment, "label": descriptor["label"] as? String ?? "",
                                    "machine": platform["machine"] as? String ?? "", "os": platform["os"] as? String ?? "",
                                    "serverVersion": descriptor["serverVersion"] as? String ?? "", "enabled": true,
                                    "pairedAt": ISO8601DateFormatter().string(from: Date())]
        if let index = list.firstIndex(where: { Self.matches($0, origin, environment) }) {
            entry["enabled"] = list[index]["enabled"] as? Bool ?? true
            entry["pairedAt"] = list[index]["pairedAt"] ?? entry["pairedAt"]
            list[index] = entry
        } else {
            list.append(entry)
        }
        store(Array(list.suffix(32)))
    }

    /// The Environments switch: whether this device keeps a connection open.
    @discardableResult
    func setEnabled(origin: String, environment: String, enabled: Bool) -> Bool {
        lock.lock(); defer { lock.unlock() }
        var list = read()
        guard let index = list.firstIndex(where: { Self.matches($0, origin, environment) }) else { return false }
        list[index]["enabled"] = enabled
        store(list)
        return true
    }

    func forget(origin: String, environment: String) {
        lock.lock(); defer { lock.unlock() }
        store(read().filter { !Self.matches($0, origin, environment) })
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
