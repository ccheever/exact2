// The one notification delegate a process has (LLP 1133.004 D7), in
// ExactKit so an embedder gets it with the session. It is set at launch,
// before the system can deliver a launching click, which a module's
// implementation could not be: a module is made when its session boots.
// A click on content a module posted (`@exact/notifications`) carries the
// host's activation entry (`userInfo["exact"]`): it opens the entry's
// `navigate` location, or raises the module's event on the session that
// posted it, held until that session can hear it. A notification shows
// with the app in front too, as a browser shows a page's, and sounds only
// when its content has a sound (a silent post has none).
import Foundation
#if !os(tvOS)
import UserNotifications
#endif

#if !os(tvOS)
/// Shows a notification while the app is in front, as a browser shows one
/// over its own page; the system would otherwise drop it silently.
final class NotificationPresenter: NSObject, UNUserNotificationCenterDelegate {
    static let shared = NotificationPresenter()
    static var available: Bool {
        #if os(macOS)
        // A sidecar Info.plist gives a bare executable an identifier, but
        // UserNotifications still requires a Launch Services app bundle.
        Bundle.main.bundleURL.pathExtension == "app"
        #else
        Bundle.main.bundleIdentifier != nil
        #endif
    }
    weak var app: ExactApp?
    var activate: ((ExactSession) -> Void)?
    private var delivered = Set<String>()
    private var order: [String] = []

    func install(on app: ExactApp, activate: ((ExactSession) -> Void)?) {
        self.app = app
        self.activate = activate
        guard Self.available else { return }
        let center = UNUserNotificationCenter.current()
        if center.delegate == nil || center.delegate === self { center.delegate = self }
    }

    func deliver(_ response: UNNotificationResponse) {
        guard response.actionIdentifier == UNNotificationDefaultActionIdentifier else { return }
        let notification = response.notification
        let request = notification.request
        deliver(id: request.identifier, date: notification.date, info: request.content.userInfo)
    }

    /// A module's activation entry (LLP 1133.004 D6, D7): the session that
    /// posted it, and a location to open or an event to raise.
    func deliver(id: String, date: Date, info: [AnyHashable: Any]) {
        // A notification an older build scheduled carries its session fields
        // and `navigate` at the top level of `userInfo`.
        guard let entry = info["exact"] as? [AnyHashable: Any] ?? (info["navigate"] != nil ? info : nil),
              let label = entry["session"] as? String,
              let owner = entry["owner"] as? String else { return }
        let location = (entry["navigate"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        let key = "\(id):\(date.timeIntervalSince1970)"
        guard delivered.insert(key).inserted else { return }
        order.append(key)
        if order.count > 128 { delivered.remove(order.removeFirst()) }
        let sessions = app?.sessions.filter { $0.state != .destroyed } ?? []
        let sameProcess = (entry["process"] as? String).map { $0 == app?.notificationProcessID } ?? true
        let session: ExactSession?
        if sameProcess { session = sessions.first { $0.notificationOwnerID == owner } }
        else { session = sessions.first { $0.label == label } ?? sessions.first }
        guard let session else {
            let line = "notification click refused: session \(label) is gone"
            sessions.forEach { $0.log(line) }
            if sessions.isEmpty { FileHandle.standardError.write(Data("\(line)\n".utf8)) }
            return
        }
        if let location { session.openLocation(location) }
        else if let name = entry["extension"] as? String, let event = entry["event"] as? String {
            session.raiseModuleEvent(extension: name, event: event, args: entry["args"] as? String ?? "[]", activation: true)
        }
        activate?(session)
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
                                withCompletionHandler completionHandler: @escaping () -> Void) {
        let handle = {
            self.deliver(response)
            completionHandler()
        }
        if Thread.isMainThread { handle() }
        else { DispatchQueue.main.sync(execute: handle) }
    }
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification,
                                withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        let silent = notification.request.content.sound == nil
        completionHandler(silent ? [.banner, .list] : [.banner, .list, .sound])
    }
}

extension ExactApp {
    /// Install at launch, before the system can deliver a launching tap.
    public func installNotificationClicks(activate: ((ExactSession) -> Void)? = nil) {
        NotificationPresenter.shared.install(on: self, activate: activate)
    }

    /// A scene's launch response may also reach the delegate; one request is delivered once.
    public func openNotification(_ response: UNNotificationResponse) {
        NotificationPresenter.shared.deliver(response)
    }
}
#endif
