// @exact/login-item on macOS (LLP 1133.025 D2): the app's own login item,
// `SMAppService.mainApp` (macOS 13 and later). The app's composed module
// calls it outside the agent; under the agent the runner's substitute
// answers, unless the drive binds it (`--bind login-item`).
//
// - `hidden` is this module's own record, in the app's defaults: macOS 13's
//   service has no hidden flag, so the host reads the record at a login
//   launch and opens no window (D6).
// - `launchedAtLogin` is the host's reading of the open-application event's
//   `keyAELaunchedAsLogInItem`, which it registers, unpersisted, as the
//   default `exact.launch.login` (`LaunchKind`, NoWindowLaunchMac.swift).
// - A registration needs a signed app in a location Launch Services knows;
//   the system's own notice of a new login item is the system's.
import ExactPackage
import Foundation
import ServiceManagement

public final class LoginItemModule: ExactPackage {
    private let context: ExactPackageContext
    private static let hiddenKey = "exact.loginItem.hidden"

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "loginItemStatus": reply(status())
            case "registerLoginItem": change(register: true, hidden: args["hidden"] as? Bool == true, reply)
            case "unregisterLoginItem": change(register: false, hidden: false, reply)
            case "openLoginItemSettings":
                SMAppService.openSystemSettingsLoginItems()
                reply([:])
            default: reply(["error": "NotSupportedError"])
            }
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "loginItemStatusSync" ? status() : ["error": "NotSupportedError"]
    }

    /// The person can turn the item off in System Settings while the app is
    /// away: asked again on return.
    public func foreground(_ shown: Bool) {
        if shown { context.changed("login-item.loginItemStatus") }
    }

    private func change(register: Bool, hidden: Bool, _ reply: @escaping ([String: Any]) -> Void) {
        let service = SMAppService.mainApp
        do {
            if register {
                // Registering an enabled item again only updates the record.
                if service.status != .enabled { try service.register() }
                UserDefaults.standard.set(hidden, forKey: Self.hiddenKey)
            } else {
                if service.status != .notRegistered { try service.unregister() }
                UserDefaults.standard.removeObject(forKey: Self.hiddenKey)
            }
        } catch {
            let ns = error as NSError
            context.log("login-item: \(register ? "register" : "unregister") failed: \(ns.domain) \(ns.code) \(ns.localizedDescription)")
            // Launch Services cannot find a registrable app (a development
            // build run from a scratch location) or the person refused it.
            return reply(["error": "NotAllowedError", "message": ns.localizedDescription])
        }
        context.changed("login-item.loginItemStatus")
        reply(status())
    }

    private func status() -> [String: Any] {
        let service = SMAppService.mainApp.status
        let (state, apple): (String, String) = switch service {
        case .enabled: ("enabled", "enabled")
        case .requiresApproval: ("requires-approval", "requiresApproval")
        case .notRegistered: ("disabled", "notRegistered")
        case .notFound: ("unknown", "notFound")
        @unknown default: ("unknown", "notFound")
        }
        let launched = UserDefaults.standard.bool(forKey: "exact.launch.login")
        return [
            "available": true, "state": state,
            "hidden": UserDefaults.standard.bool(forKey: Self.hiddenKey),
            "launchedAtLogin": launched,
            "apple": ["status": apple, "loginLaunch": launched ? "marked" : "unmarked"],
            "windows": NSNull(),
        ]
    }
}
