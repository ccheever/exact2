#if os(iOS)
// Existing X22/#127: app-owned UIKit facts for mobile safe-area-dependent chrome.
// @ref llp/1106.004-home-projection.decision.md#rendering
import UIKit

private final class T3LayoutProbe: UIView {
    var changed: (() -> Void)?
    override func layoutSubviews() { super.layoutSubviews(); changed?() }
    override func safeAreaInsetsDidChange() { super.safeAreaInsetsDidChange(); changed?() }
    override func didMoveToWindow() { super.didMoveToWindow(); changed?() }
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}

final class T3LayoutFacts: ExactNativeInstance {
    private let root = T3LayoutProbe()
    private var previous = ""
    private var alive = true
    override var view: UIView { root }
    static let factory = ExactNativeFactory { _, events in T3LayoutFacts(events: events) }
    override init(events: ExactNativeEvents) {
        super.init(events: events)
        root.backgroundColor = .clear
        root.isAccessibilityElement = false
        root.changed = { [weak self] in self?.publish() }
    }
    private func publish() {
        guard alive, let window = root.window else { return }
        let glass: Bool
        if #available(iOS 26.0, *) { glass = true } else { glass = false }
        let value: [String: Any] = ["safeBottom": Double(window.safeAreaInsets.bottom), "liquidGlass": glass]
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .sortedKeys) else { return }
        let text = String(decoding: data, as: UTF8.self)
        guard text != previous else { return }
        previous = text
        events.change(text)
    }
    override func destroy() { alive = false; root.changed = nil }
}
#endif
