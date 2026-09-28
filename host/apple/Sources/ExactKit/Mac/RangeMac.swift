// @ref LLP 1069.001 D5 — `input type="range"` is an `NSSlider`: HTML's
// `input` as the knob moves, `change` as the mouse lifts, each value clamped
// and snapped to `step` as HTML sanitizes it; the committed value is what
// the slider shows after the action (D4).
#if os(macOS)
import AppKit

extension ControlHost {
    func makeRange() -> NSControl {
        let slider = NSSlider(value: 0, minValue: 0, maxValue: 100, target: nil, action: nil)
        slider.isContinuous = true
        return slider
    }

    func configureRange(_ slider: NSSlider, _ owner: NodeView) {
        let range = RangeSpec(owner.props)
        slider.minValue = range.min
        slider.maxValue = range.max
        let shown = range.shown(owner.props)
        if slider.doubleValue != shown, NSApp.currentEvent?.type != .leftMouseDragged { slider.doubleValue = shown }
        // A value set inside the knob's tracking draws only when asked to.
        slider.needsDisplay = true
        slider.setAccessibilityValue(RangeSpec.format(shown))
    }

    /// A move or a release: `input` while the mouse is down, `change` when it lifts.
    func rangeChanged(_ slider: NSSlider) {
        let id = UInt32(slider.tag)
        guard let owner = presenter.views[id] else { return }
        let value = RangeSpec.format(RangeSpec(owner.props).sanitize(slider.doubleValue))
        let released = NSApp.currentEvent.map { $0.type == .leftMouseUp || $0.type == .keyDown } ?? true
        if value != lastRange[id] {
            lastRange[id] = value
            presenter.controlValue(id, value, input: true, change: false)
        }
        if released {
            lastRange[id] = nil
            presenter.controlValue(id, value, input: false, change: true)
            if let owner = presenter.views[id] { configureRange(slider, owner) }
        }
    }

    /// The agent's `type <range> <n>` (D9): the value a drag released
    /// there reports, `input` then `change`.
    func typeRange(_ slider: NSSlider, _ node: NodeView, _ text: String) -> [String: Any] {
        guard let n = Double(text.trimmingCharacters(in: .whitespaces)), n.isFinite else { return ["error": "\"\(text)\" is not a number"] }
        let value = RangeSpec.format(RangeSpec(node.props).sanitize(n))
        slider.doubleValue = Double(value) ?? slider.doubleValue
        presenter.controlValue(node.id, value, input: true, change: true)
        if let owner = presenter.views[node.id] { configureRange(slider, owner) }
        return ["typed": Int(node.id), "value": presenter.views[node.id]?.props["value"] ?? value, "delivery": "host-activation", "native": "control"]
    }
}
#endif
