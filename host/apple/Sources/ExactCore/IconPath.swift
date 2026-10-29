// An icon token's path on Apple (LLP 1121 D10; stage 9 A): an icon with no
// `sf` source and a template SVG arrives as absolute `M`/`L`/`C`/`Z` commands,
// `;` between its SVG's paths, and its viewBox (host/apple/src/icon_path.rs,
// the kernel's parse). Each path is filled on its own, as Linux and the web
// fill them, nonzero unless the icon says even-odd (`symbolFillRule`, stage 14 C2), under the viewBox kept in its aspect (`xMidYMid meet`,
// as Linux's `fitted` and the web's mask). The page's image views and the
// widget snapshot both draw it from here.
import CoreGraphics
#if canImport(UIKit)
import UIKit
package typealias IconImage = UIImage
#else
import AppKit
package typealias IconImage = NSImage
#endif

package enum IconPath {
    /// Each path the commands draw, in viewBox units; none for no valid one.
    package static func paths(_ commands: String) -> [CGPath] {
        commands.split(separator: ";").compactMap { path(String($0)) }
    }

    /// One path's commands as a path; nil for none or a verb it doesn't send.
    package static func path(_ commands: String) -> CGPath? {
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

    /// An icon's fill rule from its `symbolFillRule`: even-odd for `evenodd`, else nonzero.
    package static func rule(_ text: String?) -> CGPathFillRule { text == "evenodd" ? .evenOdd : .winding }

    /// `min-x min-y width height`, a 24-unit square when it says none.
    package static func viewBox(_ text: String) -> CGRect {
        let v = text.split(whereSeparator: { $0 == " " || $0 == "," }).compactMap { Double($0) }
        guard v.count == 4, v[2] > 0, v[3] > 0 else { return CGRect(x: 0, y: 0, width: 24, height: 24) }
        return CGRect(x: v[0], y: v[1], width: v[2], height: v[3])
    }

    /// The viewBox fitted into `rect`, its aspect kept and centred.
    package static func fitted(_ box: CGRect, in rect: CGRect) -> CGAffineTransform {
        let scale = min(rect.width / box.width, rect.height / box.height)
        let dx = rect.minX + (rect.width - box.width * scale) / 2 - box.minX * scale
        let dy = rect.minY + (rect.height - box.height * scale) / 2 - box.minY * scale
        return CGAffineTransform(a: scale, b: 0, c: 0, d: scale, tx: dx, ty: dy)
    }

    /// An icon's em square (`em` points, its natural size) placed in `box` by
    /// `object-fit`, as Linux places it (paint/svg.rs): `fill` and `contain`
    /// fit it in its aspect, `cover` covers, `none` keeps it, `scale-down`
    /// keeps it unless it doesn't fit; centred.
    package static func placed(em: CGFloat, fit: String, in box: CGRect) -> CGRect {
        let fitted = min(box.width, box.height), covered = max(box.width, box.height)
        let side: CGFloat
        switch fit {
        case "cover": side = covered
        case "none": side = em
        case "scale-down": side = min(em, fitted)
        default: side = fitted
        }
        return CGRect(x: box.midX - side / 2, y: box.midY - side / 2, width: side, height: side)
    }

    /// Fill each path by `rule`, placed into `rect` (a top-left origin), in `ctx`'s fill colour.
    package static func fill(_ paths: [CGPath], viewBox text: String, in rect: CGRect, rule: CGPathFillRule = .winding, _ ctx: CGContext) {
        var transform = fitted(viewBox(text), in: rect)
        for p in paths { if let placed = p.copy(using: &transform) { ctx.addPath(placed); ctx.fillPath(using: rule) } }
    }

    /// The icon as a template image `size` big (an em square for its glyph
    /// box, a larger one for the box it shows in), each path filled by `rule`.
    package static func image(_ commands: String, viewBox text: String, size: CGSize, rule: CGPathFillRule = .winding) -> IconImage? {
        let paths = paths(commands)
        guard size.width > 0, size.height > 0, !paths.isEmpty else { return nil }
        let rect = CGRect(origin: .zero, size: size)
        #if os(watchOS)
        return nil // (no image renderer on watchOS; no watch host draws the path yet)
        #elseif canImport(UIKit)
        let format = UIGraphicsImageRendererFormat.preferred(); format.opaque = false
        let image = UIGraphicsImageRenderer(size: size, format: format).image { ctx in
            ctx.cgContext.setFillColor(UIColor.black.cgColor); fill(paths, viewBox: text, in: rect, rule: rule, ctx.cgContext)
        }
        return image.withRenderingMode(.alwaysTemplate)
        #else
        // A vector image: AppKit draws it again at the size and scale it shows at.
        let image = NSImage(size: size, flipped: true) { drawn in
            guard let cg = NSGraphicsContext.current?.cgContext else { return false }
            cg.setFillColor(NSColor.black.cgColor); fill(paths, viewBox: text, in: drawn, rule: rule, cg)
            return true
        }
        image.isTemplate = true
        return image
        #endif
    }
}
