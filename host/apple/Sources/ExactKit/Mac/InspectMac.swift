// Inspect mode on the Mac (LLP 1122 D4, D5; T2c): the platform half of
// Inspect.swift. While it is on, an event monitor takes every mouse, wheel,
// gesture and key event (modifiers too) in every window of the app, its
// sheets and later windows included, so none reaches the app; a menu closes
// as it opens. The banner's close is clicked here too, and Escape leaves. A
// click selects the plan node the window's own hit test lands on (the real
// click path), from the hit view up to the nearest view that stands for a
// plan node, in the session that view is in; a walk that stops at the title
// bar (D5's chrome rule: a plan node the bar hosts, the header's heading as
// the window's title, a toolbar item, its search field or a tab of its
// segments, is selectable; AppKit's own parts select nothing, and say so).
// The mouse over a view outlines what a click would select.
#if EXACT_INSPECT && os(macOS)
import AppKit

enum InspectHost {
    nonisolated(unsafe) private static var overlays: [ObjectIdentifier: InspectOverlay] = [:]
    nonisolated(unsafe) private(set) weak static var app: ExactApp?
    nonisolated(unsafe) private static var monitor: Any?
    nonisolated(unsafe) private static var down: (window: ObjectIdentifier, at: NSPoint, close: Bool)?
    /// The menu bar's tracking, ended as it begins while inspect mode is on.
    nonisolated(unsafe) private static var menus: NSObjectProtocol?
    /// Each window's `acceptsMouseMovedEvents` before inspect mode set it.
    nonisolated(unsafe) private static var moved: [ObjectIdentifier: Bool] = [:]
    /// Inspect mode ended with a button or key still down: the monitor
    /// stays until that sequence ends, so its release reaches nothing.
    nonisolated(unsafe) private(set) static var draining = false
    /// The keys down that inspect mode took, each owned to its release.
    nonisolated(unsafe) private static var keys = Set<UInt16>()
    nonisolated(unsafe) private static var drainedFrom = Date.distantPast
    /// Whether a button, a key inspect mode took or a modifier is down.
    private static func held() -> Bool {
        NSEvent.pressedMouseButtons != 0 || !keys.isEmpty || !NSEvent.modifierFlags.intersection([.shift, .control, .option, .command, .function]).isEmpty
    }

