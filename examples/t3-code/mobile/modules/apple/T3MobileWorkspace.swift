// @ref examples/t3-code/mobile/docs/1130.011-responsive-workspace.decision.md#composition-decision
// Inspector input belongs to an ordinary view inside the workspace route.
#if os(iOS)
import UIKit

final class T3MobileWorkspace {
    private final class Ref {
        weak var value: T3WorkspaceDividerView?
        init(_ value: T3WorkspaceDividerView) { self.value = value }
    }
    private var active = true
    private var views: [Ref] = []
    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard active else { throw ExactNativeRefusal("The workspace session has ended.") }
        let instance = T3WorkspaceDividerView(events: events)
        try instance.setProps(props)
        views.removeAll { $0.value == nil }; views.append(Ref(instance))
        return instance
    }
    func destroy() {
        guard active else { return }; active = false
        for view in views { view.value?.destroy() }
        views.removeAll()
    }
}
private struct T3WorkspaceConfiguration: Decodable {
    let inspectorOwner: String
    let viewportWidth: Double
    let viewportHeight: Double
    let inspectorContentWidth: Double
    let inspectorVisible: Bool
    let inspectorResizing: Bool
    let dividerColor: String
    let dividerActiveColor: String
    static func decode(_ props: [String: String]) throws -> Self {
        guard let json = props["workspace-configuration"], let bytes = json.data(using: .utf8),
              let value = try? JSONDecoder().decode(Self.self, from: bytes), !value.inspectorOwner.isEmpty,
              [value.viewportWidth, value.viewportHeight, value.inspectorContentWidth].allSatisfy({ $0.isFinite && $0 > 0 }) else {
            throw ExactNativeRefusal("Inspector input requires its current owner and finite column geometry.")
        }
        return value
    }
}
private final class T3WorkspaceDividerView: ExactNativeInstance {
    private let root = T3WorkspaceDivider()
    private var configuration: T3WorkspaceConfiguration?
    private var drag: T3WorkspaceConfiguration?
    private var active = true
    override var view: UIView { root }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        root.resize = { [weak self] phase, translation in self?.resize(phase, translation) }
    }
    override func setProps(_ props: [String: String]) throws {
        guard active else { return }
        let next = try T3WorkspaceConfiguration.decode(props)
        if let drag, drag.inspectorOwner != next.inspectorOwner || drag.viewportWidth != next.viewportWidth
            || drag.viewportHeight != next.viewportHeight || !next.inspectorVisible {
            self.drag = nil; root.cancel()
        }
        configuration = next
        root.isUserInteractionEnabled = next.inspectorVisible
        root.accessibilityElementsHidden = !next.inspectorVisible
        root.configure(width: next.inspectorContentWidth, dragging: next.inspectorResizing,
            normal: T3WorkspaceColor.parse(next.dividerColor), active: T3WorkspaceColor.parse(next.dividerActiveColor))
    }
    private func resize(_ phase: String, _ translation: Double) {
        guard active, root.window != nil, let configuration, configuration.inspectorVisible else { return }
        if phase == "start" { drag = configuration }
        let captured = phase == "step" ? configuration : drag
        guard let captured, captured.inspectorOwner == configuration.inspectorOwner,
              captured.viewportWidth == configuration.viewportWidth, captured.viewportHeight == configuration.viewportHeight else { return }
        if phase == "end" { drag = nil }
        let event: [String: Any] = ["kind": "resize", "owner": captured.inspectorOwner,
            "phase": phase, "startWidth": captured.inspectorContentWidth, "translationX": translation]
        guard let bytes = try? JSONSerialization.data(withJSONObject: event) else { return }
        events.change(String(decoding: bytes, as: UTF8.self))
    }
    override func destroy() {
        guard active else { return }; active = false
        drag = nil; configuration = nil; root.resize = nil; root.cancel()
    }
}

/// The source divider occupies 44pt centered on its boundary. It owns input
/// recognition only; root constrains and persists width from the captured start.
private final class T3WorkspaceDivider: UIView {
    var resize: ((String, Double) -> Void)?
    private let line = UIView()
    private var lineThickness: CGFloat = 1
    private lazy var pan = T3WorkspacePan(target: self, action: #selector(panned))
    override init(frame: CGRect) {
        super.init(frame: frame); addSubview(line); addGestureRecognizer(pan)
        isAccessibilityElement = true; accessibilityLabel = "Resize detail pane"
        accessibilityTraits = .adjustable
        accessibilityCustomActions = [
            UIAccessibilityCustomAction(name: "Make pane wider", target: self, selector: #selector(widen)),
            UIAccessibilityCustomAction(name: "Make pane narrower", target: self, selector: #selector(narrow)),
        ]
    }
    required init?(coder: NSCoder) { nil }
    func configure(width: Double, dragging: Bool, normal: UIColor, active: UIColor) {
        accessibilityValue = "\(Int(width.rounded())) points wide"
        lineThickness = dragging ? 2 : 1 / max(1, traitCollection.displayScale)
        line.backgroundColor = dragging ? active : normal.withAlphaComponent(normal.cgColor.alpha * 0.7)
        setNeedsLayout()
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        line.frame = CGRect(x: bounds.midX - lineThickness / 2, y: 0, width: lineThickness, height: bounds.height)
    }
    @objc private func panned() {
        switch pan.state {
        case .began: resize?("start", 0); resize?("update", pan.translationX)
        case .changed: resize?("update", pan.translationX)
        case .ended, .cancelled: resize?("end", pan.translationX)
        default: break
        }
    }
    func cancel() { pan.isEnabled = false; pan.isEnabled = true }
    override func accessibilityIncrement() { if !isHidden && isUserInteractionEnabled { resize?("step", -24) } }
    override func accessibilityDecrement() { if !isHidden && isUserInteractionEnabled { resize?("step", 24) } }
    @objc private func widen() -> Bool { accessibilityIncrement(); return true }
    @objc private func narrow() -> Bool { accessibilityDecrement(); return true }
}
private final class T3WorkspacePan: UIGestureRecognizer {
    private var origin: CGPoint?
    private(set) var translationX: Double = 0
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard origin == nil, touches.count == 1, let touch = touches.first else {
            state = state == .possible ? .failed : .cancelled; return
        }
        origin = touch.location(in: view?.window)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let origin, let touch = touches.first else { return }
        let current = touch.location(in: view?.window)
        translationX = Double(current.x - origin.x)
        if state == .possible {
            if abs(current.y - origin.y) > 24 { state = .failed }
            else if abs(translationX) > 4 { state = .began }
        } else if state == .began || state == .changed { state = .changed }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        state = state == .possible ? .failed : .ended
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { state = .cancelled }
    override func reset() { super.reset(); origin = nil; translationX = 0 }
}
private enum T3WorkspaceColor {
    static func parse(_ value: String?) -> UIColor {
        let text = (value ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16) {
            let rgb = text.count == 9 ? hex >> 8 : hex
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                blue: CGFloat(rgb & 255) / 255, alpha: text.count == 9 ? CGFloat(hex & 255) / 255 : 1)
        }
        if text.hasPrefix("rgb"), let start = text.firstIndex(of: "("), let end = text.lastIndex(of: ")") {
            let parts = text[text.index(after: start)..<end].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if parts.count >= 3 { return UIColor(red: parts[0] / 255, green: parts[1] / 255, blue: parts[2] / 255, alpha: parts.count > 3 ? parts[3] : 1) }
        }
        return .systemBackground
    }
}
#endif
