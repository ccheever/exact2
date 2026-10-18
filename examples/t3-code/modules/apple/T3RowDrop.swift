#if os(macOS)
import AppKit

/// Files dropped on a legacy sidebar row (MIT reference, see LICENSE-T3: LegacySidebar.tsx SidebarThreadRow's
/// `makeWorkspaceFileDropHandlers` and `handleThreadFileDrop`, sidebarPendingFileDropStore.ts). Each legacy
/// thread row's button carries the `t3-row-drop` hatch and its thread key (`data-thread-key`); this adds a drop
/// target over it. A file drag over the row rings it (`ring-1 ring-inset ring-primary/70`); the drop queues the
/// files under the row's key and, unless the row is the open thread's (`data-thread-open`), presses the row, which
/// opens the thread; TypeScript then takes the queue for the open thread (`rowDropTake`, legacy-sidebar-drop.ts) and
/// the files are staged as the composer's Attach stages them (T3ComposerAttach.swift), or lets a drop go whose
/// thread did not open (`rowDropClear`). The target answers drags only: AppKit finds a drag destination by its registered
/// types (`_hitTest:dragTypes:`), not by `hitTest`, so a press, a hover or a right click still reach the row.
final class T3RowDrop {
    private var targets: [ObjectIdentifier: T3RowDropTarget] = [:]
    /// sidebarPendingFileDropStore: drops queue up (a second drop before the thread opens keeps both), oldest first.
    private(set) var pending: [(key: String, urls: [URL])] = []
    private let attach: T3ComposerAttach
    private let changed: (String) -> Void

    init(attach: T3ComposerAttach, changed: @escaping (String) -> Void) {
        self.attach = attach
        self.changed = changed
    }

    /// The keys with a queued drop, in the status (TypeScript asks for them once their thread is open).
    var status: [String: Any] { ["rowDrops": pending.map(\.key)] }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3RowDrop, let view = element.view else { return }
        let key = element.data[.threadKey] ?? ""
        if let target = targets[ObjectIdentifier(element)], target.superview === view { target.key = key; return }
        let target = T3RowDropTarget(frame: view.bounds)
        target.key = key
        target.autoresizingMask = [.width, .height]
        target.dropped = { [weak self, weak element] key, urls in
            self?.drop(key, urls, open: element?.data[.threadOpen] == "1") { element?.click() }
        }
        view.addSubview(target)
        targets[ObjectIdentifier(element)] = target
        element.owns(view: target, "the row's file drop target (LegacySidebar.tsx makeWorkspaceFileDropHandlers)")
    }

    func remove(_ element: ExactElement) {
        guard element.hatch == .t3RowDrop, let target = targets.removeValue(forKey: ObjectIdentifier(element)) else { return }
        target.unregisterDraggedTypes()
        target.removeFromSuperview()
    }

    func destroy() {
        for target in targets.values { target.unregisterDraggedTypes(); target.removeFromSuperview() }
        targets = [:]
    }

    /// handleThreadFileDrop: the drop queues; a row that is not the open thread's is pressed, which opens its thread
    /// (ChatView then takes the files). On the open thread's own row nothing navigates: the next refresh takes them.
    func drop(_ key: String, _ urls: [URL], open: Bool, press: () -> Void) {
        guard !urls.isEmpty else { return }
        queue(key, urls)
        if !open { press() }
    }

    /// queuePendingFileDrop (internal so tests can queue without a drag).
    func queue(_ key: String, _ urls: [URL]) {
        guard !key.isEmpty else { return }
        pending.append((key, urls))
        changed("t3.status")
    }

    /// clearPendingFileDrop: the drops queued under these keys go (their thread did not open, or another did).
    func clear(_ request: [String: Any]) -> [String: Any] {
        let keys = Set(request["keys"] as? [String] ?? [])
        let before = pending.count
        pending.removeAll { keys.contains($0.key) }
        if pending.count != before { changed("t3.status") }
        return ["ok": true, "generation": request["generation"] as? Int ?? 0, "value": ["cleared": before - pending.count]]
    }

    /// consumePendingFileDrop: every queued drop for these keys, oldest first, each file staged as Attach stages a
    /// picked one (images to PNG drafts, other files within `fileLimit` into staged/); the rest stay queued.
    func take(_ request: [String: Any]) -> [String: Any] {
        let keys = Set(request["keys"] as? [String] ?? [])
        let fileLimit = request["fileLimit"] as? Int ?? 0, imageMaxEdge = request["imageMaxEdge"] as? Int ?? 0
        let generation = request["generation"] as? Int ?? 0
        let matched = pending.filter { keys.contains($0.key) }
        guard !matched.isEmpty else { return ["ok": true, "generation": generation, "value": ["files": []]] }
        pending.removeAll { keys.contains($0.key) }
        let files = matched.flatMap(\.urls).prefix(100).map { attach.stage($0, fileLimit: fileLimit, imageMaxEdge: imageMaxEdge) }
        changed("t3.status")
        return ["ok": true, "generation": generation, "value": ["files": Array(files)]]
    }
}

/// The drop target laid over a row: it takes file drags and draws the row's ring while one is over it; it never
/// takes a click. Dropped folders are let go (the row's `addFolders` does nothing), so a drop of folders alone
/// opens nothing.
final class T3RowDropTarget: NSView {
    var key = ""
    var dropped: ((String, [URL]) -> Void)?
    private(set) var ringed = false {
        didSet { layer?.borderWidth = ringed ? 1 : 0 }
    }

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.cornerRadius = 6 // rounded-md
        layer?.borderWidth = 0
        registerForDraggedTypes([.fileURL])
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    /// ring-primary/70: the primary colour at 70%, light and dark.
    static let ring = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            ? NSColor(srgbRed: 0x34 / 255, green: 0x6b / 255, blue: 0xf1 / 255, alpha: 0.7)
            : NSColor(srgbRed: 0x1b / 255, green: 0x4e / 255, blue: 0xd8 / 255, alpha: 0.7)
    }
    override func updateLayer() { layer?.borderColor = Self.ring.cgColor }
    override var wantsUpdateLayer: Bool { true }

    /// Clicks, hovers and right clicks belong to the row underneath.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    static func fileURLs(_ pasteboard: NSPasteboard) -> [URL] {
        pasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
    }
    /// isFileDrag: a drag that carries files; anything else passes by.
    private func accepts(_ sender: NSDraggingInfo) -> Bool { !Self.fileURLs(sender.draggingPasteboard).isEmpty }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        guard accepts(sender) else { return [] }
        ringed = true; needsDisplay = true
        return .copy
    }
    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        guard accepts(sender) else { return [] }
        if !ringed { ringed = true; needsDisplay = true }
        return .copy
    }
    override func draggingExited(_ sender: NSDraggingInfo?) { ringed = false }
    override func draggingEnded(_ sender: NSDraggingInfo) { ringed = false }
    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        ringed = false
        let urls = Self.fileURLs(sender.draggingPasteboard)
        guard !urls.isEmpty else { return false }
        let files = Self.files(urls)
        if !files.isEmpty { dropped?(key, files) }
        return true
    }
    /// splitDroppedItems: the files of a drop, without its folders.
    static func files(_ urls: [URL]) -> [URL] {
        urls.filter { url in var directory: ObjCBool = false; return !(FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && directory.boolValue) }
    }
}
#endif
