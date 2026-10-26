// The app icon's menu (LLP 1133.023 D3, D15 items 1 and 2): the manifest's
// items, fixed per binary, then the list the app sets, which the host keeps
// in its defaults so it survives a launch with no package loaded.
//
// - iOS: the manifest's are Info.plist's `UIApplicationShortcutItems`, the
//   app's `UIApplication.shortcutItems`; each carries its location in
//   `userInfo["url"]`, delivered cold from the scene's connection options
//   and warm from `windowScene(_:performActionFor:)`.
// - macOS: the Dock menu, `applicationDockMenu(_:)`: the manifest's items
//   (Info.plist's `ExactShortcutItems`, which the build writes), then the
//   app's.
//
// The module side (packages/quick-actions/apple) posts
// `exact.shortcuts.set` `{items}`, the items as the interface's
// `ShortcutItem`, or `[]` to clear them.
import Foundation
#if os(iOS) && !os(tvOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

public enum ExactShortcuts {
    private static let key = "exact.shortcuts"
    private static var installed = false

    /// The app's list, as it last set it.
    static var items: [[String: Any]] {
        (UserDefaults.standard.data(forKey: key).flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [[String: Any]]) ?? []
    }

    /// Hear the module's list. Install at launch.
    public static func install() {
        guard !installed else { return }
        installed = true
        NotificationCenter.default.addObserver(forName: Notification.Name("exact.shortcuts.set"), object: nil, queue: .main) { note in
            guard let body = note.object as? NSDictionary else { return }
            set(body["items"] as? [[String: Any]] ?? [])
        }
        #if os(iOS) && !os(tvOS)
        // The system's copy of the app's list may be from an older build: set it again.
        set(items)
        #endif
    }

    static func set(_ list: [[String: Any]]) {
        let kept = list.filter { ($0["name"] as? String)?.isEmpty == false && ($0["url"] as? String)?.hasPrefix("/") == true }
        if let data = try? JSONSerialization.data(withJSONObject: kept) { UserDefaults.standard.set(data, forKey: key) }
        #if os(iOS) && !os(tvOS)
        // iOS shows the manifest's first, then these, as many as fit (HIG: four in all).
        UIApplication.shared.shortcutItems = kept.map { item in
            let symbol = (item["apple"] as? [String: Any])?["symbol"] as? String
            return UIApplicationShortcutItem(type: item["url"] as? String ?? "/", localizedTitle: item["name"] as? String ?? "",
                                             localizedSubtitle: item["description"] as? String,
                                             icon: symbol.map { UIApplicationShortcutIcon(systemImageName: $0) },
                                             userInfo: ["url": (item["url"] as? String ?? "/") as NSString])
        }
        #endif
    }

    #if os(iOS) && !os(tvOS)
    /// The location a quick action opens.
    public static func location(of item: UIApplicationShortcutItem) -> String? {
        (item.userInfo?["url"] as? String) ?? (item.type.hasPrefix("/") ? item.type : nil)
    }
    #endif

    #if os(macOS)
    /// The Dock menu: the manifest's items, then the app's; nil with none.
    public static func dockMenu() -> NSMenu? {
        let manifest = Bundle.main.object(forInfoDictionaryKey: "ExactShortcutItems") as? [[String: Any]] ?? []
        let all = manifest + items
        guard !all.isEmpty else { return nil }
        let menu = NSMenu()
        for item in all {
            guard let name = item["name"] as? String, let url = item["url"] as? String else { continue }
            let entry = NSMenuItem(title: name, action: #selector(Opener.open(_:)), keyEquivalent: "")
            entry.target = Opener.shared
            entry.representedObject = url
            menu.addItem(entry)
        }
        return menu
    }

    final class Opener: NSObject {
        static let shared = Opener()
        @objc func open(_ sender: NSMenuItem) {
            guard let url = sender.representedObject as? String else { return }
            NSApp.activate(ignoringOtherApps: true)
            ExactContentIndex.open(url)
        }
    }
    #endif
}

extension ExactApp {
    /// The system's ways in that a package's code does not keep (LLP
    /// 1133.023 D15 item 1): the tool door, the content index's mirror and
    /// the icon's menu. Install at launch, before the app finishes launching.
    /// `boot` boots a session with no view for a call that launched the app
    /// with none.
    public func installReach(boot: (() -> Void)? = nil) {
        ExactTools.boot = boot
        ExactTools.install()
        #if !os(tvOS)
        ExactShortcuts.install()
        #endif
    }
}
