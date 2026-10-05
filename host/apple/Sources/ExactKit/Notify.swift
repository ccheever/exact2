// `showNotification(title=, body=, tag=, showTrigger=)` and
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
        for (i, key) in (name == "closeNotification" ? ["tag"] : ["title", "body", "tag"]).enumerated() {
            if let value = text(i) { request[key] = value }
        }
        let at = args.count > 3 ? (args[3] as? NSNumber)?.doubleValue : nil
        if name == "showNotification", let at { request["showTrigger"] = at }
        guard runtime.command(request)["present"] as? Bool == true else { return }
        let log: (String) -> Void = { [weak self] line in DispatchQueue.main.async { self?.log(line) } }
        #if os(tvOS)
        log("showNotification: refused: unavailable")
        #else
        // An unbundled process has no notification centre (it traps).
        guard Bundle.main.bundleIdentifier != nil else {
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
        if center.delegate == nil { center.delegate = NotificationPresenter.shared }
        let content = UNMutableNotificationContent()
        content.title = text(0) ?? ""
        if let body = text(1) { content.body = body }
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
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification,
                                withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        completionHandler([.banner, .list, .sound])
    }
}
#endif
