// Contract/TypeScript owns presentation, domain projection and the server connections
// (src/transport/); this module owns the native views and what feeds them. Exact's core knows nothing of T3.
import Foundation
import AppKit

final class T3Module: ExactModule {
    /// The module's links to the environments the data module has connected (T3Link.swift): terminals, Browser automation, devices.
    let links = T3Links()
    let ssh: T3Ssh // Add Environment → SSH: discovery, ssh -G, tunnels (T3Ssh.swift).
    let composer: T3Composer
    /// Weak view registry for the shared Settings prompt sample.
    let promptPreview = T3ComposerPreview()
    private let media: T3MediaPreview // Attachment PDF and HTML bodies (T3MediaPreview.swift).
    private let mediaFocus = T3MediaFocus() // A press elsewhere takes the focus off a media node (T3MediaActions.swift).
    let devices: T3DeviceStreams // Device screens, input and screenshots (T3DeviceStream.swift).
    let snapShot: T3SnapShot
    let chrome = T3WindowChrome()
    let exportsRoot: URL? // Agent runs export into the isolated data root (T3ContextMenu.saveText).
    let menus = T3Menus() // Paste as Text, the updates box and the folder panels (T3Menus.swift).
    let notifications: T3Notifications // Thread notifications, sound, Dock badge (T3Notifications.swift).
    let sidebar: T3Sidebar // The legacy members menu and timed sidebar repaints (T3Sidebar.swift).
    let browserSessions: T3BrowserSessions // This session's Browser pages (T3BrowserSessions.swift)
    let browserAutomation: T3BrowserAutomation // The previewAutomation host's executor (T3BrowserAutomation.swift)
    private let latinChords: T3LatinChords // Wake, chords by physical key, hover under a still pointer (T3LatinChords.swift).
    let textMenu: T3TextContextMenu // the desktop shell's context menu wherever the page shows none (T3TextContextMenu.swift).
    // The SSH password dialog's secure field (T3SshAuth.swift). (The keybinding recorder is Contract's, key-recorder.contract.)
    override class var views: [String: ExactNativeFactory] { ["t3-ssh-password": ExactNativeFactory { props, events in T3SshPasswordField(props: props, events: events) },
                                                              "t3-browser": T3BrowserView.factory] } // A Browser tab's page (T3BrowserView.swift)

