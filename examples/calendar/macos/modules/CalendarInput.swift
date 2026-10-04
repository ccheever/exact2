// The Mac calendar's input leaves: mouse drags, context-menu commands and
// the trackpad's horizontal swipes. Contract owns every visible pixel and all
// motion; these views only report.
import Foundation
#if os(macOS)
import AppKit

final class CalendarInputModule: ExactModule {
    weak var root: CalendarInputRoot?
    private var contact: CalendarContact?
    private var serial = 0
    fileprivate var sequence = 0
    private var observers: [NSObjectProtocol] = []
    private var keys: Any?
    private var scrolls: Any?
    private var swipe = CGPoint.zero
    private var swiped = false

    required init(context: ExactModuleContext) {
        super.init(context: context)
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: NSApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
                self?.cancel("interrupted")
            },
            center.addObserver(forName: NSWindow.didResignKeyNotification, object: nil, queue: .main) { [weak self] note in
                guard let self, let window = note.object as? NSWindow, self.contact?.root?.surface.window === window else { return }
                self.cancel("interrupted")
            },
        ]
        // Escape ends a contact; it is consumed only while one is active.
        keys = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.contact != nil, event.keyCode == 53 else { return event }
            self.cancel("cancelled")
            return nil
        }
        // Every scroll reaches its own view too: the month still scrolls.
        scrolls = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
            self?.scrolled(event)
            return event
        }
    }

    /// One page a swipe: a trackpad's (or Magic Mouse's) horizontal gesture
    /// over the month turns it once, however far it travels. A wheel's
    /// notches and the momentum after the fingers lift turn nothing.
    private func scrolled(_ event: NSEvent) {
        guard event.hasPreciseScrollingDeltas, contact == nil, let root, !root.dead,
              let window = root.surface.window, event.window === window else { return }
        if event.phase.contains(.began) || event.phase.contains(.mayBegin) {
            swipe = .zero
            swiped = false
        }
        guard event.phase.contains(.began) || event.phase.contains(.changed) else {
            if event.phase.contains(.ended) || event.phase.contains(.cancelled) {
                swipe = .zero
                swiped = false
            }
            return
        }
        let point = root.surface.convert(event.locationInWindow, from: nil)
        guard point.x >= 0, point.x < root.swipeRight, point.y >= 0,
              point.y <= root.surface.bounds.height else { return }
        swipe.x += event.scrollingDeltaX
        swipe.y += event.scrollingDeltaY
        guard !swiped, abs(swipe.x) >= 30, abs(swipe.x) > abs(swipe.y) * 1.4 else { return }
        swiped = true
        // The content follows the fingers: moving it left shows the next month.
        root.post(swipe.x < 0 ? "swipe:next" : "swipe:previous")
    }

    override class var views: [String: ExactNativeFactory] {
        [
            "calendar-input-root": ExactNativeFactory(for: CalendarInputModule.self) { module, props, events in
                guard module.root == nil else { throw ExactNativeRefusal("calendar has one input root") }
                let root = CalendarInputRoot(module: module, events: events)
                try root.setProps(props)
                module.root = root
                return root
            },
            "calendar-drag-handle": ExactNativeFactory(for: CalendarInputModule.self) { module, props, events in
                let handle = CalendarDragHandle(module: module, events: events)
                try handle.setProps(props)
                return handle
            },
        ]
    }

    /// A drag begins where the button went down, so the grab offset is exact.
    func begin(_ handle: CalendarDragHandle, at point: NSPoint) {
        guard handle.enabled, let root, let window = root.surface.window,
              handle.surface.window === window, root.surface.bounds.width > 0,
              root.surface.bounds.height > 0, serial < 9_007_199_254_740_991 else { return }
        cancel("superseded")
        serial += 1
        let contact = CalendarContact(handle: handle, root: root, serial: serial,
            point: root.surface.convert(point, from: nil),
            source: handle.surface.convert(handle.surface.bounds, to: root.surface),
            viewport: root.surface.convert(root.surface.bounds, to: nil))
        self.contact = contact
        NSCursor.closedHand.push()
        emit(contact, phase: "begin")
    }

    func update(_ handle: CalendarDragHandle, at point: NSPoint, ended: Bool) {
        guard let contact, contact.handle === handle, let root = contact.root else { return }
        guard validViewport(contact) else { cancel("viewport-changed"); return }
        contact.point = root.surface.convert(point, from: nil)
        if ended {
            self.contact = nil
            NSCursor.pop()
        }
        emit(contact, phase: ended ? "end" : "move")
    }

    func cancel(_ reason: String, handle: CalendarDragHandle? = nil, silent: Bool = false) {
        guard let contact, handle == nil || contact.handle === handle else { return }
        self.contact = nil
        NSCursor.pop()
        if !silent { emit(contact, phase: "cancel", reason: reason) }
    }

    func checkViewport() {
        if let contact, !validViewport(contact) { cancel("viewport-changed") }
    }

    private func validViewport(_ contact: CalendarContact) -> Bool {
        guard let root = contact.root, root.surface.window != nil else { return false }
        let current = root.surface.convert(root.surface.bounds, to: nil), old = contact.viewport
        return abs(current.minX - old.minX) < 0.5 && abs(current.minY - old.minY) < 0.5
            && abs(current.width - old.width) < 0.5 && abs(current.height - old.height) < 0.5
    }

    private func emit(_ contact: CalendarContact, phase: String, reason: String = "") {
        guard let root = contact.root, !root.dead,
              contact.point.x.isFinite, contact.point.y.isFinite else { return }
        root.enqueue([
            "v": 1, "phase": phase, "id": contact.id, "serial": contact.serial,
            "x": contact.point.x, "y": contact.point.y, "reason": reason,
            "rx": contact.source.minX, "ry": contact.source.minY,
            "rw": contact.source.width, "rh": contact.source.height,
        ])
    }

    override func destroy() {
        cancel("cancelled", silent: true)
        for observer in observers { NotificationCenter.default.removeObserver(observer) }
        observers = []
        if let keys { NSEvent.removeMonitor(keys) }
        keys = nil
        if let scrolls { NSEvent.removeMonitor(scrolls) }
        scrolls = nil
    }
}

