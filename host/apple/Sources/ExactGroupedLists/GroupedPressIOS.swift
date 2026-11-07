import ExactKit
#if os(iOS) || os(tvOS)
import UIKit

extension GroupedCell {
    static let activeRows = ["press_background_color", "press_color", "press_opacity", "press_opacity_to", "press_scale"]

    func restorePressPresentation() {
        layer.removeAnimation(forKey: "activeOpacity")
        layer.removeAnimation(forKey: "activeScale")
    }

    /// Keep UIKit's content metrics and native selection; authored paint replaces its press only.
    func activePaint(_ style: NodeStyle, content: UIListContentConfiguration, enabled: Bool, protected: (title: Bool, secondary: Bool, image: Bool)) {
        guard Self.activeRows.contains(where: { style[$0] != nil }) else { restorePressPresentation(); return }
        let base = (alpha: alpha, transform: layer.sublayerTransform)
        let background = backgroundConfiguration ?? .clear()
        let nativeUpdate = configurationUpdateHandler
        let color = { (value: BatchValue) in UIColor { traits in
            value.channels(dark: traits.userInterfaceStyle == .dark).map(TextEngine.color) ?? .clear
        } }
        configurationUpdateHandler = { cell, state in
            let down = enabled && state.isHighlighted
            var native = state
            if down { native.isHighlighted = false }
            nativeUpdate?(cell, native)
            var ground = nativeUpdate == nil ? background.updated(for: native) : cell.backgroundConfiguration ?? .clear()
            var shown = content.updated(for: native)
            if down, let fill = style["press_background_color"] {
                ground.backgroundColor = color(fill)
                ground.backgroundColorTransformer = nil
            }
            if down, let ink = style["press_color"] {
                let ink = color(ink)
                if !protected.title { shown.textProperties.color = ink; shown.textProperties.colorTransformer = nil }
                if !protected.secondary { shown.secondaryTextProperties.color = ink; shown.secondaryTextProperties.colorTransformer = nil }
                if !protected.image {
                    shown.image = shown.image?.withRenderingMode(.alwaysTemplate)
                    shown.imageProperties.tintColor = ink
                    shown.imageProperties.tintColorTransformer = nil
                }
            }
            cell.backgroundConfiguration = ground
            cell.contentConfiguration = shown
            let opacity = down ? style["press_opacity_to"]?.number.map { CGFloat(min(1, max(0, $0))) }
                ?? base.alpha * CGFloat(min(1, max(0, style["press_opacity"]?.number ?? 1))) : base.alpha
            let scale = CGFloat(style["press_scale"]?.number ?? 1)
            let factor = down && scale.isFinite && scale > 0 ? scale : 1
            let transform = CATransform3DScale(base.transform, factor, factor, 1)
            (cell as? GroupedCell)?.pressPresentation(alpha: opacity, transform: transform, down: down)
        }
        setNeedsUpdateConfiguration()
    }

    private func pressPresentation(alpha target: CGFloat, transform: CATransform3D, down: Bool) {
        if !down && ExactEnv.agentFreezes { restorePressPresentation(); return }
        let delta = Float(target) - layer.opacity
        let oldOpacity = layer.animation(forKey: "activeOpacity") as? CABasicAnimation
        if oldOpacity?.toValue as? Float != delta || oldOpacity?.isRemovedOnCompletion == down {
            if delta != 0 || oldOpacity != nil {
                let from = (layer.presentation()?.opacity ?? layer.opacity) - layer.opacity
                animatePress("activeOpacity", path: "opacity", from: from, to: delta, additive: true, down: down)
            }
        }
        let oldScale = layer.animation(forKey: "activeScale") as? CABasicAnimation
        let previous = (oldScale?.toValue as? NSValue)?.caTransform3DValue
        if previous.map({ !CATransform3DEqualToTransform($0, transform) }) ?? true || oldScale?.isRemovedOnCompletion == down {
            if !CATransform3DEqualToTransform(layer.sublayerTransform, transform) || oldScale != nil {
                let from = layer.presentation()?.sublayerTransform ?? layer.sublayerTransform
                animatePress("activeScale", path: "sublayerTransform", from: NSValue(caTransform3D: from), to: NSValue(caTransform3D: transform), additive: false, down: down)
            }
        }
    }

    private func animatePress(_ key: String, path: String, from: Any, to: Any, additive: Bool, down: Bool) {
        let animation = CABasicAnimation(keyPath: path)
        animation.fromValue = ExactEnv.agentFreezes ? to : from
        animation.toValue = to
        animation.isAdditive = additive
        animation.duration = PressFeedback.duration
        animation.timingFunction = PressFeedback.timing
        animation.isRemovedOnCompletion = !down
        animation.fillMode = .both
        layer.add(animation, forKey: key)
    }
}

extension GroupedListHost {
    func press(_ node: NodeView, down: Bool) -> Bool {
        #if os(iOS)
        for list in lists.values where list.rows[node.id]?.custom == false {
            (list.cell(node.id) as? GroupedCell)?.forcedPressed = down
            return true
        }
        #endif
        return false
    }
}

extension GroupedListView {
    func activeSignature(_ id: UInt32) -> BatchValue {
        let style = host.presenter.views[id]?.style ?? [:]
        let owns = pressInkBoundaries(id)
        return .array(GroupedCell.activeRows.map { style[$0] ?? .null } + [owns.title, owns.secondary, owns.image].map(BatchValue.bool))
    }

    func activePaint(_ cell: GroupedCell, _ id: UInt32, content: UIListContentConfiguration) {
        let row = rows[id]
        cell.activePaint(host.presenter.views[id]?.style ?? [:], content: content,
                         enabled: row?.pressable == true && row?.disabled == false, protected: pressInkBoundaries(id))
    }

    /// The row's colour may be replaced; colours written below it keep their owner.
    private func pressInkBoundaries(_ id: UInt32) -> (title: Bool, secondary: Bool, image: Bool) {
        guard let row = host.presenter.views[id] else { return (false, false, false) }
        let symbol = symbolView(of: id)
        var texts: [Bool] = [], image = false
        func walk(_ node: NodeView, protected: Bool) {
            guard node.style["display"]?.string != "none" else { return }
            let owns = protected || node.wrote("text_color")
            if node.kind == "text", node.props["text"]?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false {
                texts.append(owns)
                return
            }
            if node === symbol { image = owns || node.style["tint_color"] != nil }
            for child in node.container.subviews.compactMap({ $0 as? NodeView }) { walk(child, protected: owns) }
        }
        for child in row.container.subviews.compactMap({ $0 as? NodeView }) { walk(child, protected: false) }
        return (texts.first ?? false, texts.dropFirst().first ?? false, image)
    }
}
#endif
