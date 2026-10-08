#if os(iOS)
// upstream 365aa87982 authClientMetadata.ts, authorization/remote.ts and mobile-storage.ts.
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
import Foundation
import Security
import UIKit

/// UIKit metadata is sampled by the module on main, then read by transport queues.
enum T3MobileIdentity {
    struct Metadata {
        let deviceType: String
        let osMajorVersion: Int
        let appVersion: String
        var socketParameters: [URLQueryItem] {
            var fields = [URLQueryItem(name: "clientSurface", value: "mobile"),
                          URLQueryItem(name: "clientOs", value: "iOS"),
                          URLQueryItem(name: "clientDeviceType", value: deviceType == "mobile" ? "phone" : deviceType)]
            if osMajorVersion > 0 { fields.append(URLQueryItem(name: "clientOsMajorVersion", value: String(osMajorVersion))) }
            if !appVersion.isEmpty { fields.append(URLQueryItem(name: "clientAppVersion", value: appVersion)) }
            // Expo has a hardware marketing-name catalog. Omit its optional field until we have that exact mapping.
            return fields
        }
    }
    private static let lock = NSLock()
    private static var stored = Metadata(deviceType: "unknown", osMajorVersion: 0, appVersion: "")
    static var metadata: Metadata { lock.lock(); defer { lock.unlock() }; return stored }
    static func configure() {
        precondition(Thread.isMainThread)
        let idiom = UIDevice.current.userInterfaceIdiom
        let next = Metadata(deviceType: idiom == .pad ? "tablet" : idiom == .phone ? "mobile" : "unknown",
                            osMajorVersion: ProcessInfo.processInfo.operatingSystemVersion.majorVersion,
                            appVersion: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "")
        lock.lock(); stored = next; lock.unlock()
    }

    /// Shares the mobile awareness identity across launches, without exposing it to Contract or preferences.
    static func deviceID(persistent: Bool) throws -> String {
        guard persistent else { return UUID().uuidString.lowercased() }
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword,
                                   kSecAttrService as String: "com.exact.t3code.ios.device-id",
                                   kSecAttrAccount as String: "agent-awareness"]
        func read() throws -> String? {
            var request = query
            request[kSecReturnData as String] = true; request[kSecMatchLimit as String] = kSecMatchLimitOne
            var item: CFTypeRef?
            let status = SecItemCopyMatching(request as CFDictionary, &item)
            if status == errSecItemNotFound { return nil }
            guard status == errSecSuccess, let bytes = item as? Data,
                  let value = String(data: bytes, encoding: .utf8), !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw T3Failure(kind: "Keychain", message: "The mobile device identity could not be read.")
            }
            return value
        }
        if let existing = try read() { return existing }
        let value = UUID().uuidString.lowercased()
        var item = query
        item[kSecValueData as String] = Data(value.utf8)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        if status == errSecDuplicateItem, let existing = try read() { return existing }
        guard status == errSecSuccess else { throw T3Failure(kind: "Keychain", message: "The mobile device identity could not be saved.") }
        return value
    }
}

#endif
