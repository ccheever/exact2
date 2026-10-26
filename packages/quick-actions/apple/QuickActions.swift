// @exact/quick-actions on Apple (LLP 1133.023 D3, D14, D17): the app's list
// set into the host's store, `context.shortcuts`, which iOS shows as Home
// Screen quick actions after the manifest's and the Mac in the Dock menu
// (ExactKit's ShortcutItems.swift). Pinning a shortcut is Android's alone.
import ExactPackage
import Foundation

public final class QuickActionsModule: ExactPackage {
    private let context: ExactPackageContext

    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        DispatchQueue.main.async { [self] in
            switch name {
            case "setAppShortcuts":
                let items = args["shortcuts"] as? [[String: Any]] ?? []
                // A location is an app path (D3); one that is not refuses the whole list.
                guard items.allSatisfy({ ($0["url"] as? String)?.hasPrefix("/") == true && ($0["name"] as? String)?.isEmpty == false }) else {
                    return reply(["error": "TypeError"])
                }
                context.shortcuts.set(items)
                reply([:])
            case "clearAppShortcuts":
                context.shortcuts.set([])
                reply([:])
            default:
                reply(["error": "NotSupportedError"])
            }
        }
    }
}