private final class CalendarContact {
    weak var handle: CalendarDragHandle?
    weak var root: CalendarInputRoot?
    let id: String
    let serial: Int
    var point: CGPoint
    let source: CGRect
    let viewport: CGRect
    init(handle: CalendarDragHandle, root: CalendarInputRoot, serial: Int,
         point: CGPoint, source: CGRect, viewport: CGRect) {
        self.handle = handle; self.root = root; self.id = handle.eventID
        self.serial = serial; self.point = point; self.source = source; self.viewport = viewport
    }
}

/// The permanently mounted coordinate space; it lets every click through.
final class CalendarInputRoot: ExactNativeInstance {
    let surface = CalendarRootSurface()
    private weak var module: CalendarInputModule?
    private var queue: [[String: Any]] = []
    private var inflight: Int?
    private(set) var dead = false
    /// The month's right edge in this view: a swipe over the inspector turns nothing.
    private(set) var swipeRight: CGFloat = 0
    override var view: NSView { surface }

    init(module: CalendarInputModule, events: ExactNativeEvents) {
        self.module = module
        super.init(events: events)
        surface.geometryChanged = { [weak module] in module?.checkViewport() }
    }

    /// A page turn is no drag packet: it bypasses the acknowledged queue.
    func post(_ command: String) {
        guard !dead else { return }
        events.message(command)
    }

    func enqueue(_ packet: [String: Any]) {
        guard !dead else { return }
        if packet["phase"] as? String == "move", let last = queue.last,
           last["phase"] as? String == "move", last["serial"] as? Int == packet["serial"] as? Int {
            queue[queue.count - 1] = packet
        } else { queue.append(packet) }
        drain()
    }

    // Contract acknowledges a packet after applying it, so its slot never
    // observes a later packet before the earlier one's continuation ran.
    private func drain() {
        guard !dead, inflight == nil, !queue.isEmpty, let module,
              module.sequence < 9_007_199_254_740_991 else { return }
        var packet = queue.removeFirst()
        module.sequence += 1
        packet["seq"] = module.sequence
        guard let data = try? JSONSerialization.data(withJSONObject: packet, options: [.sortedKeys]) else { return }
        inflight = module.sequence
        events.message(String(decoding: data, as: UTF8.self))
    }

    override func setProps(_ props: [String: String]) throws {
        let text = props["ack"] ?? "0"
        guard !text.isEmpty, text.allSatisfy({ $0.isASCII && $0.isNumber }),
              let ack = Int(text), ack <= 9_007_199_254_740_991 else {
            throw ExactNativeRefusal("calendar input root ack must be a nonnegative safe integer")
        }
        if let right = props["swipe-right"].flatMap(Double.init), right.isFinite { swipeRight = CGFloat(right) }
        if inflight == ack {
            inflight = nil
            DispatchQueue.main.async { [weak self] in self?.drain() }
        }
    }

    override func destroy() {
        if module?.root === self {
            module?.cancel("cancelled", silent: true)
            module?.root = nil
        }
        dead = true; queue.removeAll(); inflight = nil; surface.geometryChanged = nil
    }
}

