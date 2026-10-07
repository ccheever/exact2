#if os(iOS)
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
import UIKit

/// The shared transport owns I/O and credentials; mobile supplies UIKit presentation and lifecycle.
final class T3MobileModule: ExactModule {
    override class var views: [String: ExactNativeFactory] { ["t3-symbol": T3SymbolView.factory, "t3-qr-scanner": T3QRScanner.factory,
        "t3-layout-facts": T3LayoutFacts.factory,
        "t3-archive-spinner": T3ArchiveSpinner.factory,
        "t3-media-presenter": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.media.makeView(props: props, events: events)
        },
        "t3-settings-slider": T3SettingsSlider.factory,
        "t3-settings-header": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.settingsNavigation.makeView(props: props, events: events)
        },
        "t3-home-chrome": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.homeChrome.makeView(props: props, events: events)
        }] }
    private let transport: T3Transport
    private let fleet: T3Fleet
    private let activity: T3ActivityReporter
    private var alive = true
    let homeChrome = T3HomeChrome()
    let settingsNavigation = T3SettingsNavigation()
    private let alerts = T3MobileAlerts()
    private let releases = T3ReleasePages()
    let media: T3MobileMedia
    private let attachments: T3MobileAttachments
    private let homePreferences: T3MobilePreferences

    required init(context: ExactModuleContext) {
        T3MobileIdentity.configure()
        let directory = T3Storage.dataRoot(agent: context.agent, contextData: context.data)
        media = T3MobileMedia(dataRoot: directory)
        attachments = T3MobileAttachments(dataRoot: directory, agent: context.agent)
        homePreferences = T3MobilePreferences(directory: directory, changed: { context.changed("t3.mobile-preferences") })
        let credentials = T3Credentials(persistent: !context.agent)
        let saved = T3SavedEnvironments(persistent: !context.agent)
        activity = T3ActivityReporter(persistent: !context.agent)
        transport = T3Transport(persistent: !context.agent, dataDirectory: directory, credentials: credentials,
                                savedEnvironments: saved, activity: activity, changed: context.changed)
        fleet = T3Fleet(persistent: !context.agent, credentials: credentials, saved: saved,
                        activity: activity, changed: context.changed)
        super.init(context: context)
    }

    override func navigation(_ navigation: ExactNavigation) {
        T3MobileNavigation.configure(navigation)
    }
    override func route(_ route: ExactRoute) {
        T3MobileNavigation.configure(route, formSheet: route.data[.mobileFormSheet] == "true",
            scanActionID: route.data[.mobileScanAction], scannerOpen: route.data[.mobileScannerOpen] == "true",
            tint: route.controller.traitCollection.userInterfaceStyle == .dark ? .white : .black)
        homeChrome.configure(route)
        settingsNavigation.configure(route)
    }
    override func routeEnded(_ route: ExactRoute) { homeChrome.end(route); settingsNavigation.end(route) }

    override func later(_ request: [String: Any], reply: ExactReply) {
        guard alive else { reply.fail("The mobile session was closed."); return }
        if let key = request["fleet"] as? String { return fleet.perform(key, request) { reply.send($0) } }
        let generation = request["generation"] as? Int ?? 0
        func answer(_ value: [String: Any] = [:]) { reply.send(["ok": true, "generation": generation, "value": value]) }
        switch request["op"] as? String {
        case "mobileHomePreferences", "mobileToggleShelf", "mobilePreferences", "mobilePreferencesPatch":
            homePreferences.perform(request, reply: reply)
        case "mobileMediaShare":
            media.perform(request) { reply.send($0) }
        case "mobileAttachmentSource", "composerAttachPick", "composerAttachRead", "composerAttachRemove", "snapshotDraftRead", "snapshotDraftRemove", "mobileAttachmentPreview":
            attachments.perform(request) { reply.send($0) }
        case "uploadAttachment":
            guard let normalized = T3MobileAttachments.uploadRequest(request) else {
                reply.fail("The attachment image is invalid or exceeds 10 MB."); return
            }
            transport.perform(normalized) { reply.send($0) }
        case "mobileReleasePage":
            releases.perform(request, reply: reply)
        case "mobileCameraPermission":
            T3QRScanner.requestCameraPermission(agent: context.agent) { status in answer(["status": status]) }
        case "mobileAlert":
            let title = request["title"] as? String ?? "T3 Code"
            let message = request["message"] as? String ?? ""
            let kind = request["kind"] as? String ?? "info"
            let buttons: [(String, String, UIAlertAction.Style)] = kind == "remove"
                ? [("cancel", "Cancel", .cancel), ("remove", "Remove", .destructive)]
                : kind == "sign-out"
                    ? [("cancel", "Cancel", .cancel), ("sign-out", "Sign out", .destructive)]
                : kind == "delete"
                    ? [("cancel", "Cancel", .cancel), ("delete", "Delete", .destructive)]
                : kind == "update"
                    ? [("cancel", "Cancel", .cancel), ("update", "Update", .default)]
                : kind == "camera-settings"
                    ? [("cancel", "Cancel", .cancel), ("settings", "Open Settings", .default)]
                    : [("ok", "OK", .default)]
            do {
                try alerts.present(title: title, message: message, buttons: buttons) { choice in
                    if choice == "settings", let url = URL(string: UIApplication.openSettingsURLString) {
                        UIApplication.shared.open(url)
                    }
                    answer(["choice": choice])
                }
            } catch { reply.fail(String(describing: error)) }
        case "devicePresentation":
            do {
                // Shared desktop commands also send this op. Device mobile preferences own appearance.
                let preferences = try homePreferences.request(["op": "mobilePreferences"])
                let mode = preferences["themeMode"] as? String ?? "system"
                let style: UIUserInterfaceStyle = mode == "dark" ? .dark : mode == "light" ? .light : .unspecified
                for scene in UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }) {
                    for window in scene.windows { window.overrideUserInterfaceStyle = style }
                }
                answer()
            } catch { reply.fail(String(describing: error)) }
        case "sidebarNotify":
            let delay = request["delay"] as? Double ?? 0
            guard delay.isFinite, delay >= 0 else { reply.fail("The notification delay is invalid."); return }
            // Existing X19: native timer, like T3Sidebar; invalidate only while this module lives.
            DispatchQueue.main.asyncAfter(deadline: .now() + min(delay / 1000, 86_400)) { [weak self] in
                guard let self, self.alive else { return }; self.context.changed("t3.status")
            }
            answer()
        case "mobileOpenURL":
            guard !context.agent else { reply.fail("External links are unavailable in an agent session."); return }
            guard let text = request["url"] as? String, let url = URL(string: text),
                  ["https", "http"].contains(url.scheme?.lowercased() ?? ""), url.host != nil else {
                reply.fail("Choose a valid web address."); return
            }
            UIApplication.shared.open(url) { opened in
                if opened { answer(["opened": true]) } else { reply.fail("The web address could not be opened.") }
            }
        case "copyText":
            guard let text = request["text"] as? String else { reply.fail("copyText requires text."); return }
            if !context.agent { UIPasteboard.general.string = text }
            answer(["copied": !context.agent])
        default:
            transport.perform(request) { reply.send($0) }
        }
    }

    override func destroy() {
        alive = false
        media.destroy()
        attachments.destroy()
        alerts.destroy()
        releases.destroy()
        fleet.destroy(); transport.destroy(); activity.destroy()
    }
}

#else
// The build queries the native view roster on macOS before compiling for iOS.
// Match the calendar app's existing module-discovery convention.
import Foundation
final class T3MobileModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["t3-media-presenter": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-settings-slider": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-settings-header": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-archive-spinner": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-symbol": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-qr-scanner": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-layout-facts": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-home-chrome": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") }]
    }
}
#endif
let exactModule: ExactModule.Type = T3MobileModule.self
