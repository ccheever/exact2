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

    /// What follows the prefix in an even-odd icon's token (stage 14 C2); a
    /// nonzero icon's token is as it was. No viewBox starts with a letter.
    static let evenOdd = "evenodd|"

    /// The token of an icon's path, viewBox and fill rule (`symbolFillRule`);
    /// nil when there is no path.
    package static func token(path: String?, viewBox: String?, rule: String? = nil) -> String? {
        guard let path, !path.isEmpty else { return nil }
        return prefix + (rule == "evenodd" ? evenOdd : "") + (viewBox ?? "") + "|" + path
    }

    /// A face's symbol from its JSON: the name, else the path's token where
    /// the name is "" and a path is given; nil for no symbol.
    package static func of(_ json: [String: Any]) -> String? {
        guard let name = json["symbol"] as? String else { return nil }
        guard name.isEmpty else { return name }
        return token(path: json["symbolPath"] as? String, viewBox: json["symbolViewBox"] as? String, rule: json["symbolFillRule"] as? String) ?? name
    }

    /// Whether `symbol` is a path's token, not a name.
    package static func isPath(_ symbol: String) -> Bool { symbol.hasPrefix(prefix) }

    /// A face symbol as the agent reports it: the platform's name; for a
    /// path, the icon token's name as its author wrote it (`icon.reply`)
    /// where the face says it, else "" (the token is the face's drawing).
    package static func observed(_ symbol: String?, icon: String? = nil) -> String? {
        symbol.map { isPath($0) ? icon ?? "" : $0 }
    }

    /// The template image of a path's token, an em square `points` on a
    /// side (the point size the site gives an SF Symbol); nil for a name.
    package static func pathImage(_ symbol: String, points: CGFloat) -> IconImage? {
        guard isPath(symbol) else { return nil }
        var body = symbol.dropFirst(prefix.count)
        let rule: CGPathFillRule = body.hasPrefix(evenOdd) ? .evenOdd : .winding
        if rule == .evenOdd { body = body.dropFirst(evenOdd.count) }
        guard let bar = body.firstIndex(of: "|") else { return nil }
        let side = max(1, points.rounded(.up))
        return IconPath.image(String(body[body.index(after: bar)...]), viewBox: String(body[..<bar]),
                              size: CGSize(width: side, height: side), rule: rule)
    }

    #if os(macOS)
    /// The point size of an SF Symbol drawn with no configuration (the body text's).
    package static var defaultPoints: CGFloat { NSFont.systemFontSize }
    /// A toolbar item's glyph: AppKit draws a symbol there at its own size,
    /// an image as it is. Measured (FaceSymbolTests, macOS 27, by a screen
    /// capture): an SF Symbol's inked box there is a 16.7-point symbol's.
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
            rect.fill(using: .sourceIn) // the tint through the path's alpha, its own alpha kept
            return true
        }
        image.isTemplate = false
        image.accessibilityDescription = template.accessibilityDescription
        return image
    }
    #else
    /// The point size of an SF Symbol drawn with no configuration (the body text's).
    package static var defaultPoints: CGFloat { UIFont.preferredFont(forTextStyle: .body).pointSize }
    /// A tab bar item's glyph: UIKit sizes a symbol there itself, and an
    /// image as it is. Measured (FaceSymbolIOSTests and a plain UIKit app,
    /// iOS 27 simulators): an SF Symbol's inked box there is a 23-point
    /// symbol's on an iPhone at every width, stacked or inline (an iPhone 18
    /// Pro Max turned, the iPhone Duo's inner display), and a 17-point one's in
    /// a standalone tab bar given an iPad's idiom and a regular width on an
    /// iPhone, standing in for an iPad's inline items (an iPad itself shows
    /// iPadOS's floating tab bar, not measured here).
    package static func tabBarPoints(_ traits: UITraitCollection) -> CGFloat {
        traits.userInterfaceIdiom == .pad && traits.horizontalSizeClass == .regular ? 17 : 23
    }
    /// A navigation bar item's glyph, measured as the tab bar's: a 22-point
    /// symbol's (UIKit draws bar symbols larger than the body's 17).
    package static let barItemPoints: CGFloat = 22

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
        return FaceSymbol.token(path: props["symbolPath"], viewBox: props["symbolViewBox"], rule: props["symbolFillRule"])
    }
}
