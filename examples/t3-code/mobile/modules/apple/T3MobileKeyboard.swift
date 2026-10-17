// @ref llp/1109.011-responsive-workspace.decision.md#navigation-and-data-ownership
#if os(iOS)
// Pinned365aa87982 T3KeyboardCommandsModule / HardwareKeyboardCommandProvider.
// App-owned commands on the exclusive window's public host controller;
// the active route owns admission. Never claim first responder.
import UIKit

final class T3MobileKeyboard {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class PortRef { weak var value: T3KeyboardPort?; init(_ value: T3KeyboardPort) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var ports: [String: PortRef] = [:]
    private weak var window: ExactWindow?
    fileprivate var controller: UIViewController? {
        guard let window, window.isLive, window.exclusive else { return nil }
        return window.window?.rootViewController
    }
    func configureWindow(_ window: ExactWindow) {
        if self.window !== window { for port in ports.values { port.value?.detach() } }
        self.window = window
        for (key, port) in ports { if let route = routes[key]?.value { port.value?.attach(route) } }
    }
    func endWindow(_ window: ExactWindow) {
        guard self.window === window else { return }
        for port in ports.values { port.value?.detach() }
        self.window = nil
    }
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let port = T3KeyboardPort(owner: self, events: events); try port.setProps(props); return port
    }
    func configure(_ route: ExactRoute) { routes[route.key] = RouteRef(route); ports[route.key]?.value?.attach(route) }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        ports[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ port: T3KeyboardPort, key: String) {
        if let previous = ports[key]?.value, previous !== port { previous.detach() }
        ports[key] = PortRef(port); if let route = routes[key]?.value { port.attach(route) }
    }
    fileprivate func unbind(_ port: T3KeyboardPort, key: String) {
        if ports[key]?.value === port { ports.removeValue(forKey: key) }
    }
    func destroy() { for port in ports.values { port.value?.detach() }; ports.removeAll(); routes.removeAll(); window = nil }
}

private struct T3KeyboardConfiguration: Decodable, Equatable {
    let routeKey: String; let version: String; let enabled: Bool
}
// UIKit copies key commands as plain UIKeyCommand objects. Its documented
// propertyList survives copying; a fresh token owns only this registration.
private enum T3KeyboardRegistrations {
    private final class WeakPort {
        weak var value: T3KeyboardPort?
        init(_ value: T3KeyboardPort) { self.value = value }
    }
    private static var ports: [String: WeakPort] = [:]
    static func add(_ port: T3KeyboardPort, token: String) { ports[token] = WeakPort(port) }
    static func remove(_ token: String) { ports.removeValue(forKey: token) }
    static func invoke(_ command: UIKeyCommand, receiver: UIViewController) {
        guard let token = command.propertyList as? String else { return }
        ports[token]?.value?.invoke(command, receiver: receiver)
    }
}
private extension UIViewController {
    @objc(t3codeMobileCopyThreadReference:)
    func t3codeMobileCopyThreadReference(_ sender: UIKeyCommand) {
        T3KeyboardRegistrations.invoke(sender, receiver: self)
    }
}
private final class T3KeyboardPort: ExactNativeInstance {
    private weak var owner: T3MobileKeyboard?
    private weak var route: ExactRoute?
    private let root = UIView()
    private var config: T3KeyboardConfiguration?
    private var command: UIKeyCommand?
    private weak var registeredController: UIViewController?
    private var token = ""
    private var key = ""
    private var alive = true
    override var view: UIView { root }
    init(owner: T3MobileKeyboard, events: ExactNativeEvents) {
        self.owner = owner; super.init(events: events); root.isUserInteractionEnabled = false
    }
    override func setProps(_ props: [String: String]) throws {
        guard let bytes = props["configuration"]?.data(using: .utf8),
              let next = try? JSONDecoder().decode(T3KeyboardConfiguration.self, from: bytes) else {
            throw ExactNativeRefusal("Keyboard commands need a route configuration.")
        }
        if next != config { detach(); owner?.unbind(self, key: key) }
        config = next; key = next.routeKey
        if next.enabled && !key.isEmpty { owner?.bind(self, key: key) }
    }
    fileprivate func attach(_ route: ExactRoute) {
        guard alive, let config, config.enabled, config.routeKey == route.key, route.isLive else { return }
        guard let controller = owner?.controller else { detach(); return }
        if self.route === route, command != nil, registeredController === controller { return }
        detach(); self.route = route
        token = UUID().uuidString
        let command = UIKeyCommand(title: "Copy PR Link or Thread ID", image: nil,
            action: #selector(UIViewController.t3codeMobileCopyThreadReference(_:)), input: "c",
            modifierFlags: [.command, .shift], propertyList: token)
        command.discoverabilityTitle = command.title
        command.wantsPriorityOverSystemBehavior = true
        registeredController = controller
        T3KeyboardRegistrations.add(self, token: token)
        self.command = command; controller.addKeyCommand(command)
    }
    fileprivate func invoke(_ sender: UIKeyCommand, receiver: UIViewController) {
        guard alive, command != nil, !token.isEmpty, sender.propertyList as? String == token,
              sender.input == "c", sender.modifierFlags == [.command, .shift],
              sender.action == #selector(UIViewController.t3codeMobileCopyThreadReference(_:)),
              let config, config.enabled, let route, route.isLive,
              route.key == config.routeKey, let controller = owner?.controller,
              registeredController === controller,
              let window = controller.viewIfLoaded?.window,
              receiver.viewIfLoaded?.window === window,
              let bytes = try? JSONSerialization.data(withJSONObject: ["routeKey": config.routeKey,
                "version": config.version, "kind": "copyThreadReference"]) else { return }
        events.change(String(decoding: bytes, as: UTF8.self))
    }
    fileprivate func detach() {
        T3KeyboardRegistrations.remove(token)
        if let command { registeredController?.removeKeyCommand(command) }
        command = nil; registeredController = nil; token = ""; route = nil
    }
    override func destroy() { alive = false; detach(); owner?.unbind(self, key: key) }
}
#endif
