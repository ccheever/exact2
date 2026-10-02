// @ref LLP 1069.001 D5 — `input type="range"` is a `UISlider`: HTML's
// `input` as the thumb moves, `change` as the finger lifts, each value
// clamped and snapped to `step` as HTML sanitizes it; the committed value
// is what the slider shows after the action (D4).
#if os(iOS)
import UIKit

extension ControlHost {
    func makeRange() -> UIControl {
        let slider = UISlider()
        slider.isContinuous = true
        slider.addTarget(self, action: #selector(rangeMoved(_:)), for: .valueChanged)
        slider.addTarget(self, action: #selector(rangeReleased(_:)), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        return slider
    }

    func configureRange(_ slider: UISlider, _ owner: NodeView, accent: UIColor?) {
        let range = RangeSpec(owner.props)
        assign(slider, \.minimumValue, Float(range.min))
        assign(slider, \.maximumValue, Float(range.max))
        assign(slider, \.minimumTrackTintColor, accent)
        let shown = Float(range.shown(owner.props))
        if !slider.isTracking, slider.value != shown { slider.setValue(shown, animated: false) }
        assign(slider, \.accessibilityValue, RangeSpec.format(Double(shown)))
    }

    /// The value the slider reports: its position, snapped as HTML would.
    private func reported(_ slider: UISlider) -> (UInt32, String)? {
        let id = UInt32(slider.tag)
        guard let owner = presenter.views[id] else { return nil }
        return (id, RangeSpec.format(RangeSpec(owner.props).sanitize(Double(slider.value))))
    }

    @objc func rangeMoved(_ slider: UISlider) {
        guard let (id, value) = reported(slider), value != lastRange[id] else { return }
        lastRange[id] = value
        presenter.controlValue(id, value, input: true, change: false)
    }

    @objc func rangeReleased(_ slider: UISlider) {
        guard let (id, value) = reported(slider) else { return }
        lastRange[id] = nil
        presenter.controlValue(id, value, input: false, change: true)
        if let owner = presenter.views[id] { configureRange(slider, owner, accent: slider.minimumTrackTintColor) }
    }

    /// The agent's `type <range> <n>` (D9): the value a drag released
    /// there reports, `input` then `change`.
    func typeRange(_ slider: UISlider, _ node: NodeView, _ text: String) -> [String: Any] {
        guard let n = Double(text.trimmingCharacters(in: .whitespaces)), n.isFinite else { return ["error": "\"\(text)\" is not a number"] }
        let value = RangeSpec.format(RangeSpec(node.props).sanitize(n))
        slider.setValue(Float(value) ?? slider.value, animated: false)
        presenter.controlValue(node.id, value, input: true, change: true)
        if let owner = presenter.views[node.id] { configureRange(slider, owner, accent: slider.minimumTrackTintColor) }
        return ["typed": Int(node.id), "value": presenter.views[node.id]?.props["value"] ?? value, "delivery": "host-activation", "native": "control"]
    }
}
#endif
