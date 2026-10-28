// @exact/notifications on Apple (LLP 1133.004 D1–D3, D7): UserNotifications
// for posts, the status and the request; the Dock tile's label on macOS and
// `setBadgeCount` on iOS for the badge. The app's composed module calls it
// outside the agent, through `ExactPackage`'s shape; under the agent the
// runner's substitute answers, unless the drive binds it (`--bind
// notifications`).
//
// - The host keeps the one notification delegate (D7). A post's `userInfo`
//   is the host's activation entry, so a person's click raises
//   `notificationclick` on the session that posted it, or opens `navigate`.
// - `silent` leaves `sound` nil; the host's foreground presentation then
//   omits `.sound` too (it reads the content's sound).
// - A post while `notDetermined` asks first (Q4): `.alert`, `.sound` and
//   `.badge`, the options a post and a badge use.
// - The state is a fact (D8): `getNotificationSettings`, read again when
//   the app becomes active, announced when it changes.
// - An unbundled process has no notification centre (`current()` traps), so
//   there `available` is false and every call refuses `NotSupportedError`.
import ExactPackage
import Foundation
#if os(macOS)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif
#if !os(tvOS)
import UserNotifications
#endif

public final class NotificationsModule: NSObject, ExactPackage {
    private let context: ExactPackageContext
    /// The status last announced, to say a change once.
    private var said: String?
    private var watching: NSObjectProtocol?

    public init(context: ExactPackageContext) {
        self.context = context
        super.init()
    }

    deinit { if let watching { NotificationCenter.default.removeObserver(watching) } }

