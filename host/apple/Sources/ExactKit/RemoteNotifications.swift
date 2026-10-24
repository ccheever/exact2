import Foundation

/// APNs callbacks for an app-owned native module. Payloads stay opaque to Exact.
public enum ExactRemoteNotifications {
    public static var enabled: Bool { Bundle.main.object(forInfoDictionaryKey: "ExactPushNotifications") as? Bool == true }
    public static let pendingKey = "ExactPendingRemoteNotification"
    public static let tokenEvent = Notification.Name("ExactRemoteNotificationToken")
    public static let errorEvent = Notification.Name("ExactRemoteNotificationError")
    public static let openedEvent = Notification.Name("ExactRemoteNotificationOpened")

    public static func registered(_ token: Data) {
        NotificationCenter.default.post(name: tokenEvent, object: token)
    }

    public static func failed(_ error: Error) {
        NotificationCenter.default.post(name: errorEvent, object: error.localizedDescription)
    }

    /// A launching tap can precede module loading. Keep its JSON until the app acknowledges it.
    static func opened(_ info: [AnyHashable: Any], defaults: UserDefaults = .standard, center: NotificationCenter = .default) {
        guard JSONSerialization.isValidJSONObject(info), let data = try? JSONSerialization.data(withJSONObject: info) else { return }
        defaults.set(data, forKey: pendingKey)
        center.post(name: openedEvent, object: info)
    }
}
