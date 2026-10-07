#if os(iOS)
// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
import UIKit

/// The shared transport owns I/O and credentials; mobile supplies UIKit presentation and lifecycle.
final class T3MobileModule: ExactModule {
    override class var views: [String: ExactNativeFactory] { ["t3-symbol": T3SymbolView.factory, "t3-qr-scanner": T3QRScanner.factory,
        "t3-mobile-terminal": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.terminal.makeView(props: props, events: events)
        },
        "t3-document-audio": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.audio.makeView(dataRoot: module.documentRoot, props: props, events: events)
        },
        "t3-information-legal": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.informationLegal.makeView(props: props, events: events)
        },
        "t3-information-search": T3MobileInformationSearch.factory,
        "t3-information-notice": T3MobileInformationNotice.factory,
        "t3-preview-menu": T3MobilePreviewMenu.factory,
        "t3-mobile-browser": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.browser.makeView(props: props, events: events)
        },
        "t3-mobile-devices": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.devices.makeView(props: props, events: events)
        },
        "t3-document-menu": T3MobileDocumentMenu.factory,
        "t3-document-html": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            let instance = T3MobileDocumentHTML(dataRoot: module.documentRoot, audioSession: module.audioSession, events: events)
            try instance.setProps(props); return instance
        },
        "t3-thread-header": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.threadHeader.makeView(props: props, events: events)
        },
        "t3-review-viewport": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.reviewViewport.makeView(props: props, events: events)
        },
        "t3-inspector-chrome": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.inspectorChrome.makeView(props: props, events: events)
        },
        "t3-workspace-layout": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.workspace.makeView(props: props, events: events)
        },
        "t3-layout-facts": T3LayoutFacts.factory,
        "t3-archive-spinner": T3ArchiveSpinner.factory,
        "t3-media-presenter": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.media.makeView(props: props, events: events)
        },
        "t3-thread-days": T3MobileThreadPreferences.factory,
        "t3-settings-slider": T3SettingsSlider.factory,
        "t3-settings-header": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.settingsNavigation.makeView(props: props, events: events)
        },
        "t3-home-chrome": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.homeChrome.makeView(props: props, events: events)
        }] }
    private let queuedEdits: T3MobileQueuedEdit
    private let transport: T3Transport
    private let browser: T3MobileBrowser
    private let devices: T3MobileDevices
    private let fleet: T3Fleet
    private let activity: T3ActivityReporter
    private var alive = true
    let homeChrome = T3HomeChrome()
    private let threadHeader = T3MobileThreadHeader()
    private let reviewViewport = T3MobileReviewViewport()
    private let inspectorChrome = T3MobileInspector()
    lazy var workspace = T3MobileWorkspace(homeChrome: homeChrome, inspector: inspectorChrome)
    let settingsNavigation = T3SettingsNavigation()
    private let scratchClock = T3MobileScratchClock()
    private let alerts = T3MobileAlerts()
    private let releases = T3ReleasePages()
    private let scheduledControls: T3MobileScheduledControls
    private let scheduledNavigation = T3MobileScheduledNavigation()
    private let voice: T3MobileVoice
    let terminal: T3MobileTerminal
    private let documentRoot: URL
    private let information = T3MobileInformation()
    private let informationLegal: T3MobileInformationLegal
    private let audioSession: T3MobileAudioSession
    private let audio: T3MobileAudio
    private let document: T3MobileDocument
    let media: T3MobileMedia
    private let attachments: T3MobileAttachments
    private let homePreferences: T3MobilePreferences

    required init(context: ExactModuleContext) {
        let audioSession = T3MobileAudioSession()
        self.audioSession = audioSession
        informationLegal = T3MobileInformationLegal(agent: context.agent, audioSession: audioSession)
        audio = T3MobileAudio(session: audioSession)
        scheduledControls = T3MobileScheduledControls(agent: context.agent)
        voice = T3MobileVoice(agent: context.agent, audioSession: audioSession, changed: context.changed)
        T3MobileIdentity.configure()
        let directory = T3Storage.dataRoot(agent: context.agent, contextData: context.data)
        let queuedEdits = T3MobileQueuedEdit.shared(root: directory)
        self.queuedEdits = queuedEdits
        documentRoot = directory
        document = T3MobileDocument(dataRoot: directory)
        media = T3MobileMedia(dataRoot: directory, audioSession: audioSession)
        attachments = T3MobileAttachments(dataRoot: directory, agent: context.agent)
        homePreferences = T3MobilePreferences(directory: directory, changed: { context.changed("t3.mobile-preferences") })
        let credentials = T3Credentials(persistent: !context.agent)
        let saved = T3SavedEnvironments(persistent: !context.agent)
        activity = T3ActivityReporter(persistent: !context.agent)
        transport = T3Transport(persistent: !context.agent, dataDirectory: directory, credentials: credentials,
                                savedEnvironments: saved, activity: activity, queuedEdits: queuedEdits, changed: context.changed)
        terminal = T3MobileTerminal(transport: transport)
        browser = T3MobileBrowser(transport: transport, changed: context.changed, agent: context.agent, audioSession: audioSession)
        devices = T3MobileDevices(transport: transport, changed: context.changed, agent: context.agent, audioSession: audioSession)
        fleet = T3Fleet(persistent: !context.agent, credentials: credentials, saved: saved,
                        activity: activity, queuedEdits: queuedEdits, changed: context.changed)
        super.init(context: context)
    }

    override func tabContainer(_ contents: ExactTabContents) -> UIViewController? { workspace.container(contents) }

    override func navigation(_ navigation: ExactNavigation) {
        T3MobileNavigation.configure(navigation)
    }
    override func route(_ route: ExactRoute) {
        T3MobileNavigation.configure(route, formSheet: route.data[.mobileFormSheet] == "true",
            scanActionID: route.data[.mobileScanAction], scannerOpen: route.data[.mobileScannerOpen] == "true",
            tint: route.controller.traitCollection.userInterfaceStyle == .dark ? .white : .black)
        threadHeader.configure(route)
        inspectorChrome.configure(route)
        homeChrome.configure(route)
        settingsNavigation.configure(route)
        scheduledNavigation.configure(route, editor: route.data[.mobileScheduledEditor] == "true", backActionID: "back")
        informationLegal.configure(route)
    }
    override func element(_ element: ExactElement) {
        if element.hatch == .mobileReviewList || element.hatch == .mobileReviewRow { reviewViewport.configure(element) }
        if element.hatch == .mobileVoiceEditor {
            voice.editor.configure(element, owner: element.data[.mobileVoiceOwner] ?? "",
                selectionRevision: Int(element.data[.mobileVoiceSelectionRevision] ?? "0") ?? 0)
        }
    }
    override func elementEnded(_ element: ExactElement) {
        if element.hatch == .mobileReviewList || element.hatch == .mobileReviewRow { reviewViewport.end(element) }
        if element.hatch == .mobileVoiceEditor { voice.editor.end(element) }
    }
    override func routeEnded(_ route: ExactRoute) { threadHeader.end(route); inspectorChrome.end(route); homeChrome.end(route); settingsNavigation.end(route); scheduledNavigation.end(route); informationLegal.end(route) }

    override func later(_ request: [String: Any], reply: ExactReply) {
        guard alive else { reply.fail("The mobile session was closed."); return }
        if (request["op"] as? String == "mobileQueuedEdit" && ["read", "cas", "cleanup", "release", "retire"].contains(request["action"] as? String ?? ""))
            || ["composerAttachRemove", "snapshotDraftRemove"].contains(request["op"] as? String ?? "") {
            let store = queuedEdits
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let value: [String: Any]
                    switch request["action"] as? String {
                    case "read": value = try store.read()
                    case "cas": value = try store.cas(request)
                    case "cleanup": value = try store.cleanup(request)
                    case "retire": value = try store.retire(request)
                    case "release": value = try store.releaseAttachments()
                    default: value = try store.removeAttachment(request)
                    }
                    reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                } catch {
                    let problem = error as? T3Failure ?? T3Failure(kind: "Persistence", message: "The queued edit could not be saved.")
                    reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": problem.json])
                }
            }
            return
        }
        if let key = request["fleet"] as? String { return fleet.perform(key, request) { reply.send($0) } }
        let generation = request["generation"] as? Int ?? 0
        func answer(_ value: [String: Any] = [:]) { reply.send(["ok": true, "generation": generation, "value": value]) }
        switch request["op"] as? String {
        case "timelineSleep":
            guard let milliseconds = request["ms"] as? Double,
                  scratchClock.sleep(milliseconds: milliseconds, reply: { finished in
                      if finished { answer() } else { reply.fail("The mobile session was closed.") }
                  }) else { reply.fail("The scratch wait is invalid."); return }
        case "r10Wake":
            guard request["topic"] as? String == "t3.notify" else { reply.fail("The scratch wake topic is invalid."); return }
            context.changed("t3.notify"); answer()
        case "mobileScheduledMenu", "mobileScheduledTime", "mobileScheduledConfirm":
            scheduledControls.perform(request) { reply.send($0) }
        case "mobileVoice":
            voice.perform(request) { reply.send($0) }
        case "terminalRetain", "mobileTerminalControl", "mobileTerminalPermissions":
            terminal.perform(request) { reply.send($0) }
        case "localBackendStatus":
            // Shared client38352ce also asks mobile. iOS has no embedded server runtime.
            answer(["state": "refused", "refused": "Local T3 servers are not available on iOS."])
        case "mobileHomePreferences", "mobileToggleShelf", "mobilePreferences", "mobilePreferencesPatch":
            homePreferences.perform(request, reply: reply)
        case "mobileBrowser":
            browser.perform(request) { reply.send($0) }
        case "mobileDevices":
            devices.perform(request) { reply.send($0) }
        case "mobileInformation":
            answer(information.read())
        case "mobileDocumentRead":
            document.perform(request) { reply.send($0) }
        case "mobileAudioControl":
            audio.perform(request) { reply.send($0) }
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
        scratchClock.destroy()
        threadHeader.destroy()
        reviewViewport.destroy()
        workspace.destroy(); inspectorChrome.destroy()
        scheduledControls.destroy()
        voice.destroy()
        terminal.destroy()
        browser.destroy(); devices.destroy()
        audio.destroy()
        document.destroy()
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
        ["t3-document-menu": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-information-legal": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-information-search": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-information-notice": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-mobile-browser": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-mobile-devices": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-preview-menu": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-document-audio": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-document-html": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-mobile-terminal": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-media-presenter": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-thread-days": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-settings-slider": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-settings-header": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-archive-spinner": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-symbol": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-qr-scanner": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-thread-header": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-review-viewport": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-inspector-chrome": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-workspace-layout": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-layout-facts": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-home-chrome": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") }]
    }
}
#endif
let exactModule: ExactModule.Type = T3MobileModule.self
