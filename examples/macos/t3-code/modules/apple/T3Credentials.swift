import Foundation
import Security

/// Access tokens remain in the app's Keychain account, never in a Contract value.
struct T3Credentials {
    let persistent: Bool
    private let service = "com.exact.t3code.macos.access-token"

    func read(origin: String, environment: String) throws -> String? {
        guard persistent else { return nil }
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
        guard persistent else { return }
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
        guard persistent else { return }
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