    /// An unbundled process has no notification centre: `current()` traps.
    private static var available: Bool {
        #if os(tvOS)
        false
        #elseif os(macOS)
        Bundle.main.bundleURL.pathExtension == "app"
        #else
        Bundle.main.bundleIdentifier != nil
        #endif
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            guard Self.available else {
                return reply(name == "notificationStatus" || name == "requestNotificationPermission"
                    ? ["available": false, "state": "unknown", "badge": false] : ["error": "NotSupportedError"])
            }
            #if !os(tvOS)
            switch name {
            case "showNotification": show(args, reply)
            case "closeNotification":
                if let tag = args["tag"] as? String {
                    let center = UNUserNotificationCenter.current()
                    center.removePendingNotificationRequests(withIdentifiers: [tag])
                    center.removeDeliveredNotifications(withIdentifiers: [tag])
                }
                reply([:])
            case "setAppBadge":
                // `null` is absent, as the Badging API's omitted `contents` is: the flag.
                let contents = args["contents"] as? NSNumber
                badge(contents, flag: contents == nil, reply)
            case "clearAppBadge": badge(0, flag: false, reply)
            case "notificationStatus":
                watch()
                status { reply($0) }
            case "requestNotificationPermission":
                UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { [self] _, _ in
                    status { status in DispatchQueue.main.async { self.announce(status); reply(status) } }
                }
            default: reply(["error": "NotSupportedError"])
            }
            #endif
        }
    }

    #if !os(tvOS)
    private func show(_ args: [String: Any], _ reply: @escaping ([String: Any]) -> Void) {
        // An empty title is the web's too: the platform then shows the app's name.
        let title = args["title"] as? String ?? ""
        // An untagged post's click carries "" as the web's does; the identifier is the system's alone.
        let tag = args["tag"] as? String ?? ""
        let identifier = tag.isEmpty ? UUID().uuidString : tag
        let content = UNMutableNotificationContent()
        content.title = title
        if let body = args["body"] as? String { content.body = body }
        let silent = args["silent"] as? Bool == true
        // The platform's sound unless `silent` (D2): a hand-built app sets it, as this does.
        content.sound = silent ? nil : .default
        let event: [String: Any] = ["tag": tag, "data": args["data"] ?? NSNull(), "title": title, "body": args["body"] ?? NSNull()]
        content.userInfo = context.activation("notificationclick", [tag, event], navigate: args["navigate"] as? String)
        if let words = args["apple"] as? [String: Any] { apply(words, to: content) }
        let at = (args["showTrigger"] as? NSNumber)?.doubleValue
        let center = UNUserNotificationCenter.current()
        // While `notDetermined` a post asks first (Q4); while denied it refuses.
        // A refusal is journaled once, by the runner, from the reply.
        center.requestAuthorization(options: [.alert, .sound, .badge]) { [self] granted, _ in
            status { status in DispatchQueue.main.async { self.announce(status) } }
            guard granted else { return DispatchQueue.main.async { reply(["error": "NotAllowedError"]) } }
            // The wait from now, once the person answered: `showTrigger` is an instant.
            let wait = at.map { $0 / 1000 - Date().timeIntervalSince1970 } ?? 0
            let trigger = wait > 0 ? UNTimeIntervalNotificationTrigger(timeInterval: max(1, wait), repeats: false) : nil
            let request = UNNotificationRequest(identifier: identifier, content: content, trigger: trigger)
            center.add(request) { [self] error in
                DispatchQueue.main.async { [self] in
                    if error == nil { context.log(trigger == nil ? "showNotification: shown" : "showNotification: scheduled") }
                    reply(error.map { ["error": "UnknownError", "message": $0.localizedDescription] } ?? [:])
                }
            }
        }
    }

    /// The platform's words (D3), each where the content has it.
    private func apply(_ words: [String: Any], to content: UNMutableNotificationContent) {
        if let subtitle = words["subtitle"] as? String { content.subtitle = subtitle }
        if let thread = words["threadIdentifier"] as? String { content.threadIdentifier = thread }
        if let target = words["targetContentIdentifier"] as? String { content.targetContentIdentifier = target }
        if let score = (words["relevanceScore"] as? NSNumber)?.doubleValue { content.relevanceScore = score }
        switch words["interruptionLevel"] as? String {
        case "passive": content.interruptionLevel = .passive
        case "active": content.interruptionLevel = .active
        case "timeSensitive": content.interruptionLevel = .timeSensitive
        default: break
        }
    }

    /// The badge (D1, Q5): the Dock tile's label on macOS, `•` for a flag;
    /// `setBadgeCount` on iOS, 1 for a flag. Zero clears.
    private func badge(_ contents: NSNumber?, flag: Bool, _ reply: @escaping ([String: Any]) -> Void) {
        // Admission refuses a negative count; truncated and kept within `Int` whatever reaches here.
        let count = contents.map { min(max(0, $0.doubleValue.rounded(.towardZero)), 9_007_199_254_740_991) }
        #if os(macOS)
        NSApp.dockTile.badgeLabel = flag ? "•" : count.flatMap { $0 > 0 ? String(Int($0)) : nil }
        reply([:])
        #else
        UNUserNotificationCenter.current().setBadgeCount(flag ? 1 : Int(count ?? 0)) { error in
            DispatchQueue.main.async { reply(error.map { ["error": "UnknownError", "message": $0.localizedDescription] } ?? [:]) }
        }
        #endif
    }

    /// The permission fact (D8, §6), in the interface's shape.
    private func status(_ answer: @escaping ([String: Any]) -> Void) {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            let authorization: String, state: String
            switch settings.authorizationStatus {
            case .notDetermined: (authorization, state) = ("notDetermined", "prompt")
            case .denied: (authorization, state) = ("denied", "denied")
            case .provisional: (authorization, state) = ("provisional", "granted")
            #if os(iOS)
            case .ephemeral: (authorization, state) = ("ephemeral", "granted")
            #endif
            default: (authorization, state) = ("authorized", "granted")
            }
            let word = { (s: UNNotificationSetting) -> String in
                switch s { case .enabled: "enabled"; case .disabled: "disabled"; default: "notSupported" }
            }
            answer(["available": true, "state": state, "badge": true,
                    "apple": ["authorizationStatus": authorization, "alertSetting": word(settings.alertSetting),
                              "soundSetting": word(settings.soundSetting), "badgeSetting": word(settings.badgeSetting)]])
        }
    }

    /// A status read: its readers are asked again when its state changed
    /// since the last one said. Main thread.
    private func announce(_ status: [String: Any]) {
        let state = status["state"] as? String
        guard state != said else { return }
        if said != nil { context.changed("notifications.notificationStatus") }
        said = state
    }

    /// The person may change the setting in Settings: read again whenever
    /// the app becomes active, announced when the state changed.
    private func watch() {
        guard watching == nil else { return }
        #if os(macOS)
        let active = NSApplication.didBecomeActiveNotification
        #else
        let active = UIApplication.didBecomeActiveNotification
        #endif
        watching = NotificationCenter.default.addObserver(forName: active, object: nil, queue: .main) { [weak self] _ in
            self?.status { status in DispatchQueue.main.async { self?.announce(status) } }
        }
        status { [weak self] status in DispatchQueue.main.async { self?.said = status["state"] as? String } }
    }
    #endif
}