    required init(context: ExactModuleContext) {
        // A topic announced while the snapshot read is in flight lets its reply land, then asks once
        // more (exact2 #183, #109); the app no longer holds topics during a read.
        let changed: (String) -> Void = context.changed
        notifications = T3Notifications(agent: context.agent, changed: changed)
        sidebar = T3Sidebar(agent: context.agent, changed: changed)
        snapShot = T3SnapShot(directory: context.data, agent: context.agent, changed: changed)
        composer = T3Composer(changed: changed)
        let staged = context.data.appendingPathComponent("composer-files", isDirectory: true)
        media = T3MediaPreview(agent: context.agent, stagedFile: { T3MediaPreview.stagedCopy($0, in: staged) }) // a draft file's preview reads its staged copy
        devices = T3DeviceStreams(access: { [links] done in links.deviceHubAccess(done) }, changed: changed)
        ssh = T3Ssh(agent: context.agent, promptsAvailable: true, changed: changed) // the window shows the SSH password dialog
        latinChords = T3LatinChords(agent: context.agent)
        textMenu = T3TextContextMenu(agent: context.agent)
        let browser = T3BrowserSessions(agent: context.agent, changed: changed)
        browser.dataRoot = context.data // artifacts, Annotate's crops
        browserSessions = browser
        browserAutomation = T3BrowserAutomation(sessions: browser) // wired in attachBrowserAutomation (T3Module+Browser.swift)
        exportsRoot = context.agent ? context.data.appendingPathComponent("exports", isDirectory: true) : nil
        super.init(context: context)
        textMenu.richEditors = { [weak self] in [self?.composer.textView, self?.promptPreview.textView].compactMap { $0 } } // contenteditables in the reference: Select All stays enabled when empty
        DispatchQueue.main.async { [textMenu] in textMenu.install() }
        attachLocalBackend(context) // the embedded server (T3Module+Local.swift)
        attachAppControl(context) // `t3 app <dir>`: the control socket follows the embedded server (T3Module+Activation.swift)
        attachBrowserAutomation() // T3Module+Browser.swift
    }
    override func later(_ request: [String: Any], reply: ExactReply) {
        route(request, reply: reply, from: 0)
    }
    /// Each area's ops (T3Module+<Area>.swift), in turn: an area answers the ops it owns and
    /// calls `next` for the rest; what no area owns is the module's own (`status`, the preferences,
    /// `links`). No two areas share an op. A feature adds its area's method in its own file and one entry here.
    private static let areas: [(T3Module) -> ([String: Any], ExactReply, () -> Void) -> Void] = [T3Module.connectionOps, T3Module.fileOps, T3Module.timelineOps, T3Module.deviceOps, T3Module.sidebarOps, T3Module.snapshotOps, T3Module.composerOps, T3Module.windowOps, T3Module.shellOps, T3Module.mediaOps, T3Module.localOps, T3Module.activationOps, T3Module.codexAuthOps, T3Module.browserOps]
    private func route(_ request: [String: Any], reply: ExactReply, from index: Int) {
        guard index < Self.areas.count else { return forward(request, reply: reply) }
        Self.areas[index](self)(request, reply) { self.route(request, reply: reply, from: index + 1) }
    }
    /// The module's own ops. `status`: the native views' presentation state, which the data module adds to its
    /// connection's status, and the last interaction for its activity reporter.
    private func forward(_ request: [String: Any], reply: ExactReply) {
        let generation = request["generation"] as? Int ?? 0
        let failed = { (failure: T3Failure) in reply.send(["ok": false, "generation": generation, "error": failure.json]) }
        switch request["op"] as? String {
        case "status":
            DispatchQueue.main.async { [weak self] in
                guard let self else { return failed(T3Failure(kind: "Closed", message: "The window was closed.")) }
                let presentation = self.composer.status.merging(self.media.status) { first, _ in first }.merging(self.devices.status) { first, _ in first }.merging(self.browserSessions.status) { first, _ in first }.merging(self.browserAutomation.status) { first, _ in first }
                reply.send(["ok": true, "generation": generation, "value": ["presentation": presentation]])
            }
        // The data module's connected environments (src/transport/transport.ts pushLinks).
        case "links":
            DispatchQueue.main.async { [links] in links.update(request); reply.send(["ok": true, "generation": generation, "value": [:] as [String: Any]]) }
        default: failed(T3Failure(kind: "Arguments", message: "Unknown native operation."))
        }
    }
    override func element(_ element: ExactElement) {
        composer.install(element); promptPreview.install(element); chrome.install(element); menus.install(element); media.install(element); devices.install(element)
        mediaFocus.install(element)
        activationElement(element) // `t3 app`: the window whose closing fails its requests (T3Module+Activation.swift)
        if element.hatch == .t3Composer { snapShot.setComposer(key: ObjectIdentifier(element), owner: element.data[.snapshotOwner] ?? "", view: element.view, focus: { [weak element] in element?.focus() }) }
        if element.hatch == .t3SnapshotTile, let view = element.view { snapShot.installTile(id: element.data[.snapshotId] ?? "", view: view) }
    }
    override func elementEnded(_ element: ExactElement) {
        composer.remove(element); promptPreview.remove(element); media.remove(element); devices.remove(element); mediaFocus.remove(element)
        if element.hatch == .t3SnapshotTile, let view = element.view { snapShot.removeTile(view: view) }
        if element.hatch == .t3Composer { snapShot.removeComposer(key: ObjectIdentifier(element)) }
    }
    override func destroy() { detachAppControl(); links.destroy(); mediaFocus.destroy(); T3MenuDismissal.disarm(); latinChords.destroy(); textMenu.destroy(); notifications.destroy(); snapShot.destroy(); composer.destroy(); promptPreview.destroy(); media.destroy(); devices.destroy(); chrome.destroy(); menus.destroy(); ssh.destroy(); browserSessions.sync([]); T3LocalBackend.shared.detach(self) } // detachAppControl first: the window's request fails at once (renderer-unavailable), and the control lets go of the backend before the last module stops it
}

let exactModule: ExactModule.Type = T3Module.self
