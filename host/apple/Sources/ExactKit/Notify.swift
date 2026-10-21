// `showNotification(title=, body=, tag=, showTrigger=, navigate=)` and
// `closeNotification(tag)` (rules/DEFERRED.md, 2026-10-04; x2apps habits
// F13), in ExactKit so an embedder gets them with the session. The runner
// rules first (runner/src/notify.rs): bad data or no `device.notifications`
// refused, and under the agent the notification listed for
// `state.notifications`, never posted. Then UNUserNotificationCenter:
// authorization asked the first time, the content posted now or at
// `showTrigger` (the system keeps it, so it arrives with the app closed),
// identified by its tag, so a newer one replaces it and `closeNotification`
// takes it away, pending or delivered. It shows with the app in front too
// (the centre's delegate, unless the app set its own). The outcome is a
// journal line, as `share`'s is.
import Foundation
#if !os(tvOS)
import UserNotifications
#endif

extension ExactSession {
    func notify(_ name: String, _ args: [Any]) {
        let text = { (i: Int) -> String? in i < args.count ? args[i] as? String : nil }
        var request: [String: Any] = ["command": name, "agent": ExactEnv.agentMode]
        let keys = name == "closeNotification" ? ["tag"] : ["title", "body", "tag"]
        for (i, key) in keys.enumerated() {
            if let value = text(i) { request[key] = value }
        }
        let at = args.count > 3 ? (args[3] as? NSNumber)?.doubleValue : nil
        if name == "showNotification", let at { request["showTrigger"] = at }
        if name == "showNotification", let location = text(4) { request["navigate"] = location }
        guard runtime.command(request)["present"] as? Bool == true else { return }
        let log: (String) -> Void = { [weak self] line in DispatchQueue.main.async { self?.log(line) } }
        #if os(tvOS)
        log("showNotification: refused: unavailable")
        #else
        // An unbundled process has no notification centre (it traps).
        guard NotificationPresenter.available else {
            if name == "showNotification" { log("showNotification: refused: unavailable outside an app bundle") }
            return
        }
        let center = UNUserNotificationCenter.current()
        if name == "closeNotification" {
            guard let tag = text(0) else { return }
            center.removePendingNotificationRequests(withIdentifiers: [tag])
            center.removeDeliveredNotifications(withIdentifiers: [tag])
            return
        }
        if center.delegate == nil { app.installNotificationClicks() }
        let content = UNMutableNotificationContent()
        content.title = text(0) ?? ""
        if let body = text(1) { content.body = body }
        if let location = text(4) {
            content.userInfo = ["navigate": location, "session": label,
                                "owner": notificationOwnerID, "process": app.notificationProcessID]
        }
        content.sound = .default
        let wait = at.map { $0 / 1000 - Date().timeIntervalSince1970 } ?? 0
        let trigger = wait > 0 ? UNTimeIntervalNotificationTrigger(timeInterval: max(1, wait), repeats: false) : nil
        let posted = UNNotificationRequest(identifier: text(2) ?? UUID().uuidString, content: content, trigger: trigger)
        center.requestAuthorization(options: [.alert, .sound]) { granted, error in
            guard granted else {
                return log("showNotification: refused: \(error?.localizedDescription ?? "denied")")
            }
            center.add(posted) { error in
                log(error.map { "showNotification: refused: \($0.localizedDescription)" }
                    ?? (trigger == nil ? "showNotification: shown" : "showNotification: scheduled"))
            }
        }
        #endif
    }
}

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

    func deliver(id: String, date: Date, info: [AnyHashable: Any]) {
        guard let location = info["navigate"] as? String, !location.isEmpty,
              let label = info["session"] as? String,
              let owner = info["owner"] as? String else { return }
        let key = "\(id):\(date.timeIntervalSince1970)"
        guard delivered.insert(key).inserted else { return }
        order.append(key)
        if order.count > 128 { delivered.remove(order.removeFirst()) }
        let sessions = app?.sessions.filter { $0.state != .destroyed } ?? []
        let sameProcess = (info["process"] as? String).map { $0 == app?.notificationProcessID } ?? true
        let session: ExactSession?
        if sameProcess { session = sessions.first { $0.notificationOwnerID == owner } }
        else { session = sessions.first { $0.label == label } ?? sessions.first }
        guard let session else {
            let line = "notification click refused: session \(label) is gone"
            sessions.forEach { $0.log(line) }
            if sessions.isEmpty { FileHandle.standardError.write(Data("\(line)\n".utf8)) }
            return
        }
        session.openLocation(location)
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
        completionHandler([.banner, .list, .sound])
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
