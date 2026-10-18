// An icon token's path on Apple (LLP 1121 D10; stage 9 A): an icon with no
// `sf` source and a template SVG arrives as absolute `M`/`L`/`C`/`Z` commands
// and its viewBox (host/apple/src/icon_path.rs, the kernel's parse), drawn
// filled nonzero under the viewBox, kept in its aspect (`xMidYMid meet`, as
// Linux's `fitted` and the web's mask), as a template image that takes the
// node's tint: `-exact-tint-color` where written, else its `color`.
#if os(macOS)
import AppKit
typealias IconImage = NSImage
#else
import UIKit
typealias IconImage = UIImage
#endif

enum IconPath {
    /// The path the commands draw, in viewBox units; nil for none.
    static func path(_ commands: String) -> CGPath? {
        let words = commands.split(separator: " ")
        let path = CGMutablePath()
        var i = 0
        func n() -> CGFloat? { guard i < words.count, let v = Double(words[i]) else { return nil }; i += 1; return CGFloat(v) }
        while i < words.count {
            let verb = words[i]; i += 1
            switch verb {
            case "M": guard let x = n(), let y = n() else { return nil }; path.move(to: CGPoint(x: x, y: y))
            case "L": guard let x = n(), let y = n() else { return nil }; path.addLine(to: CGPoint(x: x, y: y))
            case "C":
                guard let a = n(), let b = n(), let c = n(), let d = n(), let x = n(), let y = n() else { return nil }
                path.addCurve(to: CGPoint(x: x, y: y), control1: CGPoint(x: a, y: b), control2: CGPoint(x: c, y: d))
            case "Z": path.closeSubpath()
            default: return nil
            }
        }
        return path.isEmpty ? nil : path
    }

    /// `min-x min-y width height`, a 24-unit square when it says none.
    static func viewBox(_ text: String) -> CGRect {
        let v = text.split(whereSeparator: { $0 == " " || $0 == "," }).compactMap { Double($0) }
        guard v.count == 4, v[2] > 0, v[3] > 0 else { return CGRect(x: 0, y: 0, width: 24, height: 24) }
        return CGRect(x: v[0], y: v[1], width: v[2], height: v[3])
    }

    /// The viewBox fitted into `rect`, its aspect kept and centred.
    static func fitted(_ box: CGRect, in rect: CGRect) -> CGAffineTransform {
        let scale = min(rect.width / box.width, rect.height / box.height)
        let dx = rect.minX + (rect.width - box.width * scale) / 2 - box.minX * scale
        let dy = rect.minY + (rect.height - box.height * scale) / 2 - box.minY * scale
        return CGAffineTransform(a: scale, b: 0, c: 0, d: scale, tx: dx, ty: dy)
    }

    /// The icon as a template image an em square of `points` (its glyph box,
    /// as the kernel measures an icon with no SF Symbol), filled nonzero.
    static func image(_ commands: String, viewBox text: String, points: CGFloat) -> IconImage? {
        guard points > 0, let path = path(commands) else { return nil }
        let size = CGSize(width: points, height: points), box = viewBox(text)
        var transform = fitted(box, in: CGRect(origin: .zero, size: size))
        guard let placed = path.copy(using: &transform) else { return nil }
        #if os(macOS)
        let image = NSImage(size: size, flipped: true) { _ in
            guard let cg = NSGraphicsContext.current?.cgContext else { return false }
            cg.addPath(placed); cg.setFillColor(NSColor.black.cgColor); cg.fillPath(using: .winding)
            return true
        }
        image.isTemplate = true
        return image
        #else
        let format = UIGraphicsImageRendererFormat.preferred(); format.opaque = false
        let image = UIGraphicsImageRenderer(size: size, format: format).image { ctx in
            ctx.cgContext.addPath(placed); ctx.cgContext.setFillColor(UIColor.black.cgColor); ctx.cgContext.fillPath(using: .winding)
        }
        return image.withRenderingMode(.alwaysTemplate)
        #endif
    }
}

extension NodeView {
    /// An icon path's image for this node, when it has one (`symbolPath`).
    func iconPathImage(points: CGFloat) -> IconImage? {
        guard let commands = props["symbolPath"] else { return nil }
        return IconPath.image(commands, viewBox: props["symbolViewBox"] ?? "", points: points)
    }
    /// An icon path's tint (D10): `-exact-tint-color` where written (its
    /// accent where it names one), else the node's text colour.
    var iconPathTint: PlatformColor? {
        if style["tint_color"] != nil { return symbolTint ?? inheritedTint }
        return channels("text_color").map(TextEngine.color)
    }
}
