// Calendar's input leaves report contacts; Contract owns all visible UI and motion.
import Foundation
#if os(iOS)
import UIKit

final class CalendarInputModule: ExactModule {
    weak var root: CalendarInputRoot?
    private var contact: CalendarContact?
    private var serial = 0
    fileprivate var sequence = 0
    private var interrupted: NSObjectProtocol?

    required init(context: ExactModuleContext) {
        super.init(context: context)
        interrupted = NotificationCenter.default.addObserver(
            forName: UIApplication.willResignActiveNotification, object: nil, queue: .main
        ) { [weak self] _ in self?.cancel("interrupted") }
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

    func begin(_ handle: CalendarDragHandle, gesture: UILongPressGestureRecognizer) {
        guard handle.enabled, let root, let window = root.surface.window,
              handle.surface.window === window, root.surface.bounds.width > 0,
              root.surface.bounds.height > 0, serial < 9_007_199_254_740_991 else { return }
        cancel("superseded")
        serial += 1
        let contact = CalendarContact(handle: handle, root: root, serial: serial,
            point: gesture.location(in: root.surface),
            source: handle.surface.convert(handle.surface.bounds, to: root.surface),
            viewport: root.surface.convert(root.surface.bounds, to: window))
        self.contact = contact
        emit(contact, phase: "begin")
    }

    func update(_ handle: CalendarDragHandle, gesture: UILongPressGestureRecognizer, ended: Bool) {
        guard let contact, contact.handle === handle, let root = contact.root else { return }
        guard validViewport(contact) else { cancel("viewport-changed"); return }
        contact.point = gesture.location(in: root.surface)
        if ended { self.contact = nil }
        emit(contact, phase: ended ? "end" : "move")
    }

    func cancel(_ reason: String, handle: CalendarDragHandle? = nil, silent: Bool = false) {
        guard let contact, handle == nil || contact.handle === handle else { return }
        self.contact = nil
        if !silent { emit(contact, phase: "cancel", reason: reason) }
    }

    func checkViewport() {
        if let contact, !validViewport(contact) { cancel("viewport-changed") }
    }

    private func validViewport(_ contact: CalendarContact) -> Bool {
        guard let root = contact.root, let window = root.surface.window else { return false }
        let current = root.surface.convert(root.surface.bounds, to: window), old = contact.viewport
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
        if let interrupted { NotificationCenter.default.removeObserver(interrupted) }
        interrupted = nil
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

final class CalendarInputRoot: ExactNativeInstance {
    let surface = CalendarInputSurface()
    private weak var module: CalendarInputModule?
    private var queue: [[String: Any]] = []
    private var inflight: Int?
    private(set) var dead = false
    override var view: UIView { surface }

    init(module: CalendarInputModule, events: ExactNativeEvents) {
        self.module = module
        super.init(events: events)
        surface.isUserInteractionEnabled = false
        surface.geometryChanged = { [weak module] in module?.checkViewport() }
    }

    func enqueue(_ packet: [String: Any]) {
        guard !dead else { return }
        if packet["phase"] as? String == "move", let last = queue.last,
           last["phase"] as? String == "move", last["serial"] as? Int == packet["serial"] as? Int {
            queue[queue.count - 1] = packet
        } else { queue.append(packet) }
        drain()
    }

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

final class CalendarDragHandle: ExactNativeInstance {
    let surface = CalendarInputSurface()
    private weak var module: CalendarInputModule?
    private(set) var eventID = ""
    private(set) var enabled = false
    private let target = CalendarGestureTarget()
    private let hold = UILongPressGestureRecognizer()
    private let tap = UITapGestureRecognizer()
    override var view: UIView { surface }

    init(module: CalendarInputModule, events: ExactNativeEvents) {
        self.module = module
        super.init(events: events)
        hold.minimumPressDuration = 0.3
        hold.allowableMovement = 8
        hold.addTarget(target, action: #selector(CalendarGestureTarget.held(_:)))
        tap.addTarget(target, action: #selector(CalendarGestureTarget.tapped(_:)))
        tap.require(toFail: hold)
        target.hold = { [weak self] gesture in self?.held(gesture) }
        target.tap = { [weak self] in
            guard let self, self.enabled else { return }
            self.events.press()
        }
        surface.addGestureRecognizer(hold)
        surface.addGestureRecognizer(tap)
        surface.detached = { [weak self] in
            guard let self else { return }
            self.module?.cancel("source-removed", handle: self)
        }
    }

    private func held(_ gesture: UILongPressGestureRecognizer) {
        switch gesture.state {
        case .began: module?.begin(self, gesture: gesture)
        case .changed: module?.update(self, gesture: gesture, ended: false)
        case .ended: module?.update(self, gesture: gesture, ended: true)
        case .cancelled, .failed:
            // UIKit can cancel while the host is removing this source. Let its
            // destroy callback name that terminal before the generic cancel.
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.module?.cancel("cancelled", handle: self)
            }
        default: break
        }
    }

    override func setProps(_ props: [String: String]) throws {
        let value = props["enabled"] ?? "true"
        guard let id = props["event-id"], !id.isEmpty, value == "true" || value == "false" else {
            throw ExactNativeRefusal("calendar handle requires event-id and enabled=true|false")
        }
        if id != eventID { module?.cancel("superseded", handle: self) }
        if value == "false" { module?.cancel("disabled", handle: self) }
        eventID = id; enabled = value == "true"
        hold.isEnabled = enabled; tap.isEnabled = enabled
    }

    override func destroy() {
        module?.cancel("source-removed", handle: self)
        enabled = false; target.hold = nil; target.tap = nil; surface.detached = nil
        hold.isEnabled = false; tap.isEnabled = false
    }
}

final class CalendarInputSurface: UIView {
    var geometryChanged: (() -> Void)?
    var detached: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); geometryChanged?() }
    override func didMoveToWindow() {
        super.didMoveToWindow()
        geometryChanged?()
        if window == nil { detached?() }
    }
}

private final class CalendarGestureTarget: NSObject {
    var hold: ((UILongPressGestureRecognizer) -> Void)?
    var tap: (() -> Void)?
    @objc func held(_ gesture: UILongPressGestureRecognizer) { hold?(gesture) }
    @objc func tapped(_ gesture: UITapGestureRecognizer) { if gesture.state == .ended { tap?() } }
}
#else
// The build queries the same roster on macOS before compiling the iOS artifact.
final class CalendarInputModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        let unsupported = ExactNativeFactory { _, _ in throw ExactNativeRefusal("calendar input requires iOS") }
        return ["calendar-input-root": unsupported, "calendar-drag-handle": unsupported]
    }
}
#endif
let exactModule: ExactModule.Type = CalendarInputModule.self
