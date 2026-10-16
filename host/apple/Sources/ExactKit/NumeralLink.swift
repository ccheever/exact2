// @ref LLP 1077 D15, LLP 1115.003 D3 — a text's numerals rolling per glyph as
// they change is a linked capability: the `ExactNumerals` module plays
// SwiftUI's numeric text, and an app's composition links it (and SwiftUI)
// only when its plan binds `-exact-content-transition`. Without it a paragraph
// keeps its raster, which rolls whole (`NumeralRoll`).
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// What the core asks of the Numerals capability.
package protocol NumeralCapability: AnyObject {
    /// Show `node`'s paragraph, `attributed` in its content box aligned as
    /// `align` says (0 left, 1 centre, 2 right), rolling its numerals as they
    /// change; `mode` is `numeric` or `numeric-countdown`.
    func show(_ node: NodeView, attributed: NSAttributedString, box: CGRect, align: Int, mode: String)
    /// `node` no longer rolls: its paragraph draws as any other.
    func hide(_ node: NodeView)
}

/// The Numerals module, installed by the composition.
package enum NumeralLink {
    package static var installed: NumeralCapability?
}

/// The number a paragraph's text leads with, which says which way a change
/// rolls: its first run of digits, group separators skipped, negative when a
/// minus sign comes before it (`$-12`, `−3%`); `0` when it has no digits. The
/// Linux host's `leading_number` reads it the same way.
package func leadingNumber(_ text: String) -> Double {
    guard let first = text.firstIndex(where: { $0.isASCII && $0.isNumber }) else { return 0 }
    var digits = text[..<first].contains(where: { $0 == "-" || $0 == "\u{2212}" }) ? "-" : ""
    var dot = false
    for ch in text[first...] {
        if ch.isASCII && ch.isNumber { digits.append(ch) }
        else if ch == "," { continue }
        else if ch == "." && !dot { dot = true; digits.append(ch) }
        else { break }
    }
    if digits.hasSuffix(".") { digits.removeLast() }
    return Double(digits) ?? 0
}
