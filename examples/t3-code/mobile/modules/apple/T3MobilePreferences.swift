// @ref llp/1109.004-home-projection.decision.md#decision
// @ref llp/1109.002-design-system-parity.spec.md#user-preference-and-accessibility-scaling
// Pinned upstream 365aa87982 mobile-preferences.ts and appearancePreferences.ts.
// One app-local owner. Called on the Exact module's main thread; writes are atomic.
import Foundation
import CoreFoundation

final class T3MobilePreferences {
    private let path: URL
    private let changed: () -> Void
    private var values: [String: Any]?
    private var revision = 0
    init(directory: URL, changed: @escaping () -> Void = {}) {
        path = directory.appendingPathComponent("mobile-preferences.json")
        self.changed = changed
    }
    private static let defaults: [String: Any] = [
        "themeMode": "system", "lightThemeId": "t3-code", "darkThemeId": "t3-code", "baseFontSize": 16,
        "terminalFontSize": NSNull(), "codeFontSize": NSNull(), "codeWordBreak": false,
        "composerEnterBehavior": "send", "followUpBehavior": "queue", "projectGroupingMode": "repository",
        "planModeEnabled": false, "workingEnabled": false, "workingExpanded": false, "snoozedExpanded": false, "settledExpanded": false,
    ]
    private static let themes = ["t3-code", "t3-chat", "grove", "ocean", "ember", "iris"]
    private static let choices = ["themeMode": ["system", "light", "dark"], "lightThemeId": themes, "darkThemeId": themes,
        "composerEnterBehavior": ["send", "newline"], "followUpBehavior": ["queue", "steer"],
        "projectGroupingMode": ["repository", "repository_path", "separate"]]
    private static let booleans = ["codeWordBreak", "planModeEnabled", "workingEnabled", "workingExpanded", "snoozedExpanded", "settledExpanded"]
    private func fail(_ message: String) -> NSError { NSError(domain: "T3MobilePreferences", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    private func normalize(_ key: String, _ value: Any) throws -> Any {
        if let choices = Self.choices[key] {
            guard let text = value as? String, choices.contains(text) else { throw fail("Unsupported value for \(key).") }
            return text
        }
        if Self.booleans.contains(key) {
            guard let boolean = value as? NSNumber, CFGetTypeID(boolean) == CFBooleanGetTypeID() else { throw fail("\(key) must be true or false.") }
            return boolean.boolValue
        }
        if ["baseFontSize", "terminalFontSize", "codeFontSize"].contains(key) {
            if value is NSNull && key != "baseFontSize" { return NSNull() }
            guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(), number.doubleValue.isFinite else { throw fail("\(key) must be a finite font size.") }
            let raw = number.doubleValue
            if key == "baseFontSize" { return min(22, max(11, raw.rounded())) }
            if key == "codeFontSize" { return min(18, max(8, raw.rounded())) }
            return min(14, max(6, raw))
        }
        throw fail("Unknown mobile preference: \(key).")
    }
    private func load() throws -> [String: Any] {
        if let values { return values }
        var loaded = Self.defaults
        if FileManager.default.fileExists(atPath: path.path) {
            let data = try Data(contentsOf: path)
            guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw fail("Mobile preferences must be a JSON object.") }
            for (key, value) in object { loaded[key] = try normalize(key, value) }
        }
        values = loaded
        return loaded
    }
    /// Separated from the bridge so persistence can be exercised without a UI/session.
    func request(_ request: [String: Any]) throws -> [String: Any] {
        var next = try load()
        let op = request["op"] as? String ?? ""
        switch op {
        case "mobilePreferences", "mobileHomePreferences": break
        case "mobileToggleShelf":
            guard let section = request["section"] as? String, ["working", "snoozed", "settled"].contains(section) else { throw fail("Unknown thread shelf.") }
            let key = section + "Expanded"
            next[key] = !(next[key] as? Bool ?? false)
        case "mobilePreferencesPatch":
            guard let patch = request["patch"] as? [String: Any], !patch.isEmpty else { throw fail("A mobile preference patch is required.") }
            // Validate every key before writing any part of the patch.
            for (key, value) in patch { next[key] = try normalize(key, value) }
        default: throw fail("Unknown mobile preference operation.")
        }
        if op == "mobileToggleShelf" || op == "mobilePreferencesPatch" {
            let data = try JSONSerialization.data(withJSONObject: next, options: [.sortedKeys])
            try FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
            try data.write(to: path, options: .atomic)
            values = next; revision += 1; changed()
        }
        if op == "mobileHomePreferences" || op == "mobileToggleShelf" {
            return Dictionary(uniqueKeysWithValues: ["workingEnabled", "workingExpanded", "snoozedExpanded", "settledExpanded"].map { ($0, next[$0] as? Bool ?? false) })
        }
        next["revision"] = revision
        return next
    }
    func perform(_ request: [String: Any], reply: ExactReply) {
        do { reply.send(["ok": true, "generation": 0, "value": try self.request(request)]) }
        catch { reply.send(["ok": false, "generation": 0, "error": ["kind": "Persistence", "message": error.localizedDescription, "uncertain": false]]) }
    }
}

#if os(iOS)
import UIKit

/// App-owned rendering for upstream FontSizeSliderRow's 4pt track and 26pt thumb.
/// UIKit supplies tracking/accessibility; no framework slider appearance is changed.
final class T3SettingsSlider: ExactNativeInstance {
    static let factory = ExactNativeFactory { props, events in
        let view = T3SettingsSlider(events: events); try view.setProps(props); return view
    }
    private let slider = T3PreferenceSlider()
    private var increment: Float = 1
    private var committed: Float = 0
    override var view: UIView { slider }
    override var focusTarget: UIView? { slider }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        slider.isContinuous = false
        slider.accessibilityTraits = .adjustable
        slider.addAction(UIAction { [weak self] _ in self?.commit() }, for: .valueChanged)
        slider.adjust = { [weak self] direction in
            guard let self, self.slider.isEnabled else { return }
            self.slider.value += Float(direction) * self.increment; self.commit()
        }
        slider.tap = { [weak self] x in
            guard let self, self.slider.isEnabled else { return }
            let usable = self.slider.bounds.width - 26
            let fraction = usable > 0 ? min(1, max(0, (x - 13) / usable)) : 0
            self.slider.value = self.slider.minimumValue + Float(fraction) * (self.slider.maximumValue - self.slider.minimumValue)
            self.commit()
        }
    }
    override func setProps(_ props: [String: String]) throws {
        guard let low = Float(props["minimum"] ?? ""), let high = Float(props["maximum"] ?? ""),
              let step = Float(props["increment"] ?? ""), let value = Float(props["amount"] ?? ""),
              low.isFinite, high.isFinite, step.isFinite, value.isFinite, high > low, step > 0 else {
            throw ExactNativeRefusal("Font size slider requires finite minimum, maximum, increment and amount.")
        }
        slider.minimumValue = low; slider.maximumValue = high; increment = step
        committed = min(high, max(low, value))
        if !slider.isTracking { slider.value = committed }
        slider.isEnabled = props["enabled"] == "true"
        slider.accessibilityLabel = props["slider-label"] ?? "Font size"
        slider.accessibilityValue = "\(committed) pt"
        slider.minimumTrackTintColor = T3PreferenceColors.color(props["active-color"])
        slider.maximumTrackTintColor = T3PreferenceColors.color(props["inactive-color"])
        let thumb = UIGraphicsImageRenderer(size: CGSize(width: 34, height: 36)).image { context in
            let circle = CGRect(x: 4, y: 3, width: 26, height: 26)
            context.cgContext.setShadow(offset: CGSize(width: 0, height: 2), blur: 3, color: UIColor.black.withAlphaComponent(0.18).cgColor)
            T3PreferenceColors.color(props["thumb-color"]).setFill(); UIBezierPath(ovalIn: circle).fill()
            context.cgContext.setShadow(offset: .zero, blur: 0, color: nil)
            T3PreferenceColors.color(props["border-color"]).setStroke()
            let outline = UIBezierPath(ovalIn: circle.insetBy(dx: 0.5, dy: 0.5)); outline.lineWidth = 1; outline.stroke()
        }
        slider.setThumbImage(thumb, for: .normal)
    }
    private func commit() {
        guard slider.isEnabled else { return }
        let value = min(slider.maximumValue, max(slider.minimumValue,
            slider.minimumValue + ((slider.value - slider.minimumValue) / increment).rounded() * increment))
        slider.setValue(value, animated: true)
        guard value != committed else { return }
        committed = value; slider.accessibilityValue = "\(value) pt"
        UISelectionFeedbackGenerator().selectionChanged()
        events.change(String(value))
    }
    override func agentInput(_ input: ExactNativeInput) throws {
        guard slider.isEnabled else { throw ExactNativeRefusal("The preference has not loaded.") }
        switch input {
        case .text(let value):
            guard let number = Float(value), number.isFinite else { throw ExactNativeRefusal("Font size must be a finite number.") }
            slider.value = number; commit()
        case .key(let key, let phase):
            guard ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].contains(key) else { throw ExactNativeRefusal("Unsupported font size slider key.") }
            guard phase != "up" else { return }
            if key == "Home" { slider.value = slider.minimumValue }
            else if key == "End" { slider.value = slider.maximumValue }
            else { slider.value += ["ArrowLeft", "ArrowDown"].contains(key) ? -increment : increment }
            commit()
        }
    }
    override func destroy() { slider.adjust = nil; slider.tap = nil }
}
private final class T3PreferenceSlider: UISlider {
    var adjust: ((Int) -> Void)?
    var tap: ((CGFloat) -> Void)?
    override init(frame: CGRect) {
        super.init(frame: frame)
        addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(tapped(_:))))
    }
    required init?(coder: NSCoder) { nil }
    override func trackRect(forBounds bounds: CGRect) -> CGRect { CGRect(x: bounds.minX, y: bounds.midY - 2, width: bounds.width, height: 4) }
    override func thumbRect(forBounds bounds: CGRect, trackRect rect: CGRect, value: Float) -> CGRect {
        let fraction = maximumValue > minimumValue ? CGFloat((value - minimumValue) / (maximumValue - minimumValue)) : 0
        return CGRect(x: bounds.minX + fraction * max(0, bounds.width - 26) - 4, y: bounds.midY - 16, width: 34, height: 36)
    }
    override func accessibilityIncrement() { adjust?(1) }
    override func accessibilityDecrement() { adjust?(-1) }
    @objc private func tapped(_ gesture: UITapGestureRecognizer) { if gesture.state == .ended { tap?(gesture.location(in: self).x) } }
}
private enum T3PreferenceColors {
    static func color(_ value: String?) -> UIColor {
        let text = (value ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16) {
            let rgb = text.count == 9 ? hex >> 8 : hex
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                blue: CGFloat(rgb & 255) / 255, alpha: text.count == 9 ? CGFloat(hex & 255) / 255 : 1)
        }
        if text.hasPrefix("rgb"), let start = text.firstIndex(of: "("), let end = text.lastIndex(of: ")") {
            let channels = text[text.index(after: start)..<end].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if channels.count >= 3 { return UIColor(red: channels[0] / 255, green: channels[1] / 255, blue: channels[2] / 255, alpha: channels.count == 4 ? channels[3] : 1) }
        }
        return .label
    }
}

