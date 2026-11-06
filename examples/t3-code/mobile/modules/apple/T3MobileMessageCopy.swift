#if os(iOS)
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#ordinary-message-copy-feedback
import UIKit

private struct T3MessageCopyOwner: Decodable, Equatable {
    let origin: String; let environment: String; let generation: Int; let project: String
    let thread: String; let epoch: Int; let route: String; let sourceThread: String; let sourceItem: String
}

private struct T3MessageCopyConfiguration: Decodable, Equatable {
    let id: String; let owner: T3MessageCopyOwner; let text: String
    static func read(_ props: [String: String]) throws -> Self {
        guard let data = props["configuration"]?.data(using: .utf8),
              let result = try? JSONDecoder().decode(Self.self, from: data), !result.id.isEmpty,
              !result.owner.environment.isEmpty, !result.owner.thread.isEmpty, !result.owner.route.isEmpty,
              !result.owner.sourceThread.isEmpty, !result.owner.sourceItem.isEmpty else {
            throw ExactNativeRefusal("Invalid message Copy configuration.")
        }
        return result
    }
}

private struct T3MessageCopyStamp: Codable {
    let instance: UUID; let attempt: UUID
}

final class T3MobileMessageCopy {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class ViewRef { weak var value: T3MessageCopyView?; init(_ value: T3MessageCopyView) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var instances: [UUID: ViewRef] = [:]
    private var alive = true

    func configure(_ route: ExactRoute) { if alive { routes[route.key] = RouteRef(route) } }
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard alive else { throw ExactNativeRefusal("The message Copy session ended.") }
        let instance = T3MessageCopyView(owner: self, events: events)
        try instance.setProps(props); instances[instance.id] = ViewRef(instance)
        return instance
    }
    fileprivate func physicalRoute(_ button: UIView) -> ExactRoute? {
        var responder: UIResponder? = button
        while let current = responder {
            if let controller = current as? UIViewController,
               let route = routes.values.compactMap(\.value).first(where: { $0.controller === controller }) {
                return route
            }
            responder = current.next
        }
        return nil
    }
    func consume(stamp: String, id: String, text: String, generation: Int) -> (view: T3MessageCopyView, revision: Int)? {
        guard alive, stamp.utf8.count <= 256, let data = stamp.data(using: .utf8),
              let ticket = try? JSONDecoder().decode(T3MessageCopyStamp.self, from: data),
              let instance = instances[ticket.instance]?.value,
              let revision = instance.consume(ticket.attempt, id: id, text: text, generation: generation) else { return nil }
        return (instance, revision)
    }
    fileprivate func forget(_ instance: T3MessageCopyView) {
        if instances[instance.id]?.value === instance { instances.removeValue(forKey: instance.id) }
    }
    func end(_ route: ExactRoute) {
        for instance in instances.values.compactMap(\.value) where instance.belongs(to: route) { instance.destroy() }
        if routes[route.key]?.value === route { routes.removeValue(forKey: route.key) }
    }
    func destroy() {
        alive = false
        for instance in instances.values.compactMap(\.value) { instance.destroy() }
        instances.removeAll(); routes.removeAll()
    }
}

private final class T3MessageCopyButton: UIButton {
    var layoutIcon: (() -> Void)?
    override var isHighlighted: Bool { didSet { alpha = isHighlighted ? 0.52 : 1 } }
    override func layoutSubviews() { super.layoutSubviews(); layoutIcon?() }
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool { bounds.insetBy(dx: -8, dy: -8).contains(point) }
}

final class T3MessageCopyView: ExactNativeInstance {
    let id = UUID()
    private let button = T3MessageCopyButton(type: .custom)
    private let icon: T3SymbolView
    private weak var owner: T3MobileMessageCopy?
    private weak var route: ExactRoute?
    private var bound = false
    private var configuration: T3MessageCopyConfiguration?
    private var tint = "#71717b"
    private var active = false
    private var blocked = false
    private var attempt: UUID?
    private var bindingRevision = 0
    private var feedback: Timer?
    private var feedbackSerial = 0
    private var copied = false
    private var alive = true
    override var view: UIView { button }

