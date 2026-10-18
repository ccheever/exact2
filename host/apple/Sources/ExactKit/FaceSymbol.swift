// A symbol on a native control's face (LLP 1121 D10; stage 9 A2): an SF
// Symbol's name, or, for an icon token with no `sf` and a template path, a
// token holding that path (`symbolPath` on its image node, or beside a face's
// "" `symbol` in the press face's and a grouped row's JSON). The token goes
// wherever a face carries a name — its equality, its caches and its drawn
// source keep working — and draws as a template image (ExactCore's
// IconPath) that the control tints as it tints an SF Symbol: on a control's
// face a symbol keeps the platform's or the control's tint (D9; LLP 1115 D4),
// not the page image's tint-else-colour. A multicolour SVG needs an `sf`.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import ExactCore

package enum FaceSymbol {
    /// What a path token starts with; no SF Symbol or role name holds a `:`.
    static let prefix = "icon-path:"

    /// The token of an icon's path and viewBox; nil when there is no path.
    package static func token(path: String?, viewBox: String?) -> String? {
        guard let path, !path.isEmpty else { return nil }
        return prefix + (viewBox ?? "") + "|" + path
    }

    /// A face's symbol from its JSON: the name, else the path's token where
    /// the name is "" and a path is given; nil for no symbol.
    package static func of(_ json: [String: Any]) -> String? {
        guard let name = json["symbol"] as? String else { return nil }
        guard name.isEmpty else { return name }
        return token(path: json["symbolPath"] as? String, viewBox: json["symbolViewBox"] as? String) ?? name
    }

    /// Whether `symbol` is a path's token, not a name.
    package static func isPath(_ symbol: String) -> Bool { symbol.hasPrefix(prefix) }

    /// The template image of a path's token, an em square `points` on a
    /// side (the point size the site gives an SF Symbol); nil for a name.
    package static func pathImage(_ symbol: String, points: CGFloat) -> IconImage? {
        guard isPath(symbol) else { return nil }
        let body = symbol.dropFirst(prefix.count)
        guard let bar = body.firstIndex(of: "|") else { return nil }
        let side = max(1, points.rounded(.up))
        return IconPath.image(String(body[body.index(after: bar)...]), viewBox: String(body[..<bar]),
                              size: CGSize(width: side, height: side))
    }

    #if os(macOS)
    /// The point size of an SF Symbol drawn with no configuration (the body text's).
    package static var defaultPoints: CGFloat { NSFont.systemFontSize }
    /// A toolbar item's glyph: AppKit draws a symbol there at its own size,
    /// an image as it is (the HIG's regular toolbar icon, 17 points).
    package static let toolbarPoints: CGFloat = 17

    /// A face symbol's image: a path's template image `points` square, else
    /// the named SF Symbol (nil for "" or an unknown name).
    package static func image(_ symbol: String, points: CGFloat = defaultPoints, accessibilityDescription: String? = nil) -> NSImage? {
        if isPath(symbol) {
            let image = pathImage(symbol, points: points)
            image?.accessibilityDescription = accessibilityDescription
            return image
        }
        return symbol.isEmpty ? nil : NSImage(systemSymbolName: symbol, accessibilityDescription: accessibilityDescription)
    }

    /// A path's template image drawn in `tint`, as a palette colour draws a
    /// symbol in its own colour (an authored symbol tint on a button face).
    package static func tinted(_ template: NSImage, _ tint: NSColor) -> NSImage {
        let image = NSImage(size: template.size, flipped: false) { rect in
            template.draw(in: rect)
            tint.set()
            rect.fill(using: .sourceAtop)
            return true
        }
        image.isTemplate = false
        image.accessibilityDescription = template.accessibilityDescription
        return image
    }
    #else
    /// The point size of an SF Symbol drawn with no configuration (the body text's).
    package static var defaultPoints: CGFloat { UIFont.preferredFont(forTextStyle: .body).pointSize }
    /// A tab bar item's glyph (the HIG's 25-point regular tab bar icon):
    /// UIKit sizes a symbol there itself, and an image as it is.
    package static let tabBarPoints: CGFloat = 25

    /// A face symbol's image: a path's template image `points` square, else
    /// the named SF Symbol in `configuration` (nil for "" or an unknown name).
    package static func image(_ symbol: String, points: CGFloat = defaultPoints, configuration: UIImage.SymbolConfiguration? = nil) -> UIImage? {
        if isPath(symbol) { return pathImage(symbol, points: points) }
        return symbol.isEmpty ? nil : UIImage(systemName: symbol, withConfiguration: configuration)
    }
    #endif
}

extension NodeView {
    /// The symbol this image node shows on a control's face: its SF Symbol's
    /// name, else its icon path's token (A1's `symbolPath`); nil for neither.
    var faceSymbol: String? {
        if let name = props["symbolName"], !name.isEmpty { return name }
        return FaceSymbol.token(path: props["symbolPath"], viewBox: props["symbolViewBox"])
    }
}
