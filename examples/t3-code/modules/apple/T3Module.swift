// Contract/TypeScript owns presentation and domain projection. This optional
// module owns one authenticated connection; Exact's core knows nothing of T3.
import Foundation
import AppKit

final class T3Module: ExactModule {
    let transport: T3Transport
    private let activity: T3ActivityReporter
    private let panelTabs = RightPanelTabsInput()
    private let toolIcons = T3ToolActivityIcon()
    private let timelineTips = T3TimelineTooltip()
    private let fleet: T3Fleet // Background environments (T3Fleet.swift).
    let ssh: T3Ssh // Add Environment → SSH: discovery, ssh -G, tunnels (T3Ssh.swift).
    let composer: T3Composer
    let intent: T3ComposerIntent // Send gestures and ⌘ state (T3ComposerIntent.swift).
    private let frames: T3ComposerFrames // Popover anchors in window space (T3ComposerFrames.swift).
    private let scrollEnds: R5ComposerScroll // r5-composer: ref lists' next-page signal (R5ComposerScroll.swift).
    let attach: T3ComposerAttach // Attach files: picked images become PNG drafts (T3ComposerAttach.swift).
    private let video: T3ComposerVideo // A shelf video's expanded preview (T3ComposerVideo.swift).
    private let media: R6MediaPreview // lane r6-media: attachment PDF and HTML bodies (R6MediaPreview.swift).
    let devices: R6DeviceStreams // lane r6-media: device screens, input and screenshots (R6DeviceStream.swift).
    private let timeline: T3Timeline
    let turns: T3TimelineTurns // The minimap's turns in view and jumps (T3TimelineTurns.swift).
    let mermaid: T3TimelineMermaid // Mermaid fences laid out with the server's own Mermaid (T3TimelineMermaid.swift).
    let snapShot: T3SnapShot
    let chrome = T3WindowChrome()
    let exportsRoot: URL? // Agent runs export into the isolated data root (T3ContextMenu.saveText).
    let menus = T3Menus() // Menu bar items, zoom and the ⌘Q hold (T3Menus.swift).
    let notifications: T3Notifications // Thread notifications, sound, Dock badge (T3Notifications.swift).
    let sidebar: T3Sidebar // Thread menu, modifier reads and jump hints (T3Sidebar.swift).
    private let launcher = R8KeysLauncher() // lane r8-keys: the surface launcher's focus and letters (R8KeysLauncher.swift).
    let measure = R8KeysMeasure() // lane r8-keys: drawn frames for window-level popups (R8KeysMeasure.swift).
    private let r9: R9Input // lane r9-input: composer focus and composing text, the transcript's remembered position (R9Input.swift).
    private let r10: R10Connect // lane r10-connect: wake, select on open, chords by physical key, hover under a still pointer (R10Connect.swift).
    // Settings → Keybindings capture field (T3KeyRecorder.swift).
    // The SSH password dialog's secure field (T3SshAuth.swift).
    override class var views: [String: ExactNativeFactory] { ["t3-key-recorder": ExactNativeFactory { props, events in T3KeyRecorder(props: props, events: events) },
                                                              "t3-ssh-password": ExactNativeFactory { props, events in T3SshPasswordField(props: props, events: events) },
                                                              "t3-terminal": T3TerminalView.factory] }

