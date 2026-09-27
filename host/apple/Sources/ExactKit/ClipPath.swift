// CSS clip-path validated by the kernel, shared by UIKit and AppKit: its
// normalized commands (absolute M, L, C, Z — LLP 1065 D8) and fill rule.
import CoreGraphics
import QuartzCore

enum ClipPath {
    static func path(_ value: BatchValue?) -> CGPath? {
        guard case .object(let clip) = value, let commands = clip["commands"]?.array, !commands.isEmpty else { return nil }
        let path = CGMutablePath()
        for command in commands {
            guard let kind = command.array?.first?.string, let values = command.array?.last?.numbers else { continue }
            func point(_ index: Int) -> CGPoint { CGPoint(x: values[index], y: values[index + 1]) }
            switch kind {
            case "M" where values.count == 2: path.move(to: point(0))
            case "L" where values.count == 2: path.addLine(to: point(0))
            case "C" where values.count == 6: path.addCurve(to: point(4), control1: point(0), control2: point(2))
            case "Z": path.closeSubpath()
            default: break
            }
        }
        return path
    }

    /// CSS `path()`'s fill rule: `nonzero` unless it said `evenodd`.
    static func rule(_ value: BatchValue?) -> CGPathFillRule {
        guard case .object(let clip) = value, clip["rule"]?.string == "evenodd" else { return .winding }
        return .evenOdd
    }

    static func mask(_ path: CGPath?, _ rule: CGPathFillRule) -> CALayer? {
        guard let path else { return nil }
        let mask = CAShapeLayer()
        mask.path = path
        mask.fillRule = rule == .evenOdd ? .evenOdd : .nonZero
        mask.fillColor = CGColor(gray: 1, alpha: 1)
        return mask
    }
}