/// A schedule, a sticker or a palette sticker: a click presses it, a drag
/// past three points moves it, a secondary click opens its context menu.
final class CalendarDragHandle: ExactNativeInstance {
    let surface = CalendarHandleSurface()
    private weak var module: CalendarInputModule?
    private let target = CalendarMenuTarget()
    private(set) var eventID = ""
    private(set) var enabled = false
    private var kind = "none"
    private var completed = false
    private var down: NSPoint?
    private var dragging = false
    override var view: NSView { surface }

    init(module: CalendarInputModule, events: ExactNativeEvents) {
        self.module = module
        super.init(events: events)
        surface.owner = self
        target.pick = { [weak self] command in
            guard let self, self.enabled else { return }
            self.events.message(command)
        }
    }

    fileprivate func mouseDown(_ event: NSEvent) {
        if event.modifierFlags.contains(.control) {
            if let menu = menu() { NSMenu.popUpContextMenu(menu, with: event, for: surface) }
            return
        }
        down = enabled ? event.locationInWindow : nil
        dragging = false
    }

    fileprivate func mouseDragged(_ event: NSEvent) {
        guard let down else { return }
        let point = event.locationInWindow
        if !dragging {
            guard hypot(point.x - down.x, point.y - down.y) > 3 else { return }
            dragging = true
            module?.begin(self, at: down)
        }
        module?.update(self, at: point, ended: false)
    }

    fileprivate func mouseUp(_ event: NSEvent) {
        guard down != nil else { return }
        down = nil
        if dragging {
            dragging = false
            module?.update(self, at: event.locationInWindow, ended: true)
        } else if enabled {
            events.press()
        }
    }

    fileprivate func menu() -> NSMenu? {
        guard enabled, ["event", "plan", "todo", "sticker"].contains(kind) else { return nil }
        let menu = NSMenu()
        menu.autoenablesItems = false
        func add(_ title: String, _ command: String) {
            let item = NSMenuItem(title: title, action: #selector(CalendarMenuTarget.picked(_:)), keyEquivalent: "")
            item.target = target
            item.representedObject = command
            menu.addItem(item)
        }
        if kind == "sticker" {
            add("Remove Sticker", "delete")
        } else {
            add("Edit…", "edit")
            if kind == "todo" { add(completed ? "Mark as Not Done" : "Mark as Done", "toggle") }
            menu.addItem(.separator())
            add("Delete…", "delete")
        }
        return menu
    }

    fileprivate func detached() {
        down = nil
        dragging = false
        module?.cancel("source-removed", handle: self)
    }

    override func setProps(_ props: [String: String]) throws {
        let value = props["enabled"] ?? "true"
        guard let id = props["event-id"], !id.isEmpty, value == "true" || value == "false" else {
            throw ExactNativeRefusal("calendar handle requires event-id and enabled=true|false")
        }
        if id != eventID { module?.cancel("superseded", handle: self) }
        if value == "false" { module?.cancel("disabled", handle: self) }
        eventID = id; enabled = value == "true"
        kind = props["menu-kind"] ?? "none"
        completed = props["completed"] == "true"
    }

    override func destroy() {
        module?.cancel("source-removed", handle: self)
        enabled = false; down = nil; dragging = false; target.pick = nil; surface.owner = nil
    }
}

final class CalendarRootSurface: NSView {
    var geometryChanged: (() -> Void)?
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func setFrameSize(_ newSize: NSSize) { super.setFrameSize(newSize); geometryChanged?() }
    override func setFrameOrigin(_ newOrigin: NSPoint) { super.setFrameOrigin(newOrigin); geometryChanged?() }
    override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); geometryChanged?() }
}

// The host's mouse chain and focus handling run from its own views'
// `mouseDown`; a handle never calls `super`, so a click presses it once.
final class CalendarHandleSurface: NSView {
    fileprivate weak var owner: CalendarDragHandle?
    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { owner?.mouseDown(event) }
    override func mouseDragged(with event: NSEvent) { owner?.mouseDragged(event) }
    override func mouseUp(with event: NSEvent) { owner?.mouseUp(event) }
    override func menu(for event: NSEvent) -> NSMenu? { owner?.menu() }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil { owner?.detached() }
    }
}

private final class CalendarMenuTarget: NSObject {
    var pick: ((String) -> Void)?
    @objc func picked(_ item: NSMenuItem) {
        if let command = item.representedObject as? String { pick?(command) }
    }
}
#else
// This example is the Mac's; another platform refuses both views.
final class CalendarInputModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        let unsupported = ExactNativeFactory { _, _ in throw ExactNativeRefusal("calendar input requires macOS") }
        return ["calendar-input-root": unsupported, "calendar-drag-handle": unsupported]
    }
}
#endif
let exactModule: ExactModule.Type = CalendarInputModule.self