    /// Inspect mode shown, or put away, on every window the app's sessions are in.
    static func show(_ app: ExactApp, on: Bool) {
        self.app = app
        let windows = app.sessions.compactMap { $0.view?.window }
        for (key, overlay) in overlays where !on || overlay.window.map({ w in !app.sessions.contains { $0.presenter.hosts(w) } }) ?? true { drop(key, overlay) }
        if !on {
            if let menus { NotificationCenter.default.removeObserver(menus) }
            menus = nil; down = nil
            // A release still to come is inspect mode's too.
            if held() { draining = true; drainedFrom = Date() } else { removeMonitor() }
            return
        }
        draining = false
        // A menu open now closes; one opened later closes as it opens.
        NSApp.mainMenu?.cancelTrackingWithoutAnimation()
        for window in windows { _ = overlay(on: window) }
        if monitor == nil {
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseUp, .leftMouseDragged, .rightMouseDown, .rightMouseUp, .rightMouseDragged,
                                                                  .otherMouseDown, .otherMouseUp, .otherMouseDragged, .scrollWheel, .mouseMoved, .keyDown, .keyUp,
                                                                  .flagsChanged, .mouseEntered, .mouseExited, .cursorUpdate,
                                                                  .magnify, .rotate, .swipe, .smartMagnify, .pressure]) { take($0) }
        }
        if menus == nil {
            menus = NotificationCenter.default.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: .main) { note in
                guard Inspect.on, let menu = note.object as? NSMenu else { return }
                menu.cancelTrackingWithoutAnimation()
                overlays.values.forEach { $0.say("Menus are off while inspecting (Esc leaves).") }
            }
        }
    }

    private static func removeMonitor() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil; draining = false; keys = []
    }

    private static func drop(_ key: ObjectIdentifier, _ overlay: InspectOverlay) {
        if let window = overlay.window, let was = moved[key] { window.acceptsMouseMovedEvents = was }
        overlays[key] = nil; moved[key] = nil
        overlay.removeFromSuperview()
    }

    /// The window's overlay, made on first use: a session's window, its sheet
    /// or popover, one shown since inspect mode came on.
    private static func overlay(on window: NSWindow) -> InspectOverlay? {
        if let overlay = overlays[ObjectIdentifier(window)], overlay.window === window { return overlay }
        guard let frame = window.contentView?.superview else { return nil }
        let overlay = InspectOverlay(frame: frame.bounds)
        overlay.autoresizingMask = [.width, .height]
        frame.addSubview(overlay, positioned: .above, relativeTo: nil)
        overlays[ObjectIdentifier(window)] = overlay
        moved[ObjectIdentifier(window)] = window.acceptsMouseMovedEvents
        window.acceptsMouseMovedEvents = true
        return overlay
    }

    /// An overlay put away (InspectOverlay.removeFromSuperview).
    static func forget(_ overlay: InspectOverlay) {
        for (key, o) in overlays where o === overlay {
            if let window = overlay.window, let was = moved[key] { window.acceptsMouseMovedEvents = was }
            overlays[key] = nil; moved[key] = nil
        }
    }

    /// Whether a session of the app still shows in `window`.
    static func hosts(_ window: NSWindow) -> Bool { app?.sessions.contains { $0.presenter.hosts(window) } ?? false }

    /// An event while inspect mode is on: nil, taken here, in every window of
    /// the app; only an event in another process's window (none comes here)
    /// or with no window but a key would pass. After inspect mode ends with a
    /// button or key down, the rest of that sequence is taken too.
    static func take(_ event: NSEvent) -> NSEvent? {
        if draining {
            // Every event is taken until no button, owned key or modifier is
            // down (a modifier's release would be the app's keyup), so no
            // sequence reaches the app halfway; a key whose release never
            // came frees the app after five seconds.
            if event.type == .keyUp { keys.remove(event.keyCode) }
            let stuck = Date().timeIntervalSince(drainedFrom) > 5
            if stuck || (!held() && event.type != .keyUp && event.type != .flagsChanged && event.type != .leftMouseUp && event.type != .rightMouseUp && event.type != .otherMouseUp) {
                removeMonitor()
                return event
            }
            if !held() { DispatchQueue.main.async { if !Inspect.on, draining, !held() { removeMonitor() } } }
            return nil
        }
        guard Inspect.on, let app else { return event }
        // Every key, in any window or none: no key equivalent runs a command.
        if event.type == .keyDown || event.type == .keyUp || event.type == .flagsChanged {
            if event.type == .keyDown { keys.insert(event.keyCode) } else if event.type == .keyUp { keys.remove(event.keyCode) }
            if event.type == .keyDown, event.keyCode == 53 { app.leaveInspect() } // Escape
            return nil
        }
        guard let window = event.window else { return nil }
        guard let overlay = overlay(on: window) else { return nil }
        let p = event.locationInWindow
        switch event.type {
        case .mouseMoved, .leftMouseDragged:
            if let began = down, began.close { overlay.closeHighlighted(overlay.onClose(p)); break }
            overlay.hover(target(at: p, in: window, app))
        case .leftMouseDown:
            down = (ObjectIdentifier(window), p, overlay.onClose(p))
            if down?.close == true { overlay.closeHighlighted(true) }
        case .leftMouseUp:
            defer { down = nil }
            guard let began = down, began.window == ObjectIdentifier(window) else { break }
            if began.close { overlay.closeHighlighted(false); if overlay.onClose(p) { app.leaveInspect() }; break }
            guard hypot(p.x - began.at.x, p.y - began.at.y) <= 6 else { break }
            let hit = target(at: p, in: window, app)
            if let session = hit.session, let id = hit.id { overlay.say(nil); app.inspectTapped(session, id: id) }
            else { overlay.say(hit.why ?? "Nothing here is a view of the app.") }
        default: break
        }
        return nil
    }

    /// What a click at `p` (in the window) selects: the session whose view
    /// it lands on (two sessions in one window number their views alike),
    /// the node and the view that stands for it, or why nothing.
    static func target(at p: NSPoint, in window: NSWindow, _ app: ExactApp) -> (session: ExactSession?, id: UInt32?, view: NSView?, why: String?) {
        // The session whose bar the window shows first: the title bar is its.
        let sessions = app.sessions.filter { $0.booted && $0.presenter.hosts(window) }.sorted { a, _ in a.presenter.toolbar.window === window }
        let hidden = overlays[ObjectIdentifier(window)]
        hidden?.isHidden = true
        defer { hidden?.isHidden = false }
        for session in sessions {
            let picked = pick(at: p, in: window, session.presenter)
            if let id = picked.id {
                if let node = picked.view as? NodeView, session.presenter.views[id] !== node { continue }
                return (session, id, picked.view, nil)
            }
            if let why = picked.why { return (session, nil, nil, why) }
        }
        return (nil, nil, nil, nil)
    }

    /// The node `presenter` shows at `p` in `window`, by the window's hit test.
    static func pick(at p: NSPoint, in window: NSWindow, _ presenter: Presenter) -> (id: UInt32?, view: NSView?, why: String?) {
        guard let frame = window.contentView?.superview, let content = window.contentView,
              let hit = frame.hitTest(frame.convert(p, from: nil)) else { return (nil, nil, nil) }
        // The title bar (outside the content view): a toolbar item's node, or
        // the header's heading under the window's title; AppKit's own parts
        // select nothing, and the walk goes no higher.
        if !hit.isDescendant(of: content) {
            if let id = toolbarNode(presenter, hit, p) { return (id, presenter.views[id] ?? hit, nil) }
            if let heading = presenter.toolbar.header?.heading, let title = hit as? NSTextField, title.stringValue == window.title { return (heading.id, hit, nil) }
            return (nil, nil, "That is part of the window's title bar, not a view of the app.")
        }
        var at: NSView? = hit
        while let v = at {
            if let node = v as? NodeView, !node.isHidden { return (node.id, node, nil) }
            if let button = v as? NativeButtonMac, let owner = button.owner { return (owner.id, owner, nil) }
            if let owned = v as? AgentOwned, let id = owned.agentViewId { return (id, v, nil) }
            at = v.superview
        }
        return (nil, nil, nil)
    }

    /// The node a toolbar item at `hit` (the point `p`, in the window)
    /// stands for: its own view's; the header's search input for the search
    /// field; the tab under the point for the segments; or the item AppKit's
    /// viewer for it holds (`exact.command.<id>`, `exact.menu.<id>`).
    private static func toolbarNode(_ presenter: Presenter, _ hit: NSView, _ p: NSPoint) -> UInt32? {
        let toolbar = presenter.toolbar, items = toolbar.items
        if let (id, _) = items.first(where: { $0.value.view.map { hit.isDescendant(of: $0) } == true }) { return id }
        if let field = toolbar.search.item?.searchField, hit.isDescendant(of: field), let input = toolbar.search.node { return input.id }
        var at: NSView? = hit
        while let v = at {
            if let control = v as? NSSegmentedControl, toolbar.segments.item != nil, control.segmentCount == toolbar.segments.tabs.count, control.segmentCount > 0 {
                // Its segments' widths: as set, or (automatic, AppKit's `fill`)
                // each label's width with the space left shared alike.
                let x = control.convert(p, from: nil).x
                let font = control.font ?? .systemFont(ofSize: NSFont.systemFontSize)
                var widths = (0..<control.segmentCount).map { i in control.width(forSegment: i) > 0 ? control.width(forSegment: i) : ((control.label(forSegment: i) ?? "") as NSString).size(withAttributes: [.font: font]).width + 20 }
                let spare = max(0, control.bounds.width - widths.reduce(0, +)) / CGFloat(control.segmentCount)
                widths = widths.map { $0 + spare }
                var edge: CGFloat = 0
                let index = widths.firstIndex { edge += $0; return x < edge } ?? control.segmentCount - 1
                return toolbar.segments.tabs[min(max(index, 0), control.segmentCount - 1)] // (a disabled tab too: it is the plan's)
            }
            if v.responds(to: NSSelectorFromString("item")), let item = v.value(forKey: "item") as? NSToolbarItem {
                let raw = item.itemIdentifier.rawValue
                for prefix in ["exact.command.", "exact.menu."] where raw.hasPrefix(prefix) { if let id = UInt32(raw.dropFirst(prefix.count)), items[id] != nil { return id } }
                if raw.hasPrefix("exact.search."), let input = toolbar.search.node { return input.id }
                return nil
            }
            at = v.superview
        }
        return nil
    }

    /// `outline {sites, plan}`: every copy of those sites outlined in the
    /// windows of the sessions running that plan, the first scrolled into
    /// view unless the frame answers this build's own click; `{clear}`, or
    /// an empty list, clears them. A frame for another plan leaves a
    /// window's outlines as they are.
    static func outline(_ app: ExactApp, _ frame: [String: Any]) {
        guard Inspect.on else { return }
        if frame["clear"] as? Bool == true { overlays.values.forEach { $0.mark([]) }; return }
        guard let plan = frame["plan"] as? String, let wanted = (frame["sites"] as? [Any])?.compactMap({ ($0 as? NSNumber)?.intValue }) else { return }
        let want = Set(wanted)
        var marks: [ObjectIdentifier: (window: NSWindow, places: [Place])] = [:]
        var copies: [(session: ObjectIdentifier, id: UInt32, view: NSView, shown: Bool)] = [], running: [ObjectIdentifier: Int] = [:]
        for session in app.sessions where session.booted {
            let sites = Inspect.sites(session)
            guard sites.plan == plan else { continue }
            running[ObjectIdentifier(session)] = session.generation
            let nodes = sites.views.filter { want.contains($0.site) }.compactMap { session.presenter.views[$0.id] }.filter { $0.window != nil }
            // (shown: not hidden, and not CSS `visibility: hidden`, whose box keeps its place but draws nothing)
            copies += nodes.map { (ObjectIdentifier(session), $0.id, $0, !$0.isHiddenOrHasHiddenAncestor && !$0.cssVisibilityHidden) }
            // Each in the window it is in (a sheet's, a popover's).
            for node in nodes { if let window = node.window, let place = place(of: node, in: session.presenter) { marks[ObjectIdentifier(window), default: (window, [])].places.append(place) } }
        }
        // The selection's own copy, or the first that shows (one rule, Inspect.revealing).
        if !Inspect.answersClick(frame), let chosen = Inspect.revealing(copies, sites: want, plan: plan, sessions: running) { chosen.scrollToVisible(chosen.bounds) }
        for (_, m) in marks { overlay(on: m.window)?.mark(m.places) }
    }

    /// Where a node shows, in its window's coordinates, asked each frame:
    /// its own view's box, or, for a node the title bar stands in for
    /// (hidden in the page), what AppKit draws for it: its toolbar item's
    /// view, the search field, the window's title for the header's heading.
    /// Nil for a node that shows nowhere (a tab of the bar's segments is not
    /// looked for).
    typealias Place = () -> NSRect?
    static func place(of node: NodeView, in presenter: Presenter) -> Place? {
        func box(_ view: NSView?) -> Place { { [weak view] in view.flatMap { $0.window != nil && !$0.isHiddenOrHasHiddenAncestor ? $0.convert($0.bounds, to: nil) : nil } } }
        if !node.isHiddenOrHasHiddenAncestor { return box(node) }
        let toolbar = presenter.toolbar
        if let item = toolbar.items[node.id], let window = toolbar.window {
            // Its own view, or the viewer AppKit makes for a plain item.
            if let view = item.view { return box(view) }
            return { [weak item, weak window] in
                guard let item, let window, let frame = window.contentView?.superview else { return nil }
                var pending = frame.subviews
                while !pending.isEmpty {
                    let v = pending.removeFirst()
                    if v.responds(to: NSSelectorFromString("item")), (v.value(forKey: "item") as? NSToolbarItem) === item, !v.isHiddenOrHasHiddenAncestor { return v.convert(v.bounds, to: nil) }
                    pending += v.subviews
                }
                return nil
            }
        }
        if toolbar.search.node === node { return box(toolbar.search.item?.searchField) }
        if toolbar.header?.heading === node, let window = node.window {
            return { [weak window] in
                guard let window, let frame = window.contentView?.superview else { return nil }
                // The title bar's field, never a page label with the same words.
                var pending = frame.subviews.filter { $0 !== window.contentView }
                while !pending.isEmpty { let v = pending.removeFirst(); if let t = v as? NSTextField, t.stringValue == window.title, !t.isHiddenOrHasHiddenAncestor { return t.convert(t.bounds, to: nil) }; pending += v.subviews }
                return nil
            }
        }
        return nil
    }
}