    init(owner: T3MobileMessageCopy, events: ExactNativeEvents) {
        self.owner = owner; icon = T3SymbolView(events: events)
        super.init(events: events)
        button.isAccessibilityElement = true; button.layer.cornerRadius = 9
        button.addSubview(icon.view)
        button.layoutIcon = { [weak self] in
            guard let self else { return }
            self.icon.view.frame = CGRect(x: (self.button.bounds.width - 13) / 2,
                                          y: (self.button.bounds.height - 13) / 2, width: 13, height: 13)
        }
    }
    override func setProps(_ props: [String: String]) throws {
        let next = try T3MessageCopyConfiguration.read(props), nextTint = props["row-icon"] ?? "#71717b"
        _ = try T3SymbolView.color(nextTint)
        let nextActive = props["copy-active"] == "true"
        if let physical = owner?.physicalRoute(button) {
            guard physical.key == next.owner.route, !bound || route === physical else {
                attempt = nil; bindingRevision += 1; blocked = true; return
            }
            if !bound { route = physical; bound = true }
        } else if bound, configuration?.owner != next.owner {
            attempt = nil; bindingRevision += 1; blocked = true; return
        }
        if configuration?.owner != next.owner { resetCopied() }
        if configuration != next || tint != nextTint || active != nextActive { attempt = nil; bindingRevision += 1 }
        configuration = next; tint = nextTint; active = nextActive; blocked = false
        button.isEnabled = alive && active
        button.accessibilityIdentifier = "thread-copy-\(next.id)"
        let revision = bindingRevision, actionID = UIAction.Identifier("t3-message-copy")
        button.removeAction(identifiedBy: actionID, for: .touchUpInside)
        button.addAction(UIAction(identifier: actionID) { [weak self] _ in
            guard let self, self.bindingRevision == revision else { return }; self.press()
        }, for: .touchUpInside)
        try render()
    }
    private func current() -> T3MessageCopyConfiguration? {
        guard alive, !blocked, active, let config = configuration, let physical = owner?.physicalRoute(button),
              physical.isLive, physical.key == config.owner.route, !bound || route === physical,
              let content = physical.controller.viewIfLoaded, button.isDescendant(of: content),
              button.window != nil, !button.isHidden, button.bounds.width > 0, button.bounds.height > 0,
              UIApplication.shared.applicationState == .active,
              physical.controller.navigationController?.topViewController === physical.controller,
              physical.controller.presentedViewController == nil else { return nil }
        if !bound { route = physical; bound = true }
        return config
    }
    private func press() {
        guard current() != nil else { return }
        let next = UUID(); attempt = next
        guard let data = try? JSONEncoder().encode(T3MessageCopyStamp(instance: id, attempt: next)),
              let stamp = String(data: data, encoding: .utf8) else { attempt = nil; return }
        events.message(stamp)
    }
    fileprivate func consume(_ ticket: UUID, id: String, text: String, generation: Int) -> Int? {
        guard attempt == ticket else { return nil }
        attempt = nil
        guard let config = current(), config.id == id, config.text == text, config.owner.generation == generation else { return nil }
        return bindingRevision
    }
    func didCopy(revision: Int) {
        guard revision == bindingRevision, current() != nil else { return }
        copied = true; feedback?.invalidate(); feedbackSerial += 1
        let serial = feedbackSerial
        try? render()
        let timer = Timer(timeInterval: 1.2, repeats: false) { [weak self] _ in
            guard let self, self.feedbackSerial == serial else { return }
            self.resetCopied()
        }
        feedback = timer; RunLoop.main.add(timer, forMode: .common)
    }
    private func render() throws {
        button.accessibilityLabel = copied ? "Copied" : "Copy message"
        try icon.setProps(["symbol-name": copied ? "checkmark" : "doc.on.doc", "symbol-weight": "regular", "symbol-tint": tint])
    }
    private func resetCopied() {
        feedbackSerial += 1; feedback?.invalidate(); feedback = nil; copied = false
        try? render()
    }
    fileprivate func belongs(to candidate: ExactRoute) -> Bool {
        bound ? route === candidate : configuration?.owner.route == candidate.key
    }
    override func destroy() {
        alive = false; attempt = nil; resetCopied(); button.isEnabled = false
        button.removeAction(identifiedBy: UIAction.Identifier("t3-message-copy"), for: .touchUpInside)
        button.layoutIcon = nil; owner?.forget(self); owner = nil; route = nil; configuration = nil; icon.destroy()
    }
}
#endif