    required init(context: ExactModuleContext) {
        // A topic announced while the snapshot read is in flight lets its reply land, then asks once
        // more (exact2 #183, #109); the app no longer holds topics during a read.
        let changed: (String) -> Void = context.changed
        notifications = T3Notifications(agent: context.agent, changed: changed)
        sidebar = T3Sidebar(agent: context.agent, changed: changed)
        snapShot = T3SnapShot(directory: T3Storage.dataRoot(agent: context.agent, contextData: context.data), agent: context.agent, changed: changed)
        timeline = T3Timeline(changed: changed)
        turns = T3TimelineTurns(changed: changed)
        mermaid = T3TimelineMermaid(changed: changed)
        composer = T3Composer(changed: changed)
        composer.editor.styler.imageDirectory = T3Storage.dataRoot(agent: context.agent, contextData: context.data).appendingPathComponent("snapshots/drafts", isDirectory: true) // image chips (T3ComposerImageChip.swift)
        intent = T3ComposerIntent(changed: changed)
        frames = T3ComposerFrames(changed: changed)
        scrollEnds = R5ComposerScroll(changed: changed)
        attach = T3ComposerAttach(dataRoot: T3Storage.dataRoot(agent: context.agent, contextData: context.data), agent: context.agent)
        video = T3ComposerVideo(dataRoot: T3Storage.dataRoot(agent: context.agent, contextData: context.data), muted: context.agent)
        media = R6MediaPreview(agent: context.agent)
        let credentials = T3Credentials(persistent: !context.agent), saved = T3SavedEnvironments(persistent: !context.agent)
        activity = T3ActivityReporter(persistent: !context.agent, dataDirectory: T3Storage.dataRoot(agent: context.agent, contextData: context.data))
        transport = T3Transport(persistent: !context.agent, dataDirectory: T3Storage.dataRoot(agent: context.agent, contextData: context.data), credentials: credentials, savedEnvironments: saved, activity: activity, changed: changed)
        fleet = T3Fleet(persistent: !context.agent, credentials: credentials, saved: saved, activity: activity, changed: changed)
        devices = R6DeviceStreams(access: { [transport] done in transport.deviceHubAccess(done) }, changed: changed)
        ssh = T3Ssh(agent: context.agent, promptsAvailable: true, changed: changed) // the window shows the SSH password dialog
        r9 = R9Input(agent: context.agent)
        r10 = R10Connect(agent: context.agent)
        exportsRoot = context.agent ? T3Storage.dataRoot(agent: true, contextData: context.data).appendingPathComponent("exports", isDirectory: true) : nil
        super.init(context: context)
        composer.launcher = launcher
        chrome.changed = changed // desktop-shell-details: full screen publishes t3.status (T3FullScreen.swift)
        DispatchQueue.main.async { [sidebar] in sidebar.install() }
        attachLocalBackend(context) // the embedded server (T3Module+Local.swift)
    }
    override func later(_ request: [String: Any], reply: ExactReply) {
        if let key = request["fleet"] as? String { return fleet.perform(key, request) { reply.send($0) } }
        route(request, reply: reply, from: 0)
    }
    /// Each area's ops (T3Module+<Area>.swift), in turn: an area answers the ops it owns and
    /// calls `next` for the rest; what no area owns goes to the transport. No two areas share
    /// an op. A feature adds its area's method in its own file and one entry here.
    private static let areas: [(T3Module) -> ([String: Any], ExactReply, () -> Void) -> Void] = [T3Module.connectionOps, T3Module.fileOps, T3Module.timelineOps, T3Module.deviceOps, T3Module.sidebarOps, T3Module.snapshotOps, T3Module.composerOps, T3Module.windowOps, T3Module.shellOps, T3Module.mediaOps, T3Module.terminalOps, T3Module.localOps]
    private func route(_ request: [String: Any], reply: ExactReply, from index: Int) {
        guard index < Self.areas.count else { return forward(request, reply: reply) }
        Self.areas[index](self)(request, reply) { self.route(request, reply: reply, from: index + 1) }
    }
    /// The authenticated connection's ops (T3Transport.swift); a status read gains the presentation state.
    private func forward(_ request: [String: Any], reply: ExactReply) {
        transport.perform(request) { [weak self] response in
            if request["op"] as? String == "status" {
                DispatchQueue.main.async {
                    var result = response
                    var value = result["value"] as? [String: Any] ?? [:]
                    value["presentation"] = (self?.timeline.status ?? [:]).merging(self?.composer.status ?? [:]) { first, _ in first }.merging(self?.intent.status ?? [:]) { first, _ in first }.merging(self?.frames.status ?? [:]) { first, _ in first }.merging(self?.scrollEnds.status ?? [:]) { first, _ in first }.merging(self?.sidebar.status ?? [:]) { first, _ in first }.merging(self?.media.status ?? [:]) { first, _ in first }.merging(self?.devices.status ?? [:]) { first, _ in first }.merging(self?.chrome.status ?? [:]) { first, _ in first }.merging(T3Terminals.shared.status) { first, _ in first }
                    var turned: [String: Any] = value["presentation"] as? [String: Any] ?? [:]
                    let turnStatus: [String: Any] = self?.turns.status ?? [:]
                    for (key, entry) in turnStatus { turned[key] = entry }
                    value["presentation"] = turned
                    result["value"] = value
                    reply.send(result)
                }
            } else { reply.send(response) }
        }
    }
    override func element(_ element: ExactElement) {
        T3TerminalCommandKey.install(element)
        panelTabs.install(element); toolIcons.install(element); timelineTips.install(element)
        frames.install(element); scrollEnds.install(element)
        composer.install(element); chrome.install(element); timeline.install(element); menus.install(element); turns.install(element); video.install(element); media.install(element); devices.install(element)
        launcher.install(element); measure.install(element); r9.install(element); r10.install(element)
        T3FileEditor.install(element) // the Files editor takes the focus its press began (T3PanelsNative.swift, lane r5-panels)
        if element.hook == .t3Composer { snapShot.setComposer(key: ObjectIdentifier(element), owner: element.data[.snapshotOwner] ?? "", view: element.view, focus: { [weak element] in element?.focus() }) }
        if element.hook == .t3SnapshotTile, let view = element.view { snapShot.installTile(id: element.data[.snapshotId] ?? "", view: view) }
    }
    override func elementEnded(_ element: ExactElement) {
        T3TerminalCommandKey.remove(element)
        panelTabs.remove(element); toolIcons.remove(element); timelineTips.remove(element)
        frames.remove(element); scrollEnds.remove(element)
        composer.remove(element); launcher.remove(element); measure.remove(element); r9.remove(element); timeline.remove(element); turns.remove(element); video.remove(element); media.remove(element); devices.remove(element)
        if element.hook == .t3SnapshotTile, let view = element.view { snapShot.removeTile(view: view) }
        if element.hook == .t3Composer { snapShot.removeComposer(key: ObjectIdentifier(element)) }
    }
    override func destroy() { activity.destroy(); panelTabs.destroy(); toolIcons.destroy(); timelineTips.destroy(); r10.destroy(); r9.destroy(); sidebar.destroy(); notifications.destroy(); snapShot.destroy(); composer.destroy(); video.destroy(); media.destroy(); devices.destroy(); intent.destroy(); frames.destroy(); scrollEnds.destroy(); chrome.destroy(); menus.destroy(); timeline.destroy(); turns.destroy(); transport.destroy(); fleet.destroy(); ssh.destroy(); T3LocalBackend.shared.detach(self) }
}

let exactModule: ExactModule.Type = T3Module.self