/// The overlay over a window in inspect mode: the banner (with its close),
/// the hover highlight and the outlines. It takes no click itself:
/// InspectHost.take has them all, the close's included.
final class InspectOverlay: NSView {
    private let banner = NSView(), label = NSTextField(labelWithString: ""), close = NSButton()
    private let hoverBox = NSView()
    private var marks: [(place: InspectHost.Place, box: NSView)] = []
    private var timer: Timer?
    private var hovered: InspectHost.Place?

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        hoverBox.wantsLayer = true
        hoverBox.layer?.borderWidth = 2
        hoverBox.layer?.borderColor = NSColor.systemBlue.cgColor
        hoverBox.isHidden = true
        addSubview(hoverBox)
        banner.wantsLayer = true
        banner.layer?.backgroundColor = NSColor.systemBlue.cgColor
        banner.layer?.cornerRadius = 8
        label.textColor = .white
        label.font = .systemFont(ofSize: NSFont.smallSystemFontSize)
        close.bezelStyle = .circular
        close.isBordered = false
        close.image = NSImage(systemSymbolName: "xmark.circle.fill", accessibilityDescription: "Leave inspect mode")
        close.contentTintColor = .white
        say(nil)
        banner.addSubview(label)
        banner.addSubview(close)
        addSubview(banner)
        timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] _ in self?.follow() }
    }
    required init?(coder: NSCoder) { fatalError() }

    override func removeFromSuperview() { timer?.invalidate(); timer = nil; marks = []; hovered = nil; InspectHost.forget(self); super.removeFromSuperview() }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    /// Whether `p` (in the window) is on the banner's close.
    func onClose(_ p: NSPoint) -> Bool { close.convert(close.bounds, to: nil).insetBy(dx: -4, dy: -4).contains(p) }
    func closeHighlighted(_ on: Bool) { close.isHighlighted = on }

    override func layout() {
        super.layout()
        let top = window?.contentLayoutRect.maxY ?? bounds.maxY, width = min(bounds.width - 24, 420)
        let y = isFlipped ? (bounds.height - top) + 6 : top - 6 - 32
        banner.frame = NSRect(x: (bounds.width - width) / 2, y: y, width: width, height: 32)
        label.frame = NSRect(x: 10, y: 8, width: width - 44, height: 16)
        close.frame = NSRect(x: width - 30, y: 6, width: 20, height: 20)
    }

    /// The banner's line: inspect mode's own, or why a click selected nothing.
    func say(_ why: String?) { label.stringValue = why ?? "Inspecting: click a view to select it (Esc leaves)" }

    /// The hover highlight on what a click would select.
    func hover(_ hit: (session: ExactSession?, id: UInt32?, view: NSView?, why: String?)) {
        if let session = hit.session, let id = hit.id, let node = session.presenter.views[id], let place = InspectHost.place(of: node, in: session.presenter) { hovered = place }
        else if let view = hit.view { hovered = { [weak view] in view.flatMap { $0.window != nil ? $0.convert($0.bounds, to: nil) : nil } } }
        else { hovered = nil }
        follow()
    }

    /// Every copy outlined, where each shows.
    func mark(_ places: [InspectHost.Place]) {
        for m in marks { m.box.removeFromSuperview() }
        marks = places.map { place in
            let box = NSView()
            box.wantsLayer = true
            let dash = CAShapeLayer()
            dash.strokeColor = NSColor.systemBlue.cgColor
            dash.fillColor = nil
            dash.lineWidth = 2
            dash.lineDashPattern = [6, 4]
            box.layer?.addSublayer(dash)
            addSubview(box, positioned: .below, relativeTo: banner)
            return (place, box)
        }
        follow()
    }

    /// Outlines kept on their copies as the app scrolls and lays out; the
    /// overlay goes once no session of the app shows here.
    private func follow() {
        guard let window, InspectHost.hosts(window) else { if superview != nil { removeFromSuperview() }; return }
        for m in marks {
            let r = m.place().map { convert($0, from: nil) }
            m.box.isHidden = r == nil
            guard let r else { continue }
            m.box.frame = r
            (m.box.layer?.sublayers?.first as? CAShapeLayer)?.path = CGPath(rect: CGRect(origin: .zero, size: r.size), transform: nil)
        }
        if let r = hovered?() { hoverBox.isHidden = false; hoverBox.frame = convert(r, from: nil) } else { hoverBox.isHidden = true }
    }
}
#endif
