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