/// Settings root has a native scope menu and a compact Close / iPad Back action.
/// A zero-sized native view supplies the normal Exact change-event channel.
final class T3SettingsNavigation {
    private final class RouteRef { weak var value: ExactRoute?; init(_ value: ExactRoute) { self.value = value } }
    private final class PortRef { weak var value: T3SettingsHeaderPort?; init(_ value: T3SettingsHeaderPort) { self.value = value } }
    private var routes: [String: RouteRef] = [:]
    private var ports: [String: PortRef] = [:]
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        let port = T3SettingsHeaderPort(owner: self, events: events); try port.setProps(props); return port
    }
    func configure(_ route: ExactRoute) { routes[route.key] = RouteRef(route); ports[route.key]?.value?.attach(route) }
    func end(_ route: ExactRoute) {
        guard routes[route.key]?.value === route else { return }
        ports[route.key]?.value?.detach(); routes.removeValue(forKey: route.key)
    }
    fileprivate func bind(_ port: T3SettingsHeaderPort, key: String) { ports[key] = PortRef(port); if let route = routes[key]?.value { port.attach(route) } }
    fileprivate func unbind(_ port: T3SettingsHeaderPort, key: String) { if ports[key]?.value === port { ports.removeValue(forKey: key) } }
}
private struct T3SettingsHeaderConfig: Decodable {
    struct Environment: Decodable { let id: String; let label: String; let subtitle: String; let symbol: String; let selected: Bool }
    struct Project: Decodable { let id: String; let label: String; let selected: Bool }
    let routeKey: String; let close: Bool; let back: Bool; let filtered: Bool; let all: Bool
    let selectedCount: Int; let projectKey: String; let projectLabel: String
    let environments: [Environment]; let projects: [Project]
    let addActionID: String?; let addEnabled: Bool?
}
private final class T3SettingsHeaderPort: ExactNativeInstance {
    private weak var owner: T3SettingsNavigation?
    private weak var route: ExactRoute?
    private let port = UIView()
    private var config: T3SettingsHeaderConfig?
    private var right: [UIBarButtonItem] = []
    private var left: UIBarButtonItem?
    private var key = ""
    override var view: UIView { port }
    init(owner: T3SettingsNavigation, events: ExactNativeEvents) { self.owner = owner; super.init(events: events); port.isUserInteractionEnabled = false }
    override func setProps(_ props: [String: String]) throws {
        guard let data = props["configuration"]?.data(using: .utf8), let next = try? JSONDecoder().decode(T3SettingsHeaderConfig.self, from: data), !next.routeKey.isEmpty else { throw ExactNativeRefusal("Settings header needs a valid configuration and route key.") }
        if key != next.routeKey { detach(); owner?.unbind(self, key: key); key = next.routeKey }
        config = next; owner?.bind(self, key: key)
    }
    fileprivate func attach(_ route: ExactRoute) {
        if self.route !== route { detach(); self.route = route }
        guard let config, route.isLive else { return }
        func action(_ title: String, subtitle: String? = nil, symbol: String? = nil, selected: Bool, kind: String, value: String = "") -> UIAction {
            UIAction(title: title, subtitle: subtitle, image: symbol.flatMap { UIImage(systemName: $0) }, state: selected ? .on : .off) { [weak self] _ in self?.emit(kind, value) }
        }
        let environments = [action("All connected environments", selected: config.all, kind: "all")]
            + config.environments.map { action($0.label, subtitle: $0.subtitle.isEmpty ? nil : $0.subtitle, symbol: $0.symbol, selected: $0.selected, kind: "environment", value: $0.id) }
        let projects = [action("All projects", selected: config.projectKey.isEmpty, kind: "project")]
            + config.projects.map { action($0.label, selected: $0.selected, kind: "project", value: $0.id) }
        let menu = UIMenu(title: "Settings scope", children: [
            UIMenu(title: config.all ? "All environments" : "\(config.selectedCount) \(config.selectedCount == 1 ? "environment" : "environments")", children: environments),
            UIMenu(title: config.projectLabel, children: projects)])
        let filter = UIBarButtonItem(image: UIImage(systemName: config.filtered ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease"), menu: menu)
        filter.accessibilityLabel = "Filter settings environments and projects"; filter.accessibilityIdentifier = "settings-scope"
        right = [filter]
        if let id = config.addActionID {
            let add = UIBarButtonItem(image: UIImage(systemName: "plus"), primaryAction: UIAction { [weak route] _ in route?.element(id)?.click() })
            add.accessibilityLabel = "New scheduled task"; add.isEnabled = config.addEnabled == true
            right.insert(add, at: 0)
        }
        if config.close {
            let close = UIBarButtonItem(image: UIImage(systemName: "xmark"), primaryAction: UIAction { [weak self] _ in self?.emit("close") })
            close.accessibilityLabel = "Close settings"; close.accessibilityIdentifier = "settings-close"
            right.insert(close, at: 0)
        }
        route.controller.navigationItem.rightBarButtonItems = right
        if config.back {
            let back = UIBarButtonItem(image: UIImage(systemName: "chevron.left"), primaryAction: UIAction { [weak self] _ in self?.emit("back") })
            back.accessibilityLabel = "Go back"; left = back; route.controller.navigationItem.leftBarButtonItem = back
        } else if route.controller.navigationItem.leftBarButtonItem === left {
            route.controller.navigationItem.leftBarButtonItem = nil; left = nil
        }
    }
    private func emit(_ kind: String, _ value: String = "") {
        guard let data = try? JSONSerialization.data(withJSONObject: ["kind": kind, "value": value]) else { return }
        events.change(String(decoding: data, as: UTF8.self))
    }
    fileprivate func detach() {
        if let route {
            if route.controller.navigationItem.rightBarButtonItems == right { route.controller.navigationItem.rightBarButtonItems = nil }
            if route.controller.navigationItem.leftBarButtonItem === left { route.controller.navigationItem.leftBarButtonItem = nil }
        }
        route = nil; right = []; left = nil
    }
    override func destroy() { detach(); owner?.unbind(self, key: key) }
}
#endif
