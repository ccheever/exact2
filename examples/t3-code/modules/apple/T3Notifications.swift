// Thread notifications on macOS (reference: ThreadNotificationCoordinator.tsx,
// threadNotifications.ts, apps/desktop notificationBadge.ts): notification-center
// posts tagged per thread (a click focuses the app and asks the window to open
// that thread), the completion/input sounds and the Dock badge of pending
// notifications, cleared when the window gains focus. A click's action and the
// badge stay here (exact2 #224, refused by DEFERRED); whether the window has the
// focus is the page's `exactPage().hasFocus` (exact2 #219, shell-notify.ts).
// An agent-launched app never asks for notification permission; it reports
// what the notification center already says and labels itself as such.
import AppKit
import UserNotifications

final class T3Notifications: NSObject, UNUserNotificationCenterDelegate {
    private let agent: Bool
    private let changed: (String) -> Void
    private var observers: [NSObjectProtocol] = []
    private(set) var pending: [String] = []
    private(set) var authorization = "unknown"
    private(set) var opened = ""
    private(set) var openedThread = ""
    private var openedSeq = 0
    private var sound: NSSound?
    private lazy var center: UNUserNotificationCenter? = Bundle.main.bundleIdentifier == nil ? nil : UNUserNotificationCenter.current()

    init(agent: Bool, changed: @escaping (String) -> Void) {
        self.agent = agent
        self.changed = changed
        super.init()
        DispatchQueue.main.async { [weak self] in self?.install() }
    }

    private func install() {
        let notes = NotificationCenter.default
        // The badge clears when the window gains focus (notificationBadge.ts); the page
        // reads focus itself (exactPage().hasFocus), so only a click needs the window to act.
        for name in [NSApplication.didBecomeActiveNotification, NSWindow.didBecomeKeyNotification] {
            observers.append(notes.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in self?.clear() })
        }
        center?.delegate = self
        refreshAuthorization()
    }

    private func refreshAuthorization(then done: (() -> Void)? = nil) {
        guard let center else { authorization = "unavailable"; done?(); return }
        center.getNotificationSettings { [weak self] settings in
            let value: String
            switch settings.authorizationStatus {
            case .authorized: value = "authorized"
            case .denied: value = "denied"
            case .provisional: value = "provisional"
            case .notDetermined: value = "notDetermined"
            default: value = "unknown"
            }
            DispatchQueue.main.async {
                guard let self else { return }
                self.authorization = value
                done?()
            }
        }
    }

    /// The window gained focus (or the mode changed): pending notifications and the badge clear.
    func clear() {
        if !pending.isEmpty { center?.removeDeliveredNotifications(withIdentifiers: pending) }
        pending.removeAll()
        NSApp.dockTile.badgeLabel = nil
    }

    /// playThreadNotificationSound: completion for a finished turn, input for attention.
    static func soundURL(_ kind: String, assets: String? = ProcessInfo.processInfo.environment["EXACT_ASSETS"], resources: URL? = Bundle.main.resourceURL) -> URL? {
        let file = kind == "completion" ? "notification-completion.mp3" : "notification-input.mp3"
        var candidates: [URL] = []
        if let root = assets, root.hasPrefix("/") { candidates.append(URL(fileURLWithPath: root).appendingPathComponent("assets/\(file)")) }
        if let resources { candidates.append(resources.appendingPathComponent("assets/\(file)")) }
        return candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) })
    }

    private func play(_ kind: String) -> Bool {
        guard let path = Self.soundURL(kind), let next = NSSound(contentsOf: path, byReference: false) else { return false }
        sound?.stop(); sound = next
        return next.play()
    }

    /// A posted notification is pending until the window gains focus; the Dock badge counts them.
    func record(tag: String) {
        pending.removeAll { $0 == tag }
        pending.append(tag)
        NSApp.dockTile.badgeLabel = "\(pending.count)"
    }

    /// A clicked notification: focus the window and ask it to open that thread.
    func open(threadId: String) {
        NSApp.activate(ignoringOtherApps: true)
        NSApp.windows.first { $0.isVisible }?.makeKeyAndOrderFront(nil)
        if !threadId.isEmpty {
            openedSeq += 1
            opened = "\(openedSeq):\(threadId)"
            openedThread = threadId
        }
        changed("t3.notify")
    }

    /// The page posts only while its window has no focus (`exactPage().hasFocus`), as the coordinator does.
    private func post(title: String, body: String, tag: String, threadId: String) -> Bool {
        guard let center else { return false }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.userInfo = ["threadId": threadId, "tag": tag]
        // Silent: the sound modes play their own sound (Notification({ silent: true })).
        content.sound = nil
        center.add(UNNotificationRequest(identifier: tag, content: content, trigger: nil)) { _ in }
        record(tag: tag)
        return true
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse, withCompletionHandler completionHandler: @escaping () -> Void) {
        let info = response.notification.request.content.userInfo
        let threadId = info["threadId"] as? String ?? ""
        DispatchQueue.main.async { [weak self] in
            self?.open(threadId: threadId)
            completionHandler()
        }
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification, withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        // Posted only while unfocused; a focused window shows its own toast.
        completionHandler([.banner, .list])
    }

    private func status() -> [String: Any] {
        ["authorization": authorization, "agent": agent, "opened": opened, "openedThread": openedThread, "pending": pending.count]
    }

    /// Requests: notifyStatus, notifyAuthorize, notifyPost, notifySound, notifyClear. Main thread.
    func perform(_ request: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        let generation = request["generation"] as? Int ?? 0
        let answer = { (value: [String: Any]) in reply(["ok": true, "generation": generation, "value": value]) }
        switch request["op"] as? String ?? "" {
        case "notifyStatus":
            answer(status())
        case "notifyAuthorize":
            // NotificationSettings asks when a notification mode is chosen and
            // answers with the outcome. The agent-launched app never prompts:
            // it answers with what the notification center already says.
            guard let center else { answer(status().merging(["requested": false]) { $1 }); return }
            center.getNotificationSettings { [weak self] settings in
                let decided = settings.authorizationStatus != .notDetermined
                DispatchQueue.main.async {
                    guard let self else { return }
                    if decided || self.agent {
                        self.refreshAuthorization(then: { answer(self.status().merging(["requested": false]) { $1 }) })
                        return
                    }
                    center.requestAuthorization(options: [.alert, .badge]) { _, _ in
                        DispatchQueue.main.async { self.refreshAuthorization(then: { answer(self.status().merging(["requested": true]) { $1 }) }) }
                    }
                }
            }
        case "notifyPost":
            let posted = post(title: request["title"] as? String ?? "", body: request["body"] as? String ?? "",
                              tag: request["tag"] as? String ?? UUID().uuidString, threadId: request["threadId"] as? String ?? "")
            answer(status().merging(["posted": posted]) { $1 })
        case "notifySound":
            answer(["played": play(request["kind"] as? String ?? "completion")])
        case "notifyClear":
            clear()
            answer(status())
        default:
            reply(["ok": false, "generation": generation, "error": ["kind": "Notifications", "message": "Unknown notification request.", "uncertain": false]])
        }
    }

    func destroy() {
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers.removeAll()
        clear()
        if center?.delegate === self { center?.delegate = nil }
    }
}
