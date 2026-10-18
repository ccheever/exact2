// @ref LLP 1069.001 D5 — `input type="range"` is a `UISlider`: HTML's
// `input` as the thumb moves, `change` as the finger lifts, each value
// clamped and snapped to `step` as HTML sanitizes it, and the thumb with it,
// as a browser's moves from step to step (`step="any"` glides) with no tick
// marks, as HTML draws none without a `list` and a stepped SwiftUI `Slider`
// none on iOS 27; VoiceOver moves it a step at a time. The thumb stays where
// the person put it until the bound value changes (D4, amended).
#if os(iOS)
import UIKit

extension ControlHost {
    func makeRange() -> UIControl {
        let slider = RangeSlider()
        slider.adjust = { [weak self] slider, direction in self?.adjustRange(slider, by: direction) }
        slider.isContinuous = true
        slider.addTarget(self, action: #selector(rangeMoved(_:)), for: .valueChanged)
        slider.addTarget(self, action: #selector(rangeReleased(_:)), for: [.touchUpInside, .touchUpOutside, .touchCancel])
        return slider
    }

    func configureRange(_ slider: RangeSlider, _ owner: NodeView) {
        let range = RangeSpec(owner.props)
        // Read before new bounds clamp the thumb's `Float`
        var value = slider.exact
        assign(slider, \.minimumValue, Float(range.min))
        assign(slider, \.maximumValue, Float(range.max))
        // The bound value is written when it changes, as the web build writes
        // an input's `value` (LLP 1069.001 D4, amended 2026-10-04); new bounds
        // or a new step sanitize the value again, as HTML's do.
        let bound = owner.props["value"] ?? ""
        if !slider.isTracking, appliedRange[owner.id] != bound {
            appliedRange[owner.id] = bound
            value = range.shown(owner.props)
        }
        slider.exact = range.sanitize(value)
        assign(slider, \.accessibilityValue, RangeSpec.format(slider.exact))
    }

    /// The value the slider reports: its position, snapped as HTML would.
    private func reported(_ slider: RangeSlider) -> (UInt32, String)? {
        let id = UInt32(slider.tag)
        guard let owner = presenter.views[id] else { return nil }
        return (id, RangeSpec.format(RangeSpec(owner.props).sanitize(slider.exact)))
    }

    @objc func rangeMoved(_ slider: RangeSlider) {
        let id = UInt32(slider.tag)
        guard let owner = presenter.views[id] else { return }
        // The person moved the thumb: its own value, snapped to the step
        slider.exact = RangeSpec(owner.props).sanitize(Double(slider.value))
        let value = RangeSpec.format(slider.exact)
        guard value != lastRange[id] else { return }
        lastRange[id] = value
        presenter.controlValue(id, value, input: true, change: false)
    }

    @objc func rangeReleased(_ slider: RangeSlider) {
        // Cleared before the liveness check, so a slider whose row went
        // mid-drag leaves nothing behind.
        lastRange[UInt32(slider.tag)] = nil
        guard let (id, value) = reported(slider) else { return }
        presenter.controlValue(id, value, input: false, change: true)
        if let owner = presenter.views[id] { configureRange(slider, owner) }
    }

    /// The agent's `type <range> <n>` (D9): the value a drag released
    /// there reports, `input` then `change`.
    func typeRange(_ slider: RangeSlider, _ node: NodeView, _ text: String) -> [String: Any] {
        guard let n = Double(text.trimmingCharacters(in: .whitespaces)), n.isFinite else { return ["error": "\"\(text)\" is not a number"] }
        commitRange(slider, node, n)
        return ["typed": Int(node.id), "value": RangeSpec.format(slider.exact), "delivery": "host-activation", "native": "control"]
    }

    /// VoiceOver's swipe up or down, as WebKit adjusts a range: a written
    /// `step` is one move; without one, or with `any`, a twentieth of the
    /// span, at least the default step.
    private func adjustRange(_ slider: RangeSlider, by direction: Double) {
        guard let node = presenter.views[UInt32(slider.tag)], slider.isEnabled, !node.inert else { return }
        let range = RangeSpec(node.props), from = range.sanitize(slider.exact)
        let authored = node.props["step"]?.isEmpty == false ? range.step : nil
        let to = range.sanitize(from + direction * (authored ?? Swift.max((range.max - range.min) / 20, range.step ?? 0)))
        if to != from { commitRange(slider, node, to) }
    }

    /// Puts the thumb at `value`, sanitized, and reports it as a released
    /// drag does: `input` then `change`.
    private func commitRange(_ slider: RangeSlider, _ node: NodeView, _ value: Double) {
        slider.exact = RangeSpec(node.props).sanitize(value)
        presenter.controlValue(node.id, RangeSpec.format(slider.exact), input: true, change: true)
        if let owner = presenter.views[node.id] { configureRange(slider, owner) }
    }
}

/// UIKit's slider, whose VoiceOver adjustments the host makes (`adjustRange`)
final class RangeSlider: UISlider {
    var adjust: ((RangeSlider, Double) -> Void)?
    /// The value it shows, which its `Float` `value` may round (a step of
    /// 1e-6): kept while the thumb stays where it was put
    var exact: Double {
        get { put.flatMap { Float($0) == value ? $0 : nil } ?? Double(value) }
        set { put = newValue; if value != Float(newValue) { value = Float(newValue) } }
    }
    private var put: Double?
    override func accessibilityIncrement() { adjust?(self, 1) }
    override func accessibilityDecrement() { adjust?(self, -1) }
}
#endif
