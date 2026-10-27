#if os(iOS) || os(tvOS)
import UIKit

/// While UIKit dims tints (a sheet or an alert is up), it greys every tint-coloured part of a
/// control. A control's face here paints most of its colours itself (a badge's circle, a tile's
/// fill), so inside a control every colour with a hue greys as UIKit greys a tint, and the face
/// dims as one; neutral colours (label text) stay, as UIKit leaves them. Outside controls, nothing
/// dims: a status's symbol keeps its colour beside its words.
@MainActor
enum TintDimming {
    private static let probe = UIView()
    private static var greys: [[Int]: [Double]] = [:]

    /// The grey UIKit gives a tint of this colour while it dims tints, in 0...255 channels, at the
    /// colour's own alpha (as a native button's dimmed symbol keeps its alpha)
    static func grey(_ c: [Double]) -> [Double] {
        guard c.count == 4, (c[0...2].max() ?? 0) - (c[0...2].min() ?? 0) > 24 else { return c }
        let key = c.map { Int($0.rounded()) }
        if let known = greys[key] { return known }
        probe.tintColor = UIColor(red: c[0] / 255, green: c[1] / 255, blue: c[2] / 255, alpha: c[3] / 255)
        probe.tintAdjustmentMode = .dimmed
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        guard probe.tintColor.getRed(&r, green: &g, blue: &b, alpha: &a) else { return c }
        let out = [Double(r) * 255, Double(g) * 255, Double(b) * 255, c[3]]
        greys[key] = out
        return out
    }
}

extension NodeView {
    /// This node or one above it is a control
    var insideControl: Bool {
        var current: UIView? = self
        while let view = current {
            if let node = view as? NodeView, node.isControl { return true }
            current = view.superview
        }
        return false
    }

    /// The tint this node paints with: UIKit's dimmed grey inside a control, the resting tint outside one
    var paintedTint: UIColor {
        guard tintAdjustmentMode == .dimmed else {
            if restingTint != tintColor { restingTint = tintColor }
            return tintColor
        }
        return insideControl ? tintColor : restingTint ?? tintColor
    }

    /// A colour as this node paints it now: greyed with the tint inside a control while UIKit dims tints
    func dimmedWithTint(_ c: [Double]) -> [Double] {
        guard tintAdjustmentMode == .dimmed, insideControl else { return c }
        return TintDimming.grey(c)
    }

    func dimmedWithTint(_ c: CGColor) -> CGColor {
        guard tintAdjustmentMode == .dimmed, insideControl,
              let rgb = c.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil),
              let parts = rgb.components, parts.count == 4 else { return c }
        let grey = TintDimming.grey(parts.map { Double($0) * 255 })
        return CGColor(srgbRed: grey[0] / 255, green: grey[1] / 255, blue: grey[2] / 255, alpha: grey[3] / 255)
    }
}
#endif
