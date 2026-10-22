// @fixture/bell on Apple: a module outside exact2's tree, composed into the
// app's module as any package's implementation is (LLP 1133.001 D5). A ring
// is a notification whose `userInfo` is the host's activation entry, so the
// host's delegate raises `ringclick` when a person opens it; a tag that
// starts with `chime` posts nothing and emits `chime` a moment later, which
// the runner gives the device's origin. The permission is the fixture's own,
// `prompt` until `requestBell` or a ring other than a chime.
import Foundation
#if !os(tvOS)
import UserNotifications
#endif

public final class BellModule {
    private let changed: (String) -> Void
    private let log: (String) -> Void
    private let emit: (String, [Any]) -> Void
    private let activation: (String, [Any], String?) -> [AnyHashable: Any]
    private var state = "prompt"
    private var count: Any = NSNull()

    public init(changed: @escaping (String) -> Void, agent: Bool, log: @escaping (String) -> Void,
                emit: @escaping (String, [Any]) -> Void,
                activation: @escaping (String, [Any], String?) -> [AnyHashable: Any]) {
        self.changed = changed
        self.log = log
        self.emit = emit
        self.activation = activation
    }

    public func later(_ name: String, _ args: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "bellStatus": reply(status())
            case "requestBell":
                grant()
                reply(status())
            case "ring":
                ring(args)
                reply([:])
            case "silence":
                if let tag = args["tag"] as? String { withdraw(tag) }
                reply([:])
            case "setCount":
                count = args["contents"] ?? "flag"
                log("bell: count \(count)")
                reply([:])
            case "clearCount":
                count = NSNull()
                log("bell: count cleared")
                reply([:])
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        ["error": "NotSupportedError"]
    }

    public func release(_ name: String) {}

    private func status() -> [String: Any] { ["available": true, "state": state] }

    /// An unbundled process has no notification centre: `current()` traps.
    private static var centre: Bool {
        #if os(macOS)
        Bundle.main.bundleURL.pathExtension == "app"
        #else
        Bundle.main.bundleIdentifier != nil
        #endif
    }

    private func grant() {
        guard state == "prompt" else { return }
        state = "granted"
        changed("bell.bellStatus")
    }

    private func ring(_ args: [String: Any]) {
        guard let tag = args["tag"] as? String else { return }
        if tag.hasPrefix("chime") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.02) { [self] in emit("chime", [tag]) }
            return
        }
        grant()
        #if !os(tvOS)
        guard Self.centre else { return log("bell: no notification centre outside an app bundle") }
        let content = UNMutableNotificationContent()
        content.title = "bell \(tag)"
        content.userInfo = activation("ringclick", [tag, ["tag": tag, "data": args["data"] ?? NSNull()]], args["navigate"] as? String)
        let center = UNUserNotificationCenter.current()
        center.requestAuthorization(options: [.alert]) { [self] granted, _ in
            guard granted else { return log("bell: ring refused: denied") }
            center.add(UNNotificationRequest(identifier: tag, content: content, trigger: nil)) { [self] error in
                log(error.map { "bell: ring refused: \($0.localizedDescription)" } ?? "bell: rang \(tag)")
            }
        }
        #endif
    }

    private func withdraw(_ tag: String) {
        #if !os(tvOS)
        guard Self.centre else { return }
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: [tag])
        center.removeDeliveredNotifications(withIdentifiers: [tag])
        #endif
    }
}
