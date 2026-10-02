// CSS clip-path commands validated by the kernel, shared by UIKit and AppKit:
// `{"rule": "nonzero" | "evenodd", "commands": [["M", [x, y]], …]}`.
import CoreGraphics
import QuartzCore

enum ClipPath {
    static func path(_ value: BatchValue?) -> CGPath? {
        guard let commands = field(value, "commands")?.array, !commands.isEmpty else { return nil }
        let path = CGMutablePath()
        for command in commands {
            guard let kind = command.array?.first?.string, let values = command.array?.last?.numbers else { continue }
            func point(_ index: Int) -> CGPoint { CGPoint(x: values[index], y: values[index + 1]) }
            switch kind {
            case "M" where values.count == 2: path.move(to: point(0))
            case "L" where values.count == 2: path.addLine(to: point(0))
            case "Q" where values.count == 4: path.addQuadCurve(to: point(2), control: point(0))
            case "C" where values.count == 6: path.addCurve(to: point(4), control1: point(0), control2: point(2))
            case "Z": path.closeSubpath()
            default: break
            }
        }
        return path
    }

    /// Which points a clip holds: CSS's `nonzero` unless it said `evenodd`.
    static func rule(_ value: BatchValue?) -> CGPathFillRule {
        field(value, "rule")?.string == "evenodd" ? .evenOdd : .winding
    }

    private static func field(_ value: BatchValue?, _ key: String) -> BatchValue? {
        if case .object(let o)? = value { return o[key] }
        return nil
    }

    static func mask(_ path: CGPath?, _ rule: CGPathFillRule = .winding) -> CALayer? {
        guard let path else { return nil }
        let mask = CAShapeLayer()
        mask.path = path
        mask.fillRule = rule == .evenOdd ? .evenOdd : .nonZero
        mask.fillColor = CGColor(gray: 1, alpha: 1)
        return mask
    }
}


extension NodeView {
    /// Four equal percentage radii can still be elliptical. Keep the declared
    /// unequal-corner overflow fallback, but carry this equal outline alongside
    /// clip-path instead of dropping it when CALayer's circular fast path fails.
    var ellipticalClip: CGPath? {
        #if os(macOS)
        let backdrop = layer?.backgroundFilters?.isEmpty == false
        #else
        let backdrop = false
        #endif
        guard clipsToBounds || clipBox != nil || backdrop else { return nil }
        let sizes = cornerSizes(in: bounds)
        guard let first = sizes.first, first.width != first.height,
              sizes.allSatisfy({ $0 == first }) else { return nil }
        return BorderPaint.roundedRect(bounds, sizes)
    }

    /// The layer's mask as CSS composes it — `mask-image` over `clip-path`
    /// over the clip's outline (`BoxMaskIOS.swift`, `BoxMaskMac.swift`) — for
    /// a filtered box's picture.
    func resolvedClipMask() -> CALayer? {
        composedMask(shaped: clipBox == nil ? shapedClip() : nil)
    }

    /// The clip's outline follows a new size or a backdrop coming or going.
    func syncEllipticalClip() { applyBoxMask() }
}
