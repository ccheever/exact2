#if os(iOS)
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
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
        "t3-terminal-menu": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.terminal.menus.makeView(props: props, events: events)
        },
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
        "t3-home-swipe-events": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.homeSwipes.makeView(props: props, events: events)
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
        "t3-composer-material": T3MobileComposerMaterial.factory,
        "t3-composer-attachment-button": T3MobileAttachmentButton.factory,
        "t3-composer-editor": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            let instance = T3MobileComposerEditor(voice: module.voice.editor, operations: module.composerOperations, fileHolds: module.composerFileHolds, paste: module.composerPaste, events: events)
            try instance.setProps(props); return instance
        },
        "t3-media-presenter": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.media.makeView(props: props, events: events)
        },
        "t3-thread-days": T3MobileThreadPreferences.factory,
        "t3-settings-slider": T3SettingsSlider.factory,
        "t3-settings-header": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.settingsNavigation.makeView(props: props, events: events)
        },
        "t3-keyboard-commands": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.keyboard.makeView(props: props, events: events)
        },
        "t3-archive-chrome": ExactNativeFactory(for: T3MobileModule.self) { module, props, events in
            try module.archiveChrome.makeView(props: props, events: events)
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
    lazy var homeSwipes = T3MobileHomeSwipes(context: context)
    lazy var homeChrome = T3HomeChrome(swipes: homeSwipes)
    private let keyboard = T3MobileKeyboard()
    private let archiveChrome = T3ArchiveChrome()
    private let threadHeader = T3MobileThreadHeader()
    private let sheets = T3MobileSheets()
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
    private lazy var composerFileHolds = T3MobileComposerFileHolds(coordinator: queuedEdits)
    private let composerOperations = T3MobileComposerOperations()
    private let composerPaste = T3MobileComposerPaste()
    let terminal: T3MobileTerminal
    private let documentRoot: URL
    private let information = T3MobileInformation()
    private let informationLegal: T3MobileInformationLegal
    private let audioSession: T3MobileAudioSession
    private let audio: T3MobileAudio
    private let document: T3MobileDocument
    let media: T3MobileMedia
    private let attachments: T3MobileAttachments
    private var shareForegroundObserver: NSObjectProtocol?
    private let homePreferences: T3MobilePreferences
    private let clientCache: T3MobileClientCache
    private var faviconDownload: T3MobileFaviconDownload?
    private let clientCacheQueue = DispatchQueue(label: "t3.mobile-client-cache-bridge", qos: .utility)

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
        clientCache = T3MobileClientCache(directory: directory.appendingPathComponent("mobile-client-cache", isDirectory: true))
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
        shareForegroundObserver = NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self, self.alive else { return }; self.context.changed("t3.incoming-shares")
        }
    }

    override func tabContainer(_ contents: ExactTabContents) -> UIViewController? { workspace.container(contents) }

    override func navigation(_ navigation: ExactNavigation) {
        T3MobileNavigation.configure(navigation)
        sheets.configure(navigation)
    }
    override func route(_ route: ExactRoute) {
        T3MobileNavigation.configure(route, formSheet: route.data[.mobileFormSheet] == "true",
            scanActionID: route.data[.mobileScanAction], scannerOpen: route.data[.mobileScannerOpen] == "true",
            tint: route.controller.traitCollection.userInterfaceStyle == .dark ? .white : .black)
        sheets.configure(route)
        threadHeader.configure(route)
        terminal.menus.configure(route)
        inspectorChrome.configure(route)
        keyboard.configure(route)
        archiveChrome.configure(route)
        homeChrome.configure(route)
        homeSwipes.configure(route)
        settingsNavigation.configure(route)
        scheduledNavigation.configure(route, editor: route.data[.mobileScheduledEditor] == "true", backActionID: "back")
        informationLegal.configure(route)
    }
    override func element(_ element: ExactElement) {
        if element.hatch == .mobileHomeRow { homeChrome.rowMenus.configure(element); homeChrome.customSnooze.configure(element) }
        if element.hatch == .mobileHomeRow || element.hatch == .mobileHomeSwipeList || element.hatch == .mobileHomeSnooze { homeSwipes.configure(element) }
        if element.hatch == .mobileReviewList || element.hatch == .mobileReviewRow { reviewViewport.configure(element) }
        if element.hatch == .mobileVoiceEditor {
            voice.editor.configure(element, owner: element.data[.mobileVoiceOwner] ?? "",
                selectionRevision: Int(element.data[.mobileVoiceSelectionRevision] ?? "0") ?? 0)
        }
    }
    override func elementEnded(_ element: ExactElement) {
        if element.hatch == .mobileHomeRow { homeChrome.rowMenus.end(element); homeChrome.customSnooze.end(element) }
        if element.hatch == .mobileHomeRow || element.hatch == .mobileHomeSwipeList || element.hatch == .mobileHomeSnooze { homeSwipes.end(element) }
        if element.hatch == .mobileReviewList || element.hatch == .mobileReviewRow { reviewViewport.end(element) }
        if element.hatch == .mobileVoiceEditor { voice.editor.end(element) }
    }
    override func routeEnded(_ route: ExactRoute) { keyboard.end(route); archiveChrome.end(route); homeSwipes.end(route); sheets.end(route); threadHeader.end(route); terminal.menus.end(route); inspectorChrome.end(route); homeChrome.end(route); settingsNavigation.end(route); scheduledNavigation.end(route); informationLegal.end(route) }
    override func window(_ window: ExactWindow) { keyboard.configureWindow(window) }
    override func windowEnded(_ window: ExactWindow) { keyboard.endWindow(window) }

    override func later(_ request: [String: Any], reply: ExactReply) {
        guard alive else { reply.fail("The mobile session was closed."); return }
        if request["op"] as? String == "composerPickerIntake" {
            composerFileHolds.performPicker(request, attachments: attachments) { reply.send($0) }; return
        }
        if request["op"] as? String == "composerFileHold" {
            composerFileHolds.perform(request) { reply.send($0) }; return
        }
        if request["op"] as? String == "composerEditorPasteFiles" {
            do {
                let input = try T3ComposerPasteRequest(request)
                let claimed = input.action == "stage" ? try composerPaste.claim(input) : nil
                let store = queuedEdits, leases = composerPaste
                DispatchQueue.global(qos: .userInitiated).async {
                    do {
                        let value = try store.composerPasteFiles(input, inputs: claimed)
                        DispatchQueue.main.async {
                            if ["staged", "adopted", "discarded", "retired"].contains(value["status"] as? String ?? "") { leases.completed(input) }
                            reply.send(["ok": true, "generation": input.generation, "value": value])
                        }
                    } catch {
                        let problem = error as? T3ComposerPasteError ?? .recovery
                        reply.send(["ok": false, "generation": input.generation, "error": ["kind": problem.kind, "message": problem.message]])
                    }
                }
            } catch {
                let problem = error as? T3ComposerPasteError ?? .arguments
                reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": ["kind": problem.kind, "message": problem.message]])
            }
            return
        }
        if request["op"] as? String == "composerEditorApply" {
            reply.send(composerOperations.perform(request)); return
        }
        if request["op"] as? String == "forgetEnvironment" {
            forgetEnvironment(request, reply: reply)
            return
        }
        if request["op"] as? String == "mobileFaviconImage" {
            guard let requestId = request["requestId"] as? String, !requestId.isEmpty else {
                reply.fail("Choose a project icon request."); return
            }
            let generation = request["generation"] ?? 0
            if request["action"] as? String == "cancel" {
                faviconDownload?.cancel(requestId: requestId)
                reply.send(["ok": true, "generation": generation, "value": [:]])
            } else if request["action"] as? String == "load",
                      let text = request["url"] as? String, let url = URL(string: text) {
                let download = faviconDownload ?? T3MobileFaviconDownload()
                faviconDownload = download
                download.load(requestId: requestId, url: url) { result in
                    switch result {
                    case .success(let dataUrl):
                        reply.send(["ok": true, "generation": generation, "value": ["dataUrl": dataUrl]])
                    case .failure(let error):
                        let cancelled = error is CancellationError
                        reply.send(["ok": false, "generation": generation, "error": [
                            "kind": cancelled ? "Cancelled" : "Favicon",
                            "message": cancelled ? "Project icon request was cancelled." : error.localizedDescription]])
                    }
                }
            } else {
                reply.send(["ok": false, "generation": generation,
                    "error": ["kind": "Favicon", "message": "Project icon request is invalid."]])
            }
            return
        }
        if request["op"] as? String == "mobileClientCache" {
            let owner = clientCache
            clientCacheQueue.async {
                do {
                    let value = try owner.request(request)
                    DispatchQueue.main.async {
                        reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                    }
                } catch {
                    let message = error.localizedDescription
                    DispatchQueue.main.async {
                        reply.send(["ok": false, "generation": request["generation"] ?? 0,
                                    "error": ["kind": "Cache", "message": message]])
                    }
                }
            }
            return
        }
        if request["op"] as? String == "mobileIncomingShares" {
            // GAP 006: inbox adoption is local; this build has no Share Extension producer.
            let owner = queuedEdits
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let value = try owner.incomingShares(request)
                    DispatchQueue.main.async { [weak self] in
                        reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                        if request["action"] as? String != "read", let self, self.alive { self.context.changed("t3.incoming-shares") }
                    }
                } catch {
                    let message = error.localizedDescription
                    DispatchQueue.main.async { [weak self] in
                        reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": ["kind": "IncomingShare", "message": message]])
                        if request["action"] as? String != "read", let self, self.alive { self.context.changed("t3.incoming-shares") }
                    }
                }
            }
            return
        }
        if request["op"] as? String == "mobileOutbox" {
            queuedEdits.submitOutbox(request) { result in
                switch result {
                case .success(let value): reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                case .failure(let error):
                    let problem = error as? T3Failure ?? T3Failure(kind: "Persistence", message: "The outbox could not be saved.")
                    reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": problem.json])
                }
            }
            return
        }
        if request["op"] as? String == "mobileOutboxInline",
           ["lookup", "status", "recover", "retire"].contains(request["action"] as? String ?? "") {
            let store = queuedEdits
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    let value: [String: Any]
                    if request["action"] as? String == "lookup" {
                        guard let owner = request["owner"] as? [String: Any] else {
                            throw T3Failure(kind: "Arguments", message: "Choose the captured queued command owner.")
                        }
                        value = try store.outboxInlineLookup(owner)
                    } else {
                        guard let id = request["operationId"] as? String, !id.isEmpty else {
                            throw T3Failure(kind: "Arguments", message: "Choose a saved image reservation.")
                        }
                        if request["action"] as? String == "status" { value = try store.outboxInlineStatus(id) }
                        else {
                            guard T3OutboxDeliveryReceipt.integer(request["revision"], positive: true), let revision = request["revision"] as? Int else {
                                throw T3Failure(kind: "Arguments", message: "Choose the saved image reservation revision.")
                            }
                            if request["action"] as? String == "recover" { value = try store.recoverOutboxInline(id, revision: revision) }
                            else { value = try store.retireOutboxInline(id, revision: revision) }
                        }
                    }
                    reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                } catch {
                    let problem = error as? T3Failure ?? T3Failure(kind: "Persistence", message: "The image reservation needs recovery.", uncertain: true)
                    reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": problem.json])
                }
            }
            return
        }
        if request["op"] as? String == "mobileOutboxDelivery",
           ["status", "recover", "retire", "complete", "draftHandoffStatus", "draftHandoffComplete"].contains(request["action"] as? String ?? "") {
            let store = queuedEdits
            DispatchQueue.global(qos: .userInitiated).async {
                if request["action"] as? String == "complete" {
                    store.completeOutboxDelivery(request) { result in
                        switch result {
                        case .success(let value): reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                        case .failure(let error):
                            let problem = error as? T3Failure ?? T3Failure(kind: "Persistence", message: "The queued command cleanup needs recovery.", uncertain: true)
                            reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": problem.json])
                        }
                    }
                    return
                }
                do {
                    guard let id = request["operationId"] as? String, !id.isEmpty else {
                        throw T3Failure(kind: "Arguments", message: "Choose a saved queued command.")
                    }
                    let value: [String: Any]
                    if request["action"] as? String == "draftHandoffStatus" { value = try store.outboxDraftHandoffStatus(id) }
                    else if request["action"] as? String == "draftHandoffComplete" {
                        guard let handoff = request["handoff"] as? [String: Any], handoff["operationId"] as? String == id else {
                            throw T3Failure(kind: "Arguments", message: "Choose the exact saved draft handoff.")
                        }
                        value = try store.completeOutboxDraftHandoff(handoff)
                    }
                    else if request["action"] as? String == "status" { value = try store.outboxDeliveryStatus(id) }
                    else {
                        guard T3OutboxDeliveryReceipt.integer(request["revision"], positive: true), let revision = request["revision"] as? Int else {
                            throw T3Failure(kind: "Arguments", message: "Choose the saved command revision.")
                        }
                        if request["action"] as? String == "recover" { value = try store.recoverOutboxDelivery(id, revision: revision) }
                        else { value = try store.retireOutboxDelivery(id, revision: revision) }
                    }
                    reply.send(["ok": true, "generation": request["generation"] ?? 0, "value": value])
                } catch {
                    let problem = error as? T3Failure ?? T3Failure(kind: "Persistence", message: "The queued command receipt needs recovery.", uncertain: true)
                    reply.send(["ok": false, "generation": request["generation"] ?? 0, "error": problem.json])
                }
            }
            return
        }
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
                      if finished { answer(["monotonicMs": ProcessInfo.processInfo.systemUptime * 1000]) }
                      else { reply.fail("The mobile session was closed.") }
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
        case "composerAttachPick", "composerAttachRead", "composerAttachRemove", "snapshotDraftRead", "snapshotDraftRemove", "mobileAttachmentPreview":
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
        case "mobileCustomSnooze":
            do { try homeChrome.customSnooze.present(request) { answer($0) } }
            catch { reply.fail(String(describing: error)) }
        case "mobilePrompt":
            do {
                try alerts.prompt(title: request["title"] as? String ?? "Rename thread",
                                  initialValue: request["initialValue"] as? String ?? "",
                                  cancelLabel: request["cancelLabel"] as? String ?? "Cancel",
                                  submitLabel: request["submitLabel"] as? String ?? "OK") { choice, text in
                    answer(["choice": choice, "text": text])
                }
            } catch { reply.fail(String(describing: error)) }
        case "mobileHomeHaptic":
            if request["kind"] as? String == "light" { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
            else if request["kind"] as? String == "success" { UINotificationFeedbackGenerator().notificationOccurred(.success) }
            else { reply.fail("Unknown Home feedback."); return }
            answer()
        case "mobileAlert":
            let title = request["title"] as? String ?? "T3 Code"
            let message = request["message"] as? String ?? ""
            let kind = request["kind"] as? String ?? "info"
            let buttons: [(String, String, UIAlertAction.Style)] = kind == "remove"
                ? [("cancel", "Cancel", .cancel), ("remove", "Remove", .destructive)]
                : kind == "clear-client-cache"
                    ? [("cancel", "Cancel", .cancel), ("clear", "Clear Cache", .destructive)]
                : kind == "clear-client-caches"
                    ? [("cancel", "Cancel", .cancel), ("clear", "Clear All Caches", .destructive)]
                : kind == "sign-out"
                    ? [("cancel", "Cancel", .cancel), ("sign-out", "Sign out", .destructive)]
                : kind == "delete"
                    ? [("cancel", "Cancel", .cancel), ("delete", "Delete", .destructive)]
                : kind == "discard"
                    ? [("cancel", "Cancel", .cancel), ("discard", "Discard", .destructive)]
                : kind == "update"
                    ? [("cancel", "Cancel", .cancel), ("update", "Update", .default)]
                : kind == "share-import"
                    ? [("cancel-import", "Cancel import", .cancel), ("retry", "Retry", .default)]
                : kind == "share-cancel"
                    ? [("retry-import", "Retry import", .default), ("retry-cancel", "Retry cancel", .default)]
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

    // @ref llp/1109.009-mobile-settings.decision.md#offline-cache-storage
    // Forget must retire durable cache even when Exact abandons the reply.
    private func forgetEnvironment(_ request: [String: Any], reply: ExactReply) {
        guard let environmentId = request["environmentId"] as? String,
              !environmentId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              environmentId.utf8.count <= 512, !environmentId.contains("\0") else {
            reply.send(["ok": false, "generation": request["generation"] ?? 0,
                        "error": ["kind": "Arguments", "message": "Choose an environment to forget."]])
            return
        }
        let owner = clientCache, queue = clientCacheQueue
        let completed: ([String: Any]) -> Void = { result in
            guard result["ok"] as? Bool == true else { reply.send(result); return }
            // Do not gate on module or answer lifetime after the real forget
            // succeeded. This owner and queue finish the admitted local cleanup.
            queue.async {
                let response: [String: Any]
                do {
                    _ = try owner.request(["action": "clear", "environmentId": environmentId])
                    response = result
                } catch {
                    response = ["ok": false, "generation": result["generation"] ?? 0,
                                "error": ["kind": "Cache", "message": "The environment was forgotten, but its offline cache could not be cleared."]]
                }
                DispatchQueue.main.async { reply.send(response) }
            }
        }
        if let key = request["fleet"] as? String { fleet.perform(key, request, completion: completed) }
        else { transport.perform(request, completion: completed) }
    }

    override func destroy() {
        alive = false
        composerFileHolds.destroy()
        composerOperations.destroy()
        composerPaste.destroy()
        keyboard.destroy()
        faviconDownload?.shutdown(); faviconDownload = nil
        if let shareForegroundObserver { NotificationCenter.default.removeObserver(shareForegroundObserver) }; shareForegroundObserver = nil
        homeSwipes.destroy()
        homeChrome.rowMenus.destroy()
        homeChrome.customSnooze.destroy()
        sheets.destroy()
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
         "t3-composer-attachment-button": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-terminal-menu": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
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
         "t3-home-swipe-events": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-review-viewport": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-inspector-chrome": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-workspace-layout": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-layout-facts": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-keyboard-commands": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-archive-chrome": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") },
         "t3-home-chrome": ExactNativeFactory { _, _ in throw ExactNativeRefusal("T3 Code mobile requires iOS") }]
    }
}
#endif
let exactModule: ExactModule.Type = T3MobileModule.self
